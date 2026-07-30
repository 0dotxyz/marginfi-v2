use crate::{assert_struct_align, assert_struct_size, math_error, KaminoMocksError};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::price::{
    collateral_to_liquidity_from_scaled, convert_decimals as shared_convert_decimals,
    liquidity_to_collateral_from_scaled, scale_supplies,
};

// Constants for account discriminators
pub const RESERVE_DISCRIMINATOR: [u8; 8] = [43, 242, 204, 202, 26, 247, 59, 127];
pub const LENDING_MARKET_DISCRIMINATOR: [u8; 8] = [246, 114, 50, 98, 72, 157, 28, 120];
pub const OBLIGATION_DISCRIMINATOR: [u8; 8] = [168, 206, 141, 106, 88, 76, 172, 167];

/// Mirrors Kamino's `CurvePoint` (`BorrowRateCurve` point). bps: 10_000 = 100%.
/// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/utils/borrow_rate_curve.rs#L74-L91
#[zero_copy]
#[repr(C)]
pub struct CurvePoint {
    pub utilization_rate_bps: u32,
    pub borrow_rate_bps: u32,
}

/// Mirrors Kamino's `BorrowRateCurve`: a fixed 11-point curve.
/// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/utils/borrow_rate_curve.rs#L23-L25
#[zero_copy]
#[repr(C)]
pub struct BorrowRateCurve {
    pub points: [CurvePoint; 11],
}

assert_struct_size!(CurvePoint, 8);
assert_struct_size!(BorrowRateCurve, 88);
assert_struct_size!(ReserveConfig, 952);
assert_struct_align!(ReserveConfig, 8);
/// Mirrors Kamino's `ReserveConfig` through `borrow_rate_curve`; the remaining trailing fields are
/// grouped as `_rest`. Total size matches Kamino's `RESERVE_CONFIG_SIZE` (952).
/// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/reserve.rs#L1573-L1602
#[zero_copy]
#[repr(C)]
pub struct ReserveConfig {
    pub status: u8,
    pub asset_tier: u8,
    pub host_fixed_interest_rate_bps: u16,
    pub min_deleveraging_bonus_bps: u16,
    pub block_ctoken_usage: u8,
    pub early_repay_remaining_interest_pct: u8,
    pub emergency_mode: u8,
    pub _padding1: [u8; 4],
    pub protocol_order_execution_fee_pct: u8,
    /// Percentage of interest taken by the protocol (0..100). Read as `from_percent(pct)`.
    pub protocol_take_rate_pct: u8,
    pub _padding2: [u8; 48],
    pub _padding3: [u8; 1],
    pub borrow_rate_curve: BorrowRateCurve,
    pub borrow_factor_pct: u64,
    /// Total liquidity ceiling in native mint units; `u64::MAX` is unlimited.
    pub deposit_limit: u64,
    pub borrow_limit: u64,
    pub _padding4: [u8; 512],
    pub _padding5: [u8; 128],
    pub _padding6: [u8; 96],
    pub _padding7: [u8; 24],
    /// Reward tokens emitted per slot, native mint units.
    pub rewards_amount_per_slot: u64,
    pub _padding8: [u8; 8],
}

assert_struct_size!(MinimalReserve, 8616);
assert_struct_size!(MinimalObligation, 3336);
assert_struct_align!(MinimalReserve, 8);
assert_struct_align!(MinimalObligation, 8);

#[account(zero_copy, discriminator = &RESERVE_DISCRIMINATOR)]
#[repr(C)]
pub struct MinimalReserve {
    pub version: u64,

    // `LastUpdate`
    /// Kamino reserves are only good for one slot, e.g. `refresh_reserve` must have run within the
    /// same slot as any ix that needs a non-stale reserve e.g. withdraw.
    pub slot: u64,
    /// True if the reserve is stale, which will cause various ixes like withdraw to fail. Typically
    /// set to true in any tx that modifies reserve balance, and set to false at the end of a
    /// successful `refresh_reserve`
    /// * 0 = false, 1 = true
    pub stale: u8,
    /// Each bit represents a passed check in price status.
    /// * 63 = all checks passed
    ///
    /// Otherwise:
    /// * PRICE_LOADED =        0b_0000_0001; // 1
    /// * PRICE_AGE_CHECKED =   0b_0000_0010; // 2
    /// * TWAP_CHECKED =        0b_0000_0100; // 4
    /// * TWAP_AGE_CHECKED =    0b_0000_1000; // 8
    /// * HEURISTIC_CHECKED =   0b_0001_0000; // 16
    /// * PRICE_USAGE_ALLOWED = 0b_0010_0000; // 32
    pub price_status: u8,
    pub placeholder: [u8; 6],

