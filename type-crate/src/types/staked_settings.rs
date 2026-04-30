#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

use crate::{assert_struct_align, assert_struct_size, constants::discriminators};

use super::{RiskTier, WrappedI80F48};
use fixed_macro::types::I80F48;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

assert_struct_size!(StakedSettings, 256);
assert_struct_align!(StakedSettings, 8);
/// Unique per-group. Staked Collateral banks created under a group automatically use these
/// settings. Groups that have not created this struct cannot create staked collateral banks. When
/// this struct updates, changes must be permissionlessly propagated to staked collateral banks.
/// Administrators can also edit the bank manually, i.e. with configure_bank, to temporarily make
/// changes such as raising the deposit limit for a single bank.
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone)
)]
pub struct StakedSettings {
    /// This account's own key. A PDA derived from `marginfi_group` and `STAKED_SETTINGS_SEED`
    pub key: Pubkey,
    /// Group for which these settings apply
    pub marginfi_group: Pubkey,
    /// Generally, the Pyth push oracle for SOL
    pub oracle: Pubkey,

    pub asset_weight_init: WrappedI80F48,
    pub asset_weight_maint: WrappedI80F48,

    pub deposit_limit: u64,
    pub total_asset_value_init_limit: u64,

    pub oracle_max_age: u16,
    pub risk_tier: RiskTier,
    _pad0: [u8; 5],

    /// The following values are irrelevant because staked collateral positions do not support
    /// borrowing.
    // * interest_config,
    // * liability_weight_init
    // * liability_weight_maint
    // * borrow_limit
    _reserved0: [u8; 8],
    _reserved1: [u8; 32],
    _reserved2: [u8; 64],
}

impl StakedSettings {
    pub const LEN: usize = std::mem::size_of::<StakedSettings>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::STAKED_SETTINGS;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: Pubkey,
        marginfi_group: Pubkey,
        oracle: Pubkey,
        asset_weight_init: WrappedI80F48,
        asset_weight_maint: WrappedI80F48,
        deposit_limit: u64,
        total_asset_value_init_limit: u64,
        oracle_max_age: u16,
        risk_tier: RiskTier,
    ) -> Self {
        StakedSettings {
            key,
            marginfi_group,
            oracle,
            asset_weight_init,
            asset_weight_maint,
            deposit_limit,
            total_asset_value_init_limit,
            oracle_max_age,
            risk_tier,
            ..Default::default()
        }
    }
}

impl Default for StakedSettings {
    fn default() -> Self {
        StakedSettings {
            key: Pubkey::default(),
            marginfi_group: Pubkey::default(),
            oracle: Pubkey::default(),
            asset_weight_init: I80F48!(0.8).into(),
            asset_weight_maint: I80F48!(0.9).into(),
            deposit_limit: 1_000_000,
            total_asset_value_init_limit: 1_000_000,
            oracle_max_age: 10,
            risk_tier: RiskTier::Collateral,
            _pad0: [0; 5],
            _reserved0: [0; 8],
            _reserved1: [0; 32],
            _reserved2: [0; 64],
        }
    }
}
