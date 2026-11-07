//! Current SUPPLY (lender) APR per bank, normalized to I80F48 (1.0 == 100%) and NET of each
//! venue's protocol cut, so rates are comparable across venues for the auto-rebalance order.
//!
//! The protocol-faithful rate math lives in each integration's mock crate (`MinimalReserve::supply_apr`,
//! `MinimalSpotMarket::deposit_rate`, `TokenReserve::supply_rate`), co-located with the state mirror it
//! reads and unit-tested there. This module only dispatches by `asset_tag`, loads/staleness-checks
//! the rate-bearing account, and maps the pure `Option` result to a marginfi error:
//!
//! - Native marginfi banks: the bank's own interest curve at its post-deposit utilization (net;
//!   fees fall on borrowers) plus its spread and streamed emissions over that supply. The caller
//!   accrues first.
//! - Kamino: `borrow_apr(util) * util * (1 - protocol_take_rate)` plus reserve rewards; a per-slot
//!   reserve is rescaled from klend's slot-year to a wall-clock year at measured chain pacing.
//! - Drift: `borrow_apr(util) * util * (1 - insurance_fund.total_factor)`.
//! - Solend: `borrow_apr(util) * util * (1 - protocol_take_rate)` (3-slope borrow curve).
//! - JupLend: the liquidity-layer supply rate, a deposit priced on the mint's `RateModel` at the
//!   post-deposit utilization, plus the fToken's `LendingRewardsRateModel` rewards.
//!
//! [`yield_index_of`] / [`debt_index_of`] serve the REALIZED rate: growth of a monotonic share
//! index across a span, which a single-transaction rate spike cannot move.
//!
//! Integration reserve/market accounts MUST be refreshed in the same slot by the caller
//! (`refresh_reserve` / `update_spot_market_cumulative_interest` / JupLend liquidity-program
//! `update_exchange_price`, which refreshes the `TokenReserve` the supply rate reads).

use crate::state::bank::BankImpl;
use crate::state::interest_rate::{InterestRateConfigImpl, LendingCurve};
use crate::state::price::{
    load_drift_spot_market, load_juplend_lending, load_kamino_reserve, load_solend_reserve,
    OraclePriceFeedAdapter,
};
use crate::{
    check, constants::MIN_EMISSIONS_SHARE_SUPPLY, math_error, prelude::*,
    utils::is_integration_asset_tag,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use drift_mocks::state::MinimalSpotMarket;
use fixed::types::I80F48;
use juplend_mocks::state::{
    LendingRewardsRateModel, RateModel, TokenReserve, EXCHANGE_PRICES_PRECISION,
};
use kamino_mocks::state::{MinimalLendingMarket, KLEND_SLOTS_PER_SECOND};
use marginfi_type_crate::constants::{
    ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOL,
    ASSET_TAG_SOLEND, ASSET_TAG_STAKED, SECONDS_PER_YEAR,
};
use marginfi_type_crate::pdas::JUPLEND_LIQUIDITY_PROGRAM_ID;
use marginfi_type_crate::types::{Bank, BankConfig, MarginfiGroup, OraclePriceType, RateReading};

/// Accounts a venue needs beyond its rate-bearing account to price a deposit and its reward
/// emissions, each bound to the bank's own venue state; reading the stored rate needs none.
#[derive(Default, Clone, Copy)]
pub struct RewardsAccounts<'info> {
    /// Kamino: the reserve's `LendingMarket`, which caps the emission APR.
    pub lending_market: Option<&'info AccountInfo<'info>>,
    /// JupLend: the `LendingRewardsRateModel` the bank's `Lending` references.
    pub rewards_model: Option<&'info AccountInfo<'info>>,
    /// JupLend: the fToken mint, whose supply is the rewards denominator.
    pub ftoken_mint: Option<&'info AccountInfo<'info>>,
    /// JupLend: the mint's liquidity-layer `RateModel`, which prices the post-deposit borrow rate.
    pub rate_model: Option<&'info AccountInfo<'info>>,
}

/// Supply APR (I80F48, 1.0 == 100%) for `bank` after `extra_native` more tokens are supplied,
/// dispatched on `asset_tag` (the canonical integration identifier, consistent with the
/// `is_*_asset_tag` checks used across deposit/withdraw). `venue` is the rate-bearing account
/// (`None` for native, which prices from its own totals); `token_reserve` is JupLend's
/// `TokenReserve` (`None` otherwise). Unknown tags fail rather than default to native. The caller
/// refreshes the venue this slot and locates it (see `rate_of`).
pub fn current_supply_apr<'info>(
    bank: &Bank,
    venue: Option<&'info AccountInfo<'info>>,
    token_reserve: Option<&AccountInfo>,
    rewards: RewardsAccounts<'info>,
    extra_native: u64,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let tag = bank.config.asset_tag;
    if matches!(tag, ASSET_TAG_DEFAULT | ASSET_TAG_SOL | ASSET_TAG_STAKED) {
        return NativeRateModel::new(bank, clock.unix_timestamp)?.rate_at(extra_native);
    }
    let venue = venue.ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    match tag {
        ASSET_TAG_KAMINO => kamino_supply_apr(&bank.config, venue, rewards, extra_native, clock),
        ASSET_TAG_DRIFT => drift_supply_apr(&bank.config, venue, extra_native, clock),
        ASSET_TAG_SOLEND => solend_supply_apr(&bank.config, venue, extra_native),
        ASSET_TAG_JUPLEND => juplend_supply_apr(
            &bank.config,
            venue,
            token_reserve,
            rewards,
            extra_native,
            clock,
        ),
        _ => err!(MarginfiError::InvalidOracleSetup),
    }
}

/// A native bank's interest curve, current totals and emissions, captured once after accrual so
/// the bank can be priced at any deposit size without re-reading it.
pub struct NativeRateModel {
    curve: LendingCurve,
    assets: I80F48,
    liabilities: I80F48,
    /// Native units of emissions spread per year, shared by every deposit.
    emissions_per_year: I80F48,
    /// Native units of streamed emissions paid per year, shared by every deposit once the supply
    /// reaches `stream_min_assets`.
    stream_per_year: I80F48,
    stream_min_assets: I80F48,
}

impl NativeRateModel {
    pub fn new(bank: &Bank, now: i64) -> MarginfiResult<Self> {
        Ok(Self {
            curve: LendingCurve::new(&bank.config.interest_rate_config)?,
            assets: bank.get_asset_amount(bank.total_asset_shares.into())?,
            liabilities: bank.get_liability_amount(bank.total_liability_shares.into())?,
            emissions_per_year: emissions_spread_rate_at(bank, now)
                .saturating_mul(SECONDS_PER_YEAR),
            stream_per_year: emissions_stream_rate(bank)?.saturating_mul(SECONDS_PER_YEAR),
            stream_min_assets: bank.get_asset_amount(MIN_EMISSIONS_SHARE_SUPPLY)?,
        })
    }

