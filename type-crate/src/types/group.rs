#[cfg(not(feature = "anchor"))]
use super::Pubkey;

use bytemuck::{Pod, Zeroable};

use crate::{assert_struct_size, constants::discriminators};

use super::{PanicStateCache, WrappedI80F48};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

assert_struct_size!(MarginfiGroup, 1056);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Default, Debug, PartialEq, Eq)]
pub struct MarginfiGroup {
    /// Broadly able to modify anything, and can set/remove other admins at will.
    pub admin: Pubkey,
    /// Bitmask for group settings flags.
    /// * 0: `PROGRAM_FEES_ENABLED` If set, program-level fees are enabled.
    /// * 1: `ARENA_GROUP` Deprecated, available for future use.
    /// * Bits 1-63: Reserved for future use.
    pub group_flags: u64,
    /// Caches information from the global `FeeState` so the FeeState can be omitted on certain ixes
    pub fee_state_cache: FeeStateCache,
    // For groups initialized in versions 0.1.2 or greater, this is an authoritative count
    // of the number of banks under this group. For groups initialized prior to 0.1.2,
    // a non-authoritative count of the number of banks initiated after 0.1.2 went live.
    pub banks: u16,
    pub pad0: [u8; 6],
    /// This admin can configure collateral ratios above (but not below) the collateral ratio of
    /// certain banks , e.g. allow SOL to count as 90% collateral when borrowing an LST instead of
    /// the default rate.
    pub emode_admin: Pubkey,
    // Can modify the fields in `config.interest_rate_config` but nothing else, for every bank under
    // this group
    pub delegate_curve_admin: Pubkey,
    /// Can modify the `deposit_limit`, `borrow_limit`, `total_asset_value_init_limit` but nothing
    /// else, for every bank under this group
    pub delegate_limit_admin: Pubkey,
    /// Can modify the emissions `flags`, `emissions_rate` and `emissions_mint`, but nothing else,
    /// for every bank under this group
    pub delegate_emissions_admin: Pubkey,
    /// When program keeper temporarily puts the program into panic mode, information about the
    /// duration of the lockup will be available here.
    pub panic_state_cache: PanicStateCache,
    /// Keeps track of the liquidity withdrawn from the group over the day as a result of
    /// deleverages. Used as a protection mechanism against too big (and unwanted) withdrawals (e.g.
    /// when the risk admin is compromised).
    pub deleverage_withdraw_window_cache: WithdrawWindowCache,

    /// Can run bankruptcy and forced deleverage ixes to e.g. sunset risky/illiquid assets
    pub risk_admin: Pubkey,
    /// Can modify a Bank's metadata, and nothing else.
    pub metadata_admin: Pubkey,

    pub emode_max_init_leverage: u32,
    pub emode_max_maint_leverage: u32,

    pub _padding: [u8; 8],
    pub _padding_0: [[u64; 2]; 11],
    pub _padding_1: [[u64; 2]; 32],
}

impl MarginfiGroup {
    pub const LEN: usize = std::mem::size_of::<MarginfiGroup>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::GROUP;
}

#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorSerialize, AnchorDeserialize))]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct FeeStateCache {
    pub global_fee_wallet: Pubkey,
    pub program_fee_fixed: WrappedI80F48,
    pub program_fee_rate: WrappedI80F48,
    pub last_update: i64,
}

#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorSerialize, AnchorDeserialize))]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct WithdrawWindowCache {
    pub daily_limit: u32,
    pub withdrawn_today: u32, // in USD, approximate and rounded
    pub last_daily_reset_timestamp: i64,
}
