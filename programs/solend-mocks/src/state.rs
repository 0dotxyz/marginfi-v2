use crate::{math_error, SolendMocksError};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::price::{
    collateral_to_liquidity_from_scaled, convert_decimals as shared_convert_decimals,
    liquidity_to_collateral_from_scaled, scale_supplies,
};

// Account versions (Solend uses versions instead of discriminators)
pub const PROGRAM_VERSION: u8 = 1;
pub const UNINITIALIZED_VERSION: u8 = 0;

// EXPERIMENTAL: Using Solend's version byte as an Anchor discriminator
// Solend accounts start with version=1, we treat this as discriminator [1]
pub const RESERVE_DISCRIMINATOR: [u8; 1] = [1];

// Account sizes
// Solend's official reserve size is 619 bytes (includes 1-byte version field)
// Since Anchor adds its own 1-byte discriminator, our struct is 618 bytes
// Total size when loaded: 1 (discriminator) + 618 (struct) = 619 bytes
pub const RESERVE_LEN: usize = 619;
// Obligation size constant for manual validation (Solend's official size)
pub const OBLIGATION_LEN: usize = 1300;
pub const LENDING_MARKET_LEN: usize = 290;

// EXPERIMENTAL: Using Anchor's zero_copy with manual 1-byte discriminator
// This treats Solend's version byte as an Anchor discriminator
// WARNING: This is an experimental approach to load Solend accounts through Anchor
#[account(zero_copy, discriminator = &RESERVE_DISCRIMINATOR)]
#[repr(C, packed)]
pub struct SolendMinimalReserve {
    // NOTE: Version field removed - Anchor handles the discriminator
    // Solend's version=1 becomes our discriminator [1]

    // LastUpdate section (bytes 0-9 after discriminator)
    /// Last slot when supply and rates updated
    pub last_update_slot: u64, // offset 0-8
    /// True when marked stale
    pub last_update_stale: u8, // offset 8-9

    /// Lending market address
    pub lending_market: Pubkey, // offset 9-41

    // Liquidity section
    pub liquidity_mint_pubkey: Pubkey,               // offset 41-73
    pub liquidity_mint_decimals: u8,                 // offset 73-74
    pub liquidity_supply_pubkey: Pubkey,             // offset 74-106
    pub liquidity_pyth_oracle_pubkey: Pubkey,        // offset 106-138
    pub liquidity_switchboard_oracle_pubkey: Pubkey, // offset 138-170

    // Liquidity amounts
    pub liquidity_available_amount: u64, // offset 170-178
    pub liquidity_borrowed_amount_wads: [u8; 16], // offset 178-194
    pub liquidity_cumulative_borrow_rate_wads: [u8; 16], // offset 194-210
    pub liquidity_market_price: [u8; 16], // offset 210-226

    // Collateral section
    pub collateral_mint_pubkey: Pubkey,    // offset 226-258
    pub collateral_mint_total_supply: u64, // offset 258-266
    pub collateral_supply_pubkey: Pubkey,  // offset 266-298

    // Config rates - we only care about first 4
    pub config_optimal_utilization_rate: u8, // offset 298-299
    pub config_loan_to_value_ratio: u8,      // offset 299-300
    pub config_liquidation_bonus: u8,        // offset 300-301
    pub config_liquidation_threshold: u8,    // offset 301-302

    // Padding to reach protocol fees (skipping fields we don't need)
    // Padding: 70 bytes total
    // 70 = 64 + 6
    _padding_to_fees_64: [u8; 64], // offset 302-366
    _padding_to_fees_6: [u8; 6],   // offset 366-372

    pub liquidity_accumulated_protocol_fees_wads: [u8; 16], // offset 372-388

    // Final padding to reach exactly 618 bytes
    // Padding: 230 bytes total
    // 230 = 128 + 64 + 32 + 6
    _padding_final_128: [u8; 128], // offset 388-516
    _padding_final_64: [u8; 64],   // offset 516-580
    _padding_final_32: [u8; 32],   // offset 580-612
    _padding_final_6: [u8; 6],     // offset 612-618
}