    /// Supply rate after `extra_native` more tokens are deposited: the lending rate (zero with no
    /// borrows, as in `update_bank_cache`) plus spread and streamed emissions over that supply.
    pub fn rate_at(&self, extra_native: u64) -> MarginfiResult<I80F48> {
        let assets = self
            .assets
            .checked_add(I80F48::from_num(extra_native))
            .ok_or_else(math_error!())?;
        if assets == I80F48::ZERO {
            return Ok(I80F48::ZERO);
        }
        let emissions_per_year = if assets >= self.stream_min_assets {
            self.emissions_per_year.saturating_add(self.stream_per_year)
        } else {
            self.emissions_per_year
        };
        let emissions = if emissions_per_year == I80F48::ZERO {
            I80F48::ZERO
        } else {
            emissions_per_year
                .checked_div(assets)
                .ok_or_else(math_error!())?
        };
        if self.liabilities == I80F48::ZERO {
            return Ok(emissions);
        }
        let utilization = self
            .liabilities
            .checked_div(assets)
            .ok_or_else(math_error!())?;
        Ok(self
            .curve
            .lending_rate(utilization)?
            .checked_add(emissions)
            .ok_or_else(math_error!())?)
    }
}

/// Native units of `bank`'s emissions still pending at `now`: what its last emissions deposit left
/// to spread, less what `emissions_spread_rate` has spread since.
pub fn pending_emissions(bank: &Bank, now: i64) -> I80F48 {
    let tokens = I80F48::from(bank.emissions_spread_pending);
    if tokens <= I80F48::ZERO {
        return I80F48::ZERO;
    }
    let elapsed = now.saturating_sub(bank.emissions_spread_updated_at).max(0);
    let spread = I80F48::from(bank.emissions_spread_rate).saturating_mul(I80F48::from_num(elapsed));
    tokens.saturating_sub(spread).max(I80F48::ZERO)
}

/// Native units of `bank`'s emissions spread per second at `now`, zero once none are pending.
pub fn emissions_spread_rate_at(bank: &Bank, now: i64) -> I80F48 {
    if pending_emissions(bank, now) > I80F48::ZERO {
        bank.emissions_spread_rate.into()
    } else {
        I80F48::ZERO
    }
}

/// Native units of `bank`'s emissions stream paid per second, zero once the stream ends.
pub fn emissions_stream_rate(bank: &Bank) -> MarginfiResult<I80F48> {
    if bank.emissions_stream_seconds == 0 {
        return Ok(I80F48::ZERO);
    }
    Ok(I80F48::from(bank.emissions_stream_pending)
        .checked_div(I80F48::from_num(bank.emissions_stream_seconds))
        .ok_or_else(math_error!())?)
}

/// Rejects a stale Drift market. Drift stamps `last_interest_ts` only while interest accrues, so a
/// market with no borrows is exempt; its rate is zero.
fn check_drift_market_fresh(market: &MinimalSpotMarket, clock: &Clock) -> MarginfiResult {
    check!(
        !market.is_stale(clock.unix_timestamp) || market.has_no_borrows(),
        MarginfiError::DriftSpotMarketStale
    );
    Ok(())
}

/// The bank's rate-bearing venue account within `oracle_ais`: the LAST one, since the price oracle,
/// if any, precedes it. Works for fixed and live-oracle variants alike; native banks have none.
fn venue_ai<'info>(
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
) -> Option<&'info AccountInfo<'info>> {
    if is_integration_asset_tag(bank.config.asset_tag) {
        oracle_ais.last()
    } else {
        None
    }
}

/// Supply APR for `bank` as it would read after `extra_native` NATIVE tokens are supplied to it,
/// diluting utilization. Pass `0` for the current rate.
pub fn rate_at<'info>(
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    token_reserve: Option<&AccountInfo>,
    rewards: RewardsAccounts<'info>,
    extra_native: u64,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    current_supply_apr(
        bank,
        venue_ai(bank, oracle_ais),
        token_reserve,
        rewards,
        extra_native,
        clock,
    )
}

/// Current supply APR for `bank` (I80F48, 1.0 == 100%), via [`current_supply_apr`] (dispatched on
/// `asset_tag`). `token_reserve` is JupLend's `TokenReserve`.
pub fn rate_of<'info>(
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    token_reserve: Option<&AccountInfo>,
    rewards: RewardsAccounts<'info>,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    current_supply_apr(
        bank,
        venue_ai(bank, oracle_ais),
        token_reserve,
        rewards,
        0,
        clock,
    )
}

/// The bank's venue exchange-rate multiplier at `clock` (Kamino cToken rate, Drift cumulative
/// interest, JupLend exchange price; 1 for native banks), read from its configured oracle/venue
/// accounts. The spot price itself is discarded, but reading it applies the bank's staleness and
/// confidence gates, so every caller blocks while the oracle is untrustworthy.
pub fn venue_multiplier<'info>(
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let (_, priced) = OraclePriceFeedAdapter::get_price_and_confidence_and_cache_of_type(
        bank,
        oracle_ais,
        clock,
        OraclePriceType::RealTime,
        false,
    )?;
    Ok(priced.price_multiplier)
}

/// Monotonic per-share supply index for `bank` at `now`: `asset_share_value` times the venue
/// multiplier, with emissions spread over `EMISSIONS_SPREAD_SECONDS`. Its growth is the yield.
pub fn yield_index_of(bank: &Bank, multiplier: I80F48, now: i64) -> MarginfiResult<I80F48> {
    let index = I80F48::from(bank.asset_share_value)
        .checked_mul(multiplier)
        .ok_or_else(math_error!())?;
    let pending_growth = pending_emissions_growth(bank, now);
    if pending_growth == I80F48::ONE {
        return Ok(index);
    }
    Ok(index
        .checked_div(pending_growth)
        .ok_or_else(math_error!())?)
}

/// How far `bank`'s share value runs ahead of its yield index at `now`: one plus the share-value
/// growth its pending emissions stand for.
pub fn pending_emissions_growth(bank: &Bank, now: i64) -> I80F48 {
    let pending = pending_emissions(bank, now);
    let base = I80F48::from(bank.emissions_spread_base);
    if pending == I80F48::ZERO || base <= I80F48::ZERO {
        return I80F48::ONE;
    }
    I80F48::ONE.saturating_add(pending.saturating_div(base))
}

/// Debt-side counterpart to [`yield_index_of`]: `liability_share_value` times the venue multiplier,
/// which staked collateral needs because it is borrowable and prices above 1.
pub fn debt_index_of(bank: &Bank, multiplier: I80F48) -> MarginfiResult<I80F48> {
    I80F48::from(bank.liability_share_value)
        .checked_mul(multiplier)
        .ok_or_else(math_error!())
        .map_err(Into::into)
}

/// `(current / anchor - 1) * SECONDS_PER_YEAR / elapsed`: the time-weighted rate realized over the
/// span, so a spike contributes only its own duration. Negative when the index fell.
pub fn realized_apr(anchor: I80F48, current: I80F48, elapsed: i64) -> MarginfiResult<I80F48> {
    check!(anchor > I80F48::ZERO, MarginfiError::MathError);
    check!(elapsed > 0, MarginfiError::MathError);
    current
        .checked_div(anchor)
        .and_then(|g| g.checked_sub(I80F48::ONE))
        .and_then(|growth| growth.checked_mul(SECONDS_PER_YEAR))
        .and_then(|annual| annual.checked_div(I80F48::from_num(elapsed)))
        .ok_or_else(math_error!())
        .map_err(Into::into)
}

/// The reading a rate window of `window` seconds starts at, and the seconds elapsed since it.
fn rate_window_start(bank: &Bank, window: u32, now: i64) -> MarginfiResult<(&RateReading, i64)> {
    let reading = bank
        .rate_reading_at_least(i64::from(window), now)
        .ok_or(MarginfiError::RateHistoryTooShort)?;
    let elapsed = now
        .checked_sub(reading.timestamp)
        .ok_or_else(math_error!())?;
    Ok((reading, elapsed))
}

