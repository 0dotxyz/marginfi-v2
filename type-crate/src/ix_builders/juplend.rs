//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::types::JuplendConfigCompact;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`juplend_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JuplendDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    /// Owned by authority, the source account for the token deposit.
    pub signer_token_account: Pubkey,
    /// The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).
    /// NOTE: JupLend marks the signer as writable in their deposit instruction.
    pub liquidity_vault_authority: Pubkey,
    /// Bank liquidity vault (holds underlying mint and is used as depositor_token_account).
    pub liquidity_vault: Pubkey,
    /// Underlying mint.
    pub mint: Pubkey,
    /// JupLend lending state account.
    pub integration_acc_1: Pubkey,
    /// JupLend fToken mint.
    pub f_token_mint: Pubkey,
    /// Bank's fToken vault (validated via has_one on bank).
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
    pub const DISCRIMINATOR: [u8; 8] = [114, 11, 218, 81, 183, 165, 143, 255];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for JuplendDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Deposit into a JupLend lending pool through a marginfi account.
/// * amount - in the underlying token (e.g., USDC), in native decimals
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn juplend_deposit(accounts: &JuplendDeposit, amount: u64) -> Instruction {
    juplend_deposit_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn juplend_deposit_with_program_id(
    program_id: Pubkey,
    accounts: &JuplendDeposit,
    amount: u64,
) -> Instruction {
    let mut data = JuplendDeposit::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`juplend_init_position`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JuplendInitPosition {
    /// Provides a nominal deposit amount.
    pub fee_payer: Pubkey,
    /// Token account owned by the fee payer holding the underlying mint.
    pub signer_token_account: Pubkey,
    pub bank: Pubkey,
    /// The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).
    pub liquidity_vault_authority: Pubkey,
    /// Bank liquidity vault (holds underlying mint and is used as depositor_token_account).
    pub liquidity_vault: Pubkey,
    /// Underlying mint (must match bank mint and JupLend lending state mint).
    pub mint: Pubkey,
    /// JupLend lending state account.
    pub integration_acc_1: Pubkey,
    /// JupLend fToken mint.
    pub f_token_mint: Pubkey,
    /// Bank's fToken vault (validated via has_one on bank).
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
    pub const DISCRIMINATOR: [u8; 8] = [176, 255, 151, 106, 5, 207, 74, 215];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for JuplendInitPosition {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Initialize the bank-level JupLend position.
///
/// This creates the bank's fToken ATA (owned by the bank liquidity vault authority) and
/// performs a nominal seed deposit into JupLend, then flips the bank from `Paused` to
/// `Operational`.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn juplend_init_position(accounts: &JuplendInitPosition, amount: u64) -> Instruction {
    juplend_init_position_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn juplend_init_position_with_program_id(
    program_id: Pubkey,
    accounts: &JuplendInitPosition,
    amount: u64,
) -> Instruction {
    let mut data = JuplendInitPosition::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`juplend_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JuplendWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    /// Token account that will receive the underlying withdrawal.
    /// WARN: Completely unchecked!
    pub destination_token_account: Pubkey,
    /// The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).
    /// NOTE: JupLend marks the signer as writable in their withdraw instruction.
    pub liquidity_vault_authority: Pubkey,
    /// Underlying mint.
    pub mint: Pubkey,
    /// JupLend lending state account.
    pub integration_acc_1: Pubkey,
    /// JupLend fToken mint.
    pub f_token_mint: Pubkey,
    /// Bank's fToken vault (validated via has_one on bank).
    pub integration_acc_2: Pubkey,
    /// Withdraw intermediary ATA (authority = liquidity_vault_authority).
    /// This must be an ATA to satisfy JupLend's withdraw constraints.
    pub integration_acc_3: Pubkey,
    pub lending_admin: Pubkey,
    pub supply_token_reserves_liquidity: Pubkey,
    pub lending_supply_position_on_liquidity: Pubkey,
    pub rate_model: Pubkey,
    pub vault: Pubkey,
    /// JupLend claim account for liquidity_vault_authority.
    /// TEMPORARY: Mainnet currently requires this account (passing None causes ConstraintMut errors),
    /// but an upcoming upgrade is expected to make it truly optional. The account is never actually
    /// validated or used - you can pass any mutable account. We create the canonical PDA for consistency.
    /// Seeds: ["user_claim", liquidity_vault_authority, mint] on Liquidity program.
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
    pub const DISCRIMINATOR: [u8; 8] = [245, 164, 253, 202, 53, 77, 251, 221];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for JuplendWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Withdraw from a JupLend lending pool through a marginfi account.
/// * amount - in the underlying token (e.g., USDC), in native decimals
/// * if group rate limits are enabled, include the withdrawn bank's oracle group in
/// `remaining_accounts`
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn juplend_withdraw(
    accounts: &JuplendWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    juplend_withdraw_with_program_id(crate::ID, accounts, amount, withdraw_all)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn juplend_withdraw_with_program_id(
    program_id: Pubkey,
    accounts: &JuplendWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    let mut data = JuplendWithdraw::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    withdraw_all
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_pool_add_bank_juplend`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankJuplend {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub fee_payer: Pubkey,
    /// Must match the mint used by the JupLend lending state.
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    /// JupLend lending state account that must match the bank mint.
    pub integration_acc_1: Pubkey,
    /// Will be authority of the bank's liquidity vault. Used as intermediary for deposits/withdraws.
    pub liquidity_vault_authority: Pubkey,
    /// For JupLend banks, the `liquidity_vault` is used as an intermediary when depositing/
    /// withdrawing, e.g., withdrawn funds move from JupLend -> here -> the user's token account.
    pub liquidity_vault: Pubkey,
    /// Note: Currently does nothing.
    pub insurance_vault_authority: Pubkey,
    /// Note: Currently does nothing.
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub f_token_mint: Pubkey,
    /// The bank's fToken vault holds the fTokens received when depositing into JupLend.
    ///
    pub integration_acc_2: Pubkey,
    /// Token program for both underlying mint and fToken mint (SPL Token or Token-2022).
    /// JupLend creates fToken mints using the same token program as the underlying.
    pub token_program: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolAddBankJuplend {
    pub const DISCRIMINATOR: [u8; 8] = [18, 208, 117, 90, 53, 111, 195, 41];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
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
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolAddBankJuplend {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (slow governance_admin) Add a JupLend bank to the marginfi group.
///
/// Remaining accounts (for oracle validation):
/// 0. underlying oracle feed (pyth push or switchboard pull)
/// 1. JupLend `Lending` state
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_juplend(
    accounts: &LendingPoolAddBankJuplend,
    bank_config: JuplendConfigCompact,
    bank_seed: u64,
) -> Instruction {
    lending_pool_add_bank_juplend_with_program_id(crate::ID, accounts, bank_config, bank_seed)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_juplend_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolAddBankJuplend,
    bank_config: JuplendConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankJuplend::DISCRIMINATOR.to_vec();
    bank_config
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    bank_seed
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
