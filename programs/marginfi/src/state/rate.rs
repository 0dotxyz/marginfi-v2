//! Current SUPPLY (lender) APR per bank, normalized to I80F48 (1.0 == 100%) and NET of each
//! venue's protocol cut, so rates are comparable across venues for the auto-rebalance order.
//!
//! The protocol-faithful rate math lives in each integration's mock crate (`MinimalReserve::supply_apr`,
//! `MinimalSpotMarket::deposit_rate`, `TokenReserve::supply_rate`), co-located with the state mirror it
//! reads and unit-tested there. This module only dispatches by `asset_tag`, loads/staleness-checks
//! the rate-bearing account, and maps the pure `Option` result to a marginfi error:
//!
//! - Native marginfi banks: read the cached `lending_rate` (net by construction; fees fall on
//!   borrowers). Must be fresh (crank `accrue_bank_interest`/`update_bank_cache` first).
//! - Kamino: `borrow_apr(util) * util * (1 - protocol_take_rate)`, rescaled from klend's slot-year
//!   to a wall-clock year at measured chain pacing.
//! - Drift: `borrow_apr(util) * util * (1 - insurance_fund.total_factor)`.
//! - Solend: `borrow_apr(util) * util * (1 - protocol_take_rate)` (3-slope borrow curve).
//! - JupLend: the liquidity-layer supply rate (rewards APR is layered on OFF-CHAIN by the
//!   keeper; the on-chain figure is the conservative base gate).
//!
//! [`yield_index_of`] / [`debt_index_of`] serve the REALIZED rate: growth of a monotonic share
//! index across a span, which a single-transaction spike cannot move.
//!
//! Integration reserve/market accounts MUST be refreshed in the same slot by the caller
//! (`refresh_reserve` / `update_spot_market_cumulative_interest` / JupLend liquidity-program
//! `update_exchange_price`, which refreshes the `TokenReserve` the supply rate reads).

use crate::state::price::{
    load_drift_spot_market, load_juplend_lending, load_kamino_reserve, load_solend_reserve,
    OraclePriceFeedAdapter,
};
use crate::{check, math_error, prelude::*, utils::is_integration_asset_tag};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use drift_mocks::state::MinimalSpotMarket;
use fixed::types::I80F48;
use juplend_mocks::state::{LendingRewardsRateModel, TokenReserve, EXCHANGE_PRICES_PRECISION};
use kamino_mocks::state::{MinimalLendingMarket, KLEND_SLOTS_PER_SECOND};
use marginfi_type_crate::constants::{
    ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOL,
    ASSET_TAG_SOLEND, ASSET_TAG_STAKED, SECONDS_PER_YEAR,
};
use marginfi_type_crate::types::{u32_to_milli, Bank, BankConfig, OraclePriceType};

/// Accounts a venue needs beyond its rate-bearing account to price its reward emissions, each bound
/// to the bank's own venue state. Callers that need only the base rate pass the default.
#[derive(Default, Clone, Copy)]
pub struct RewardsAccounts<'info> {
    /// Kamino: the reserve's `LendingMarket`, which caps the emission APR.
    pub lending_market: Option<&'info AccountInfo<'info>>,
    /// JupLend: the `LendingRewardsRateModel` the bank's `Lending` references.
    pub rewards_model: Option<&'info AccountInfo<'info>>,
    /// JupLend: the fToken mint, whose supply is the rewards denominator.
    pub ftoken_mint: Option<&'info AccountInfo<'info>>,
}

/// Supply APR (I80F48, 1.0 == 100%) for `bank`, dispatched on `asset_tag` (the canonical integration
/// identifier, consistent with the `is_*_asset_tag` checks used across deposit/withdraw). `venue` is
/// the rate-bearing account (`None` for native, which prices from the bank cache); `token_reserve`
/// is JupLend's `TokenReserve` (`None` otherwise). Unknown tags fail rather than default to
/// native. The caller refreshes the venue this slot and locates it (see `rate_of`).
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
        // Native banks price from the stored cache, so `extra_native` does not change the rate.
        return Ok(u32_to_milli(bank.cache.lending_rate));
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
    )?;
    Ok(priced.price_multiplier)
}

/// Monotonic per-share supply index for `bank`: `asset_share_value` times the venue exchange-rate
/// multiplier, excluding the oracle spot price. Growth over a window is the realized supply yield a
/// depositor earned; being an accrued integral, a single-tx rate spike cannot move it.
pub fn yield_index_of(bank: &Bank, multiplier: I80F48) -> MarginfiResult<I80F48> {
    I80F48::from(bank.asset_share_value)
        .checked_mul(multiplier)
        .ok_or_else(math_error!())
        .map_err(Into::into)
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
    let base = reserve
        .supply_rate_at(extra_native)
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

/// Every supply-rate path must return I80F48 in the same units (`1.0 == 100%`), so the rebalance
/// order can rank a native bank's rate directly against any integration bank's rate. For each target
/// percentage this builds the equivalent per-venue config and asserts every venue reports the SAME
/// percentage.
#[cfg(test)]
mod unit_consistency {
    use drift_mocks::state::drift_deposit_rate_from_parts;
    use juplend_mocks::state::juplend_supply_rate_from_parts;
    use kamino_mocks::state::{kamino_supply_apr_from_parts, CurvePoint, KLEND_SLOTS_PER_SECOND};
    use marginfi_type_crate::types::{milli_to_u32, u32_to_milli};
    use solend_mocks::state::solend_supply_rate_from_parts;

    use super::I80F48;

    /// The net supply rate each venue reports for `target_bps` (e.g. `1_000` == 10%), built from an
    /// equivalent per-venue config. Returned as `(native, kamino, drift, solend, juplend)`.
    fn venue_rates(target_bps: u32) -> (I80F48, I80F48, I80F48, I80F48, I80F48) {
        // The target percentage as an I80F48 fraction (1.0 == 100%).
        let pct = I80F48::from_num(target_bps) / I80F48::from_num(10_000u32);

        // Native: the bank cache stores the lending rate as a u32 on a 0..1000% scale.
        let native = u32_to_milli(milli_to_u32(pct));

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

            // Native stores its rate as a u32 on a 0..1000% scale. Its exact value is that same
            // quantization round-trip, so assert against the stored-value round-trip, not the input.
            let native_quantized = u32_to_milli(milli_to_u32(expected));
            assert_eq!(native, native_quantized, "native at {target_bps}bps");
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

    const YEAR: i64 = 31_536_000;

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