    // Fills up to the offset of `ReserveLiquidity`
    pub lending_market: Pubkey,

    pub farm_collateral: Pubkey,
    pub farm_debt: Pubkey,

    // `ReserveLiquidity`
    pub mint_pubkey: Pubkey,
    /// * A PDA
    pub supply_vault: Pubkey,
    /// * A PDA
    pub fee_vault: Pubkey,
    /// In simple terms: (amount in supply vault - outstanding borrows)
    /// * In token, with `mint_decimals`
    pub available_amount: u64,
    /// * In token, with `mint_decimals`
    /// * Actually an I68F60, stored as a u128 (i.e. BN) in Kamino.
    pub borrowed_amount_sf: [u8; 16],
    /// * Actually an I68F60, stored as a u128 (i.e. BN) in Kamino.
    pub market_price_sf: [u8; 16],
    pub market_price_last_updated_ts: u64,
    pub mint_decimals: u64,

    // Fields from deposit_limit_crossed_timestamp to cumulative_borrow_rate_bsf
    pub deposit_limit_crossed_timestamp: u64,
    pub borrow_limit_crossed_timestamp: u64,
    pub cumulative_borrow_rate_bsf: [u8; 48],

    // Fields for exchange rate calculation
    /// * In token, with `mint_decimals`
    /// * Actually an I68F60, stored as a u128 (i.e. BN) in Kamino.
    pub accumulated_protocol_fees_sf: [u8; 16],
    /// * In token, with `mint_decimals`
    /// * Actually an I68F60, stored as a u128 (i.e. BN) in Kamino.
    pub accumulated_referrer_fees_sf: [u8; 16],
    /// * In token, with `mint_decimals`
    /// * Actually an I68F60, stored as a u128 (i.e. BN) in Kamino.
    pub pending_referrer_fees_sf: [u8; 16],
    /// * In token, with `mint_decimals`
    /// * Actually an I68F60, stored as a u128 (i.e. BN) in Kamino.
    pub absolute_referral_rate_sf: [u8; 16],
    /// Token or Token22. If token22, note that Kamino does not support all Token22 extensions.
    pub token_program: Pubkey,
    /// Undistributed reward tokens, native mint units. `distribute_rewards` moves these into
    /// `available_amount`, raising the cToken exchange rate.
    pub rewards_amount_available: u64,
    // Padding to completion of ReserveLiquidity
    _padding1: [u8; 512],
    _padding2: [u8; 256],
    _padding3: [u8; 128],
    _padding4: [u8; 16],
    // end of reserve liquidity
    _padding5: [u8; 1024],
    _padding6: [u8; 128],
    _padding7: [u8; 48],

    // ReserveCollateral section
    /// Mints collateral tokens
    /// * A PDA
    /// * technically 6 decimals, but uses `mint_decimals` regardless for all purposes
    /// * authority = lending_market_authority
    pub collateral_mint_pubkey: Pubkey,
    /// Total number of collateral tokens
    /// * uses `mint_decimals`, even though it's technically 6 decimals under the hood
    pub mint_total_supply: u64,
    /// * A PDA
    pub collateral_supply_vault: Pubkey,

    _padding8: [u8; 1024],

    _padding9: [u8; 1024],
    _padding10: [u8; 128],
    _padding11: [u8; 48],
    pub config: ReserveConfig,
    _padding12: [u8; 2048],
    _padding13: [u8; 512],
    _padding14: [u8; 256],
}

/// Kamino's `LendingMarket`, mirrored only as far as `reserve_rewards_max_apr_bps`, which caps
/// every reserve's rewards emission. Size matches klend's `LENDING_MARKET_SIZE`.
/// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/lending_market.rs#L230
#[account(zero_copy, discriminator = &LENDING_MARKET_DISCRIMINATOR)]
#[repr(C)]
pub struct MinimalLendingMarket {
    _padding1: [u8; 2048],
    _padding2: [u8; 1024],
    _padding3: [u8; 256],
    _padding4: [u8; 22],
    /// Ceiling on the APR a reserve may emit as rewards, in bps.
    pub reserve_rewards_max_apr_bps: u16,
    _padding5: [u8; 1024],
    _padding6: [u8; 256],
    _padding7: [u8; 24],
}

const _: () = assert!(core::mem::size_of::<MinimalLendingMarket>() == 4656);

