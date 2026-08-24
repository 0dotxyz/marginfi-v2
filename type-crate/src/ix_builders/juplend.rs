use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::JuplendConfigCompact;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`juplend_init_position`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JuplendInitPosition {
    pub fee_payer: Pubkey,
    pub signer_token_account: Pubkey,
    pub bank: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub mint: Pubkey,
    pub integration_acc_1: Pubkey,
    pub f_token_mint: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_admin: Pubkey,
    pub supply_token_reserves_liquidity: Pubkey,
    pub lending_supply_position_on_liquidity: Pubkey,
    pub rate_model: Pubkey,
    pub vault: Pubkey,
    pub liquidity: Pubkey,
    pub liquidity_program: Pubkey,
    pub rewards_rate_model: Pubkey,
    pub juplend_program: Pubkey,
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
}

impl JuplendInitPosition {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::JUPLEND_INIT_POSITION;
}

impl ToAccountMetas for JuplendInitPosition {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.f_token_mint, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.lending_admin, false),
            AccountMeta::new(self.supply_token_reserves_liquidity, false),
            AccountMeta::new(self.lending_supply_position_on_liquidity, false),
            AccountMeta::new_readonly(self.rate_model, false),
            AccountMeta::new(self.vault, false),
            AccountMeta::new(self.liquidity, false),
            AccountMeta::new_readonly(self.liquidity_program, false),
            AccountMeta::new_readonly(self.rewards_rate_model, false),
            AccountMeta::new_readonly(self.juplend_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.associated_token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (permissionless) Initialize the bank-level JupLend position.
///
/// This creates the bank's fToken ATA (owned by the bank liquidity vault authority) and
/// performs a nominal seed deposit into JupLend, then flips the bank from `Paused` to
/// `Operational`.
pub fn juplend_init_position(accounts: &JuplendInitPosition, amount: u64) -> Instruction {
    let mut data = JuplendInitPosition::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`juplend_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JuplendDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub signer_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub mint: Pubkey,
    pub integration_acc_1: Pubkey,
    pub f_token_mint: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_admin: Pubkey,
    pub supply_token_reserves_liquidity: Pubkey,
    pub lending_supply_position_on_liquidity: Pubkey,
    pub rate_model: Pubkey,
    pub vault: Pubkey,
    pub liquidity: Pubkey,
    pub liquidity_program: Pubkey,
    pub rewards_rate_model: Pubkey,
    pub juplend_program: Pubkey,
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
}

impl JuplendDeposit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::JUPLEND_DEPOSIT;
}

impl ToAccountMetas for JuplendDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.f_token_mint, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.lending_admin, false),
            AccountMeta::new(self.supply_token_reserves_liquidity, false),
            AccountMeta::new(self.lending_supply_position_on_liquidity, false),
            AccountMeta::new_readonly(self.rate_model, false),
            AccountMeta::new(self.vault, false),
            AccountMeta::new(self.liquidity, false),
            AccountMeta::new_readonly(self.liquidity_program, false),
            AccountMeta::new_readonly(self.rewards_rate_model, false),
            AccountMeta::new_readonly(self.juplend_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.associated_token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (user) Deposit into a JupLend lending pool through a marginfi account.
/// * amount - in the underlying token (e.g., USDC), in native decimals
pub fn juplend_deposit(accounts: &JuplendDeposit, amount: u64) -> Instruction {
    let mut data = JuplendDeposit::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`juplend_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JuplendWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub destination_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub mint: Pubkey,
    pub integration_acc_1: Pubkey,
    pub f_token_mint: Pubkey,
    pub integration_acc_2: Pubkey,
    pub integration_acc_3: Pubkey,
    pub lending_admin: Pubkey,
    pub supply_token_reserves_liquidity: Pubkey,
    pub lending_supply_position_on_liquidity: Pubkey,
    pub rate_model: Pubkey,
    pub vault: Pubkey,
    pub claim_account: Pubkey,
    pub liquidity: Pubkey,
    pub liquidity_program: Pubkey,
    pub rewards_rate_model: Pubkey,
    pub juplend_program: Pubkey,
    pub token_program: Pubkey,
    pub associated_token_program: Pubkey,
    pub system_program: Pubkey,
}

impl JuplendWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::JUPLEND_WITHDRAW;
}

impl ToAccountMetas for JuplendWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.f_token_mint, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new(self.integration_acc_3, false),
            AccountMeta::new_readonly(self.lending_admin, false),
            AccountMeta::new(self.supply_token_reserves_liquidity, false),
            AccountMeta::new(self.lending_supply_position_on_liquidity, false),
            AccountMeta::new_readonly(self.rate_model, false),
            AccountMeta::new(self.vault, false),
            AccountMeta::new(self.claim_account, false),
            AccountMeta::new(self.liquidity, false),
            AccountMeta::new_readonly(self.liquidity_program, false),
            AccountMeta::new_readonly(self.rewards_rate_model, false),
            AccountMeta::new_readonly(self.juplend_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.associated_token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (user) Withdraw from a JupLend lending pool through a marginfi account.
/// * amount - in the underlying token (e.g., USDC), in native decimals
/// * `remaining_accounts` must hold a bank and its oracles for every active balance, plus
///   the withdrawn bank's oracle group when group rate limits are enabled.
pub fn juplend_withdraw(
    accounts: &JuplendWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    let mut data = JuplendWithdraw::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    withdraw_all.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_add_bank_juplend`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankJuplend {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub fee_payer: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub integration_acc_1: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub f_token_mint: Pubkey,
    pub integration_acc_2: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl LendingPoolAddBankJuplend {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_ADD_BANK_JUPLEND;
}

impl ToAccountMetas for LendingPoolAddBankJuplend {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.integration_acc_1, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.f_token_mint, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (admin) Add a JupLend bank to the marginfi group.
///
/// `remaining_accounts` must contain the underlying oracle feed (Pyth Push or Switchboard Pull)
/// followed by `integration_acc_1`, the JupLend `Lending` state.
pub fn lending_pool_add_bank_juplend(
    accounts: &LendingPoolAddBankJuplend,
    bank_config: JuplendConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankJuplend::DISCRIMINATOR.to_vec();
    bank_config.serialize(&mut data).unwrap();
    bank_seed.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}
