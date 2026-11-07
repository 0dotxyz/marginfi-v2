//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::types::{StakedSettingsConfig, StakedSettingsEditConfig, WrappedI80F48};
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`config_group_fee`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigGroupFee {
    pub marginfi_group: Pubkey,
    /// `global_fee_admin` of the FeeState
    pub global_fee_admin: Pubkey,
    pub fee_state: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl ConfigGroupFee {
    pub const DISCRIMINATOR: [u8; 8] = [231, 205, 66, 242, 220, 87, 145, 38];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.global_fee_admin, true),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for ConfigGroupFee {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (global fee admin only) Enable or disable program fees for any group. Does not require the
/// group admin to sign: the global fee state admin can turn program fees on or off for any
/// group
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn config_group_fee(accounts: &ConfigGroupFee, enable_program_fee: bool) -> Instruction {
    config_group_fee_with_program_id(crate::ID, accounts, enable_program_fee)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn config_group_fee_with_program_id(
    program_id: Pubkey,
    accounts: &ConfigGroupFee,
    enable_program_fee: bool,
) -> Instruction {
    let mut data = ConfigGroupFee::DISCRIMINATOR.to_vec();
    enable_program_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`configure_bank_rate_limits`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureBankRateLimits {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl ConfigureBankRateLimits {
    pub const DISCRIMINATOR: [u8; 8] = [175, 84, 85, 221, 206, 220, 110, 174];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for ConfigureBankRateLimits {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin or delegate_limit_admin) Configure bank-level rate limits for withdraw/borrow.
/// Rate limits track net outflow in native tokens. Deposits offset withdraws.
/// Set to 0 to disable. Hourly and daily windows are independent.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn configure_bank_rate_limits(
    accounts: &ConfigureBankRateLimits,
    hourly_max_outflow: Option<u64>,
    daily_max_outflow: Option<u64>,
) -> Instruction {
    configure_bank_rate_limits_with_program_id(
        crate::ID,
        accounts,
        hourly_max_outflow,
        daily_max_outflow,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn configure_bank_rate_limits_with_program_id(
    program_id: Pubkey,
    accounts: &ConfigureBankRateLimits,
    hourly_max_outflow: Option<u64>,
    daily_max_outflow: Option<u64>,
) -> Instruction {
    let mut data = ConfigureBankRateLimits::DISCRIMINATOR.to_vec();
    hourly_max_outflow
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    daily_max_outflow
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`configure_deleverage_withdrawal_limit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureDeleverageWithdrawalLimit {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl ConfigureDeleverageWithdrawalLimit {
    pub const DISCRIMINATOR: [u8; 8] = [28, 132, 205, 158, 67, 77, 177, 63];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for ConfigureDeleverageWithdrawalLimit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin or delegate_limit_admin) Set the daily withdrawal limit for deleverages per group.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn configure_deleverage_withdrawal_limit(
    accounts: &ConfigureDeleverageWithdrawalLimit,
    limit: u32,
) -> Instruction {
    configure_deleverage_withdrawal_limit_with_program_id(crate::ID, accounts, limit)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn configure_deleverage_withdrawal_limit_with_program_id(
    program_id: Pubkey,
    accounts: &ConfigureDeleverageWithdrawalLimit,
    limit: u32,
) -> Instruction {
    let mut data = ConfigureDeleverageWithdrawalLimit::DISCRIMINATOR.to_vec();
    limit
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`configure_group_rate_limits`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureGroupRateLimits {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl ConfigureGroupRateLimits {
    pub const DISCRIMINATOR: [u8; 8] = [111, 47, 213, 142, 158, 51, 226, 102];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for ConfigureGroupRateLimits {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (admin or delegate_limit_admin) Configure group-level rate limits for withdraw/borrow.
/// Rate limits track aggregate net outflow in USD.
/// Example: $10M = 10_000_000. Set to 0 to disable.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn configure_group_rate_limits(
    accounts: &ConfigureGroupRateLimits,
    hourly_max_outflow_usd: Option<u64>,
    daily_max_outflow_usd: Option<u64>,
) -> Instruction {
    configure_group_rate_limits_with_program_id(
        crate::ID,
        accounts,
        hourly_max_outflow_usd,
        daily_max_outflow_usd,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn configure_group_rate_limits_with_program_id(
    program_id: Pubkey,
    accounts: &ConfigureGroupRateLimits,
    hourly_max_outflow_usd: Option<u64>,
    daily_max_outflow_usd: Option<u64>,
) -> Instruction {
    let mut data = ConfigureGroupRateLimits::DISCRIMINATOR.to_vec();
    hourly_max_outflow_usd
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    daily_max_outflow_usd
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`disable_staked_oracles`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisableStakedOracles {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub staked_settings: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl DisableStakedOracles {
    pub const DISCRIMINATOR: [u8; 8] = [43, 90, 152, 55, 66, 101, 232, 200];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.staked_settings, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for DisableStakedOracles {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (slow governance_admin only) Disable stake pricing, i.e. effectively forbidding all operations
/// involving stake banks.
/// To be used during the rollout of the SVSP upgrade.
/// To be removed once SVSP update is rolled out (likely in 1.10)
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn disable_staked_oracles(accounts: &DisableStakedOracles) -> Instruction {
    disable_staked_oracles_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn disable_staked_oracles_with_program_id(
    program_id: Pubkey,
    accounts: &DisableStakedOracles,
) -> Instruction {
    let data = DisableStakedOracles::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`edit_fee_state_premium`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditFeeStatePremium {
    /// Admin of the global FeeState
    pub global_fee_admin: Pubkey,
    pub fee_state: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl EditFeeStatePremium {
    pub const DISCRIMINATOR: [u8; 8] = [159, 148, 85, 109, 94, 212, 93, 233];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.global_fee_admin, true),
            AccountMeta::new(self.fee_state, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for EditFeeStatePremium {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (global fee admin only) Adjust the variable-borrow premium wallet on the fee state.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn edit_fee_state_premium(
    accounts: &EditFeeStatePremium,
    premium_wallet: Pubkey,
) -> Instruction {
    edit_fee_state_premium_with_program_id(crate::ID, accounts, premium_wallet)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn edit_fee_state_premium_with_program_id(
    program_id: Pubkey,
    accounts: &EditFeeStatePremium,
    premium_wallet: Pubkey,
) -> Instruction {
    let mut data = EditFeeStatePremium::DISCRIMINATOR.to_vec();
    premium_wallet
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`edit_global_fee_state`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditGlobalFeeState {
    /// Admin of the global FeeState
    pub global_fee_admin: Pubkey,
    pub fee_state: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl EditGlobalFeeState {
    pub const DISCRIMINATOR: [u8; 8] = [52, 62, 35, 129, 93, 69, 165, 202];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.global_fee_admin, true),
            AccountMeta::new(self.fee_state, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for EditGlobalFeeState {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (global fee admin only) Adjust fees, admin, wallet, or pause delegate admin
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn edit_global_fee_state(
    accounts: &EditGlobalFeeState,
    admin: Option<Pubkey>,
    fee_wallet: Option<Pubkey>,
    bank_init_flat_sol_fee: Option<u32>,
    liquidation_flat_sol_fee: Option<u32>,
    order_init_flat_sol_fee: Option<u32>,
    program_fee_fixed: Option<WrappedI80F48>,
    program_fee_rate: Option<WrappedI80F48>,
    liquidation_max_fee: Option<WrappedI80F48>,
    order_execution_max_fee: Option<WrappedI80F48>,
    pause_delegate_admin: Option<Pubkey>,
    account_transfer_fee: Option<u32>,
) -> Instruction {
    edit_global_fee_state_with_program_id(
        crate::ID,
        accounts,
        admin,
        fee_wallet,
        bank_init_flat_sol_fee,
        liquidation_flat_sol_fee,
        order_init_flat_sol_fee,
        program_fee_fixed,
        program_fee_rate,
        liquidation_max_fee,
        order_execution_max_fee,
        pause_delegate_admin,
        account_transfer_fee,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn edit_global_fee_state_with_program_id(
    program_id: Pubkey,
    accounts: &EditGlobalFeeState,
    admin: Option<Pubkey>,
    fee_wallet: Option<Pubkey>,
    bank_init_flat_sol_fee: Option<u32>,
    liquidation_flat_sol_fee: Option<u32>,
    order_init_flat_sol_fee: Option<u32>,
    program_fee_fixed: Option<WrappedI80F48>,
    program_fee_rate: Option<WrappedI80F48>,
    liquidation_max_fee: Option<WrappedI80F48>,
    order_execution_max_fee: Option<WrappedI80F48>,
    pause_delegate_admin: Option<Pubkey>,
    account_transfer_fee: Option<u32>,
) -> Instruction {
    let mut data = EditGlobalFeeState::DISCRIMINATOR.to_vec();
    admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    fee_wallet
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    bank_init_flat_sol_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    liquidation_flat_sol_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    order_init_flat_sol_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    program_fee_fixed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    program_fee_rate
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    liquidation_max_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    order_execution_max_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    pause_delegate_admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    account_transfer_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`edit_staked_settings`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditStakedSettings {
    pub marginfi_group: Pubkey,
    pub governance_admin: Pubkey,
    pub staked_settings: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl EditStakedSettings {
    pub const DISCRIMINATOR: [u8; 8] = [11, 108, 215, 87, 240, 9, 66, 241];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.staked_settings, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for EditStakedSettings {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (slow governance_admin only) Edit the staked collateral settings for the group.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn edit_staked_settings(
    accounts: &EditStakedSettings,
    settings: StakedSettingsEditConfig,
) -> Instruction {
    edit_staked_settings_with_program_id(crate::ID, accounts, settings)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn edit_staked_settings_with_program_id(
    program_id: Pubkey,
    accounts: &EditStakedSettings,
    settings: StakedSettingsEditConfig,
) -> Instruction {
    let mut data = EditStakedSettings::DISCRIMINATOR.to_vec();
    settings
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`enable_staked_oracle_onramp`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnableStakedOracleOnramp {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub staked_settings: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl EnableStakedOracleOnramp {
    pub const DISCRIMINATOR: [u8; 8] = [114, 248, 244, 6, 74, 212, 222, 230];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.staked_settings, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for EnableStakedOracleOnramp {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (slow governance_admin only) Enable SPL single-pool on-ramp lamports in staked-collateral oracle
/// pricing.
/// To be removed once SVSP update is rolled out (likely in 1.10)
/// This flips a per-group config flag so that every staked oracle uses the canonical single-pool NAV
/// formula.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn enable_staked_oracle_onramp(accounts: &EnableStakedOracleOnramp) -> Instruction {
    enable_staked_oracle_onramp_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn enable_staked_oracle_onramp_with_program_id(
    program_id: Pubkey,
    accounts: &EnableStakedOracleOnramp,
) -> Instruction {
    let data = EnableStakedOracleOnramp::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`init_global_fee_state`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitGlobalFeeState {
    /// Pays the init fee
    pub payer: Pubkey,
    pub fee_state: Pubkey,
    pub system_program: Pubkey,
}
impl InitGlobalFeeState {
    pub const DISCRIMINATOR: [u8; 8] = [82, 48, 247, 59, 220, 109, 231, 44];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.payer, true),
            AccountMeta::new(self.fee_state, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for InitGlobalFeeState {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (Runs once per program) Configures the fee state account, where the global admin sets fees
/// that are assessed to the protocol
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn init_global_fee_state(
    accounts: &InitGlobalFeeState,
    admin: Pubkey,
    fee_wallet: Pubkey,
    bank_init_flat_sol_fee: u32,
    liquidation_flat_sol_fee: u32,
    order_init_flat_sol_fee: u32,
    program_fee_fixed: WrappedI80F48,
    program_fee_rate: WrappedI80F48,
    liquidation_max_fee: WrappedI80F48,
    order_execution_max_fee: WrappedI80F48,
) -> Instruction {
    init_global_fee_state_with_program_id(
        crate::ID,
        accounts,
        admin,
        fee_wallet,
        bank_init_flat_sol_fee,
        liquidation_flat_sol_fee,
        order_init_flat_sol_fee,
        program_fee_fixed,
        program_fee_rate,
        liquidation_max_fee,
        order_execution_max_fee,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn init_global_fee_state_with_program_id(
    program_id: Pubkey,
    accounts: &InitGlobalFeeState,
    admin: Pubkey,
    fee_wallet: Pubkey,
    bank_init_flat_sol_fee: u32,
    liquidation_flat_sol_fee: u32,
    order_init_flat_sol_fee: u32,
    program_fee_fixed: WrappedI80F48,
    program_fee_rate: WrappedI80F48,
    liquidation_max_fee: WrappedI80F48,
    order_execution_max_fee: WrappedI80F48,
) -> Instruction {
    let mut data = InitGlobalFeeState::DISCRIMINATOR.to_vec();
    admin
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    fee_wallet
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    bank_init_flat_sol_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    liquidation_flat_sol_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    order_init_flat_sol_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    program_fee_fixed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    program_fee_rate
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    liquidation_max_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    order_execution_max_fee
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`init_staked_settings`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitStakedSettings {
    pub marginfi_group: Pubkey,
    pub governance_admin: Pubkey,
    /// Pays the init fee
    pub fee_payer: Pubkey,
    pub staked_settings: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl InitStakedSettings {
    pub const DISCRIMINATOR: [u8; 8] = [52, 35, 149, 44, 69, 86, 69, 80];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new(self.staked_settings, false),
            AccountMeta::new_readonly(self.system_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for InitStakedSettings {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (slow governance_admin only) Init the Staked Settings account, which is used to create staked
/// collateral banks, and must run before any staked collateral bank can be created with
/// `add_pool_permissionless`. Running this ix effectively opts the group into the staked
/// collateral feature.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn init_staked_settings(
    accounts: &InitStakedSettings,
    settings: StakedSettingsConfig,
) -> Instruction {
    init_staked_settings_with_program_id(crate::ID, accounts, settings)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn init_staked_settings_with_program_id(
    program_id: Pubkey,
    accounts: &InitStakedSettings,
    settings: StakedSettingsConfig,
) -> Instruction {
    let mut data = InitStakedSettings::DISCRIMINATOR.to_vec();
    settings
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`panic_pause`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanicPause {
    /// Global fee admin or the dedicated pause delegate admin.
    pub pause_authority: Pubkey,
    /// Global fee state account containing the panic state
    pub fee_state: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl PanicPause {
    pub const DISCRIMINATOR: [u8; 8] = [76, 164, 123, 25, 4, 43, 79, 165];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.pause_authority, true),
            AccountMeta::new(self.fee_state, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for PanicPause {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (global_fee_admin or pause_delegate_admin only) Pause the protocol. Auto-expires after 6
/// hours. Limited to 3 pauses per day and 4 consecutive pauses.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn panic_pause(accounts: &PanicPause) -> Instruction {
    panic_pause_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn panic_pause_with_program_id(program_id: Pubkey, accounts: &PanicPause) -> Instruction {
    let data = PanicPause::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`panic_unpause`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanicUnpause {
    /// Global fee admin only.
    pub global_fee_admin: Pubkey,
    pub fee_state: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl PanicUnpause {
    pub const DISCRIMINATOR: [u8; 8] = [236, 107, 194, 242, 99, 51, 121, 128];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.global_fee_admin, true),
            AccountMeta::new(self.fee_state, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for PanicUnpause {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (global_fee_admin only) Unpause the protocol before auto-expiry.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn panic_unpause(accounts: &PanicUnpause) -> Instruction {
    panic_unpause_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn panic_unpause_with_program_id(program_id: Pubkey, accounts: &PanicUnpause) -> Instruction {
    let data = PanicUnpause::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`panic_unpause_permissionless`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanicUnpausePermissionless {
    pub fee_state: Pubkey,
}
impl PanicUnpausePermissionless {
    pub const DISCRIMINATOR: [u8; 8] = [245, 139, 50, 159, 213, 62, 91, 248];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![AccountMeta::new(self.fee_state, false)]
    }
}
impl ToAccountMetas for PanicUnpausePermissionless {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Unpause the protocol when pause time has expired
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn panic_unpause_permissionless(accounts: &PanicUnpausePermissionless) -> Instruction {
    panic_unpause_permissionless_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn panic_unpause_permissionless_with_program_id(
    program_id: Pubkey,
    accounts: &PanicUnpausePermissionless,
) -> Instruction {
    let data = PanicUnpausePermissionless::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`propagate_fee_state`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropagateFeeState {
    pub fee_state: Pubkey,
    /// Any group, this ix is permisionless and can propagate the fee to any group
    pub marginfi_group: Pubkey,
}
impl PropagateFeeState {
    pub const DISCRIMINATOR: [u8; 8] = [64, 3, 166, 194, 129, 21, 101, 155];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.marginfi_group, false),
        ]
    }
}
impl ToAccountMetas for PropagateFeeState {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (Permissionless) Force any group to adopt the current FeeState settings
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn propagate_fee_state(accounts: &PropagateFeeState) -> Instruction {
    propagate_fee_state_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn propagate_fee_state_with_program_id(
    program_id: Pubkey,
    accounts: &PropagateFeeState,
) -> Instruction {
    let data = PropagateFeeState::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`propagate_staked_settings`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropagateStakedSettings {
    pub marginfi_group: Pubkey,
    pub staked_settings: Pubkey,
    pub bank: Pubkey,
}
impl PropagateStakedSettings {
    pub const DISCRIMINATOR: [u8; 8] = [210, 30, 152, 69, 130, 99, 222, 170];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new_readonly(self.staked_settings, false),
            AccountMeta::new(self.bank, false),
        ]
    }
}
impl ToAccountMetas for PropagateStakedSettings {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Propagate updated staked settings to a staked collateral bank.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn propagate_staked_settings(accounts: &PropagateStakedSettings) -> Instruction {
    propagate_staked_settings_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn propagate_staked_settings_with_program_id(
    program_id: Pubkey,
    accounts: &PropagateStakedSettings,
) -> Instruction {
    let data = PropagateStakedSettings::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`purge_deleverage_balance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurgeDeleverageBalance {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub risk_admin: Pubkey,
    pub bank: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl PurgeDeleverageBalance {
    pub const DISCRIMINATOR: [u8; 8] = [132, 187, 25, 149, 181, 59, 253, 136];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.risk_admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for PurgeDeleverageBalance {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (risk admin only) Purge a user's lending balance on a bank being sunset, without paying the
/// user anything. Only usable after all the debt has been settled on a bank in deleveraging
/// mode, i.e. `TOKENLESS_REPAYMENTS_ALLOWED` and `TOKENLESS_REPAYMENTS_COMPLETE`. Used to clear
/// abandoned lending positions in a now-worthless bank so it can be closed via
/// `lending_pool_close_bank`.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn purge_deleverage_balance(accounts: &PurgeDeleverageBalance) -> Instruction {
    purge_deleverage_balance_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn purge_deleverage_balance_with_program_id(
    program_id: Pubkey,
    accounts: &PurgeDeleverageBalance,
) -> Instruction {
    let data = PurgeDeleverageBalance::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`resize_global_fee_state`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResizeGlobalFeeState {
    /// Not an AccountLoader so an undersized fee state can still be resized under the future
    /// (larger-struct) program.
    pub fee_state: Pubkey,
    /// Funds the rent for the added account space.
    pub payer: Pubkey,
    pub system_program: Pubkey,
}
impl ResizeGlobalFeeState {
    pub const DISCRIMINATOR: [u8; 8] = [141, 111, 97, 79, 111, 143, 77, 159];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.fee_state, false),
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for ResizeGlobalFeeState {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Resize the fee-state account to the v2 layout size; `payer` funds the
/// added rent.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn resize_global_fee_state(accounts: &ResizeGlobalFeeState) -> Instruction {
    resize_global_fee_state_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn resize_global_fee_state_with_program_id(
    program_id: Pubkey,
    accounts: &ResizeGlobalFeeState,
) -> Instruction {
    let data = ResizeGlobalFeeState::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`super_admin_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuperAdminDeposit {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
    pub admin_token_account: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl SuperAdminDeposit {
    pub const DISCRIMINATOR: [u8; 8] = [241, 189, 199, 17, 207, 225, 64, 75];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.admin_token_account, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for SuperAdminDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (primary admin only) Deposit directly into a bank liquidity vault and raise
/// `asset_share_value` proportionally. No marginfi account is involved.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn super_admin_deposit(accounts: &SuperAdminDeposit, amount: u64) -> Instruction {
    super_admin_deposit_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn super_admin_deposit_with_program_id(
    program_id: Pubkey,
    accounts: &SuperAdminDeposit,
    amount: u64,
) -> Instruction {
    let mut data = SuperAdminDeposit::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`super_admin_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuperAdminWithdraw {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
    pub destination_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl SuperAdminWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = [202, 67, 85, 126, 104, 138, 79, 197];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for SuperAdminWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (primary admin only) Withdraw directly from a bank liquidity vault and lower
/// `asset_share_value` proportionally. No marginfi account is involved.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn super_admin_withdraw(accounts: &SuperAdminWithdraw, amount: u64) -> Instruction {
    super_admin_withdraw_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn super_admin_withdraw_with_program_id(
    program_id: Pubkey,
    accounts: &SuperAdminWithdraw,
    amount: u64,
) -> Instruction {
    let mut data = SuperAdminWithdraw::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`update_deleverage_withdrawals`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateDeleverageWithdrawals {
    pub marginfi_group: Pubkey,
    pub delegate_flow_admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl UpdateDeleverageWithdrawals {
    pub const DISCRIMINATOR: [u8; 8] = [56, 3, 181, 118, 27, 247, 207, 227];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.delegate_flow_admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for UpdateDeleverageWithdrawals {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (delegate_flow_admin only) Update the deleverage daily withdraw outflow with
/// aggregated data. The delegate flow admin aggregates
/// `DeleverageWithdrawFlowEvent` events off-chain and calls this instruction at intervals.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn update_deleverage_withdrawals(
    accounts: &UpdateDeleverageWithdrawals,
    outflow_usd: u32,
    update_seq: u64,
    event_start_slot: u64,
    event_end_slot: u64,
) -> Instruction {
    update_deleverage_withdrawals_with_program_id(
        crate::ID,
        accounts,
        outflow_usd,
        update_seq,
        event_start_slot,
        event_end_slot,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn update_deleverage_withdrawals_with_program_id(
    program_id: Pubkey,
    accounts: &UpdateDeleverageWithdrawals,
    outflow_usd: u32,
    update_seq: u64,
    event_start_slot: u64,
    event_end_slot: u64,
) -> Instruction {
    let mut data = UpdateDeleverageWithdrawals::DISCRIMINATOR.to_vec();
    outflow_usd
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    update_seq
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    event_start_slot
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    event_end_slot
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`update_group_rate_limiter`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateGroupRateLimiter {
    pub marginfi_group: Pubkey,
    pub delegate_flow_admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl UpdateGroupRateLimiter {
    pub const DISCRIMINATOR: [u8; 8] = [23, 78, 60, 139, 187, 44, 129, 37];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.delegate_flow_admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for UpdateGroupRateLimiter {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (delegate_flow_admin only) Update the group rate limiter with aggregated
/// inflow/outflow. The delegate flow admin aggregates
/// `RateLimitFlowEvent` events off-chain, converts to USD, and calls this instruction at
/// intervals to update group rate limiter state.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn update_group_rate_limiter(
    accounts: &UpdateGroupRateLimiter,
    outflow_usd: Option<u64>,
    inflow_usd: Option<u64>,
    update_seq: u64,
    event_start_slot: u64,
    event_end_slot: u64,
) -> Instruction {
    update_group_rate_limiter_with_program_id(
        crate::ID,
        accounts,
        outflow_usd,
        inflow_usd,
        update_seq,
        event_start_slot,
        event_end_slot,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn update_group_rate_limiter_with_program_id(
    program_id: Pubkey,
    accounts: &UpdateGroupRateLimiter,
    outflow_usd: Option<u64>,
    inflow_usd: Option<u64>,
    update_seq: u64,
    event_start_slot: u64,
    event_end_slot: u64,
) -> Instruction {
    let mut data = UpdateGroupRateLimiter::DISCRIMINATOR.to_vec();
    outflow_usd
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    inflow_usd
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    update_seq
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    event_start_slot
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    event_end_slot
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