// Notable Kamino naming conventions:
// * `mint_total_supply` aka `total_col` - total amount of collateral tokens that exist
// * `total_supply` aka `total_liq` - total amount of liquidity tokens under the reserve's control
impl MinimalReserve {
    /// Returns `(total_liquidity_tokens, total_collateral_tokens)` both in “no-decimals” I80F48
    /// form (i.e. scaled down by 10^mint_decimals).
    /// klend builds the same (liquidity = total_supply, collateral = mint_total_supply) pair in
    /// `Reserve::collateral_exchange_rate`; we additionally divide both by 10^mint_decimals:
    /// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/reserve.rs#L549-L552
    pub fn scaled_supplies(&self) -> Result<(I80F48, I80F48)> {
        let total_liq_raw = self.calculate_total_supply_i80f48();
        let (total_liq, total_col) = scale_supplies(
            total_liq_raw,
            self.mint_total_supply,
            self.mint_decimals as u8,
        )
        .ok_or_else(math_error!())?;
        Ok((total_liq, total_col))
    }

    // Note: our conversion has less precision than Kamino's internal representation (which uses
    //  U256 to avoid any precision loss), but sufficient for our purposes because we only use these
    //  to sanity check that the user got the expected amount of tokens +/- 1 when
    //  depositing/withdrawing

    /// Convert collateral tokens to equivalent liquidity tokens. Mirrors klend
    /// `CollateralExchangeRate::collateral_to_liquidity`:
    /// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/reserve.rs#L1404-L1441
    /// * Returns liquidity tokens (uses `mint_decimals`)
    pub fn collateral_to_liquidity(&self, collateral: u64) -> Result<u64> {
        let (total_liq, total_col) = self.scaled_supplies()?;
        collateral_to_liquidity_from_scaled(collateral, total_liq, total_col)
            .ok_or(KaminoMocksError::MathError.into())
    }

    /// Convert liquidity tokens to equivalent value in collateral token. Mirrors klend
    /// `CollateralExchangeRate::liquidity_to_collateral`:
    /// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/reserve.rs#L1488-L1514
    /// * Returns collateral equivalent (in `mint_decimals`)
    pub fn liquidity_to_collateral(&self, liquidity: u64) -> Result<u64> {
        let (total_liq, total_col) = self.scaled_supplies()?;
        liquidity_to_collateral_from_scaled(liquidity, total_liq, total_col)
            .ok_or(KaminoMocksError::MathError.into())
    }

    pub fn borrowed_amount_sf(&self) -> I80F48 {
        u68f60_to_i80f48(self.borrowed_amount_sf)
    }
    pub fn accumulated_protocol_fees_sf(&self) -> I80F48 {
        u68f60_to_i80f48(self.accumulated_protocol_fees_sf)
    }
    pub fn accumulated_referrer_fees_sf(&self) -> I80F48 {
        u68f60_to_i80f48(self.accumulated_referrer_fees_sf)
    }
    pub fn pending_referrer_fees_sf(&self) -> I80F48 {
        u68f60_to_i80f48(self.pending_referrer_fees_sf)
    }

    /// Calculate total supply of liquidity mint
    /// * In `mint_decimals`, adjusted to I80F48
    pub fn calculate_total_supply_i80f48(&self) -> I80F48 {
        let available_amount: I80F48 = I80F48::from_num(self.available_amount);

        let borrowed_amount_sf: I80F48 = self.borrowed_amount_sf();
        let accumulated_protocol_fees: I80F48 = self.accumulated_protocol_fees_sf();
        let accumulated_referrer_fees: I80F48 = self.accumulated_referrer_fees_sf();
        let pending_referrer_fees: I80F48 = self.pending_referrer_fees_sf();

        // Total supply
        available_amount + borrowed_amount_sf
            - accumulated_protocol_fees
            - accumulated_referrer_fees
            - pending_referrer_fees
    }

    /// Returns true if this reserve has not been refreshed in `current_slot`.
    ///
    /// We call this on-chain (in `ensure_kamino_reserve_fresh`) to require that a Kamino
    /// reserve was refreshed in the current slot before it trusts the reserve's collateral
    /// exchange rate when pricing a Kamino bank.
    ///
    /// Kamino itself treats a reserve as stale in two cases: its `last_update.slot` is older than
    /// the current slot, OR its `last_update.stale` flag is set. Kamino sets that flag after every
    /// operation that mutates the reserve (deposit, withdraw, borrow, repay). This check looks at
    /// ONLY the slot and intentionally ignores the `stale` flag, because of the order of events in
    /// a marginfi instruction that touches a Kamino position (e.g. withdraw):
    ///   1. refresh the reserve   -> sets `slot = current`, accrues interest, clears `stale`
    ///   2. CPI into Kamino       -> Kamino sets `stale = true` again as part of the deposit/withdraw
    ///   3. read the exchange rate -> happens during marginfi's post-operation health check
    ///
    /// At step 3 the reserve has `stale = true` but `slot == current`. The exchange rate does not
    /// change within a slot once the reserve has been refreshed (interest accrues per slot, and a
    /// same-slot deposit/withdraw does not move the rate), so "refreshed in this slot" is the only
    /// property we need. If this function also failed on the `stale` flag, step 3 would
    /// always fail
    pub fn is_stale(&self, current_slot: u64) -> bool {
        // Stale once the reserve's recorded slot falls behind the current slot; a `refresh_reserve`
        // in the same slot brings it current. Keepers reading a venue rate must refresh in-tx.
        self.slot < current_slot
    }
}