impl SolendMinimalReserve {
    /// Returns (total_liquidity, total_collateral) both as I80F48
    /// scaled down by 10^liquidity_mint_decimals
    pub fn scaled_supplies(&self) -> Result<(I80F48, I80F48)> {
        let total_liq_raw: I80F48 = self.calculate_total_liquidity()?;
        let (total_liq, total_col) = scale_supplies(
            total_liq_raw,
            self.collateral_mint_total_supply,
            self.liquidity_mint_decimals,
        )
        .ok_or_else(math_error!())?;
        Ok((total_liq, total_col))
    }

    /// Convert collateral tokens to liquidity tokens
    /// Both use the same decimals (liquidity_mint_decimals)
    pub fn collateral_to_liquidity(&self, collateral: u64) -> Result<u64> {
        let (total_liq, total_col) = self.scaled_supplies()?;

        collateral_to_liquidity_from_scaled(collateral, total_liq, total_col)
            .ok_or(SolendMocksError::MathError.into())
    }

    /// Convert liquidity tokens to collateral tokens
    pub fn liquidity_to_collateral(&self, liquidity: u64) -> Result<u64> {
        let (total_liq, total_col) = self.scaled_supplies()?;

        liquidity_to_collateral_from_scaled(liquidity, total_liq, total_col)
            .ok_or(SolendMocksError::MathError.into())
    }

    /// Calculate total liquidity supply
    /// Returns total in liquidity_mint_decimals
    /// Formula: available + borrowed - protocol_fees (matches Solend exactly)
    pub fn calculate_total_liquidity(&self) -> Result<I80F48> {
        let available = I80F48::from_num(self.liquidity_available_amount);
        let borrowed = decimal_to_i80f48(self.liquidity_borrowed_amount_wads)?;
        let fees = decimal_to_i80f48(self.liquidity_accumulated_protocol_fees_wads)?;

        Ok(available + borrowed - fees)
    }

    /// Check if reserve is stale
    pub fn is_stale(&self) -> Result<bool> {
        let clock = Clock::get()?;
        // let stale = self.last_update_stale != 0;
        let slot_expired = self.last_update_slot < clock.slot;
        Ok(slot_expired)
    }

    /// Get the initial collateral exchange rate (used when supply is 0)
    pub fn initial_exchange_rate(&self) -> I80F48 {
        // Solend uses INITIAL_COLLATERAL_RATE = 1
        I80F48::from_num(1)
    }
}

/// Convert a Solend WAD-scaled `u128` (value × 10¹⁸) to `I80F48`.
///
/// * Assumes the on-chain number is **always non-negative** (Solend never
///   writes negatives; protocol logic would fail long before that).
/// * Returns `Err` only if the integer part would overflow the 80-bit
///   signed-integer field of `I80F48`.
pub fn decimal_to_i80f48(bits_le: [u8; 16]) -> Result<I80F48> {
    const WAD: u128 = 1_000_000_000_000_000_000; // 10¹⁸
    const TWO48: u128 = 1u128 << 48; // 2⁴⁸

    // 1) decode the little-endian bytes as *unsigned* u128
    let raw: u128 = u128::from_le_bytes(bits_le);

    // 2) split into integer tokens and the 10¹⁸ remainder
    let int_part = raw / WAD; // upper 80 bits target
    let rem = raw % WAD; // [0, 10¹⁸-1]

    // 3) sanity-check the integer part fits in 79 usable bits
    //    (uppermost bit is sign in i128 after the later shift)
    if int_part > ((1u128 << 79) - 1) {
        return Err(SolendMocksError::MathError.into());
    }

    // 4) convert the decimal remainder to a 48-bit binary fraction:
    //       frac_bits = remainder / 10¹⁸  *  2⁴⁸
    //    rearranged to keep everything in integer space
    let frac_bits: u128 = (rem * TWO48) / WAD; // guaranteed < 2⁴⁸

    // 5) assemble the I80F48 bit pattern
    let bits: i128 = ((int_part as i128) << 48) | (frac_bits as i128);

    Ok(I80F48::from_bits(bits))
}

/// Convert between different decimal representations
pub fn convert_decimals(n: I80F48, from_dec: u8, to_dec: u8) -> Result<I80F48> {
    Ok(shared_convert_decimals(n, from_dec, to_dec).ok_or_else(math_error!())?)
}