/// The supply APR `bank` has realized since its youngest reading at least `window` seconds old,
/// given its yield index `yield_index_now`. A gap in the readings only lengthens the span.
pub fn realized_supply_apr(
    bank: &Bank,
    window: u32,
    yield_index_now: I80F48,
    now: i64,
) -> MarginfiResult<I80F48> {
    let (reading, elapsed) = rate_window_start(bank, window, now)?;
    realized_apr(reading.asset_index(), yield_index_now, elapsed)
}

/// The borrow APR `bank` has realized since its youngest reading at least `window` seconds old,
/// given its debt index `debt_index_now`. A gap in the readings only lengthens the span.
pub fn realized_borrow_apr(
    bank: &Bank,
    window: u32,
    debt_index_now: I80F48,
    now: i64,
) -> MarginfiResult<I80F48> {
    let (reading, elapsed) = rate_window_start(bank, window, now)?;
    realized_apr(reading.debt_index(), debt_index_now, elapsed)
}

/// The bank's borrow APR at the utilization `extra_native` more of borrowing would produce. Pass
/// `0` for the current rate. Native banks only, which is every borrowable bank.
pub fn borrow_rate_at(
    bank: &Bank,
    group: &MarginfiGroup,
    extra_native: u64,
) -> MarginfiResult<I80F48> {
    let total_assets = bank.get_asset_amount(bank.total_asset_shares.into())?;
    check!(total_assets > I80F48::ZERO, MarginfiError::MathError);
    let total_liabilities = bank
        .get_liability_amount(bank.total_liability_shares.into())?
        .checked_add(I80F48::from_num(extra_native))
        .ok_or_else(math_error!())?;
    let utilization = total_liabilities
        .checked_div(total_assets)
        .ok_or_else(math_error!())?;
    Ok(bank
        .config
        .interest_rate_config
        .create_interest_rate_calculator(group)
        .calc_interest_rate(utilization)?
        .borrowing_rate_apr)
}

/// Tokens the bank's underlying venue can still accept, in NATIVE units of the bank's mint; `None`
/// when the venue has no cap to read. A `Some(n)` bounds a deposit but does not guarantee it lands.
pub fn venue_remaining_capacity<'info>(
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    clock: &Clock,
) -> MarginfiResult<Option<u64>> {
    let tag = bank.config.asset_tag;
    if !is_integration_asset_tag(tag) {
        return Ok(None);
    }
    let venue = venue_ai(bank, oracle_ais).ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    match tag {
        ASSET_TAG_KAMINO => {
            let loader = load_kamino_reserve(&bank.config, venue)?;
            let r = loader.load()?;
            check!(!r.is_stale(clock.slot), MarginfiError::ReserveStale);
            Ok(Some(r.remaining_deposit_capacity()))
        }
        ASSET_TAG_DRIFT => {
            let loader = load_drift_spot_market(&bank.config, venue)?;
            let m = loader.load()?;
            check_drift_market_fresh(&m, clock)?;
            Ok(Some(
                m.remaining_deposit_capacity().ok_or_else(math_error!())?,
            ))
        }
        ASSET_TAG_SOLEND => {
            let loader = load_solend_reserve(&bank.config, venue)?;
            let r = loader.load()?;
            check!(!r.is_stale()?, MarginfiError::SolendReserveStale);
            Ok(Some(r.remaining_deposit_capacity()?))
        }
        // JupLend has no supply cap
        ASSET_TAG_JUPLEND => Ok(None),
        _ => err!(MarginfiError::InvalidOracleSetup),
    }
}

/// Shortest elapsed slice of an epoch that yields a usable pacing sample.
const SLOT_PACING_MIN_SAMPLE_SECONDS: i64 = 3_600;
/// Bounds a real chain has ever paced within; outside them the sample is treated as unusable.
const SLOT_PACING_MIN_PER_SECOND: u8 = 1;
const SLOT_PACING_MAX_PER_SECOND: u8 = 4;

/// Slots per second over a `seconds`-long sample, or `None` when the sample is too short to be
/// meaningful or paces outside anything a live chain produces.
fn slots_per_second_from(slots: u64, seconds: i64) -> Option<I80F48> {
    if seconds < SLOT_PACING_MIN_SAMPLE_SECONDS {
        return None;
    }
    let measured = I80F48::from_num(slots).checked_div(I80F48::from_num(seconds))?;
    (I80F48::from_num(SLOT_PACING_MIN_PER_SECOND)..=I80F48::from_num(SLOT_PACING_MAX_PER_SECOND))
        .contains(&measured)
        .then_some(measured)
}

/// Real slots per second over the elapsed part of the current epoch, for venues that denominate
/// their rate in slots. Falls back to klend's own convention when the sample is unusable.
fn measured_slots_per_second(clock: &Clock) -> I80F48 {
    let nominal = I80F48::from_num(KLEND_SLOTS_PER_SECOND);
    let Ok(schedule) = EpochSchedule::get() else {
        return nominal;
    };
    let Some(slots) = clock
        .slot
        .checked_sub(schedule.get_first_slot_in_epoch(clock.epoch))
    else {
        return nominal;
    };
    slots_per_second_from(
        slots,
        clock
            .unix_timestamp
            .saturating_sub(clock.epoch_start_timestamp),
    )
    .unwrap_or(nominal)
}

// The venue account is supplied by a permissionless keeper, so it must be tied to the bank: these
// loaders `require_keys_eq!(venue.key, bank_config.oracle_keys[1])` (and check owner+discriminator),
// exactly as the pricing path does. Without this a keeper could pass a fabricated-rate account.
fn kamino_supply_apr<'info>(
    bank_config: &BankConfig,
    reserve_ai: &'info AccountInfo<'info>,
    rewards: RewardsAccounts<'info>,
    extra_native: u64,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let loader = load_kamino_reserve(bank_config, reserve_ai)?;
    let r = loader.load()?;
    check!(!r.is_stale(clock.slot), MarginfiError::ReserveStale);
    let extra = I80F48::from_num(extra_native);
    let slots_per_second = measured_slots_per_second(clock);
    let base = r
        .supply_apr_at(extra, slots_per_second)
        .ok_or_else(math_error!())?;

    // Reward emissions land in the reserve's available liquidity, raising the cToken exchange rate,
    // so they add to the interest rate on the same base.
    let market_ai = rewards
        .lending_market
        .ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    require_keys_eq!(
        *market_ai.key,
        r.lending_market,
        MarginfiError::InvalidBankAccount
    );
    let market = AccountLoader::<MinimalLendingMarket>::try_from(market_ai)
        .map_err(|_| error!(MarginfiError::InvalidBankAccount))?;
    let max_apr_bps = market.load()?.reserve_rewards_max_apr_bps;
    let rewards_apr = r
        .rewards_apr_at(max_apr_bps, extra, slots_per_second)
        .ok_or_else(math_error!())?;
    Ok(base.checked_add(rewards_apr).ok_or_else(math_error!())?)
}

fn drift_supply_apr<'info>(
    bank_config: &BankConfig,
    spot_ai: &'info AccountInfo<'info>,
    extra_native: u64,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let loader = load_drift_spot_market(bank_config, spot_ai)?;
    let m = loader.load()?;
    check_drift_market_fresh(&m, clock)?;
    Ok(m.deposit_rate_at(u128::from(extra_native))
        .ok_or_else(math_error!())?)
}

