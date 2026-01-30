use crate::{
    assert_struct_align, assert_struct_size,
    constants::{
        ASSET_TAG_DEFAULT, MAX_ORACLE_KEYS, PYTH_PUSH_MIGRATED_DEPRECATED,
        TOTAL_ASSET_VALUE_INIT_LIMIT_INACTIVE,
    },
    types::{
        BankOperationalState, InterestRateConfig, InterestRateConfigCompact, InterestRateConfigOpt,
        OracleSetup, RiskTier,
    },
};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use bytemuck::{Pod, Zeroable};
use fixed::types::I80F48;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;
use super::WrappedI80F48;

assert_struct_size!(BankConfig, 544);
assert_struct_align!(BankConfig, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone, Eq)]
pub struct BankConfig {
    pub asset_weight_init: WrappedI80F48,
    pub asset_weight_maint: WrappedI80F48,

    pub liability_weight_init: WrappedI80F48,
    pub liability_weight_maint: WrappedI80F48,

    pub deposit_limit: u64,

    pub interest_rate_config: InterestRateConfig,
    pub operational_state: BankOperationalState,

    pub oracle_setup: OracleSetup,
    pub oracle_keys: [Pubkey; MAX_ORACLE_KEYS],

    // Note: Pubkey is aligned 1, so borrow_limit is the first aligned-8 value after deposit_limit
    pub _pad0: [u8; 6], // Bank state (1) + Oracle Setup (1) + 6 = 8

    pub borrow_limit: u64,

    pub risk_tier: RiskTier,

    /// Determines what kinds of assets users of this bank can interact with. Options:
    /// * `ASSET_TAG_DEFAULT` (0) - A regular asset that can be comingled with any other regular
    ///   asset or with `ASSET_TAG_SOL`
    /// * `ASSET_TAG_SOL` (1) - Accounts with a SOL position can comingle with **either**
    /// `ASSET_TAG_DEFAULT` or `ASSET_TAG_STAKED` positions, but not both
    /// * `ASSET_TAG_STAKED` (2) - Staked SOL assets. Accounts with a STAKED position can only
    /// deposit other STAKED assets or SOL (`ASSET_TAG_SOL`) and can only borrow SOL
    /// * `ASSET_TAG_KAMINO` (3) - Treated the same as `ASSET_TAG_DEFAULT`
    /// * `ASSET_TAG_DRIFT` (4) - Treated the same as `ASSET_TAG_DEFAULT`
    /// * `ASSET_TAG_SOLEND` (5) - Treated the same as `ASSET_TAG_DEFAULT`
    pub asset_tag: u8,

    /// Flags for various config options
    /// * 1 - Always set if bank created in 0.1.4 or later, or if migrated to the new pyth oracle
    ///   setup from a prior version. Not set in 0.1.3 or earlier banks using pyth that have not yet
    ///   migrated. Does nothing for banks that use switchboard.
    /// * 2, 4, 8, 16, etc - reserved for future use.
    pub config_flags: u8,

    pub _pad1: [u8; 5],

    /// USD denominated limit for calculating asset value for initialization margin requirements.
    /// Example, if total SOL deposits are equal to $1M and the limit it set to $500K, then SOL
    /// assets will be discounted by 50%.
    ///
    /// In other words the max value of liabilities that can be backed by the asset is $500K. This
    /// is useful for limiting the damage of orcale attacks.
    ///
    /// Value is UI USD value, for example value 100 -> $100
    pub total_asset_value_init_limit: u64,

    /// Time window in seconds for the oracle price feed to be considered live.
    pub oracle_max_age: u16,

    // pad to next 4-byte alignment to meet u32's requirements.
    pub _padding0: [u8; 2],

    /// From 0-100%, if the confidence exceeds this value, the oracle is considered invalid. Note:
    /// the confidence adjustment is capped at 5% regardless of this value.
    /// * 0 falls back to using the default 10% instead, i.e., U32_MAX_DIV_10
    /// * A %, as u32, e.g. 100% = u32::MAX, 50% = u32::MAX/2, etc.
    pub oracle_max_confidence: u32,