/// Validate a Solend obligation
/// Returns Ok(()) if valid, error otherwise
pub fn validate_solend_obligation(account: &AccountInfo, expected_reserve: Pubkey) -> Result<()> {
    // Verify owner is Solend program
    require_keys_eq!(
        *account.owner,
        crate::ID,
        SolendMocksError::InvalidAccountData
    );

    let data = account.try_borrow_data()?;

    // Check size (including version byte)
    require!(
        data.len() >= OBLIGATION_LEN,
        SolendMocksError::InvalidAccountData
    );

    // Check version byte (first byte should be 1)
    require_eq!(data[0], 1u8, SolendMocksError::InvalidAccountData);

    // Manual validation without deserialization
    // Byte positions calculated from pack_into_slice in obligation.rs:
    //
    // mut_array_refs![output,
    //     1,        // version → Byte 0
    //     8,        // last_update_slot → Byte 1-8
    //     1,        // last_update_stale → Byte 9
    //     32,       // lending_market → Byte 10-41
    //     32,       // owner → Byte 42-73
    //     16,       // deposited_value → Byte 74-89
    //     16,       // borrowed_value → Byte 90-105
    //     16,       // allowed_borrow_value → Byte 106-121
    //     16,       // unhealthy_borrow_value → Byte 122-137
    //     16,       // borrowed_value_upper_bound → Byte 138-153
    //     1,        // borrowing_isolated_asset → Byte 154
    //     16,       // super_unhealthy_borrow_value → Byte 155-170
    //     16,       // unweighted_borrowed_value → Byte 171-186
    //     1,        // closeable → Byte 187
    //     14,       // _padding → Byte 188-201
    //     1,        // deposits_len → Byte 202
    //     1,        // borrows_len → Byte 203
    //     1096      // data_flat → Byte 204-1299
    // ];
    //
    // Within data_flat (starting at byte 204):
    // - deposits: deposits_len * 88 bytes each
    // - borrows: borrows_len * 112 bytes each
    // First deposit structure (88 bytes):
    // - deposit_reserve: Byte 204-235 (32 bytes)
    // - deposited_amount: Byte 236-243 (8 bytes)
    // - market_value: Byte 244-259 (16 bytes)
    // - padding: Byte 260-291 (32 bytes)
    //

    // Check deposits_len at position 202 (should be 1 for single deposit)
    require_eq!(
        data[202],
        1u8,
        SolendMocksError::InvalidObligationCollateral
    );

    // Check borrows_len at position 203 (should be 0 for no borrows)
    require_eq!(data[203], 0u8, SolendMocksError::InvalidObligationLiquidity);

    // First deposit starts at position 204 in data_flat array
    // Each deposit is 88 bytes: [Pubkey (32) + u64 (8) + u128 (16) + padding (32)]
    let deposit_start = 204;

    // Check first deposit reserve matches expected (32 bytes)
    let deposit_reserve_bytes = &data[deposit_start..deposit_start + 32];
    let deposit_reserve = Pubkey::try_from(deposit_reserve_bytes)
        .map_err(|_| SolendMocksError::InvalidObligationCollateral)?;
    require_keys_eq!(
        deposit_reserve,
        expected_reserve,
        SolendMocksError::InvalidObligationCollateral
    );

    // Check first deposit amount is non-zero (8 bytes at position 236-243)
    let deposit_amount_bytes = &data[deposit_start + 32..deposit_start + 40];
    let deposit_amount = u64::from_le_bytes(
        deposit_amount_bytes
            .try_into()
            .map_err(|_| SolendMocksError::InvalidObligationCollateral)?,
    );
    require!(
        deposit_amount > 0,
        SolendMocksError::InvalidObligationCollateral
    );

    // Since deposits_len = 1, we don't need to check other deposits
    // The dataFlat buffer only contains exactly 1 deposit (88 bytes)
    // followed by 0 borrows, so there are no other deposits to validate

    Ok(())
}