// `SolendMinimalReserve::is_stale` reads the clock itself (slot-based), so no `clock` is needed here.
fn solend_supply_apr<'info>(
    bank_config: &BankConfig,
    reserve_ai: &'info AccountInfo<'info>,
    extra_native: u64,
) -> MarginfiResult<I80F48> {
    let loader = load_solend_reserve(bank_config, reserve_ai)?;
    let r = loader.load()?;
    check!(!r.is_stale()?, MarginfiError::SolendReserveStale);
    Ok(r.supply_rate_at(I80F48::from_num(extra_native))
        .ok_or_else(math_error!())?)
}

/// JupLend liquidity-layer supply rate. `lending_ai` is the bank's Lending account (the venue,
/// validated against `bank_config.oracle_keys[1]`); `token_reserve` is the JupLend `TokenReserve` it
/// references, validated here against `lending.token_reserves_liquidity` before its rate is read.
fn juplend_supply_apr<'info>(
    bank_config: &BankConfig,
    lending_ai: &'info AccountInfo<'info>,
    token_reserve: Option<&AccountInfo>,
    rewards: RewardsAccounts<'info>,
    extra_native: u64,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let tr = token_reserve.ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    let loader = load_juplend_lending(bank_config, lending_ai)?;
    let lending = *loader.load()?;
    require_keys_eq!(
        *tr.key,
        lending.token_reserves_liquidity,
        MarginfiError::JuplendLendingValidationFailed
    );

    let reserve = TokenReserve::from_account_data(&tr.try_borrow_data()?)
        .ok_or(error!(MarginfiError::JuplendLendingValidationFailed))?;
    check!(
        !reserve.is_stale(clock.unix_timestamp),
        MarginfiError::JuplendLendingStale
    );
    let rate_model = match rewards.rate_model {
        Some(ai) => Some(load_juplend_rate_model(ai)?),
        None => None,
    };
    let base = reserve
        .supply_rate_at(extra_native, rate_model.as_ref())
        .ok_or_else(math_error!())?;

    // The bank holds fTokens, whose price grows by the liquidity-layer return plus the reward
    // schedule; upstream adds the two before applying them.
    let model_ai = rewards
        .rewards_model
        .ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    require_keys_eq!(
        *model_ai.key,
        lending.rewards_rate_model,
        MarginfiError::JuplendLendingValidationFailed
    );
    let mint_ai = rewards
        .ftoken_mint
        .ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    require_keys_eq!(
        *mint_ai.key,
        lending.f_token_mint,
        MarginfiError::JuplendLendingValidationFailed
    );

    let model = LendingRewardsRateModel::from_account_data(&model_ai.try_borrow_data()?)
        .ok_or(error!(MarginfiError::JuplendLendingValidationFailed))?;
    let ftoken_supply = InterfaceAccount::<Mint>::try_from(mint_ai)
        .map_err(|_| error!(MarginfiError::JuplendLendingValidationFailed))?
        .supply;
    let total_assets = u128::from(lending.token_exchange_price)
        .checked_mul(u128::from(ftoken_supply))
        .ok_or_else(math_error!())?
        .checked_div(EXCHANGE_PRICES_PRECISION)
        .ok_or_else(math_error!())?;
    // Emissions are shared over post-deposit TVL; the arriving tokens are in the mint's units,
    // which is what `total_assets` counts.
    let rewards_apr = model
        .rewards_apr(
            total_assets
                .checked_add(u128::from(extra_native))
                .ok_or_else(math_error!())?,
            clock.unix_timestamp as u64,
        )
        .ok_or_else(math_error!())?;
    Ok(base.checked_add(rewards_apr).ok_or_else(math_error!())?)
}

/// JupLend's `RateModel` from `ai`, whose address the caller has already bound to the bank's mint.
fn load_juplend_rate_model(ai: &AccountInfo) -> MarginfiResult<RateModel> {
    require_keys_eq!(
        *ai.owner,
        JUPLEND_LIQUIDITY_PROGRAM_ID,
        MarginfiError::JuplendLendingValidationFailed
    );
    RateModel::from_account_data(&ai.try_borrow_data()?)
        .ok_or(error!(MarginfiError::JuplendLendingValidationFailed))
}

/// Every supply-rate path must return I80F48 in the same units (`1.0 == 100%`), so the rebalance
/// order can rank a native bank's rate directly against any integration bank's rate. For each target
/// percentage this builds the equivalent per-venue config and asserts every venue reports the SAME
/// percentage.
#[cfg(test)]
mod unit_consistency {
    use bytemuck::Zeroable;
    use drift_mocks::state::drift_deposit_rate_from_parts;
    use juplend_mocks::state::juplend_supply_rate_from_parts;
    use kamino_mocks::state::{
        kamino_supply_apr_from_parts, CurvePoint, InterestRateBasis, KLEND_SLOTS_PER_SECOND,
    };
    use marginfi_type_crate::types::{
        make_points, milli_to_u32, u32_to_milli, InterestRateConfig, INTEREST_CURVE_SEVEN_POINT,
    };
    use solend_mocks::state::solend_supply_rate_from_parts;

    use super::{Bank, NativeRateModel, I80F48};

