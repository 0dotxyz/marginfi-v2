use anchor_lang::prelude::*;
use marginfi_type_crate::assert_struct_size;

// Anchor discriminator for Marinade's `State`, sha256("account:State")[0..8]. Verified against the
// live mainnet State account 8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC.
pub const STATE_DISCRIMINATOR: [u8; 8] = [216, 146, 107, 94, 104, 75, 182, 177];

/// Denominator for Marinade's cached `msol_price`: `msol_to_sol = msol_price / 2^32`.
pub const MSOL_PRICE_PRECISION: u128 = 1 << 32;

/// Minimal zero-copy view of Marinade's `State`.
#[account(zero_copy, discriminator = &STATE_DISCRIMINATOR)]
#[repr(C, packed)]
pub struct MinimalMarinadeState {
    pub msol_mint: Pubkey, // 0
    pub _padding1: [u8; 128],
    pub _padding2: [u8; 32],
    pub _padding3: [u8; 16],
    pub _padding4: [u8; 8],
    pub _padding5: [u8; 2],
    pub delayed_unstake_cooling_down: u64, // 218 (stake_system)
    pub _padding6: [u8; 128],
    pub _padding7: [u8; 8],
    pub _padding8: [u8; 4],
    pub _padding9: [u8; 2],
    pub total_active_balance: u64, // 368 (validator_system)
    pub _padding10: [u8; 64],
    pub _padding11: [u8; 32],
    pub _padding12: [u8; 16],
    pub available_reserve_balance: u64,  // 488
    pub msol_supply: u64,                // 496
    pub msol_price: u64,                 // 504 (cached; unused by pricing)
    pub _padding13: [u8; 8],             // circulating_ticket_count @512
    pub circulating_ticket_balance: u64, // 520
    pub _padding14: [u8; 32],
    pub emergency_cooling_down: u64, // 560
}

assert_struct_size!(MinimalMarinadeState, 568);

impl MinimalMarinadeState {
    #[inline]
    pub fn msol_mint(&self) -> Pubkey {
        self.msol_mint
    }

    #[inline]
    pub fn msol_price(&self) -> u64 {
        self.msol_price
    }

    #[inline]
    pub fn msol_supply(&self) -> u64 {
        self.msol_supply
    }

    /// Mirrors Marinade's `total_virtual_staked_lamports` (`None` only on additive overflow):
    /// https://github.com/marinade-finance/liquid-staking-program/blob/b8fe3f8f9a2bb0978fb40ba5bb1c2855dd12940f/programs/marinade-finance/src/state/mod.rs#L229
    #[inline]
    pub fn total_virtual_staked_lamports(&self) -> Option<u64> {
        let under_control = self
            .total_active_balance
            .checked_add(self.delayed_unstake_cooling_down)?
            .checked_add(self.emergency_cooling_down)?
            .checked_add(self.available_reserve_balance)?;
        Some(under_control.saturating_sub(self.circulating_ticket_balance))
    }
}