impl BorrowRateCurve {
    /// Kamino's 11-point piecewise-linear borrow-rate curve evaluated at `utilization_rate`
    /// (a ratio, 1.0 == 100%). Mirrors `BorrowRateCurve::get_borrow_rate`:
    /// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/utils/borrow_rate_curve.rs#L281-L320
    pub fn get_borrow_rate(&self, utilization_rate: I80F48) -> Option<I80F48> {
        get_borrow_rate_from_points(&self.points, utilization_rate)
    }
}

impl MinimalReserve {
    /// Net lender supply APR (I80F48, 1.0 == 100%): `borrow_rate(util) * util * (1 - protocol_take_rate)`
    /// with `util = borrowed / total_supply`. The caller must ensure the reserve was refreshed this
    /// slot (see [`MinimalReserve::is_stale`]). Returns `None` on zero supply or overflow. Mirrors
    /// klend's net-supply derivation:
    /// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/reserve.rs#L559
    pub fn supply_apr(&self, slots_per_second: I80F48) -> Option<I80F48> {
        self.supply_apr_at(I80F48::ZERO, slots_per_second)
    }

    /// [`MinimalReserve::supply_apr`] as it would read after `extra` native tokens were supplied,
    /// which dilutes utilization.
    pub fn supply_apr_at(&self, extra: I80F48, slots_per_second: I80F48) -> Option<I80F48> {
        kamino_supply_apr_from_parts(
            self.calculate_total_supply_i80f48().checked_add(extra)?,
            self.borrowed_amount_sf(),
            &self.config.borrow_rate_curve.points,
            self.config.protocol_take_rate_pct,
            slots_per_second,
        )
    }

    /// Reward-token APR accruing to cToken holders (I80F48, 1.0 == 100%), additive with
    /// [`MinimalReserve::supply_apr`]. `max_apr_bps` comes from the reserve's `LendingMarket`.
    /// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/reserve.rs#L586-L643
    pub fn rewards_apr(&self, max_apr_bps: u16, slots_per_second: I80F48) -> Option<I80F48> {
        self.rewards_apr_at(max_apr_bps, I80F48::ZERO, slots_per_second)
    }

    /// [`MinimalReserve::rewards_apr`] after `extra` native tokens were supplied; emissions are
    /// shared over a larger base, so this too dilutes.
    pub fn rewards_apr_at(
        &self,
        max_apr_bps: u16,
        extra: I80F48,
        slots_per_second: I80F48,
    ) -> Option<I80F48> {
        kamino_rewards_apr_from_parts(
            self.calculate_total_supply_i80f48().checked_add(extra)?,
            self.config.rewards_amount_per_slot,
            self.rewards_amount_available,
            self.mint_total_supply,
            max_apr_bps,
            slots_per_second,
        )
    }

    /// Liquidity the reserve still accepts, in native mint units; `u64::MAX` when uncapped.
    /// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/lending_market/lending_operations.rs#L144-L158
    pub fn remaining_deposit_capacity(&self) -> u64 {
        // `deposit_reserve_liquidity_and_obligation_collateral` rejects Obsolete (1) and emergency
        // mode; Hidden (2) and `block_ctoken_usage` do not gate it.
        if self.config.status == 1 || self.config.emergency_mode != 0 {
            return 0;
        }
        let limit = self.config.deposit_limit;
        if limit == u64::MAX {
            return u64::MAX;
        }
        let ceiling = I80F48::from_num(limit);
        let headroom =
            (ceiling - self.calculate_total_supply_i80f48()).clamp(I80F48::ZERO, ceiling);
        headroom.floor().checked_to_num::<u64>().unwrap_or(0)
    }
}

/// Kamino slots-per-year, at klend's fixed 2-slots-per-second convention.
const KAMINO_SLOTS_PER_YEAR: u128 = 63_072_000;

/// The slots-per-second [`KAMINO_SLOTS_PER_YEAR`] is built on.
pub const KLEND_SLOTS_PER_SECOND: u8 = 2;