    /// The net supply rate each venue reports for `target_bps` (e.g. `1_000` == 10%), built from an
    /// equivalent per-venue config. Returned as `(native, kamino, drift, solend, juplend)`.
    fn venue_rates(target_bps: u32) -> (I80F48, I80F48, I80F48, I80F48, I80F48) {
        // The target percentage as an I80F48 fraction (1.0 == 100%).
        let pct = I80F48::from_num(target_bps) / I80F48::from_num(10_000u32);

        // Native: a curve flat at `pct`, with the bank fully utilized.
        let mut bank = Bank::zeroed();
        bank.config.interest_rate_config = InterestRateConfig {
            zero_util_rate: milli_to_u32(pct),
            hundred_util_rate: milli_to_u32(pct),
            points: make_points(&[]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };
        bank.asset_share_value = I80F48::ONE.into();
        bank.liability_share_value = I80F48::ONE.into();
        bank.total_asset_shares = I80F48::from_num(1_000).into();
        bank.total_liability_shares = I80F48::from_num(1_000).into();
        let native = NativeRateModel::new(&bank, 0).unwrap().rate_at(0).unwrap();

        // Kamino: a flat borrow curve at `target_bps`, evaluated at 100% utilization with no cut,
        // priced at klend's own pacing.
        let mut points = [CurvePoint {
            utilization_rate_bps: 0,
            borrow_rate_bps: target_bps,
        }; 11];
        for (i, p) in points.iter_mut().enumerate() {
            p.utilization_rate_bps = (i as u32) * 1_000; // 0..10_000 bps, strictly increasing
        }
        let kamino = kamino_supply_apr_from_parts(
            I80F48::from_num(1), // total_supply
            I80F48::from_num(1), // borrowed -> 100% utilization
            &points,
            0, // protocol_take_rate_pct
            InterestRateBasis::Legacy,
            I80F48::from_num(KLEND_SLOTS_PER_SECOND),
        )
        .unwrap();

        // Drift: rates are 1e6 units. At util == optimal, borrow_rate == optimal_borrow_rate; with no
        // insurance cut and 100% utilization, deposit_rate == borrow_rate.
        let rate_1e6 = u128::from(target_bps) * 100; // bps -> 1e6 scale
        let drift = drift_deposit_rate_from_parts(
            1_000_000,    // deposit
            1_000_000,    // borrow -> 100% utilization
            1_000_000,    // optimal_utilization (100%)
            rate_1e6,     // optimal_borrow_rate
            rate_1e6 * 2, // max_borrow_rate (not reached below optimal)
            0,            // min_borrow_rate (no floor)
            0,            // insurance total_factor
        )
        .unwrap();

        // Solend: I80F48 ratios. At util == optimal_util, borrow_rate == optimal_borrow_rate; no cut.
        let solend = solend_supply_rate_from_parts(
            I80F48::from_num(1), // curve_utilization
            I80F48::from_num(1), // supply_utilization
            I80F48::from_num(1), // optimal_utilization
            I80F48::from_num(1), // max_utilization
            I80F48::ZERO,        // min_borrow_rate
            pct,                 // optimal_borrow_rate
            pct,                 // max_borrow_rate
            pct,                 // super_max_borrow_rate
            I80F48::ZERO,        // protocol_take_rate
        )
        .unwrap();

        // JupLend: 1e4-scaled fields. With no interest-free split the formula reduces to
        // borrow * util * (1 - fee): `target_bps` borrow at 100% utilization, no fee.
        let juplend = juplend_supply_rate_from_parts(
            u128::from(target_bps), // borrow_rate
            0,                      // fee_on_interest
            10_000,                 // utilization (100%)
            1_000_000_000_000,      // supply_exchange_price (nonzero)
            1_000_000_000_000,      // borrow_exchange_price (nonzero)
            1_000_000,              // total_supply_with_interest
            0,                      // total_supply_interest_free
            1_000_000,              // total_borrow_with_interest
            0,                      // total_borrow_interest_free
        )
        .unwrap();

        (native, kamino, drift, solend, juplend)
    }

    #[test]
    fn percentage_is_identical_across_all_venues() {
        for target_bps in [500u32, 1_000, 2_500, 5_000] {
            let expected = I80F48::from_num(target_bps) / I80F48::from_num(10_000u32);
            let (native, kamino, drift, solend, juplend) = venue_rates(target_bps);

            // Direct cross-venue equality: all integration venues report the exact same percentage.
            assert_eq!(kamino, expected, "kamino at {target_bps}bps");
            assert_eq!(drift, kamino, "drift != kamino at {target_bps}bps");
            assert_eq!(solend, kamino, "solend != kamino at {target_bps}bps");
            assert_eq!(juplend, kamino, "juplend != kamino at {target_bps}bps");

            // Native curve rates are stored as u32 on a 0..1000% scale, so the exact value is that
            // quantization round-trip; assert against it, not the input.
            let native_quantized = u32_to_milli(milli_to_u32(expected));
            assert_eq!(native, native_quantized, "native at {target_bps}bps");
        }
    }
}

/// A native bank is priced on its own curve at the utilization it would have after the deposit.
#[cfg(test)]
mod native_dilution {
    use bytemuck::Zeroable;
    use marginfi_type_crate::types::{
        make_points, milli_to_u32, u32_to_milli, InterestRateConfig, INTEREST_CURVE_SEVEN_POINT,
    };

    use super::{Bank, NativeRateModel, I80F48};

    #[test]
    fn the_rate_dilutes_with_the_incoming_deposit() {
        // A linear curve from 0 to `one`, so the lending rate is `one * util^2`.
        let one = u32_to_milli(milli_to_u32(I80F48::ONE));
        let mut bank = Bank::zeroed();
        bank.config.interest_rate_config = InterestRateConfig {
            hundred_util_rate: milli_to_u32(I80F48::ONE),
            points: make_points(&[]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };
        bank.asset_share_value = I80F48::ONE.into();
        bank.liability_share_value = I80F48::ONE.into();
        bank.total_asset_shares = I80F48::from_num(1_000).into();
        bank.total_liability_shares = I80F48::from_num(500).into();

        // Half utilized now; a deposit equal to its current supply leaves it a quarter utilized.
        let model = NativeRateModel::new(&bank, 0).unwrap();
        assert_eq!(model.rate_at(0).unwrap(), one * I80F48::from_num(0.25));
        assert_eq!(
            model.rate_at(1_000).unwrap(),
            one * I80F48::from_num(0.0625)
        );

        bank.total_liability_shares = I80F48::ZERO.into();
        assert_eq!(
            NativeRateModel::new(&bank, 0)
                .unwrap()
                .rate_at(1_000)
                .unwrap(),
            I80F48::ZERO
        );
    }
}

/// A native bank's supply rate counts its emissions stream at the pace the stream pays. Every
/// figure here is exact in I80F48.
#[cfg(test)]
mod emissions_stream {
    use super::*;
    use bytemuck::Zeroable;
    use marginfi_type_crate::types::{
        make_points, milli_to_u32, InterestRateConfig, INTEREST_CURVE_SEVEN_POINT,
    };

    /// Native units over which a stream of one unit a second pays 25% a year.
    fn quarter_supply() -> I80F48 {
        SECONDS_PER_YEAR * 4
    }

