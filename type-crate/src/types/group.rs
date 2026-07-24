#[cfg(not(feature = "anchor"))]
use super::Pubkey;

use bytemuck::{Pod, Zeroable};

use crate::{assert_struct_size, constants::discriminators};

use super::{
    GroupRateLimiter, PanicStateCache, PremiumEntry, PremiumSettings, WrappedI80F48,
    MAX_PREMIUM_ENTRIES,
};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

assert_struct_size!(MarginfiGroup, 9248);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Debug, PartialEq, Eq)]
pub struct MarginfiGroup {
    /// Broadly able to modify anything, and can set/remove other admins at will.
    pub admin: Pubkey,
    /// Bitmask for group settings flags.
    /// * Bit 0 (1): `PROGRAM_FEES_ENABLED` — If set, program-level fees are enabled.
    /// * Bits 1-63: Reserved for future use.
    pub group_flags: u64,
    /// Caches information from the global `FeeState` so the FeeState can be omitted on certain ixes
    pub fee_state_cache: FeeStateCache,
    /// For groups initialized in versions 0.1.2 or greater, this is an authoritative count
    /// of the number of banks under this group. For groups initialized prior to 0.1.2,
    /// a non-authoritative count of the number of banks initiated after 0.1.2 went live.
    pub banks: u16,
    pub pad0: [u8; 6],
    /// This admin can configure collateral ratios above (but not below) the collateral ratio of
    /// certain banks, e.g. allow SOL to count as 90% collateral when borrowing an LST instead of
    /// the default rate.
    pub emode_admin: Pubkey,
    /// Can modify the fields in `config.interest_rate_config` but nothing else, for every bank
    /// under this group
    pub delegate_curve_admin: Pubkey,
    /// Can modify the `deposit_limit`, `borrow_limit`, `total_asset_value_init_limit` but nothing
    /// else, for every bank under this group
    pub delegate_limit_admin: Pubkey,
    /// DEPRECATED: currently has no on-chain authority.
    /// Preserved in account layout for backward compatibility and historical metadata only.
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

    /// Maximum leverage allowed for emode positions (initial margin), stored as u32 basis.
    /// Use `u32_to_basis` to convert to I80F48. Range: 1-100.
    pub emode_max_init_leverage: u32,
    /// Maximum leverage allowed for emode positions (maintenance margin), stored as u32 basis.
    /// Must be > emode_max_init_leverage. Range: 1-100.
    pub emode_max_maint_leverage: u32,

    /// Encoded same-asset automatic emode leverage for initial margin.
    /// Decode with `u32_to_basis`. Same-asset treatment is disabled when the decoded leverage is
    /// less than or equal to 1 and also requires each participating bank to opt in.
    pub same_asset_emode_init_leverage: u32,
    /// Encoded same-asset automatic emode leverage for maintenance margin.
    /// Decode with `u32_to_basis`. Ordering is validated in decoded space.
    pub same_asset_emode_maint_leverage: u32,

    /// Rate limiter for controlling aggregate withdraw/borrow outflow across all banks.
    /// Tracks net outflow in USD.
    pub rate_limiter: GroupRateLimiter,

    /// Last slot covered by an admin group rate limiter aggregation update.
    pub rate_limiter_last_admin_update_slot: u64,
    /// Monotonic sequence number for admin group rate limiter updates.
    /// This is used to enforce strict ordering and prevent duplicate/replayed batches
    /// when slot ranges overlap or multiple updates happen in the same slot.
    pub rate_limiter_last_admin_update_seq: u64,

    /// Last slot covered by an admin deleverage withdraw-limit aggregation update.
    pub deleverage_withdraw_last_admin_update_slot: u64,
    /// Monotonic sequence number for admin deleverage withdraw-limit updates.
    pub deleverage_withdraw_last_admin_update_seq: u64,

    /// Can modify flow-control status for the group, i.e. update the withdraw caches with flow
    /// information from banks. Typically this is a hot wallet that lives in e.g. some cron job. If
    /// compromised, flow control can be effectively disabled until the admin is restored, which
    /// does not itself compromise any funds, and is merely annoying.
    pub delegate_flow_admin: Pubkey,

    /// Header for the pairwise variable-borrow premium matrix stored in `premium_entries`.
    /// Occupies the former `_padding_0`/`_padding_1` region of the v1 layout, so v1 accounts
    /// resize to a zeroed header (matrix off).
    pub premium_settings: PremiumSettings,
    /// Pairwise variable-borrow premium rates, keyed by (collateral `premium_tag`, liability
    /// `premium_tag`). Live entries occupy the first `premium_settings.entry_count` slots.
    /// Read only via `find_premium_rate`. Future capacity growth carves from `_padding_2`.
    pub premium_entries: [PremiumEntry; MAX_PREMIUM_ENTRIES],
    pub _padding_2: [[u64; 32]; 32],
}

impl MarginfiGroup {
    pub const LEN: usize = std::mem::size_of::<MarginfiGroup>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::GROUP;
    /// Struct size of the PREVIOUS (v1) group layout — the size of accounts created before
    /// `_padding_2` existed, and a byte-identical prefix of the current layout.
    pub const V1_LEN: usize = 1056;
}

impl Default for MarginfiGroup {
    fn default() -> Self {
        Self::zeroed()
    }
}

#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorSerialize, AnchorDeserialize))]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
/// Cached fee configuration propagated from the global FeeState
pub struct FeeStateCache {
    /// The wallet that receives program-level fees
    pub global_fee_wallet: Pubkey,
    /// Fixed fee APR charged to borrowers (program-level)
    pub program_fee_fixed: WrappedI80F48,
    /// Proportional fee rate on interest (program-level)
    pub program_fee_rate: WrappedI80F48,
    /// Unix timestamp of the last fee state propagation
    pub last_update: i64,
}

#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorSerialize, AnchorDeserialize))]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
/// Tracks deleverage withdrawal limits to protect against compromised risk admin
pub struct WithdrawWindowCache {
    /// Maximum USD value that can be withdrawn per day via deleverage (0 = no limit)
    pub daily_limit: u32,
    /// USD value withdrawn today via deleverage (approximate, rounded)
    pub withdrawn_today: u32,
    /// Unix timestamp of the last daily counter reset
    pub last_daily_reset_timestamp: i64,
}
