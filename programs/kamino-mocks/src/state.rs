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

    // Remaining padding to match account size
    padding4_part1: [u8; 4096],
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

    /// Convert collateral tokens to equivalent liquidity tokens
    /// * Returns liquidity tokens (uses `mint_decimals`)
    pub fn collateral_to_liquidity(&self, collateral: u64) -> Result<u64> {
        let (total_liq, total_col) = self.scaled_supplies()?;
        collateral_to_liquidity_from_scaled(collateral, total_liq, total_col)
            .ok_or(KaminoMocksError::MathError.into())
    }

    /// Convert liquidity tokens to equivalent value in collateral token.
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
        // TODO Kamino executes `deposit_reserve.last_update.mark_stale()` after any deposit, so
        // should we ignored this?
        let _stale = self.stale == 1;
        // Slot expired
        self.slot < current_slot
    }
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