    /// A bank of `supply` native units at a share value of 1, streaming `pending` over `seconds`.
    /// Its curve runs linearly from 0 to 100%.
    fn streaming_bank(supply: I80F48, pending: u64, seconds: u64) -> Bank {
        let mut bank = Bank::zeroed();
        bank.config.interest_rate_config = InterestRateConfig {
            hundred_util_rate: milli_to_u32(I80F48::ONE),
            points: make_points(&[]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };
        bank.asset_share_value = I80F48::ONE.into();
        bank.liability_share_value = I80F48::ONE.into();
        bank.total_asset_shares = supply.into();
        bank.emissions_stream_pending = I80F48::from_num(pending).into();
        bank.emissions_stream_seconds = seconds;
        bank
    }

    fn rate_at(bank: &Bank, extra_native: u64) -> I80F48 {
        NativeRateModel::new(bank, 0)
            .unwrap()
            .rate_at(extra_native)
            .unwrap()
    }

    #[test]
    fn the_stream_is_shared_over_the_supply_after_the_deposit() {
        let bank = streaming_bank(quarter_supply(), 1_000, 1_000);
        assert_eq!(rate_at(&bank, 0), I80F48::from_num(0.25));
        assert_eq!(
            rate_at(&bank, quarter_supply().to_num()),
            I80F48::from_num(0.125)
        );
    }

    #[test]
    fn the_stream_adds_to_the_lending_rate() {
        let mut bank = streaming_bank(quarter_supply(), 1_000, 1_000);
        bank.total_liability_shares = (quarter_supply() / 2).into();
        let lending = LendingCurve::new(&bank.config.interest_rate_config)
            .unwrap()
            .lending_rate(I80F48::from_num(0.5))
            .unwrap();
        assert_eq!(rate_at(&bank, 0), lending + I80F48::from_num(0.25));
    }

    /// The pace is what is left over the seconds left: two units in the last second count twice
    /// one unit a second, and an ended stream counts nothing.
    #[test]
    fn the_stream_counts_the_pace_of_what_is_left() {
        let last_second = streaming_bank(quarter_supply(), 2, 1);
        assert_eq!(rate_at(&last_second, 0), I80F48::from_num(0.5));

        let ended = streaming_bank(quarter_supply(), 0, 0);
        assert_eq!(rate_at(&ended, 0), I80F48::ZERO);
    }

    /// The stream counts from `MIN_EMISSIONS_SHARE_SUPPLY`: under it, on it and over it.
    #[test]
    fn the_stream_counts_from_the_minimum_share_supply() {
        let min = MIN_EMISSIONS_SHARE_SUPPLY;
        let bank = streaming_bank(min - I80F48::from_num(2), 1_000, 1_000);
        assert_eq!(rate_at(&bank, 1), I80F48::ZERO);
        assert_eq!(rate_at(&bank, 2), SECONDS_PER_YEAR / min);
        assert_eq!(rate_at(&bank, 3), SECONDS_PER_YEAR / (min + I80F48::ONE));
    }

    /// A bank still spreading a lump deposit while it streams counts both.
    #[test]
    fn the_stream_adds_to_the_spread_emissions() {
        let mut bank = streaming_bank(quarter_supply(), 1_000, 1_000);
        bank.emissions_spread_pending = I80F48::from_num(1_000).into();
        bank.emissions_spread_rate = I80F48::ONE.into();
        assert_eq!(rate_at(&bank, 0), I80F48::from_num(0.5));
    }

    /// A year of `pay_streamed_emissions` at one native unit a second grows the share value by the
    /// rate the model counted for it.
    #[test]
    fn the_counted_rate_is_what_a_year_of_payments_pays() {
        let year: u64 = SECONDS_PER_YEAR.to_num();
        let mut bank = streaming_bank(quarter_supply(), 2 * year, 2 * year);
        let counted = rate_at(&bank, 0);
        bank.pay_streamed_emissions(year).unwrap();
        assert_eq!(I80F48::from(bank.asset_share_value) - I80F48::ONE, counted);
    }

    /// The largest stream, `u64::MAX` in its last second, saturates its yearly pace. Over the
    /// smallest paying supply it prices down to a share value of 2^-20 and fails below it.
    #[test]
    fn the_largest_stream_over_the_smallest_supply_is_priced_down_to_its_limit() {
        let rate_at_share_value = |share_value: I80F48| {
            let mut bank = streaming_bank(MIN_EMISSIONS_SHARE_SUPPLY, u64::MAX, 1);
            bank.asset_share_value = share_value.into();
            NativeRateModel::new(&bank, 0).unwrap().rate_at(0)
        };
        assert_eq!(
            rate_at_share_value(I80F48::ONE).unwrap(),
            I80F48::MAX / MIN_EMISSIONS_SHARE_SUPPLY
        );
        assert_eq!(rate_at_share_value(I80F48::ONE >> 20).unwrap(), I80F48::MAX);
        assert!(rate_at_share_value(I80F48::ONE >> 21).is_err());
    }

    #[test]
    fn every_native_tag_prices_its_stream() {
        for tag in [ASSET_TAG_DEFAULT, ASSET_TAG_SOL, ASSET_TAG_STAKED] {
            let mut bank = streaming_bank(quarter_supply(), 1_000, 1_000);
            bank.config.asset_tag = tag;
            let rate = current_supply_apr(
                &bank,
                None,
                None,
                RewardsAccounts::default(),
                quarter_supply().to_num(),
                &Clock::default(),
            )
            .unwrap();
            assert_eq!(rate, I80F48::from_num(0.125));
        }
    }
}

/// Pacing samples accepted for converting slot-denominated rates.
#[cfg(test)]
mod slot_pacing {
    use super::*;

    #[test]
    fn only_a_live_looking_sample_yields_a_pacing() {
        // 400ms slots over the minimum one-hour window.
        assert_eq!(
            slots_per_second_from(9_000, 3_600).unwrap(),
            I80F48::from_num(2.5)
        );
        assert!(slots_per_second_from(9_000, 3_599).is_none());
        // The bounds themselves are usable; a hair outside either is not.
        assert_eq!(
            slots_per_second_from(3_600, 3_600).unwrap(),
            I80F48::from_num(SLOT_PACING_MIN_PER_SECOND)
        );
        assert_eq!(
            slots_per_second_from(14_400, 3_600).unwrap(),
            I80F48::from_num(SLOT_PACING_MAX_PER_SECOND)
        );
        assert!(slots_per_second_from(3_599, 3_600).is_none());
        assert!(slots_per_second_from(14_401, 3_600).is_none());
    }
}

/// Realized rates are read as share-index growth across a span, so the annualization must be exact
/// and a fall in the index must read as a negative rate.
#[cfg(test)]
mod realized_rates {
    use super::*;
    use bytemuck::Zeroable;
    use marginfi_type_crate::constants::INTEREST_MIN_WINDOW_SECONDS;

    const YEAR: i64 = 31_536_000;
    const WINDOW: u32 = INTEREST_MIN_WINDOW_SECONDS;

    /// A bank whose ring holds one reading `age` seconds before `now`, at a supply index of 1 and
    /// a debt index of 2.
    fn bank_with_reading(age: i64, now: i64) -> Bank {
        let mut bank = Bank::zeroed();
        let reading = RateReading::new(I80F48::ONE, I80F48::from_num(2), now - age).unwrap();
        bank.record_rate_reading(reading);
        bank
    }

    /// Growth is annualized over the reading's actual age, which is the youngest reading at least
    /// a window old; a reading older than the window lengthens the span.
    #[test]
    fn the_realized_rate_spans_the_youngest_reading_at_least_a_window_old() {
        let now = 1_700_000_000;
        let per_year = |age: i64| SECONDS_PER_YEAR / I80F48::from_num(age);

        let bank = bank_with_reading(i64::from(WINDOW), now);
        assert_eq!(
            realized_supply_apr(&bank, WINDOW, I80F48::from_num(1.0625), now).unwrap(),
            I80F48::from_num(0.0625) * per_year(i64::from(WINDOW))
        );

        let older = bank_with_reading(2 * i64::from(WINDOW), now);
        assert_eq!(
            realized_supply_apr(&older, WINDOW, I80F48::from_num(1.0625), now).unwrap(),
            I80F48::from_num(0.0625) * per_year(2 * i64::from(WINDOW))
        );
    }

    #[test]
    fn the_borrow_rate_is_measured_on_the_debt_index() {
        let now = 1_700_000_000;
        let bank = bank_with_reading(i64::from(WINDOW), now);
        assert_eq!(
            realized_borrow_apr(&bank, WINDOW, I80F48::from_num(2.125), now).unwrap(),
            I80F48::from_num(0.0625) * SECONDS_PER_YEAR / I80F48::from_num(WINDOW)
        );
    }

    #[test]
    fn a_ring_with_no_reading_old_enough_has_no_measurement() {
        let now = 1_700_000_000;
        let young = bank_with_reading(i64::from(WINDOW) - 1, now);
        for bank in [young, Bank::zeroed()] {
            assert_eq!(
                realized_supply_apr(&bank, WINDOW, I80F48::ONE, now).unwrap_err(),
                MarginfiError::RateHistoryTooShort.into()
            );
        }
    }