    /// Stored oracle price for `OracleSetup::Fixed`, otherwise does nothing
    pub fixed_price: WrappedI80F48,

    pub _padding1: [u8; 16],
}

impl Default for BankConfig {
    fn default() -> Self {
        Self {
            asset_weight_init: I80F48::ZERO.into(),
            asset_weight_maint: I80F48::ZERO.into(),
            liability_weight_init: I80F48::ONE.into(),
            liability_weight_maint: I80F48::ONE.into(),
            deposit_limit: 0,
            borrow_limit: 0,
            interest_rate_config: Default::default(),
            operational_state: BankOperationalState::Paused,
            oracle_setup: OracleSetup::None,
            oracle_keys: [Pubkey::default(); MAX_ORACLE_KEYS],
            _pad0: [0; 6],
            risk_tier: RiskTier::Isolated,
            asset_tag: ASSET_TAG_DEFAULT,
            config_flags: 0,
            _pad1: [0; 5],
            total_asset_value_init_limit: TOTAL_ASSET_VALUE_INIT_LIMIT_INACTIVE,
            oracle_max_age: 0,
            _padding0: [0; 2],
            oracle_max_confidence: 0,
            fixed_price: I80F48::ZERO.into(),
            _padding1: [0; 16],
        }
    }
}

#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Default, Clone, PartialEq, Eq)]
pub struct BankConfigOpt {
    pub asset_weight_init: Option<WrappedI80F48>,
    pub asset_weight_maint: Option<WrappedI80F48>,

    pub liability_weight_init: Option<WrappedI80F48>,
    pub liability_weight_maint: Option<WrappedI80F48>,

    pub deposit_limit: Option<u64>,
    pub borrow_limit: Option<u64>,

    pub operational_state: Option<BankOperationalState>,

    pub interest_rate_config: Option<InterestRateConfigOpt>,

    pub risk_tier: Option<RiskTier>,

    pub asset_tag: Option<u8>,

    pub total_asset_value_init_limit: Option<u64>,

    pub oracle_max_confidence: Option<u32>,

    pub oracle_max_age: Option<u16>,

    pub permissionless_bad_debt_settlement: Option<bool>,
    pub freeze_settings: Option<bool>,
    pub tokenless_repayments_allowed: Option<bool>,
}

#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Eq)]
pub struct BankConfigCompact {
    pub asset_weight_init: WrappedI80F48,
    pub asset_weight_maint: WrappedI80F48,

    pub liability_weight_init: WrappedI80F48,
    pub liability_weight_maint: WrappedI80F48,

    pub deposit_limit: u64,

    pub interest_rate_config: InterestRateConfigCompact,
    pub operational_state: BankOperationalState,

    pub borrow_limit: u64,

    pub risk_tier: RiskTier,

    /// Determines what kinds of assets users of this bank can interact with. Options:
    /// * `ASSET_TAG_DEFAULT` (0) - A regular asset that can be comingled with any other regular
    ///   asset or with `ASSET_TAG_SOL`
    /// * `ASSET_TAG_SOL` (1) - Accounts with a SOL position can comingle with **either**
    /// `ASSET_TAG_DEFAULT` or `ASSET_TAG_STAKED` positions, but not both
    /// * `ASSET_TAG_STAKED` (2) - Staked SOL assets. Accounts with a STAKED position can only
    /// deposit other STAKED assets or SOL (`ASSET_TAG_SOL`) and can only borrow SOL
    /// * `ASSET_TAG_KAMINO` (3) - Treated the same as `ASSET_TAG_DEFAULT`
    /// * `ASSET_TAG_DRIFT` (4) - Treated the same as `ASSET_TAG_DEFAULT`
    /// * `ASSET_TAG_SOLEND` (5) - Treated the same as `ASSET_TAG_DEFAULT`
    pub asset_tag: u8,

