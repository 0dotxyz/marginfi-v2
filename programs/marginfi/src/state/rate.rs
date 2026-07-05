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
//! - Kamino: `borrow_apr(util) * util * (1 - protocol_take_rate)`.
//! - Drift: `borrow_apr(util) * util * (1 - insurance_fund.total_factor)`.
//! - Solend: `borrow_apr(util) * util * (1 - protocol_take_rate)` (3-slope borrow curve).
//! - JupLend: the Fluid liquidity-layer supply rate (rewards APR is layered on OFF-CHAIN by the
//!   keeper; the on-chain figure is the conservative base gate).
//!
//! Integration reserve/market accounts MUST be refreshed in the same slot by the caller
//! (`refresh_reserve` / `update_spot_market_cumulative_interest` / JupLend liquidity-program
//! `update_exchange_price`, which refreshes the `TokenReserve` the supply rate reads).

use crate::state::price::{
    load_drift_spot_market, load_juplend_lending, load_kamino_reserve, load_solend_reserve,
};
use crate::{check, math_error, prelude::*, utils::is_integration_asset_tag};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use juplend_mocks::state::TokenReserve;
use marginfi_type_crate::constants::{
    ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOL,
    ASSET_TAG_SOLEND, ASSET_TAG_STAKED,
};
use marginfi_type_crate::types::{u32_to_milli, Bank, BankConfig};

/// Supply APR (I80F48, 1.0 == 100%) for `bank`, dispatched on `asset_tag` (the canonical integration
/// identifier, consistent with the `is_*_asset_tag` checks used across deposit/withdraw). `venue` is
/// the rate-bearing account (`None` for native, which prices from the bank cache); `token_reserve`
/// is JupLend's Fluid `TokenReserve` (`None` otherwise). Unknown tags fail rather than default to
/// native. The caller refreshes the venue this slot and locates it (see `rate_of`).
pub fn current_supply_apr<'info>(
    bank: &Bank,
    venue: Option<&'info AccountInfo<'info>>,
    token_reserve: Option<&AccountInfo>,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let tag = bank.config.asset_tag;
    if matches!(tag, ASSET_TAG_DEFAULT | ASSET_TAG_SOL | ASSET_TAG_STAKED) {
        return Ok(u32_to_milli(bank.cache.lending_rate));
    }
    let venue = venue.ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    match tag {
        ASSET_TAG_KAMINO => kamino_supply_apr(&bank.config, venue, clock),
        ASSET_TAG_DRIFT => drift_supply_apr(&bank.config, venue, clock),
        ASSET_TAG_SOLEND => solend_supply_apr(&bank.config, venue),
        ASSET_TAG_JUPLEND => juplend_supply_apr(&bank.config, venue, token_reserve, clock),
        _ => err!(MarginfiError::InvalidOracleSetup),
    }
}

/// Current supply APR for `bank` (I80F48, 1.0 == 100%), via [`current_supply_apr`] (dispatched on
/// `asset_tag`). The integration venue is the LAST remaining oracle account — the price oracle, if
/// any, precedes it, so `oracle_ais.last()` works for fixed and live-oracle variants alike; native
/// banks have none. `token_reserve` is JupLend's `TokenReserve`.
pub fn rate_of<'info>(
    bank: &Bank,
    oracle_ais: &'info [AccountInfo<'info>],
    token_reserve: Option<&AccountInfo>,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let venue = if is_integration_asset_tag(bank.config.asset_tag) {
        oracle_ais.last()
    } else {
        None
    };
    current_supply_apr(bank, venue, token_reserve, clock)
}

// The venue account is supplied by a permissionless keeper, so it must be tied to the bank: these
// loaders `require_keys_eq!(venue.key, bank_config.oracle_keys[1])` (and check owner+discriminator),
// exactly as the pricing path does. Without this a keeper could pass a fabricated-rate account.
fn kamino_supply_apr<'info>(
    bank_config: &BankConfig,
    reserve_ai: &'info AccountInfo<'info>,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let loader = load_kamino_reserve(bank_config, reserve_ai)?;
    let r = loader.load()?;
    check!(!r.is_stale(clock.slot), MarginfiError::ReserveStale);
    Ok(r.supply_apr().ok_or_else(math_error!())?)
}

