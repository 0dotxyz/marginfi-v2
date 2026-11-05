#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

use crate::{
    assert_struct_align, assert_struct_size,
    constants::{discriminators, STAKED_ORACLE_DISABLED, STAKED_ORACLE_PRICE_USES_ONRAMP},
};

use super::{OracleSetup, RiskTier, WrappedI80F48};
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
    /// The SOL/USD base feed: the Pyth push oracle, or a Scope `OraclePrices` account when
    /// `oracle_setup` is `StakedWithScope`.
    pub oracle: Pubkey,

    pub asset_weight_init: WrappedI80F48,
    pub asset_weight_maint: WrappedI80F48,

    pub deposit_limit: u64,
    pub total_asset_value_init_limit: u64,

    pub oracle_max_age: u16,
    pub risk_tier: RiskTier,
    _pad0: u8,
    /// Scope entry for the SOL/USD price in `oracle`; read only by `StakedWithScope`.
    pub scope_entry_index: u16,
    /// The staked banks' `OracleSetup` as u8: `StakedWithPythPush` or `StakedWithScope`. Zero
    /// (accounts written before this field existed) means `StakedWithPythPush`.
    pub oracle_setup: u8,
    _pad1: u8,

    /// Desired bitmask for staked-bank transition flags. These bits are copied to `Bank.flags`
    /// when staked settings are propagated or when a new staked bank is created.
    /// * Bit 9 (512): `STAKED_ORACLE_DISABLED` — staked oracle pricing is temporarily disabled.
    /// * Bit 10 (1024): `STAKED_ORACLE_PRICE_USES_ONRAMP` — staked oracle pricing includes the SPL
    ///   single-pool on-ramp account in NAV.
    pub flags: u64,

    /// Scope entry for the time-weighted SOL/USD price in `oracle` (a `PythLazerEMA` or
    /// `ScopeTwap*` slot of the base entry); 0 = none. Read only by
    /// `StakedWithScope`, copied into `BankConfig.scope_smoothed_entry_index` on propagation.
    pub scope_smoothed_entry_index: u16,

    /// The following values are irrelevant because staked collateral positions do not support
    /// borrowing.
    // * interest_config,
    // * liability_weight_init
    // * liability_weight_maint
    // * borrow_limit
    _reserved1: [u8; 30],
    _reserved2: [u8; 64],
}

// To be removed once SVSP update is rolled out (likely in 1.10)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnRampTransition {
    PreTransition,
    StakeOraclesDisabled,
    OnRampEnabled,
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

    /// `Some(entry_index)` when staked banks price SOL from a Scope entry, `None` for Pyth.
    pub fn scope_base_feed(&self) -> Option<u16> {
        (self.staked_oracle_setup() == OracleSetup::StakedWithScope)
            .then_some(self.scope_entry_index)
    }

    /// The setup staked banks are propagated to.
    pub fn staked_oracle_setup(&self) -> OracleSetup {
        if self.oracle_setup == OracleSetup::StakedWithScope as u8 {
            OracleSetup::StakedWithScope
        } else {
            OracleSetup::StakedWithPythPush
        }
    }

    /// Points the staked banks at Pyth (`None`) or at a Scope entry (`Some(entry_index)`). Pyth
    /// staked banks have no use for a Scope smoothed entry, so switching to Pyth clears it.
    pub fn set_scope_base_feed(&mut self, entry_index: Option<u16>) {
        match entry_index {
            Some(index) => {
                self.oracle_setup = OracleSetup::StakedWithScope as u8;
                self.scope_entry_index = index;
            }
            None => {
                self.oracle_setup = OracleSetup::StakedWithPythPush as u8;
                self.scope_entry_index = 0;
                self.set_scope_smoothed_entry(None);
            }
        }
    }

    /// `Some(entry_index)` when staked banks take their time-weighted SOL/USD price from a Scope
    /// entry, `None` when they use the spot price for both price types. Only `StakedWithScope`
    /// can carry one.
    pub fn scope_smoothed_entry(&self) -> Option<u16> {
        (self.staked_oracle_setup() == OracleSetup::StakedWithScope
            && self.scope_smoothed_entry_index != 0)
            .then_some(self.scope_smoothed_entry_index)
    }

    pub fn set_scope_smoothed_entry(&mut self, entry_index: Option<u16>) {
        self.scope_smoothed_entry_index = entry_index.unwrap_or(0);
    }

    // To be removed once SVSP update is rolled out (likely in 1.10)
    pub fn on_ramp_transition(&self) -> OnRampTransition {
        if self.flags & STAKED_ORACLE_PRICE_USES_ONRAMP != 0 {
            OnRampTransition::OnRampEnabled
        } else if self.flags & STAKED_ORACLE_DISABLED != 0 {
            OnRampTransition::StakeOraclesDisabled
        } else {
            OnRampTransition::PreTransition
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
            _pad0: 0,
            scope_entry_index: 0,
            oracle_setup: OracleSetup::StakedWithPythPush as u8,
            _pad1: 0,
            flags: 0,
            scope_smoothed_entry_index: 0,
            _reserved1: [0; 30],
            _reserved2: [0; 64],
        }
    }
}