    /// Flags for various config options
    /// * 1 - Always set if bank created in 0.1.4 or later, or if migrated to the new oracle setup
    ///   from a prior version. Not set in 0.1.3 or earlier banks that have not yet migrated.
    /// * 2, 4, 8, 16, etc - reserved for future use.
    pub config_flags: u8,
    pub _pad0: [u8; 5],

    /// USD denominated limit for calculating asset value for initialization margin requirements.
    /// Example, if total SOL deposits are equal to $1M and the limit it set to $500K, then SOL
    /// assets will be discounted by 50%.
    ///
    /// In other words the max value of liabilities that can be backed by the asset is $500K. This
    /// is useful for limiting the damage of orcale attacks.
    ///
    /// Value is UI USD value, for example value 100 -> $100
    pub total_asset_value_init_limit: u64,

    /// Time window in seconds for the oracle price feed to be considered live.
    pub oracle_max_age: u16,

    /// From 0-100%, if the confidence exceeds this value, the oracle is considered invalid. Note:
    /// the confidence adjustment is capped at 5% regardless of this value.
    /// * 0% = use the default (10%)
    /// * A %, as u32, e.g. 100% = u32::MAX, 50% = u32::MAX/2, etc.
    pub oracle_max_confidence: u32,
}

impl Default for BankConfigCompact {
    fn default() -> Self {
        Self {
            asset_weight_init: I80F48::ZERO.into(),
            asset_weight_maint: I80F48::ZERO.into(),
            liability_weight_init: I80F48::ONE.into(),
            liability_weight_maint: I80F48::ONE.into(),
            deposit_limit: 0,
            borrow_limit: 0,
            interest_rate_config: InterestRateConfigCompact::default(),
            operational_state: BankOperationalState::Paused,
            config_flags: PYTH_PUSH_MIGRATED_DEPRECATED,
            _pad0: [0; 5],
            risk_tier: RiskTier::Isolated,
            asset_tag: ASSET_TAG_DEFAULT,
            total_asset_value_init_limit: TOTAL_ASSET_VALUE_INIT_LIMIT_INACTIVE,
            oracle_max_age: 0,
            oracle_max_confidence: 0,
        }
    }
}

impl From<BankConfigCompact> for BankConfig {
    fn from(config: BankConfigCompact) -> Self {
        let keys = [
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
        ];
        Self {
            asset_weight_init: config.asset_weight_init,
            asset_weight_maint: config.asset_weight_maint,
            liability_weight_init: config.liability_weight_init,
            liability_weight_maint: config.liability_weight_maint,
            deposit_limit: config.deposit_limit,
            interest_rate_config: config.interest_rate_config.into(),
            operational_state: config.operational_state,
            oracle_setup: OracleSetup::None,
            oracle_keys: keys,
            _pad0: [0; 6],
            borrow_limit: config.borrow_limit,
            risk_tier: config.risk_tier,
            asset_tag: config.asset_tag,
            config_flags: config.config_flags,
            _pad1: [0; 5],
            total_asset_value_init_limit: config.total_asset_value_init_limit,
            oracle_max_age: config.oracle_max_age,
            _padding0: [0; 2],
            oracle_max_confidence: config.oracle_max_confidence,
            fixed_price: I80F48::ZERO.into(),
            _padding1: [0; 16],
        }
    }
}

impl From<BankConfig> for BankConfigCompact {
    fn from(config: BankConfig) -> Self {
        Self {
            asset_weight_init: config.asset_weight_init,
            asset_weight_maint: config.asset_weight_maint,
            liability_weight_init: config.liability_weight_init,
            liability_weight_maint: config.liability_weight_maint,
            deposit_limit: config.deposit_limit,
            interest_rate_config: config.interest_rate_config.into(),
            operational_state: config.operational_state,
            borrow_limit: config.borrow_limit,
            risk_tier: config.risk_tier,
            asset_tag: config.asset_tag,
            config_flags: PYTH_PUSH_MIGRATED_DEPRECATED,
            _pad0: [0; 5],
            total_asset_value_init_limit: config.total_asset_value_init_limit,
            oracle_max_age: config.oracle_max_age,
            oracle_max_confidence: config.oracle_max_confidence,
        }
    }
}