fn drift_supply_apr<'info>(
    bank_config: &BankConfig,
    spot_ai: &'info AccountInfo<'info>,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let loader = load_drift_spot_market(bank_config, spot_ai)?;
    let m = loader.load()?;
    check!(
        !m.is_stale(clock.unix_timestamp),
        MarginfiError::DriftSpotMarketStale
    );
    Ok(m.deposit_rate().ok_or_else(math_error!())?)
}

// `SolendMinimalReserve::is_stale` reads the clock itself (slot-based), so no `clock` is needed here.
fn solend_supply_apr<'info>(
    bank_config: &BankConfig,
    reserve_ai: &'info AccountInfo<'info>,
) -> MarginfiResult<I80F48> {
    let loader = load_solend_reserve(bank_config, reserve_ai)?;
    let r = loader.load()?;
    check!(!r.is_stale()?, MarginfiError::SolendReserveStale);
    Ok(r.supply_rate().ok_or_else(math_error!())?)
}

/// JupLend liquidity-layer supply rate. `lending_ai` is the bank's Lending account (the venue,
/// validated against `bank_config.oracle_keys[1]`); `token_reserve` is the Fluid `TokenReserve` it
/// references, validated here against `lending.token_reserves_liquidity` before its rate is read.
fn juplend_supply_apr<'info>(
    bank_config: &BankConfig,
    lending_ai: &'info AccountInfo<'info>,
    token_reserve: Option<&AccountInfo>,
    clock: &Clock,
) -> MarginfiResult<I80F48> {
    let tr = token_reserve.ok_or(MarginfiError::WrongNumberOfOracleAccounts)?;
    let loader = load_juplend_lending(bank_config, lending_ai)?;
    require_keys_eq!(
        *tr.key,
        loader.load()?.token_reserves_liquidity,
        MarginfiError::JuplendLendingValidationFailed
    );

    let reserve = TokenReserve::from_account_data(&tr.try_borrow_data()?)
        .ok_or(error!(MarginfiError::JuplendLendingValidationFailed))?;
    check!(
        !reserve.is_stale(clock.unix_timestamp),
        MarginfiError::JuplendLendingStale
    );
    Ok(reserve.supply_rate().ok_or_else(math_error!())?)
}

/// Every supply-rate path must return I80F48 in the same units — `1.0 == 100%` — so the rebalance
/// order can rank a native bank's rate directly against any integration bank's rate. For each target
/// percentage this builds the equivalent per-venue config and asserts every venue reports the SAME
/// percentage.
#[cfg(test)]
mod unit_consistency {
    use drift_mocks::state::drift_deposit_rate_from_parts;
    use juplend_mocks::state::juplend_supply_rate_from_parts;
    use kamino_mocks::state::{kamino_supply_apr_from_parts, CurvePoint};
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

        // Kamino: a flat borrow curve at `target_bps`, evaluated at 100% utilization with no cut.
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
            0,            // insurance total_factor
        )
        .unwrap();

        // Solend: I80F48 ratios. At util == optimal_util, borrow_rate == optimal_borrow_rate; no cut.
        let solend = solend_supply_rate_from_parts(
            I80F48::from_num(1), // utilization
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
        // Native stores its rate as a u32 on a 0..1000% scale, so its percentage is quantized to the
        // nearest ~2.3e-9; the integration venues compute through exact fixed-point paths and must be
        // bit-for-bit equal.
        let native_quantization = I80F48::from_num(1e-6);

        for target_bps in [500u32, 1_000, 2_500, 5_000] {
            let expected = I80F48::from_num(target_bps) / I80F48::from_num(10_000u32);
            let (native, kamino, drift, solend, juplend) = venue_rates(target_bps);

            // Direct cross-venue equality: all integration venues report the exact same percentage.
            assert_eq!(kamino, expected, "kamino at {target_bps}bps");
            assert_eq!(drift, kamino, "drift != kamino at {target_bps}bps");
            assert_eq!(solend, kamino, "solend != kamino at {target_bps}bps");
            assert_eq!(juplend, kamino, "juplend != kamino at {target_bps}bps");

            // Native matches that same percentage within its u32 quantization.
            assert!(
                (native - kamino).abs() < native_quantization,
                "native {native} != {kamino} at {target_bps}bps"
            );
        }
    }
}