/// Scales a klend rate from its slot-denominated year to a wall-clock year: klend divides by
/// [`KAMINO_SLOTS_PER_YEAR`] but accrues over real elapsed slots.
fn wall_clock_scalar(slots_per_second: I80F48) -> Option<I80F48> {
    slots_per_second.checked_div(I80F48::from_num(KLEND_SLOTS_PER_SECOND))
}

/// Pure reward-APR computation from reserve parts, decoupled from account loading for unit testing
/// and off-chain reuse. Mirrors `distribute_rewards`: the per-slot emission, capped by the market's
/// APR ceiling and by the remaining reward balance, annualized over the supply base. Returns `None`
/// on arithmetic failure, and zero when rewards are unconfigured.
///
/// The cap is kept fractional where klend floors it against elapsed slots, so this reads slightly
/// high for a reserve refreshed every slot.
pub fn kamino_rewards_apr_from_parts(
    total_supply: I80F48,
    rewards_amount_per_slot: u64,
    rewards_amount_available: u64,
    mint_total_supply: u64,
    max_apr_bps: u16,
    slots_per_second: I80F48,
) -> Option<I80F48> {
    if max_apr_bps == 0
        || rewards_amount_per_slot == 0
        || rewards_amount_available == 0
        || mint_total_supply == 0
        || total_supply <= I80F48::ZERO
    {
        return Some(I80F48::ZERO);
    }

    let slots_per_year = I80F48::from_num(KAMINO_SLOTS_PER_YEAR);
    let cap_per_slot = total_supply
        .checked_mul(I80F48::from_num(max_apr_bps))?
        .checked_div(I80F48::from_num(10_000u32).checked_mul(slots_per_year)?)?;
    let per_slot = I80F48::from_num(rewards_amount_per_slot)
        .min(cap_per_slot)
        .min(I80F48::from_num(rewards_amount_available));
    per_slot
        .checked_mul(slots_per_year)?
        .checked_div(total_supply)?
        .checked_mul(wall_clock_scalar(slots_per_second)?)
}

/// Pure net-supply-APR computation from reserve parts, decoupled from account loading for unit
/// testing and off-chain reuse. `total_supply`/`borrowed` are dimensionless I80F48 token units;
/// `take_rate_pct` is 0..100; `slots_per_second` is real chain pacing, which converts klend's
/// slot-denominated year into the wall-clock rate a depositor realizes. Returns `None` on zero
/// supply or arithmetic overflow.
pub fn kamino_supply_apr_from_parts(
    total_supply: I80F48,
    borrowed: I80F48,
    points: &[CurvePoint; 11],
    take_rate_pct: u8,
    slots_per_second: I80F48,
) -> Option<I80F48> {
    if total_supply <= I80F48::ZERO {
        return None;
    }
    // `ReserveLiquidity::utilization_rate`: borrowed / total_supply.
    let utilization = borrowed.checked_div(total_supply)?;
    let borrow_rate = get_borrow_rate_from_points(points, utilization)?;
    let protocol_take_rate = I80F48::from_num(take_rate_pct) / I80F48::from_num(100u8);
    borrow_rate
        .checked_mul(utilization)?
        .checked_mul(I80F48::ONE - protocol_take_rate)?
        .checked_mul(wall_clock_scalar(slots_per_second)?)
}

/// klend `Fraction::from_bps`: bps / 10_000.
fn from_bps(x: u32) -> I80F48 {
    I80F48::from_num(x) / I80F48::from_num(10_000u32)
}

/// Mirrors klend `BorrowRateCurve::get_borrow_rate`: clamp util to 1.0, round to bps, find the
/// bracketing [start, end] knots, short-circuit on an exact knot, else interpolate via the segment.
/// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/utils/borrow_rate_curve.rs#L281-L320
pub fn get_borrow_rate_from_points(
    points: &[CurvePoint; 11],
    utilization_rate: I80F48,
) -> Option<I80F48> {
    let one = I80F48::ONE;
    let utilization_rate = if utilization_rate > one {
        one
    } else {
        utilization_rate
    };
    let utilization_rate_bps: u32 = (utilization_rate * I80F48::from_num(10_000u32))
        .round()
        .to_num::<u32>();
    let (mut start_pt, mut end_pt) = (points[0], points[1]);
    for window in points.windows(2) {
        if utilization_rate_bps >= window[0].utilization_rate_bps
            && utilization_rate_bps <= window[1].utilization_rate_bps
        {
            start_pt = window[0];
            end_pt = window[1];
            break;
        }
    }
    if utilization_rate_bps == start_pt.utilization_rate_bps {
        return Some(from_bps(start_pt.borrow_rate_bps));
    }
    if utilization_rate_bps == end_pt.utilization_rate_bps {
        return Some(from_bps(end_pt.borrow_rate_bps));
    }
    segment_borrow_rate(start_pt, end_pt, utilization_rate)
}

