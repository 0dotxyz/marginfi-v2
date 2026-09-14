//! Mirrored account layouts of Kamino's Scope oracle aggregator
//! (<https://github.com/Kamino-Finance/scope>, `programs/scope/src/states/`).
//! The compile-time asserts below pin the layout against the upstream program.

use anchor_lang::prelude::*;
use marginfi_type_crate::{assert_struct_align, assert_struct_size};

/// `sha256("account:OraclePrices")[..8]`
pub const SCOPE_ORACLE_PRICES_DISCRIMINATOR: [u8; 8] = [89, 128, 118, 221, 6, 72, 180, 146];

/// Scope's fixed price-array length (`MAX_ENTRIES` in programs/scope/src/lib.rs).
pub const SCOPE_MAX_ENTRIES: usize = 512;

/// Scope's `Price` (programs/scope/src/states/dated_price.rs): decimal price is
/// `value / 10^exp`.
#[zero_copy]
#[derive(Debug, Default)]
pub struct ScopePrice {
    pub value: u64,
    pub exp: u64,
}

/// Scope's `DatedPrice` (programs/scope/src/states/dated_price.rs).
#[zero_copy]
#[derive(Debug, Default)]
pub struct ScopeDatedPrice {
    pub price: ScopePrice,
    pub last_updated_slot: u64,
    pub unix_timestamp: u64,
    pub generic_data: [u8; 24],
}

/// Scope's `OraclePrices` (programs/scope/src/states/oracle_prices.rs). On-chain size is
/// 8 (discriminator) + 28,704 (this struct) = 28,712 bytes.
#[account(zero_copy, discriminator = &SCOPE_ORACLE_PRICES_DISCRIMINATOR)]
#[repr(C)]
#[derive(Debug)]
pub struct ScopeOraclePrices {
    pub oracle_mappings: Pubkey,
    pub prices: [ScopeDatedPrice; SCOPE_MAX_ENTRIES],
}

assert_struct_size!(ScopePrice, 16);
assert_struct_size!(ScopeDatedPrice, 56);
assert_struct_size!(ScopeOraclePrices, 32 + SCOPE_MAX_ENTRIES * 56);
assert_struct_align!(ScopeOraclePrices, 8);
