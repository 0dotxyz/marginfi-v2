use anchor_lang::prelude::*;
use marginfi_type_crate::assert_struct_size;

// Anchor discriminator for Marinade's `State`, sha256("account:State")[0..8]. Verified against the
// live mainnet State account 8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC.
pub const STATE_DISCRIMINATOR: [u8; 8] = [216, 146, 107, 94, 104, 75, 182, 177];

/// Denominator for Marinade's cached `msol_price`: `msol_to_sol = msol_price / 2^32`.
pub const MSOL_PRICE_PRECISION: u128 = 1 << 32;

/// Minimal zero-copy view of Marinade's `State`: the cached `msol_price` at byte offset 512 (504
/// past the 8-byte discriminator). Padding is split into `bytemuck`-Pod array sizes.
#[account(zero_copy, discriminator = &STATE_DISCRIMINATOR)]
#[repr(C, packed)]
pub struct MinimalMarinadeState {
    pub _padding_0: [u8; 256],
    pub _padding_1: [u8; 128],
    pub _padding_2: [u8; 64],
    pub _padding_3: [u8; 32],
    pub _padding_4: [u8; 16],
    pub _padding_5: [u8; 8],
    /// Cached mSOL price: `msol_to_sol = msol_price / 2^32`.
    pub msol_price: u64,
}

assert_struct_size!(MinimalMarinadeState, 512);

impl MinimalMarinadeState {
    /// `msol_price` read by value (packed-safe).
    #[inline]
    pub fn msol_price(&self) -> u64 {
        self.msol_price
    }
}