/// Mirrors klend `CurveSegment::from_points` + `CurveSegment::get_borrow_rate`: the slope
/// `slope_nom / slope_denom` between the segment's two knots, applied as
/// `start.rate + (util - start.util) * slope`. `None` on a degenerate segment (`end.util <= start.util`).
/// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/utils/borrow_rate_curve.rs#L120-L140
fn segment_borrow_rate(
    start_pt: CurvePoint,
    end_pt: CurvePoint,
    utilization_rate: I80F48,
) -> Option<I80F48> {
    // `CurveSegment::from_points`: slopes from the two knots (rate/utilization must be ever-growing).
    let slope_nom = end_pt
        .borrow_rate_bps
        .checked_sub(start_pt.borrow_rate_bps)?;
    let slope_denom = end_pt
        .utilization_rate_bps
        .checked_sub(start_pt.utilization_rate_bps)?;
    if slope_denom == 0 {
        return None;
    }
    // `CurveSegment::get_borrow_rate`: base_rate (slope * coef) + offset.
    let start_utilization_rate = from_bps(start_pt.utilization_rate_bps);
    let coef = utilization_rate - start_utilization_rate;
    let nom = coef * I80F48::from_num(slope_nom);
    let base_rate = nom / I80F48::from_num(slope_denom);
    let offset = from_bps(start_pt.borrow_rate_bps);
    Some(base_rate + offset)
}

/// A minimal copy of Kamino's Obligation for zero-copy deserialization
#[account(zero_copy, discriminator = &OBLIGATION_DISCRIMINATOR)]
#[repr(C)]
pub struct MinimalObligation {
    pub tag: u64,
    /// Kamino obligations are only good for one slot, e.g. `refresh_obligation` must have run within the
    /// same slot as any ix that needs a non-stale obligation e.g. withdraw.
    pub last_update_slot: u64,
    /// True if the obligation is stale, which will cause various ixes like withdraw to fail. Typically
    /// set to true in any tx that modifies obligation balance, and set to false at the end of a
    /// successful `refresh_obligation`
    /// * 0 = false, 1 = true
    pub last_update_stale: u8,
    /// Each bit represents a passed check in price status.
    /// * 63 = all checks passed
    ///
    /// Otherwise:
    /// * PRICE_LOADED =        0b_0000_0001; // 1
    /// * PRICE_AGE_CHECKED =   0b_0000_0010; // 2
    /// * TWAP_CHECKED =        0b_0000_0100; // 4
    /// * TWAP_AGE_CHECKED =    0b_0000_1000; // 8
    /// * HEURISTIC_CHECKED =   0b_0001_0000; // 16
    /// * PRICE_USAGE_ALLOWED = 0b_0010_0000; // 32
    pub last_update_price_status: u8,
    pub last_update_placeholder: [u8; 6],

    pub lending_market: Pubkey,
    /// For mrgn banks, the bank's Liquidity Vault Authority (a pda which can be derived if the bank
    /// key is known)
    pub owner: Pubkey,

    pub deposits: [MinimalObligationCollateral; 8],
    pub lowest_reserve_deposit_liquidation_ltv: u64,
    pub deposited_value_sf: [u8; 16],

    // Rest of the struct padded out to match size, split into smaller chunks
    // because bytemuck::Zeroable is not implemented for arrays larger than 512 bytes
    _padding1: [u8; 2048],
    padding_part5a: [u8; 64],
    padding_part5c: [u8; 24],
}

#[account(zero_copy)]
#[repr(C)]
pub struct MinimalObligationCollateral {
    pub deposit_reserve: Pubkey,
    /// In collateral token (NOT liquidity token), use `collateral_to_liquidity` to convert back to
    /// liquidity token!
    /// * Always 6 decimals
    pub deposited_amount: u64,
    /// * In dollars, based on last oracle price update
    /// * Actually an I68F60, stored as a u128 (i.e. BN) in Kamino.
    /// * A float (arbitrary decimals)
    pub market_value_sf: [u8; 16],
    pub borrowed_amount_against_this_collateral_in_elevation_group: u64,
    pub padding: [u64; 9],
}