    #[test]
    fn growth_annualizes_over_the_span_it_was_measured_on() {
        // A quarter of a year at 25% growth annualizes to 100%.
        assert_eq!(
            realized_apr(I80F48::ONE, I80F48::from_num(1.25), YEAR / 4).unwrap(),
            I80F48::ONE
        );
        // The same growth over half a year is half the rate, and the anchor's scale cancels.
        assert_eq!(
            realized_apr(I80F48::from_num(2), I80F48::from_num(2.5), YEAR / 2).unwrap(),
            I80F48::from_num(0.5)
        );
        // A full year of growth is the growth itself.
        assert_eq!(
            realized_apr(I80F48::ONE, I80F48::from_num(1.0625), YEAR).unwrap(),
            I80F48::from_num(0.0625)
        );
    }

    #[test]
    fn a_flat_index_is_zero_and_a_falling_one_is_negative() {
        assert_eq!(
            realized_apr(I80F48::from_num(3), I80F48::from_num(3), YEAR).unwrap(),
            I80F48::ZERO
        );
        // Only a venue drawdown moves a supply index down; it reads as negative yield earned.
        assert_eq!(
            realized_apr(I80F48::ONE, I80F48::from_num(0.5), YEAR).unwrap(),
            I80F48::from_num(-0.5)
        );
    }

    #[test]
    fn a_zero_anchor_or_span_is_rejected() {
        assert!(realized_apr(I80F48::ZERO, I80F48::ONE, YEAR).is_err());
        assert!(realized_apr(I80F48::ONE, I80F48::ONE, 0).is_err());
        assert!(realized_apr(I80F48::ONE, I80F48::ONE, -1).is_err());
    }
}

/// An emissions deposit enters a native bank's yield index and supply rate evenly over
/// `EMISSIONS_SPREAD_SECONDS`. Every figure here is exact in I80F48.
#[cfg(test)]
mod emissions_spread {
    use super::*;
    use crate::state::interest_rate::LendingCurve;
    use bytemuck::Zeroable;
    use marginfi_type_crate::constants::EMISSIONS_SPREAD_SECONDS;
    use marginfi_type_crate::types::{
        make_points, milli_to_u32, InterestRateConfig, INTEREST_CURVE_SEVEN_POINT,
    };

    const T0: i64 = 1_700_000_000;
    /// Deposits before the drop: four spreads' worth of native units. A drop of one spread's worth
    /// lifts the share value from 1 to 1.25 and spreads at exactly one unit a second.
    const SUPPLY: i64 = 4 * EMISSIONS_SPREAD_SECONDS;
    const DROP: u64 = EMISSIONS_SPREAD_SECONDS as u64;

    /// A bank holding `SUPPLY` at a share value of 1 that has just taken a `DROP` at `T0`.
    fn dropped_bank() -> Bank {
        let mut bank = Bank::zeroed();
        bank.total_asset_shares = I80F48::from_num(SUPPLY).into();
        bank.asset_share_value = I80F48::from_num(1.25).into();
        bank.spread_emissions(DROP, I80F48::ONE, T0).unwrap();
        bank
    }

    #[test]
    fn a_deposit_leaves_the_yield_index_where_it_was() {
        let bank = dropped_bank();
        assert_eq!(
            I80F48::from(bank.emissions_spread_pending),
            I80F48::from_num(DROP)
        );
        assert_eq!(I80F48::from(bank.emissions_spread_rate), I80F48::ONE);
        assert_eq!(
            I80F48::from(bank.emissions_spread_base),
            I80F48::from_num(SUPPLY)
        );
        assert_eq!(bank.emissions_spread_updated_at, T0);
        assert_eq!(yield_index_of(&bank, I80F48::ONE, T0).unwrap(), I80F48::ONE);
    }

    #[test]
    fn the_yield_index_takes_the_deposit_in_evenly_and_then_holds() {
        let bank = dropped_bank();
        let half = T0 + EMISSIONS_SPREAD_SECONDS / 2;
        assert_eq!(pending_emissions(&bank, half), I80F48::from_num(DROP / 2));
        assert_eq!(
            yield_index_of(&bank, I80F48::ONE, half).unwrap(),
            I80F48::from_num(1.25) / I80F48::from_num(1.125)
        );

        let end = T0 + EMISSIONS_SPREAD_SECONDS;
        for now in [end, end + EMISSIONS_SPREAD_SECONDS] {
            assert_eq!(pending_emissions(&bank, now), I80F48::ZERO);
            assert_eq!(emissions_spread_rate_at(&bank, now), I80F48::ZERO);
            assert_eq!(
                yield_index_of(&bank, I80F48::ONE, now).unwrap(),
                I80F48::from_num(1.25)
            );
        }
    }

    /// The venue multiplier scales the index; the spread divides the result.
    #[test]
    fn the_spread_applies_on_top_of_the_venue_multiplier() {
        let bank = dropped_bank();
        let half = T0 + EMISSIONS_SPREAD_SECONDS / 2;
        assert_eq!(
            yield_index_of(&bank, I80F48::from_num(2), half).unwrap(),
            I80F48::from_num(2.5) / I80F48::from_num(1.125)
        );
    }

    /// A donation into a bank still spreading a drop adds its tokens at the same rate: the spread
    /// runs past the original end by exactly the donation's own seconds.
    #[test]
    fn a_small_deposit_cannot_slow_the_spread() {
        let mut bank = dropped_bank();
        let half = T0 + EMISSIONS_SPREAD_SECONDS / 2;
        let before = I80F48::from(bank.asset_share_value);
        bank.asset_share_value = (before + I80F48::from_num(4) / I80F48::from_num(SUPPLY)).into();
        bank.spread_emissions(4, before, half).unwrap();

        assert_eq!(I80F48::from(bank.emissions_spread_rate), I80F48::ONE);
        let end = T0 + EMISSIONS_SPREAD_SECONDS;
        assert_eq!(pending_emissions(&bank, end), I80F48::from_num(4));
        assert_eq!(pending_emissions(&bank, end + 4), I80F48::ZERO);
    }

    /// A drop larger than what is pending re-spreads the whole remainder over a fresh spread.
    #[test]
    fn a_large_deposit_raises_the_rate() {
        let mut bank = dropped_bank();
        let half = T0 + EMISSIONS_SPREAD_SECONDS / 2;
        let before = I80F48::from(bank.asset_share_value);
        let drop = 3 * DROP / 2;
        bank.asset_share_value =
            (before + I80F48::from_num(drop) / I80F48::from_num(SUPPLY)).into();
        bank.spread_emissions(drop, before, half).unwrap();

        assert_eq!(
            I80F48::from(bank.emissions_spread_rate),
            I80F48::from_num(2)
        );
        assert_eq!(
            pending_emissions(&bank, half + EMISSIONS_SPREAD_SECONDS - 1),
            I80F48::from_num(2)
        );
        assert_eq!(
            pending_emissions(&bank, half + EMISSIONS_SPREAD_SECONDS),
            I80F48::ZERO
        );
    }

