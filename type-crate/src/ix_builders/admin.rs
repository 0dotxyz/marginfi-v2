use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::{StakedSettingsConfig, StakedSettingsEditConfig, WrappedI80F48};
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`init_global_fee_state`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitGlobalFeeState {
    pub payer: Pubkey,
    pub fee_state: Pubkey,
    pub system_program: Pubkey,
}

impl InitGlobalFeeState {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::INIT_GLOBAL_FEE_STATE;
}

impl ToAccountMetas for InitGlobalFeeState {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.payer, true),
            AccountMeta::new(self.fee_state, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (Runs once per program) Configures the fee state account, where the global admin sets fees
/// that are assessed to the protocol
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
    let mut data = InitGlobalFeeState::DISCRIMINATOR.to_vec();
    admin.serialize(&mut data).unwrap();
    fee_wallet.serialize(&mut data).unwrap();
    bank_init_flat_sol_fee.serialize(&mut data).unwrap();
    liquidation_flat_sol_fee.serialize(&mut data).unwrap();
    order_init_flat_sol_fee.serialize(&mut data).unwrap();
    program_fee_fixed.serialize(&mut data).unwrap();
    program_fee_rate.serialize(&mut data).unwrap();
    liquidation_max_fee.serialize(&mut data).unwrap();
    order_execution_max_fee.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`edit_global_fee_state`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditGlobalFeeState {
    pub global_fee_admin: Pubkey,
    pub fee_state: Pubkey,
}

impl EditGlobalFeeState {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::EDIT_GLOBAL_FEE_STATE;
}

impl ToAccountMetas for EditGlobalFeeState {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.global_fee_admin, true),
            AccountMeta::new(self.fee_state, false),
        ]
    }
}

/// (global fee admin only) Adjust fees, admin, wallet, or pause delegate admin
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
    let mut data = EditGlobalFeeState::DISCRIMINATOR.to_vec();
    admin.serialize(&mut data).unwrap();
    fee_wallet.serialize(&mut data).unwrap();
    bank_init_flat_sol_fee.serialize(&mut data).unwrap();
    liquidation_flat_sol_fee.serialize(&mut data).unwrap();
    order_init_flat_sol_fee.serialize(&mut data).unwrap();
    program_fee_fixed.serialize(&mut data).unwrap();
    program_fee_rate.serialize(&mut data).unwrap();
    liquidation_max_fee.serialize(&mut data).unwrap();
    order_execution_max_fee.serialize(&mut data).unwrap();
    pause_delegate_admin.serialize(&mut data).unwrap();
    account_transfer_fee.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`resize_global_fee_state`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResizeGlobalFeeState {
    pub fee_state: Pubkey,
    pub payer: Pubkey,
    pub system_program: Pubkey,
}

impl ResizeGlobalFeeState {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::RESIZE_GLOBAL_FEE_STATE;
}

impl ToAccountMetas for ResizeGlobalFeeState {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.fee_state, false),
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (permissionless) Resize the fee-state account to the v2 layout size; `payer` funds the
/// added rent.
pub fn resize_global_fee_state(accounts: &ResizeGlobalFeeState) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: ResizeGlobalFeeState::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`propagate_fee_state`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropagateFeeState {
    pub fee_state: Pubkey,
    pub marginfi_group: Pubkey,
}

impl PropagateFeeState {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::PROPAGATE_FEE_STATE;
}

impl ToAccountMetas for PropagateFeeState {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.marginfi_group, false),
        ]
    }
}

/// (Permissionless) Force any group to adopt the current FeeState settings
pub fn propagate_fee_state(accounts: &PropagateFeeState) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: PropagateFeeState::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`init_staked_settings`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitStakedSettings {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub fee_payer: Pubkey,
    pub staked_settings: Pubkey,
    pub system_program: Pubkey,
}

impl InitStakedSettings {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::INIT_STAKED_SETTINGS;
}