/// Convert a Kamino Fraction (U68F60) to MarginFi's fixed-point type (I80F48) without going through
/// Kamino's Fraction type.
///
/// * `bits_le` - The raw little-endian u128 bits from a Kamino stored U68F60 (Fraction)
pub fn u68f60_to_i80f48(bits_le: [u8; 16]) -> I80F48 {
    // The difference in fractional bits between Kamino's U68F60 and MarginFi's I80F48
    const FRAC_BITS_DIFF: u32 = 60 - 48;

    let raw_u128 = u128::from_le_bytes(bits_le);
    // Shift right to adjust for the different number of fractional bits. This will lose the lowest
    // 12 bits of precision, which is acceptable
    let raw = raw_u128 >> FRAC_BITS_DIFF;
    // Convert to i128 for I80F48 - safe because U68F60 values will fit in I80F48 (68 integer bits
    // in U68F60 is less than 80 integer bits in I80F48), and U68F60 can never be negative.
    let signed_bits: i128 = raw as i128;

    I80F48::from_bits(signed_bits)
}

/// Given a value that is currently using `from_dec` decimals, convert into `to_dec` decimals
pub fn convert_decimals(n: I80F48, from_dec: u8, to_dec: u8) -> Result<I80F48> {
    Ok(shared_convert_decimals(n, from_dec, to_dec).ok_or_else(math_error!())?)
}

// Note: see "local_tests.rs" in the mrgnfi program for cargo tests for above functions. We
// typically run `cargo test --lib` on just marginfi to save time in CI so this is easier than
// workspace configuration.

#[cfg(test)]
mod capacity_tests {
    use super::*;
    use bytemuck::Zeroable;

    fn reserve(limit: u64, available: u64) -> MinimalReserve {
        let mut r = MinimalReserve::zeroed();
        r.config.deposit_limit = limit;
        r.available_amount = available;
        r
    }

    #[test]
    fn capacity_is_the_exact_headroom_to_the_limit() {
        assert_eq!(reserve(1_000, 900).remaining_deposit_capacity(), 100);
        assert_eq!(reserve(1_000, 1_000).remaining_deposit_capacity(), 0);
        assert_eq!(reserve(1_000, 1_200).remaining_deposit_capacity(), 0);
        assert_eq!(
            reserve(u64::MAX, 900).remaining_deposit_capacity(),
            u64::MAX
        );
    }

    /// A reserve that rejects every deposit is full whatever its limit says; Hidden (2) and
    /// `block_ctoken_usage` are not such states.
    #[test]
    fn blocked_reserves_report_no_capacity() {
        let mut obsolete = reserve(1_000, 0);
        obsolete.config.status = 1;
        assert_eq!(obsolete.remaining_deposit_capacity(), 0);

        let mut emergency = reserve(1_000, 0);
        emergency.config.emergency_mode = 1;
        assert_eq!(emergency.remaining_deposit_capacity(), 0);

        let mut hidden = reserve(1_000, 0);
        hidden.config.status = 2;
        assert_eq!(hidden.remaining_deposit_capacity(), 1_000);

        let mut ctoken_blocked = reserve(1_000, 0);
        ctoken_blocked.config.block_ctoken_usage = 1;
        assert_eq!(ctoken_blocked.remaining_deposit_capacity(), 1_000);
    }
}

#[cfg(test)]
mod rate_tests {
    use super::*;

    /// Rewards are annualized from the per-slot emission and bounded by both the market's APR cap
    /// and the remaining reward balance.
    #[test]
    fn rewards_apr_is_bounded_by_cap_and_balance() {
        let supply = I80F48::from_num(63_072_000u64); // 1 token/slot == 100% APR on this base
        assert_eq!(
            kamino_rewards_apr_from_parts(supply, 1, u64::MAX, 1, 20_000, klend_pace()).unwrap(),
            I80F48::ONE
        );
        // The market cap binds first: 625 bps == 6.25%.
        assert_eq!(
            kamino_rewards_apr_from_parts(supply, 1, u64::MAX, 1, 625, klend_pace()).unwrap(),
            I80F48::from_num(0.0625)
        );
        // Unconfigured rewards contribute nothing.
        assert_eq!(
            kamino_rewards_apr_from_parts(supply, 0, u64::MAX, 1, 20_000, klend_pace()).unwrap(),
            I80F48::ZERO
        );
        assert_eq!(
            kamino_rewards_apr_from_parts(supply, 1, 0, 1, 20_000, klend_pace()).unwrap(),
            I80F48::ZERO
        );
    }

    /// klend's own pacing convention, at which the wall-clock scalar is exactly 1.
    fn klend_pace() -> I80F48 {
        I80F48::from_num(KLEND_SLOTS_PER_SECOND)
    }