    /// The spread emissions are shared over the supply after the deposit, the same supply the bank
    /// holds once that deposit lands, and stop once the drop is fully spread.
    #[test]
    fn the_supply_rate_counts_emissions_over_the_larger_supply() {
        let mut bank = dropped_bank();
        bank.config.interest_rate_config = InterestRateConfig {
            hundred_util_rate: milli_to_u32(I80F48::ONE),
            points: make_points(&[]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };
        let assets = 5 * EMISSIONS_SPREAD_SECONDS;
        let half = T0 + EMISSIONS_SPREAD_SECONDS / 2;
        let model = NativeRateModel::new(&bank, half).unwrap();
        assert_eq!(
            model.rate_at(0).unwrap(),
            SECONDS_PER_YEAR / I80F48::from_num(assets)
        );
        assert_eq!(
            model.rate_at(assets as u64).unwrap(),
            SECONDS_PER_YEAR / I80F48::from_num(2 * assets)
        );

        bank.liability_share_value = I80F48::ONE.into();
        bank.total_liability_shares = I80F48::from_num(assets / 2).into();
        let lending = LendingCurve::new(&bank.config.interest_rate_config)
            .unwrap()
            .lending_rate(I80F48::from_num(0.5))
            .unwrap();
        assert_eq!(
            NativeRateModel::new(&bank, half)
                .unwrap()
                .rate_at(0)
                .unwrap(),
            lending + SECONDS_PER_YEAR / I80F48::from_num(assets)
        );
        assert_eq!(
            NativeRateModel::new(&bank, T0 + EMISSIONS_SPREAD_SECONDS)
                .unwrap()
                .rate_at(0)
                .unwrap(),
            lending
        );
    }

    /// A rate reading taken at the drop and one taken a half spread later realize the spread
    /// share-value growth, not the jump.
    #[test]
    fn the_realized_supply_rate_counts_emissions_between_readings() {
        let mut bank = dropped_bank();
        let half_spread = EMISSIONS_SPREAD_SECONDS / 2;
        bank.take_rate_reading(I80F48::ONE, T0).unwrap();
        let now = T0 + half_spread;
        let index_now = yield_index_of(&bank, I80F48::ONE, now).unwrap();
        assert_eq!(
            realized_supply_apr(&bank, half_spread as u32, index_now, now).unwrap(),
            (I80F48::from_num(1.25) / I80F48::from_num(1.125) - I80F48::ONE) * SECONDS_PER_YEAR
                / I80F48::from_num(half_spread)
        );
    }

    #[test]
    fn a_bank_without_emissions_keeps_its_raw_index() {
        let mut bank = Bank::zeroed();
        bank.asset_share_value = I80F48::from_num(1.5).into();
        assert_eq!(
            yield_index_of(&bank, I80F48::from_num(2), T0).unwrap(),
            I80F48::from_num(3)
        );
        assert_eq!(emissions_spread_rate_at(&bank, T0), I80F48::ZERO);

        // A share value the deposit did not move leaves nothing to spread.
        bank.spread_emissions(1, I80F48::from_num(1.5), T0).unwrap();
        assert_eq!(I80F48::from(bank.emissions_spread_pending), I80F48::ZERO);
    }

    /// Ten small donations across the spread keep its rate and push its end out by exactly their
    /// own tokens' seconds.
    #[test]
    fn repeated_small_deposits_never_slow_the_spread() {
        let mut bank = dropped_bank();
        let step = EMISSIONS_SPREAD_SECONDS / 20;
        for k in 1..=10 {
            let before = I80F48::from(bank.asset_share_value);
            bank.asset_share_value =
                (before + I80F48::from_num(4) / I80F48::from_num(SUPPLY)).into();
            bank.spread_emissions(4, before, T0 + k * step).unwrap();
            assert_eq!(I80F48::from(bank.emissions_spread_rate), I80F48::ONE);
        }
        let end = T0 + EMISSIONS_SPREAD_SECONDS + 40;
        assert_eq!(pending_emissions(&bank, end - 1), I80F48::ONE);
        assert_eq!(pending_emissions(&bank, end), I80F48::ZERO);
    }

    /// The largest drop a token amount allows, into the smallest supply that accepts emissions,
    /// spreads without overflow: the index stays at 1 and the rate is the drop over the spread.
    #[test]
    fn the_largest_drop_spreads_without_overflow() {
        let mut bank = Bank::zeroed();
        bank.config.interest_rate_config = InterestRateConfig {
            points: make_points(&[]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };
        let shares = MIN_EMISSIONS_SHARE_SUPPLY;
        bank.total_asset_shares = shares.into();
        let assets = shares + I80F48::from_num(u64::MAX);
        bank.asset_share_value = (assets / shares).into();
        bank.spread_emissions(u64::MAX, I80F48::ONE, T0).unwrap();

        assert_eq!(yield_index_of(&bank, I80F48::ONE, T0).unwrap(), I80F48::ONE);
        let rate = I80F48::from_num(u64::MAX) / I80F48::from_num(EMISSIONS_SPREAD_SECONDS);
        assert_eq!(I80F48::from(bank.emissions_spread_rate), rate);
        assert_eq!(
            NativeRateModel::new(&bank, T0).unwrap().rate_at(0).unwrap(),
            rate * SECONDS_PER_YEAR / assets
        );
    }

    /// The share-value growth of a deposit times the growth still pending must stay under I80F48's
    /// 2^79 ceiling: at 2^78 the deposit spreads, at 2^79 it reverts and the spread is untouched.
    #[test]
    fn a_deposit_past_the_fixed_point_range_reverts() {
        let pending_growth_of = |exponent: u32| {
            let mut bank = Bank::zeroed();
            bank.emissions_spread_pending =
                (I80F48::from_num(1u128 << exponent) - I80F48::ONE).into();
            bank.emissions_spread_base = I80F48::ONE.into();
            bank.emissions_spread_rate = I80F48::ONE.into();
            bank.emissions_spread_updated_at = T0;
            bank
        };

        let mut bank = pending_growth_of(39);
        bank.asset_share_value = I80F48::from_num(1u128 << 39).into();
        bank.spread_emissions(1, I80F48::ONE, T0).unwrap();

        let mut bank = pending_growth_of(39);
        bank.asset_share_value = I80F48::from_num(1u128 << 40).into();
        let untouched = bank;
        assert_eq!(
            bank.spread_emissions(1, I80F48::ONE, T0).unwrap_err(),
            MarginfiError::MathError.into()
        );
        assert_eq!(bank, untouched);
    }

    /// A rate so large that its spread saturates I80F48 leaves nothing pending.
    #[test]
    fn a_saturated_spread_leaves_nothing_pending() {
        let mut bank = Bank::zeroed();
        bank.emissions_spread_pending = I80F48::from_num(1u64 << 62).into();
        bank.emissions_spread_rate = I80F48::from_num(1u64 << 60).into();
        bank.emissions_spread_updated_at = T0;
        let now = T0 + (1 << 30);
        assert_eq!(pending_emissions(&bank, now), I80F48::ZERO);
        assert_eq!(emissions_spread_rate_at(&bank, now), I80F48::ZERO);
    }

    /// Every native asset tag, the banks emissions deposits accept, prices its spread emissions.
    #[test]
    fn every_native_tag_counts_its_emissions() {
        let half = T0 + EMISSIONS_SPREAD_SECONDS / 2;
        let clock = Clock {
            unix_timestamp: half,
            ..Clock::default()
        };
        for tag in [ASSET_TAG_DEFAULT, ASSET_TAG_SOL, ASSET_TAG_STAKED] {
            let mut bank = dropped_bank();
            bank.config.asset_tag = tag;
            bank.config.interest_rate_config = InterestRateConfig {
                points: make_points(&[]),
                curve_type: INTEREST_CURVE_SEVEN_POINT,
                ..Default::default()
            };
            assert_eq!(
                current_supply_apr(&bank, None, None, RewardsAccounts::default(), 0, &clock)
                    .unwrap(),
                SECONDS_PER_YEAR / I80F48::from_num(5 * EMISSIONS_SPREAD_SECONDS)
            );
        }
    }
}
