use anchor_lang::prelude::*;
use marginfi_type_crate::assert_struct_size;

// Anchor discriminator for Exponent's PT vault account.
pub const VAULT_DISCRIMINATOR: [u8; 8] = [211, 8, 232, 43, 2, 152, 117, 119];

/// Minimal zero-copy view of an Exponent PT vault, exposing `start_ts` / `duration` (u32s) at byte
/// offsets 264 / 268. The PT price accretes linearly to par over `[start_ts, start_ts + duration]`.
#[account(zero_copy, discriminator = &VAULT_DISCRIMINATOR)]
#[repr(C, packed)]
pub struct MinimalExponentVault {
    pub _padding: [u8; 256],
    /// Unix timestamp (seconds) when PT pricing begins.
    pub start_ts: u32,
    /// Seconds from `start_ts` to maturity.
    pub duration: u32,
}

assert_struct_size!(MinimalExponentVault, 264);

impl MinimalExponentVault {
    /// `start_ts` read by value (packed-safe).
    #[inline]
    pub fn start_ts(&self) -> u32 {
        self.start_ts
    }

    /// `duration` read by value (packed-safe).
    #[inline]
    pub fn duration(&self) -> u32 {
        self.duration
    }
}