    fn cp(util_bps: u32, rate_bps: u32) -> CurvePoint {
        CurvePoint {
            utilization_rate_bps: util_bps,
            borrow_rate_bps: rate_bps,
        }
    }

    /// A straight line from (0, 0) to (10000 bps, `max_bps`) sampled at 11 evenly spaced points, so
    /// the piecewise-linear curve equals `borrow = max_bps/10000 * util` everywhere.
    fn linear_curve(max_bps: u32) -> [CurvePoint; 11] {
        let mut pts = [cp(0, 0); 11];
        for (i, p) in pts.iter_mut().enumerate() {
            *p = cp(i as u32 * 1000, i as u32 * max_bps / 10);
        }
        pts
    }

    #[test]
    fn curve_endpoints_and_interpolation() {
        let pts = linear_curve(5000); // (0,0)..(100%, 50%)
        assert_eq!(
            get_borrow_rate_from_points(&pts, I80F48::ZERO).unwrap(),
            I80F48::ZERO
        );
        assert_eq!(
            get_borrow_rate_from_points(&pts, I80F48::ONE).unwrap(),
            I80F48::from_num(0.5)
        );
        assert_eq!(
            get_borrow_rate_from_points(&pts, I80F48::from_num(0.5)).unwrap(),
            I80F48::from_num(0.25)
        );
        // 56.25% falls inside the [50%, 60%] segment and interpolates to 0.25 + 0.0625 * 0.5.
        assert_eq!(
            get_borrow_rate_from_points(&pts, I80F48::from_num(0.5625)).unwrap(),
            I80F48::from_num(0.28125)
        );
    }

    #[test]
    fn supply_apr_nets_the_take_rate() {
        let pts = linear_curve(5000);
        // util 0.5 -> borrow 0.25; supply = 0.25 * 0.5 * (1 - 0.25) = 0.09375.
        let r = kamino_supply_apr_from_parts(
            I80F48::from_num(1000),
            I80F48::from_num(500),
            &pts,
            25,
            klend_pace(),
        );
        assert_eq!(r.unwrap(), I80F48::from_num(0.09375));
    }

    #[test]
    fn supply_apr_zero_supply_is_none() {
        let pts = linear_curve(3040);
        assert!(kamino_supply_apr_from_parts(
            I80F48::ZERO,
            I80F48::from_num(500),
            &pts,
            10,
            klend_pace()
        )
        .is_none());
    }

    /// klend prices per slot against a year fixed at two slots per second but accrues over real
    /// slots, so both legs scale by actual pacing. At 2.5 slots/s a depositor realizes 1.25x.
    #[test]
    fn rates_scale_with_real_chain_pacing() {
        let pace = I80F48::from_num(2.5);
        // util 0.5 -> borrow 0.25; supply = 0.25 * 0.5 = 0.125 at klend's own pacing.
        let pts = linear_curve(5000);
        let (supply, borrowed) = (I80F48::from_num(1000), I80F48::from_num(500));
        assert_eq!(
            kamino_supply_apr_from_parts(supply, borrowed, &pts, 0, klend_pace()).unwrap(),
            I80F48::from_num(0.125)
        );
        assert_eq!(
            kamino_supply_apr_from_parts(supply, borrowed, &pts, 0, pace).unwrap(),
            I80F48::from_num(0.15625)
        );
        // 1 token/slot over a 63_072_000 base is 100% at klend's pacing.
        let base = I80F48::from_num(63_072_000u64);
        assert_eq!(
            kamino_rewards_apr_from_parts(base, 1, u64::MAX, 1, 20_000, pace).unwrap(),
            I80F48::from_num(1.25)
        );
    }

    /// `supply_apr()` decodes the U68F60 `borrowed_amount_sf` and nets the three fee balances out of
    /// `calculate_total_supply_i80f48` before pricing the curve.
    #[test]
    fn supply_apr_method_decodes_fees_and_balances() {
        use bytemuck::Zeroable;
        // available 1200 + borrowed 1000 - 200 fees -> total_supply 2000, util 0.5; take 25%.
        let to_u68f60 = |x: u128| (x << 60).to_le_bytes();
        let mut r = MinimalReserve::zeroed();
        r.available_amount = 1200;
        r.borrowed_amount_sf = to_u68f60(1000);
        r.accumulated_protocol_fees_sf = to_u68f60(100);
        r.accumulated_referrer_fees_sf = to_u68f60(60);
        r.pending_referrer_fees_sf = to_u68f60(40);
        r.config.protocol_take_rate_pct = 25;
        r.config.borrow_rate_curve.points = linear_curve(5000);
        assert_eq!(r.supply_apr(klend_pace()), Some(I80F48::from_num(0.09375)));
    }
}