impl ToAccountMetas for InitStakedSettings {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new(self.staked_settings, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (group admin only) Init the Staked Settings account, which is used to create staked
/// collateral banks, and must run before any staked collateral bank can be created with
/// `add_pool_permissionless`. Running this ix effectively opts the group into the staked
/// collateral feature.
pub fn init_staked_settings(
    accounts: &InitStakedSettings,
    settings: StakedSettingsConfig,
) -> Instruction {
    let mut data = InitStakedSettings::DISCRIMINATOR.to_vec();
    settings.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`propagate_staked_settings`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropagateStakedSettings {
    pub marginfi_group: Pubkey,
    pub staked_settings: Pubkey,
    pub bank: Pubkey,
}

impl PropagateStakedSettings {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::PROPAGATE_STAKED_SETTINGS;
}

impl ToAccountMetas for PropagateStakedSettings {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new_readonly(self.staked_settings, false),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (permissionless) Propagate updated staked settings to a staked collateral bank.
///
/// `remaining_accounts` must hold the settings' oracle account; propagation after an oracle
/// change fails without it.
pub fn propagate_staked_settings(accounts: &PropagateStakedSettings) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: PropagateStakedSettings::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`enable_staked_oracle_onramp`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnableStakedOracleOnramp {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub staked_settings: Pubkey,
}

impl EnableStakedOracleOnramp {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::ENABLE_STAKED_ORACLE_ONRAMP;
}

impl ToAccountMetas for EnableStakedOracleOnramp {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.staked_settings, false),
        ]
    }
}

/// (admin only) Enable SPL single-pool on-ramp lamports in staked-collateral oracle pricing.
/// To be removed once SVSP update is rolled out (likely in 1.10)
/// This flips a per-group config flag so that every staked oracle uses the canonical single-pool NAV
/// formula.
pub fn enable_staked_oracle_onramp(accounts: &EnableStakedOracleOnramp) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: EnableStakedOracleOnramp::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`disable_staked_oracles`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisableStakedOracles {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub staked_settings: Pubkey,
}

impl DisableStakedOracles {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::DISABLE_STAKED_ORACLES;
}

impl ToAccountMetas for DisableStakedOracles {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.staked_settings, false),
        ]
    }
}

/// (admin only) Disable stake pricing, i.e. effectively forbidding all operations involving stake banks.
/// To be used during the rollout of the SVSP upgrade.
/// To be removed once SVSP update is rolled out (likely in 1.10)
pub fn disable_staked_oracles(accounts: &DisableStakedOracles) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: DisableStakedOracles::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`panic_pause`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanicPause {
    pub pause_authority: Pubkey,
    pub fee_state: Pubkey,
}

impl PanicPause {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::PANIC_PAUSE;
}

impl ToAccountMetas for PanicPause {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.pause_authority, true),
            AccountMeta::new(self.fee_state, false),
        ]
    }
}

/// (global_fee_admin or pause_delegate_admin only) Pause the protocol. Auto-expires after 6
/// hours. Limited to 3 pauses per day and 4 consecutive pauses.
pub fn panic_pause(accounts: &PanicPause) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: PanicPause::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`panic_unpause`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanicUnpause {
    pub global_fee_admin: Pubkey,
    pub fee_state: Pubkey,
}

impl PanicUnpause {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::PANIC_UNPAUSE;
}

impl ToAccountMetas for PanicUnpause {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.global_fee_admin, true),
            AccountMeta::new(self.fee_state, false),
        ]
    }
}

/// (global_fee_admin only) Unpause the protocol before auto-expiry.
pub fn panic_unpause(accounts: &PanicUnpause) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: PanicUnpause::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`panic_unpause_permissionless`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanicUnpausePermissionless {
    pub fee_state: Pubkey,
}

impl PanicUnpausePermissionless {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::PANIC_UNPAUSE_PERMISSIONLESS;
}

impl ToAccountMetas for PanicUnpausePermissionless {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![AccountMeta::new(self.fee_state, false)]
    }
}

/// (permissionless) Unpause the protocol when pause time has expired
pub fn panic_unpause_permissionless(accounts: &PanicUnpausePermissionless) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: PanicUnpausePermissionless::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`super_admin_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuperAdminDeposit {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
    pub admin_token_account: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}

impl SuperAdminDeposit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::SUPER_ADMIN_DEPOSIT;
}

impl ToAccountMetas for SuperAdminDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.admin_token_account, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (primary admin only) Deposit directly into a bank liquidity vault and raise
/// `asset_share_value` proportionally. No marginfi account is involved.
/// When the bank's mint is Token-2022, `remaining_accounts` must begin with that mint.
pub fn super_admin_deposit(accounts: &SuperAdminDeposit, amount: u64) -> Instruction {
    let mut data = SuperAdminDeposit::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`super_admin_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuperAdminWithdraw {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
    pub destination_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}

impl SuperAdminWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::SUPER_ADMIN_WITHDRAW;
}

