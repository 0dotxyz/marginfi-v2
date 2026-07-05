use crate::{assert_struct_align, assert_struct_size, math_error, KaminoMocksError};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::price::{
    collateral_to_liquidity_from_scaled, convert_decimals as shared_convert_decimals,
    liquidity_to_collateral_from_scaled, scale_supplies,
};

// Constants for account discriminators
pub const RESERVE_DISCRIMINATOR: [u8; 8] = [43, 242, 204, 202, 26, 247, 59, 127];
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
assert_struct_align!(ReserveConfig, 4);
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
    pub _reserved_1: [u8; 4],
    pub protocol_order_execution_fee_pct: u8,
    /// Percentage of interest taken by the protocol (0..100). Read as `from_percent(pct)`.
    /// https://github.com/Kamino-Finance/klend/blob/master/programs/klend/src/state/reserve.rs#L1578
    pub protocol_take_rate_pct: u8,
    pub _gap_to_curve_a: [u8; 48],
    pub _gap_to_curve_b: [u8; 1],
    pub borrow_rate_curve: BorrowRateCurve,
    pub _rest_1: [u8; 512],
    pub _rest_2: [u8; 256],
    pub _rest_3: [u8; 32],
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
    // Padding to completion of ReserveLiquidity
    padding2_part1: [u8; 256],
    padding2_part2: [u8; 128],
    padding2_part3: [u8; 24],
    padding3: [u8; 512],
    // end of reserve liquidity
    padding_part1: [u8; 512],
    padding_part2: [u8; 512],
    padding_part3: [u8; 128],
    padding_part4: [u8; 48],

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

    padding1_reserve_collateral: [u8; 512],
    padding2_reserve_collateral: [u8; 512],

    _pre_config_1: [u8; 512],
    _pre_config_2: [u8; 512],
    _pre_config_3: [u8; 128],
    _pre_config_4: [u8; 48],
    pub config: ReserveConfig,
    _post_config_1: [u8; 512],
    _post_config_2: [u8; 512],
    _post_config_3: [u8; 512],
    _post_config_4: [u8; 256],
    _post_config_5: [u8; 128],
    _post_config_6: [u8; 24],
    padding4_part2: [u8; 512],
    padding4_part3: [u8; 256],
    padding4_part4: [u8; 64],
    padding4_part5: [u8; 32],
    padding4_part6: [u8; 8],
}

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
    pub fn supply_apr(&self) -> Option<I80F48> {
        kamino_supply_apr_from_parts(
            self.calculate_total_supply_i80f48(),
            self.borrowed_amount_sf(),
            &self.config.borrow_rate_curve.points,
            self.config.protocol_take_rate_pct,
        )
    }
}

/// Pure net-supply-APR computation from reserve parts, decoupled from account loading for unit
/// testing and off-chain reuse. `total_supply`/`borrowed` are dimensionless I80F48 token units;
/// `take_rate_pct` is 0..100. Returns `None` on zero supply or arithmetic overflow.
pub fn kamino_supply_apr_from_parts(
    total_supply: I80F48,
    borrowed: I80F48,
    points: &[CurvePoint; 11],
    take_rate_pct: u8,
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
        .checked_mul(I80F48::ONE - protocol_take_rate)
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
    padding_part1: [u8; 512],
    padding_part2: [u8; 512],
    padding_part3: [u8; 512],
    padding_part4: [u8; 512],
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
mod rate_tests {
    use super::*;

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

    fn approx(actual: I80F48, expected: f64) {
        let a = actual.to_num::<f64>();
        assert!((a - expected).abs() < 1e-5, "got {a}, expected {expected}");
    }

    #[test]
    fn curve_endpoints_and_interpolation() {
        let pts = linear_curve(3040); // (0,0)..(100%, 30.4%)
        approx(
            get_borrow_rate_from_points(&pts, I80F48::ZERO).unwrap(),
            0.0,
        );
        approx(
            get_borrow_rate_from_points(&pts, I80F48::ONE).unwrap(),
            0.304,
        );
        approx(
            get_borrow_rate_from_points(&pts, I80F48::from_num(0.5)).unwrap(),
            0.152,
        );
        // 45% lands between sampled points and must interpolate to exactly 0.45 * 0.304.
        approx(
            get_borrow_rate_from_points(&pts, I80F48::from_num(0.45)).unwrap(),
            0.1368,
        );
    }

    #[test]
    fn supply_apr_nets_the_take_rate() {
        let pts = linear_curve(3040);
        // util 0.5 -> borrow 0.152; supply = 0.152 * 0.5 * (1 - 0.10) = 0.0684.
        let r =
            kamino_supply_apr_from_parts(I80F48::from_num(1000), I80F48::from_num(500), &pts, 10);
        approx(r.unwrap(), 0.0684);
    }

    #[test]
    fn supply_apr_zero_supply_is_none() {
        let pts = linear_curve(3040);
        assert!(
            kamino_supply_apr_from_parts(I80F48::ZERO, I80F48::from_num(500), &pts, 10).is_none()
        );
    }

    /// The `supply_apr()` method (decodes the U68F60 `borrowed_amount_sf` and sums
    /// `calculate_total_supply_i80f48`) must agree with `kamino_supply_apr_from_parts` fed the
    /// hand-derived total_supply / borrowed.
    #[test]
    fn supply_apr_method_matches_from_parts() {
        use bytemuck::Zeroable;
        // available 1000 + borrowed 500 - 0 fees -> total_supply 1500; protocol take 10%.
        let to_u68f60 = |x: u128| (x << 60).to_le_bytes();
        let mut r = MinimalReserve::zeroed();
        r.available_amount = 1000;
        r.borrowed_amount_sf = to_u68f60(500);
        r.config.protocol_take_rate_pct = 10;
        r.config.borrow_rate_curve.points = linear_curve(3040);
        let points = r.config.borrow_rate_curve.points;
        assert_eq!(
            r.supply_apr(),
            kamino_supply_apr_from_parts(
                I80F48::from_num(1500),
                I80F48::from_num(500),
                &points,
                10
            )
        );
    }
}