/// Get the deposit amount at position 0 from a Solend obligation
pub fn get_solend_obligation_deposit_amount(account: &AccountInfo) -> Result<u64> {
    // Verify owner is Solend program
    require_keys_eq!(
        *account.owner,
        crate::ID,
        SolendMocksError::InvalidAccountData
    );

    let data = account.try_borrow_data()?;

    // Check size (including version byte)
    require!(
        data.len() >= OBLIGATION_LEN,
        SolendMocksError::InvalidAccountData
    );

    // Check version byte
    require_eq!(data[0], 1u8, SolendMocksError::InvalidAccountData);

    // Manual extraction without deserialization
    // First deposit starts at position 204 in data_flat array
    // Each deposit is 88 bytes: [Pubkey (32) + u64 (8) + u128 (16) + padding (32)]
    let deposit_start = 204;

    // Get first deposit amount (8 bytes at position 236-243)
    let deposit_amount_bytes = &data[deposit_start + 32..deposit_start + 40];
    let deposit_amount = u64::from_le_bytes(
        deposit_amount_bytes
            .try_into()
            .map_err(|_| SolendMocksError::InvalidObligationCollateral)?,
    );

    Ok(deposit_amount)
}

/// Validate a Solend reserve account with comprehensive checks including staleness
/// Uses direct byte parsing like validate_solend_obligation for consistency
/// Returns Ok(()) if valid, error otherwise
pub fn validate_solend_reserve(
    account: &AccountInfo,
    expected_lending_market: Pubkey,
) -> Result<()> {
    // Verify owner is Solend program
    require_keys_eq!(
        *account.owner,
        crate::ID,
        SolendMocksError::InvalidAccountData
    );

    let data = account.try_borrow_data()?;

    // Check size (including version byte)
    require!(
        data.len() == RESERVE_LEN,
        SolendMocksError::InvalidAccountData
    );

    // Check version byte (first byte should be 1)
    require_eq!(data[0], 1u8, SolendMocksError::InvalidAccountData);

    // Extract and validate lending market (bytes 10-41)
    let lending_market_bytes = &data[10..42];
    let lending_market =
        Pubkey::try_from(lending_market_bytes).map_err(|_| SolendMocksError::InvalidAccountData)?;
    require_keys_eq!(
        lending_market,
        expected_lending_market,
        SolendMocksError::InvalidReserveLendingMarket
    );

    // Extract staleness data (bytes 1-9)
    let last_update_slot_bytes = &data[1..9];
    let last_update_slot = u64::from_le_bytes(
        last_update_slot_bytes
            .try_into()
            .map_err(|_| SolendMocksError::InvalidAccountData)?,
    );

    // Get current slot from Clock
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Check staleness - reserve is stale if
    // The last update slot is behind current slot
    let is_stale = last_update_slot < current_slot;

    if is_stale {
        msg!(
            "Solend reserve is stale: current_slot={}, last_update_slot={}",
            current_slot,
            last_update_slot,
        );
        return Err(SolendMocksError::ReserveStale.into());
    }

    Ok(())
}

/// Helper to get exchange rate between collateral and liquidity
pub struct CollateralExchangeRate(pub I80F48);

impl CollateralExchangeRate {
    /// Create from reserve state
    pub fn from_reserve(reserve: &SolendMinimalReserve) -> Result<Self> {
        let total_liquidity: I80F48 = reserve.calculate_total_liquidity()?;

        if reserve.collateral_mint_total_supply == 0 || total_liquidity == I80F48::ZERO {
            // Use initial rate when no supply
            Ok(CollateralExchangeRate(reserve.initial_exchange_rate()))
        } else {
            let mint_supply: I80F48 = I80F48::from_num(reserve.collateral_mint_total_supply);

            // Safe to do the unchecked version here since we explicitly check for zeros above
            let rate: I80F48 = mint_supply
                .checked_div(total_liquidity)
                .ok_or_else(math_error!())?;

            Ok(CollateralExchangeRate(rate))
        }
    }

    /// Convert collateral to liquidity using this rate
    pub fn collateral_to_liquidity(&self, collateral_amount: u64) -> Result<u64> {
        let collateral: I80F48 = I80F48::from_num(collateral_amount);
        let liquidity: I80F48 = collateral.checked_div(self.0).ok_or_else(math_error!())?;

        liquidity
            .checked_to_num::<u64>()
            .ok_or(SolendMocksError::MathError.into())
    }

    /// Convert liquidity to collateral using this rate
    pub fn liquidity_to_collateral(&self, liquidity_amount: u64) -> Result<u64> {
        let liquidity: I80F48 = I80F48::from_num(liquidity_amount);
        let collateral: I80F48 = liquidity.checked_mul(self.0).ok_or_else(math_error!())?;

        collateral
            .checked_to_num::<u64>()
            .ok_or(SolendMocksError::MathError.into())
    }
}
