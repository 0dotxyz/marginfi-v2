use anchor_lang::prelude::*;
use marginfi_type_crate::assert_struct_size;

// Anchor discriminator for Exponent's PT vault account.
pub const VAULT_DISCRIMINATOR: [u8; 8] = [211, 8, 232, 43, 2, 152, 117, 119];

/// `status` bit that allows `merge`, Exponent's only PT redemption path.
pub const STATUS_CAN_MERGE: u8 = 0b0000_0010;

/// Scaling factor of Exponent's `Number` type.
pub const SY_EXCHANGE_RATE_PRECISION: u128 = 1_000_000_000_000;

/// Minimal zero-copy view of an Exponent PT vault. The PT price accretes linearly to par over
/// `[start_ts, start_ts + duration]`; `sy_for_pt` / `pt_supply` / `last_seen_sy_exchange_rate` give
/// the redemption backing that caps it.
///
/// Mirrors the leading fields of Exponent's `Vault`; see `pt_redemption_rate` and
/// `sy_backing_for_pt` for the redemption math:
/// <https://github.com/exponent-finance/exponent-core/blob/main/programs/exponent_core/src/state/vault.rs>
#[account(zero_copy, discriminator = &VAULT_DISCRIMINATOR)]
#[repr(C, packed)]
pub struct MinimalExponentVault {
    pub _padding_0: [u8; 96],
    pub mint_pt: Pubkey,
    pub _padding_1: [u8; 128],
    pub start_ts: u32,
    pub duration: u32,
    pub _padding_2: [u8; 32],
    pub _padding_3: [u8; 32],
    pub _padding_4: [u8; 1],
    /// Asset per SY, scaled by `SY_EXCHANGE_RATE_PRECISION`. Exponent stores it as a 256-bit
    /// integer, i.e. four u64 words, least significant first.
    pub last_seen_sy_exchange_rate: [u64; 4],
    /// Exponent's watermark: the highest SY rate seen before expiry. A live rate below it (negative
    /// yield) shrinks `sy_for_pt`, which already marks the PT down.
    pub all_time_high_sy_exchange_rate: [u64; 4],
    pub _padding_6: [u8; 32],
    pub _padding_7: [u8; 8],
    /// Total SY set aside to back PT holders.
    pub sy_for_pt: u64,
    pub pt_supply: u64,
    pub _padding_8: [u8; 64],
    pub _padding_9: [u8; 2],
    /// Operation flags; the pause admin can clear `STATUS_CAN_MERGE` to halt PT redemption.
    pub status: u8,
}

assert_struct_size!(MinimalExponentVault, 516);

impl MinimalExponentVault {
    #[inline]
    pub fn mint_pt(&self) -> Pubkey {
        self.mint_pt
    }

    #[inline]
    pub fn start_ts(&self) -> u32 {
        self.start_ts
    }

    #[inline]
    pub fn duration(&self) -> u32 {
        self.duration
    }

    #[inline]
    pub fn sy_for_pt(&self) -> u64 {
        self.sy_for_pt
    }

    #[inline]
    pub fn pt_supply(&self) -> u64 {
        self.pt_supply
    }

    /// Exponent has paused merging, so PT can't be redeemed.
    #[inline]
    pub fn is_in_emergency(&self) -> bool {
        self.status & STATUS_CAN_MERGE == 0
    }

    /// The rate as a plain `u64`, or `None` if it does not fit in one. Real rates run ~1e12-1e14
    /// once scaled, five orders of magnitude short of `u64::MAX`, so a value needing the upper
    /// words is not an exchange rate.
    #[inline]
    pub fn last_seen_sy_exchange_rate_raw(&self) -> Option<u64> {
        let words = self.last_seen_sy_exchange_rate;
        (words[1] == 0 && words[2] == 0 && words[3] == 0).then_some(words[0])
    }
}