impl ToAccountMetas for SuperAdminWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (primary admin only) Withdraw directly from a bank liquidity vault and lower
/// `asset_share_value` proportionally. No marginfi account is involved.
/// When the bank's mint is Token-2022, `remaining_accounts` must begin with that mint.
pub fn super_admin_withdraw(accounts: &SuperAdminWithdraw, amount: u64) -> Instruction {
    let mut data = SuperAdminWithdraw::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`configure_deleverage_withdrawal_limit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureDeleverageWithdrawalLimit {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
}

impl ConfigureDeleverageWithdrawalLimit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::CONFIGURE_DELEVERAGE_WITHDRAWAL_LIMIT;
}

impl ToAccountMetas for ConfigureDeleverageWithdrawalLimit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
        ]
    }
}

/// (admin or delegate_limit_admin) Set the daily withdrawal limit for deleverages per group.
pub fn configure_deleverage_withdrawal_limit(
    accounts: &ConfigureDeleverageWithdrawalLimit,
    limit: u32,
) -> Instruction {
    let mut data = ConfigureDeleverageWithdrawalLimit::DISCRIMINATOR.to_vec();
    limit.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`update_deleverage_withdrawals`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateDeleverageWithdrawals {
    pub marginfi_group: Pubkey,
    pub delegate_flow_admin: Pubkey,
}

impl UpdateDeleverageWithdrawals {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::UPDATE_DELEVERAGE_WITHDRAWALS;
}

impl ToAccountMetas for UpdateDeleverageWithdrawals {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.delegate_flow_admin, true),
        ]
    }
}

/// (delegate_flow_admin only) Update the deleverage daily withdraw outflow with
/// aggregated data. The delegate flow admin aggregates
/// `DeleverageWithdrawFlowEvent` events off-chain and calls this instruction at intervals.
pub fn update_deleverage_withdrawals(
    accounts: &UpdateDeleverageWithdrawals,
    outflow_usd: u32,
    update_seq: u64,
    event_start_slot: u64,
    event_end_slot: u64,
) -> Instruction {
    let mut data = UpdateDeleverageWithdrawals::DISCRIMINATOR.to_vec();
    outflow_usd.serialize(&mut data).unwrap();
    update_seq.serialize(&mut data).unwrap();
    event_start_slot.serialize(&mut data).unwrap();
    event_end_slot.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`edit_staked_settings`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditStakedSettings {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub staked_settings: Pubkey,
}

impl EditStakedSettings {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::EDIT_STAKED_SETTINGS;
}

impl ToAccountMetas for EditStakedSettings {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.staked_settings, false),
        ]
    }
}

/// (admin only) Edit the staked collateral settings for the group.
pub fn edit_staked_settings(
    accounts: &EditStakedSettings,
    settings: StakedSettingsEditConfig,
) -> Instruction {
    let mut data = EditStakedSettings::DISCRIMINATOR.to_vec();
    settings.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`configure_bank_rate_limits`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureBankRateLimits {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
}

impl ConfigureBankRateLimits {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::CONFIGURE_BANK_RATE_LIMITS;
}

impl ToAccountMetas for ConfigureBankRateLimits {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (admin or delegate_limit_admin) Configure bank-level rate limits for withdraw/borrow.
/// Rate limits track net outflow in native tokens. Deposits offset withdraws.
pub fn configure_bank_rate_limits(
    accounts: &ConfigureBankRateLimits,
    hourly_max_outflow: Option<u64>,
    daily_max_outflow: Option<u64>,
) -> Instruction {
    let mut data = ConfigureBankRateLimits::DISCRIMINATOR.to_vec();
    hourly_max_outflow.serialize(&mut data).unwrap();
    daily_max_outflow.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`configure_group_rate_limits`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureGroupRateLimits {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
}

impl ConfigureGroupRateLimits {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::CONFIGURE_GROUP_RATE_LIMITS;
}

impl ToAccountMetas for ConfigureGroupRateLimits {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
        ]
    }
}

/// (admin or delegate_limit_admin) Configure group-level rate limits for withdraw/borrow.
/// Rate limits track aggregate net outflow in USD.
pub fn configure_group_rate_limits(
    accounts: &ConfigureGroupRateLimits,
    hourly_max_outflow_usd: Option<u64>,
    daily_max_outflow_usd: Option<u64>,
) -> Instruction {
    let mut data = ConfigureGroupRateLimits::DISCRIMINATOR.to_vec();
    hourly_max_outflow_usd.serialize(&mut data).unwrap();
    daily_max_outflow_usd.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`update_group_rate_limiter`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateGroupRateLimiter {
    pub marginfi_group: Pubkey,
    pub delegate_flow_admin: Pubkey,
}

impl UpdateGroupRateLimiter {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::UPDATE_GROUP_RATE_LIMITER;
}

impl ToAccountMetas for UpdateGroupRateLimiter {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.delegate_flow_admin, true),
        ]
    }
}

/// (delegate_flow_admin only) Update the group rate limiter with aggregated
/// inflow/outflow. The delegate flow admin aggregates
pub fn update_group_rate_limiter(
    accounts: &UpdateGroupRateLimiter,
    outflow_usd: Option<u64>,
    inflow_usd: Option<u64>,
    update_seq: u64,
    event_start_slot: u64,
    event_end_slot: u64,
) -> Instruction {
    let mut data = UpdateGroupRateLimiter::DISCRIMINATOR.to_vec();
    outflow_usd.serialize(&mut data).unwrap();
    inflow_usd.serialize(&mut data).unwrap();
    update_seq.serialize(&mut data).unwrap();
    event_start_slot.serialize(&mut data).unwrap();
    event_end_slot.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}
