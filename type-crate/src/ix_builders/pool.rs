use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::BankConfigOpt;
use crate::types::{
    BankConfigCompact, EmodeEntry, InterestRateConfigOpt, WrappedI80F48, MAX_EMODE_ENTRIES,
};
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`marginfi_group_initialize`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiGroupInitialize {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub fee_state: Pubkey,
    pub system_program: Pubkey,
}

impl MarginfiGroupInitialize {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_GROUP_INITIALIZE;
}

impl ToAccountMetas for MarginfiGroupInitialize {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, true),
            AccountMeta::new(self.admin, true),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (admin only) Initialize a new marginfi group. The signer becomes the group admin.
pub fn marginfi_group_initialize(accounts: &MarginfiGroupInitialize) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiGroupInitialize::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_pool_accrue_bank_interest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAccrueBankInterest {
    pub group: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolAccrueBankInterest {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_ACCRUE_BANK_INTEREST;
}

impl ToAccountMetas for LendingPoolAccrueBankInterest {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (permissionless) Accrue interest on a bank, updating share values and collecting fees.
pub fn lending_pool_accrue_bank_interest(accounts: &LendingPoolAccrueBankInterest) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingPoolAccrueBankInterest::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_pool_pulse_bank_price_cache`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolPulseBankPriceCache {
    pub group: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolPulseBankPriceCache {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_PULSE_BANK_PRICE_CACHE;
}

impl ToAccountMetas for LendingPoolPulseBankPriceCache {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (Permissionless) Refresh the cached oracle price for a bank.
///
/// `remaining_accounts` must hold the bank's oracle accounts, which
/// [`crate::pdas::bank_observation_keys`] derives from `oracle_setup`.
pub fn lending_pool_pulse_bank_price_cache(
    accounts: &LendingPoolPulseBankPriceCache,
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingPoolPulseBankPriceCache::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_pool_collect_bank_fees`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolCollectBankFees {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub fee_state: Pubkey,
    pub fee_ata: Pubkey,
    pub token_program: Pubkey,
}

impl LendingPoolCollectBankFees {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_COLLECT_BANK_FEES;
}

impl ToAccountMetas for LendingPoolCollectBankFees {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.fee_ata, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (permissionless) Transfer accrued fees from the liquidity vault to insurance/fee/program
/// vaults.
/// When the bank's mint is Token-2022, `remaining_accounts` must begin with that mint.
pub fn lending_pool_collect_bank_fees(accounts: &LendingPoolCollectBankFees) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingPoolCollectBankFees::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_pool_withdraw_fees`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolWithdrawFees {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub admin: Pubkey,
    pub fee_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub dst_token_account: Pubkey,
    pub token_program: Pubkey,
}

impl LendingPoolWithdrawFees {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_WITHDRAW_FEES;
}

impl ToAccountMetas for LendingPoolWithdrawFees {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.dst_token_account, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (admin only) Withdraw collected group fees from the fee vault.
///
/// For a Token-2022 bank, `remaining_accounts` must begin with the bank mint. Append any extra
/// accounts required by the mint's transfer-hook program after it, in transfer-hook resolution
/// order. Legacy SPL Token banks require neither.
pub fn lending_pool_withdraw_fees(accounts: &LendingPoolWithdrawFees, amount: u64) -> Instruction {
    let mut data = LendingPoolWithdrawFees::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_withdraw_fees_permissionless`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolWithdrawFeesPermissionless {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub fee_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fees_destination_account: Pubkey,
    pub token_program: Pubkey,
}

impl LendingPoolWithdrawFeesPermissionless {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_WITHDRAW_FEES_PERMISSIONLESS;
}

impl ToAccountMetas for LendingPoolWithdrawFeesPermissionless {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fees_destination_account, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (permissionless) Withdraw group fees to the pre-configured `fees_destination_account`.
///
/// For a Token-2022 bank, `remaining_accounts` must begin with the bank mint. Append any extra
/// accounts required by the mint's transfer-hook program after it, in transfer-hook resolution
/// order. Legacy SPL Token banks require neither.
pub fn lending_pool_withdraw_fees_permissionless(
    accounts: &LendingPoolWithdrawFeesPermissionless,
    amount: u64,
) -> Instruction {
    let mut data = LendingPoolWithdrawFeesPermissionless::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_withdraw_insurance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolWithdrawInsurance {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub admin: Pubkey,
    pub insurance_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub dst_token_account: Pubkey,
    pub token_program: Pubkey,
}

impl LendingPoolWithdrawInsurance {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_WITHDRAW_INSURANCE;
}

impl ToAccountMetas for LendingPoolWithdrawInsurance {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.dst_token_account, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (admin only) Withdraw from the insurance vault.
///
/// For a Token-2022 bank, `remaining_accounts` must begin with the bank mint. Append any extra
/// accounts required by the mint's transfer-hook program after it, in transfer-hook resolution
/// order. Legacy SPL Token banks require neither.
pub fn lending_pool_withdraw_insurance(
    accounts: &LendingPoolWithdrawInsurance,
    amount: u64,
) -> Instruction {
    let mut data = LendingPoolWithdrawInsurance::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_update_fees_destination_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolUpdateFeesDestinationAccount {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub admin: Pubkey,
    pub destination_account: Pubkey,
}

impl LendingPoolUpdateFeesDestinationAccount {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::LENDING_POOL_UPDATE_FEES_DESTINATION_ACCOUNT;
}

impl ToAccountMetas for LendingPoolUpdateFeesDestinationAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new_readonly(self.destination_account, false),
        ]
    }
}

/// (admin only) Set the destination wallet for permissionless fee withdrawals.
pub fn lending_pool_update_fees_destination_account(
    accounts: &LendingPoolUpdateFeesDestinationAccount,
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingPoolUpdateFeesDestinationAccount::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_pool_emissions_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolEmissionsDeposit {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub mint: Pubkey,
    pub emissions_funding_account: Pubkey,
    pub depositor: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}

impl LendingPoolEmissionsDeposit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_EMISSIONS_DEPOSIT;
}

impl ToAccountMetas for LendingPoolEmissionsDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.emissions_funding_account, false),
            AccountMeta::new(self.depositor, true),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (permissionless) Deposit same-bank emissions directly into liquidity vault and increase
/// depositors' value via `asset_share_value`.
pub fn lending_pool_emissions_deposit(
    accounts: &LendingPoolEmissionsDeposit,
    amount: u64,
) -> Instruction {
    let mut data = LendingPoolEmissionsDeposit::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_configure_bank`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBank {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolConfigureBank {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_CONFIGURE_BANK;
}

impl ToAccountMetas for LendingPoolConfigureBank {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (admin only) Configure bank parameters. If the bank has `FREEZE_SETTINGS`, only
/// deposit/borrow limits are updated and all other config changes are silently ignored.
pub fn lending_pool_configure_bank(
    accounts: &LendingPoolConfigureBank,
    bank_config_opt: BankConfigOpt,
) -> Instruction {
    let mut data = LendingPoolConfigureBank::DISCRIMINATOR.to_vec();
    bank_config_opt.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_configure_bank_oracle`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankOracle {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolConfigureBankOracle {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_CONFIGURE_BANK_ORACLE;
}

impl ToAccountMetas for LendingPoolConfigureBankOracle {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (admin only)
///
/// `remaining_accounts` must hold the oracle accounts the new `setup` validates against.
pub fn lending_pool_configure_bank_oracle(
    accounts: &LendingPoolConfigureBankOracle,
    setup: u8,
    oracle: Pubkey,
) -> Instruction {
    let mut data = LendingPoolConfigureBankOracle::DISCRIMINATOR.to_vec();
    setup.serialize(&mut data).unwrap();
    oracle.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_clear_circuit_breaker`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolClearCircuitBreaker {
    pub group: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolClearCircuitBreaker {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_CLEAR_CIRCUIT_BREAKER;
}

impl ToAccountMetas for LendingPoolClearCircuitBreaker {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (admin or risk_admin) Clear an active circuit-breaker halt on a bank.
/// * `reseed_reference` - If true, also zero the EMA reference so the next pulse reseeds it
///   from live oracle data (use when clearing because the new price level is valid and the
///   pre-halt reference would cause an immediate re-halt).
pub fn lending_pool_clear_circuit_breaker(
    accounts: &LendingPoolClearCircuitBreaker,
    reseed_reference: bool,
) -> Instruction {
    let mut data = LendingPoolClearCircuitBreaker::DISCRIMINATOR.to_vec();
    reseed_reference.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_handle_bankruptcy`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolHandleBankruptcy {
    pub group: Pubkey,
    pub signer: Pubkey,
    pub bank: Pubkey,
    pub marginfi_account: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub token_program: Pubkey,
}

impl LendingPoolHandleBankruptcy {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_HANDLE_BANKRUPTCY;
}

impl ToAccountMetas for LendingPoolHandleBankruptcy {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.signer, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (risk_admin or admin, unless `PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG` is set on the bank)
/// Handle bad debt of a bankrupt marginfi account for a given bank. Covers bad debt from the
/// insurance fund and socializes any remainder among depositors.
///
/// `remaining_accounts` is the Token-2022 mint when the bank uses one, then a bank and its
/// oracles for every active balance on the bankrupt account.
pub fn lending_pool_handle_bankruptcy(accounts: &LendingPoolHandleBankruptcy) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingPoolHandleBankruptcy::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`sync_indexer_flags`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncIndexerFlags {
    pub payer: Pubkey,
}

impl SyncIndexerFlags {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::SYNC_INDEXER_FLAGS;
}

impl ToAccountMetas for SyncIndexerFlags {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![AccountMeta::new(self.payer, true)]
    }
}

/// (Permissionless) Batch-sync balance-derived indexer flags for existing accounts.
/// Pass MarginfiAccounts as writable remaining_accounts.
pub fn sync_indexer_flags(accounts: &SyncIndexerFlags) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: SyncIndexerFlags::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`marginfi_group_configure`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiGroupConfigure {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
}

impl MarginfiGroupConfigure {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_GROUP_CONFIGURE;
}

impl ToAccountMetas for MarginfiGroupConfigure {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
        ]
    }
}

/// (admin only) Configure group admin keys and emode leverage caps. All admin keys must be
/// provided on every call. Emode leverage caps are set if provided, otherwise the existing
/// (non-zero) values are kept. Pass `Some(value)` to update, `None` to leave unchanged.
/// Same-asset emode leverage is disabled by configuring both init and maint leverage to `1`;
/// values below `1`, including `0`, are invalid.
///
/// Note: `new_emissions_admin` is deprecated and currently has no on-chain effect.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_group_configure(
    accounts: &MarginfiGroupConfigure,
    new_admin: Option<Pubkey>,
    new_emode_admin: Option<Pubkey>,
    new_curve_admin: Option<Pubkey>,
    new_limit_admin: Option<Pubkey>,
    new_flow_admin: Option<Pubkey>,
    new_emissions_admin: Option<Pubkey>,
    new_metadata_admin: Option<Pubkey>,
    new_risk_admin: Option<Pubkey>,
    emode_max_init_leverage: Option<WrappedI80F48>,
    emode_max_maint_leverage: Option<WrappedI80F48>,
    same_asset_emode_init_leverage: Option<WrappedI80F48>,
    same_asset_emode_maint_leverage: Option<WrappedI80F48>,
) -> Instruction {
    let mut data = MarginfiGroupConfigure::DISCRIMINATOR.to_vec();
    new_admin.serialize(&mut data).unwrap();
    new_emode_admin.serialize(&mut data).unwrap();
    new_curve_admin.serialize(&mut data).unwrap();
    new_limit_admin.serialize(&mut data).unwrap();
    new_flow_admin.serialize(&mut data).unwrap();
    new_emissions_admin.serialize(&mut data).unwrap();
    new_metadata_admin.serialize(&mut data).unwrap();
    new_risk_admin.serialize(&mut data).unwrap();
    emode_max_init_leverage.serialize(&mut data).unwrap();
    emode_max_maint_leverage.serialize(&mut data).unwrap();
    same_asset_emode_init_leverage.serialize(&mut data).unwrap();
    same_asset_emode_maint_leverage
        .serialize(&mut data)
        .unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_add_bank`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBank {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub fee_payer: Pubkey,
    pub fee_state: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl LendingPoolAddBank {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_ADD_BANK;
}

impl ToAccountMetas for LendingPoolAddBank {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, true),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (admin only) Add a new bank to the lending pool
pub fn lending_pool_add_bank(
    accounts: &LendingPoolAddBank,
    bank_config: BankConfigCompact,
) -> Instruction {
    let mut data = LendingPoolAddBank::DISCRIMINATOR.to_vec();
    bank_config.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_add_bank_with_seed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankWithSeed {
    pub marginfi_group: Pubkey,
    pub admin: Pubkey,
    pub fee_payer: Pubkey,
    pub fee_state: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl LendingPoolAddBankWithSeed {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_ADD_BANK_WITH_SEED;
}

impl ToAccountMetas for LendingPoolAddBankWithSeed {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (admin only) A copy of lending_pool_add_bank with an additional bank seed.
/// This seed is used to create a PDA for the bank's signature.
/// lending_pool_add_bank is preserved for backwards compatibility.
pub fn lending_pool_add_bank_with_seed(
    accounts: &LendingPoolAddBankWithSeed,
    bank_config: BankConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankWithSeed::DISCRIMINATOR.to_vec();
    bank_config.serialize(&mut data).unwrap();
    bank_seed.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_backfill_bank_is_t22_flag`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolBackfillBankIsT22Flag {
    pub bank: Pubkey,
    pub group: Pubkey,
    pub mint: Pubkey,
}

impl LendingPoolBackfillBankIsT22Flag {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_BACKFILL_BANK_IS_T22_FLAG;
}

impl ToAccountMetas for LendingPoolBackfillBankIsT22Flag {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.mint, false),
        ]
    }
}

/// (permissionless) Backfill `IS_T22` on existing banks created before this flag existed.
/// Also optionally backfills `bank_seed` in the same call.
/// Pass `None` to skip seed backfill, `Some(seed)` to backfill (including `Some(0)`).
pub fn lending_pool_backfill_bank_is_t22_flag(
    accounts: &LendingPoolBackfillBankIsT22Flag,
    bank_seed: Option<u64>,
) -> Instruction {
    let mut data = LendingPoolBackfillBankIsT22Flag::DISCRIMINATOR.to_vec();
    bank_seed.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_clone_emode`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolCloneEmode {
    pub group: Pubkey,
    pub signer: Pubkey,
    pub copy_from_bank: Pubkey,
    pub copy_to_bank: Pubkey,
}

impl LendingPoolCloneEmode {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_CLONE_EMODE;
}

impl ToAccountMetas for LendingPoolCloneEmode {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.signer, true),
            AccountMeta::new_readonly(self.copy_from_bank, false),
            AccountMeta::new(self.copy_to_bank, false),
        ]
    }
}

/// (admin or emode_admin) Copies emode settings from one bank to another. Useful when applying
/// emode settings from e.g. one LST to another.
pub fn lending_pool_clone_emode(accounts: &LendingPoolCloneEmode) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingPoolCloneEmode::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_pool_configure_bank_emode`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankEmode {
    pub group: Pubkey,
    pub emode_admin: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolConfigureBankEmode {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_CONFIGURE_BANK_EMODE;
}

impl ToAccountMetas for LendingPoolConfigureBankEmode {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.emode_admin, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (emode_admin only)
pub fn lending_pool_configure_bank_emode(
    accounts: &LendingPoolConfigureBankEmode,
    emode_tag: u16,
    entries: [EmodeEntry; MAX_EMODE_ENTRIES],
) -> Instruction {
    let mut data = LendingPoolConfigureBankEmode::DISCRIMINATOR.to_vec();
    emode_tag.serialize(&mut data).unwrap();
    entries.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_configure_bank_interest_only`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankInterestOnly {
    pub group: Pubkey,
    pub delegate_curve_admin: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolConfigureBankInterestOnly {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_CONFIGURE_BANK_INTEREST_ONLY;
}

impl ToAccountMetas for LendingPoolConfigureBankInterestOnly {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.delegate_curve_admin, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (delegate_curve_admin only) Update interest rate config. Does nothing if bank has
/// `FREEZE_SETTINGS`.
pub fn lending_pool_configure_bank_interest_only(
    accounts: &LendingPoolConfigureBankInterestOnly,
    interest_rate_config: InterestRateConfigOpt,
) -> Instruction {
    let mut data = LendingPoolConfigureBankInterestOnly::DISCRIMINATOR.to_vec();
    interest_rate_config.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_configure_bank_limits_only`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolConfigureBankLimitsOnly {
    pub group: Pubkey,
    pub delegate_limit_admin: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolConfigureBankLimitsOnly {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_CONFIGURE_BANK_LIMITS_ONLY;
}

impl ToAccountMetas for LendingPoolConfigureBankLimitsOnly {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.delegate_limit_admin, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (delegate_limit_admin only) Update deposit/borrow/init limits only.
pub fn lending_pool_configure_bank_limits_only(
    accounts: &LendingPoolConfigureBankLimitsOnly,
    deposit_limit: Option<u64>,
    borrow_limit: Option<u64>,
    total_asset_value_init_limit: Option<u64>,
) -> Instruction {
    let mut data = LendingPoolConfigureBankLimitsOnly::DISCRIMINATOR.to_vec();
    deposit_limit.serialize(&mut data).unwrap();
    borrow_limit.serialize(&mut data).unwrap();
    total_asset_value_init_limit.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_init_same_asset_emode_registry`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolInitSameAssetEmodeRegistry {
    pub group: Pubkey,
    pub signer: Pubkey,
    pub same_asset_emode_registry: Pubkey,
    pub system_program: Pubkey,
}

impl LendingPoolInitSameAssetEmodeRegistry {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::LENDING_POOL_INIT_SAME_ASSET_EMODE_REGISTRY;
}

impl ToAccountMetas for LendingPoolInitSameAssetEmodeRegistry {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.signer, true),
            AccountMeta::new(self.same_asset_emode_registry, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (admin or emode_admin only) Initialize the per-group same-asset e-mode registry.
pub fn lending_pool_init_same_asset_emode_registry(
    accounts: &LendingPoolInitSameAssetEmodeRegistry,
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingPoolInitSameAssetEmodeRegistry::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_pool_resize_group_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolResizeGroupAccount {
    pub group: Pubkey,
    pub payer: Pubkey,
    pub system_program: Pubkey,
}

impl LendingPoolResizeGroupAccount {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_RESIZE_GROUP_ACCOUNT;
}

impl ToAccountMetas for LendingPoolResizeGroupAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (permissionless) Resize the group account to the v2 layout size; `payer` funds the
/// added rent.
pub fn lending_pool_resize_group_account(accounts: &LendingPoolResizeGroupAccount) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingPoolResizeGroupAccount::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_pool_set_bank_same_asset_emode_eligibility`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolSetBankSameAssetEmodeEligibility {
    pub group: Pubkey,
    pub signer: Pubkey,
    pub bank: Pubkey,
    pub same_asset_emode_registry: Pubkey,
}

impl LendingPoolSetBankSameAssetEmodeEligibility {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::LENDING_POOL_SET_BANK_SAME_ASSET_EMODE_ELIGIBILITY;
}

impl ToAccountMetas for LendingPoolSetBankSameAssetEmodeEligibility {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.signer, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.same_asset_emode_registry, false),
        ]
    }
}

/// (admin or emode_admin only) Opt a bank in/out of same-asset e-mode participation.
pub fn lending_pool_set_bank_same_asset_emode_eligibility(
    accounts: &LendingPoolSetBankSameAssetEmodeEligibility,
    enabled: bool,
) -> Instruction {
    let mut data = LendingPoolSetBankSameAssetEmodeEligibility::DISCRIMINATOR.to_vec();
    enabled.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_set_fixed_oracle_price`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolSetFixedOraclePrice {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub bank: Pubkey,
}

impl LendingPoolSetFixedOraclePrice {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_SET_FIXED_ORACLE_PRICE;
}

impl ToAccountMetas for LendingPoolSetFixedOraclePrice {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (admin only)
///
/// This switches the bank to its fixed-price oracle setup. `remaining_accounts` must hold the
/// oracle accounts the resulting setup validates against; an integration bank keeps its own.
pub fn lending_pool_set_fixed_oracle_price(
    accounts: &LendingPoolSetFixedOraclePrice,
    price: WrappedI80F48,
) -> Instruction {
    let mut data = LendingPoolSetFixedOraclePrice::DISCRIMINATOR.to_vec();
    price.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`init_bank_metadata`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitBankMetadata {
    pub bank: Pubkey,
    pub fee_payer: Pubkey,
    pub metadata: Pubkey,
    pub system_program: Pubkey,
}

impl InitBankMetadata {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::INIT_BANK_METADATA;
}

impl ToAccountMetas for InitBankMetadata {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new(self.metadata, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (permissionless) pay the rent to open metadata for a bank. The bank account does not have
/// to exist yet — callers can pre-create metadata for an upcoming bank pubkey at their own
/// rent expense. When the bank is initialized and its seed is on-chain, the PDA is verified.
pub fn init_bank_metadata(accounts: &InitBankMetadata) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: InitBankMetadata::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`write_bank_metadata`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteBankMetadata {
    pub group: Pubkey,
    pub bank: Pubkey,
    pub metadata_admin: Pubkey,
    pub metadata: Pubkey,
}

impl WriteBankMetadata {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::WRITE_BANK_METADATA;
}

impl ToAccountMetas for WriteBankMetadata {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.metadata_admin, true),
            AccountMeta::new(self.metadata, false),
        ]
    }
}

/// (metadata admin only) Write ticker/description for an initialized bank. The bank account
/// must exist; when its seed is on-chain, the canonical PDA is verified.
pub fn write_bank_metadata(
    accounts: &WriteBankMetadata,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Instruction {
    let mut data = WriteBankMetadata::DISCRIMINATOR.to_vec();
    ticker.serialize(&mut data).unwrap();
    description.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`write_bank_metadata_pre_init`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteBankMetadataPreInit {
    pub group: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub metadata_admin: Pubkey,
    pub metadata: Pubkey,
}

impl WriteBankMetadataPreInit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::WRITE_BANK_METADATA_PRE_INIT;
}

impl ToAccountMetas for WriteBankMetadataPreInit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.metadata_admin, true),
            AccountMeta::new(self.metadata, false),
        ]
    }
}

/// (metadata admin only) Write ticker/description before bank initialization, for canonical
/// seeded banks only.
pub fn write_bank_metadata_pre_init(
    accounts: &WriteBankMetadataPreInit,
    bank_seed: u64,
    ticker: Option<Vec<u8>>,
    description: Option<Vec<u8>>,
) -> Instruction {
    let mut data = WriteBankMetadataPreInit::DISCRIMINATOR.to_vec();
    bank_seed.serialize(&mut data).unwrap();
    ticker.serialize(&mut data).unwrap();
    description.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}
