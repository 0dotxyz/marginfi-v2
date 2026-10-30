//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::types::SolendConfigCompact;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`lending_pool_add_bank_solend`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankSolend {
    pub group: Pubkey,
    pub governance_admin: Pubkey,
    pub fee_payer: Pubkey,
    /// Must match the mint used by `integration_acc_1`, Solend calls this the `liquidity.mint_pubkey`
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    /// Solend reserve account that must match the bank mint
    pub integration_acc_1: Pubkey,
    /// Obligation PDA for this bank in Solend
    /// Will be initialized and transferred to Solend in init_obligation instruction
    pub integration_acc_2: Pubkey,
    /// Will be authority of the bank's liquidity vault. Used as intermediary for deposits/withdraws
    pub liquidity_vault_authority: Pubkey,
    /// For Solend banks, the `liquidity_vault` never holds assets, but is instead used as an
    /// intermediary when depositing/withdrawing, e.g., withdrawn funds move from Solend -> here ->
    /// the user's token account.
    pub liquidity_vault: Pubkey,
    /// Note: Currently does nothing.
    pub insurance_vault_authority: Pubkey,
    /// Note: Currently does nothing.
    pub insurance_vault: Pubkey,
    /// Note: Currently does nothing.
    pub fee_vault_authority: Pubkey,
    /// Note: Currently does nothing.
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl LendingPoolAddBankSolend {
    pub const DISCRIMINATOR: [u8; 8] = [81, 233, 203, 199, 47, 226, 0, 68];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.governance_admin, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank_mint, false),
            AccountMeta::new(self.bank, false),
            AccountMeta::new_readonly(self.integration_acc_1, false),
            AccountMeta::new_readonly(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.insurance_vault_authority, false),
            AccountMeta::new(self.insurance_vault, false),
            AccountMeta::new_readonly(self.fee_vault_authority, false),
            AccountMeta::new(self.fee_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.system_program, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingPoolAddBankSolend {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (slow governance_admin) Add a Solend bank to the marginfi group
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_solend(
    accounts: &LendingPoolAddBankSolend,
    bank_config: SolendConfigCompact,
    bank_seed: u64,
) -> Instruction {
    lending_pool_add_bank_solend_with_program_id(crate::ID, accounts, bank_config, bank_seed)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_pool_add_bank_solend_with_program_id(
    program_id: Pubkey,
    accounts: &LendingPoolAddBankSolend,
    bank_config: SolendConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankSolend::DISCRIMINATOR.to_vec();
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

/// Fixed accounts for [`solend_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolendDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    /// Owned by authority, the source account for the token deposit.
    pub signer_token_account: Pubkey,
    /// The bank's liquidity vault authority, which owns the Solend obligation
    pub liquidity_vault_authority: Pubkey,
    /// Used as an intermediary to deposit tokens into Solend
    pub liquidity_vault: Pubkey,
    /// The Solend obligation account
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    /// Derived from the lending market
    pub lending_market_authority: Pubkey,
    /// The Solend reserve that holds liquidity
    pub integration_acc_1: Pubkey,
    /// Bank's liquidity token mint (e.g., USDC)
    pub mint: Pubkey,
    /// Reserve's liquidity supply SPL Token account
    pub reserve_liquidity_supply: Pubkey,
    /// The reserve's mint for cTokens
    pub reserve_collateral_mint: Pubkey,
    /// The reserve's collateral supply account (where cTokens are stored)
    pub reserve_collateral_supply: Pubkey,
    /// The user's destination for cTokens (collateral). This is a temporary account owned by
    /// liquidity_vault_authority that will hold cTokens between deposit and obligation update.
    pub user_collateral: Pubkey,
    /// Oracle accounts - required by Solend even if not actively used
    pub pyth_price: Pubkey,
    pub switchboard_feed: Pubkey,
    pub solend_program: Pubkey,
    pub token_program: Pubkey,
}
impl SolendDeposit {
    pub const DISCRIMINATOR: [u8; 8] = [56, 127, 176, 148, 12, 25, 3, 24];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.lending_market, false),
            AccountMeta::new_readonly(self.lending_market_authority, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.reserve_liquidity_supply, false),
            AccountMeta::new(self.reserve_collateral_mint, false),
            AccountMeta::new(self.reserve_collateral_supply, false),
            AccountMeta::new(self.user_collateral, false),
            AccountMeta::new_readonly(self.pyth_price, false),
            AccountMeta::new_readonly(self.switchboard_feed, false),
            AccountMeta::new_readonly(self.solend_program, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}
impl ToAccountMetas for SolendDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Deposit into a Solend reserve through a marginfi account
/// * amount - in the underlying token (e.g., USDC), in native decimals
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn solend_deposit(accounts: &SolendDeposit, amount: u64) -> Instruction {
    solend_deposit_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn solend_deposit_with_program_id(
    program_id: Pubkey,
    accounts: &SolendDeposit,
    amount: u64,
) -> Instruction {
    let mut data = SolendDeposit::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`solend_init_obligation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolendInitObligation {
    /// Pays to init the obligation and pays a nominal amount to ensure the obligation has a
    /// non-zero balance.
    pub fee_payer: Pubkey,
    pub bank: Pubkey,
    /// The fee payer must provide a nominal amount of bank tokens so the obligation is not empty.
    /// This amount is irrecoverable and will prevent the obligation from ever being closed.
    pub signer_token_account: Pubkey,
    /// The liquidity vault authority (PDA that will own the Solend obligation)
    pub liquidity_vault_authority: Pubkey,
    /// Used as an intermediary to deposit a nominal amount of token into the obligation.
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    /// Derived from the lending market
    pub lending_market_authority: Pubkey,
    pub integration_acc_1: Pubkey,
    /// Bank's liquidity token mint (e.g., USDC)
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    /// The reserve's mint for cTokens
    pub reserve_collateral_mint: Pubkey,
    /// The reserve's collateral supply account (where cTokens are stored)
    pub reserve_collateral_supply: Pubkey,
    /// The user's destination for cTokens (collateral). This is a temporary account owned by
    /// liquidity_vault_authority that will hold cTokens between deposit and obligation update.
    pub user_collateral: Pubkey,
    /// Oracle accounts - required by Solend even if not actively used
    pub pyth_price: Pubkey,
    pub switchboard_feed: Pubkey,
    pub solend_program: Pubkey,
    pub token_program: Pubkey,
    pub rent: Pubkey,
    pub system_program: Pubkey,
}
impl SolendInitObligation {
    pub const DISCRIMINATOR: [u8; 8] = [81, 96, 123, 149, 218, 116, 235, 196];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.bank, false),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new_readonly(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new_readonly(self.lending_market, false),
            AccountMeta::new_readonly(self.lending_market_authority, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new(self.mint, false),
            AccountMeta::new(self.reserve_liquidity_supply, false),
            AccountMeta::new(self.reserve_collateral_mint, false),
            AccountMeta::new(self.reserve_collateral_supply, false),
            AccountMeta::new(self.user_collateral, false),
            AccountMeta::new_readonly(self.pyth_price, false),
            AccountMeta::new_readonly(self.switchboard_feed, false),
            AccountMeta::new_readonly(self.solend_program, false),
            AccountMeta::new_readonly(self.token_program, false),
            AccountMeta::new_readonly(self.rent, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for SolendInitObligation {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Initialize a Solend obligation for a marginfi bank
/// Requires a minimum deposit to ensure the obligation remains active
/// * amount - minimum deposit amount (at least 10 units) in native decimals
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn solend_init_obligation(accounts: &SolendInitObligation, amount: u64) -> Instruction {
    solend_init_obligation_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn solend_init_obligation_with_program_id(
    program_id: Pubkey,
    accounts: &SolendInitObligation,
    amount: u64,
) -> Instruction {
    let mut data = SolendInitObligation::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`solend_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolendWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    /// Token account that will receive the withdrawn tokens. Mint/owner are validated by the
    /// SPL transfer; the caller controls the destination.
    pub destination_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    /// The Solend obligation account
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    /// Derived from the lending market
    pub lending_market_authority: Pubkey,
    /// The Solend reserve that holds liquidity
    pub integration_acc_1: Pubkey,
    /// Bank's liquidity token mint (e.g., USDC)
    pub mint: Pubkey,
    /// Reserve's liquidity supply SPL Token account
    pub reserve_liquidity_supply: Pubkey,
    /// The reserve's mint for cTokens
    pub reserve_collateral_mint: Pubkey,
    /// The reserve's collateral supply account (where cTokens are stored)
    pub reserve_collateral_supply: Pubkey,
    /// The user's destination for cTokens (collateral). This is a temporary account owned by
    /// liquidity_vault_authority that holds cTokens.
    pub user_collateral: Pubkey,
    pub solend_program: Pubkey,
    pub token_program: Pubkey,
}
impl SolendWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = [238, 144, 170, 199, 21, 72, 155, 36];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new(self.liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new(self.integration_acc_2, false),
            AccountMeta::new(self.lending_market, false),
            AccountMeta::new_readonly(self.lending_market_authority, false),
            AccountMeta::new(self.integration_acc_1, false),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.reserve_liquidity_supply, false),
            AccountMeta::new(self.reserve_collateral_mint, false),
            AccountMeta::new(self.reserve_collateral_supply, false),
            AccountMeta::new(self.user_collateral, false),
            AccountMeta::new_readonly(self.solend_program, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}
impl ToAccountMetas for SolendWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Withdraw from a Solend reserve through a marginfi account
/// * amount - in collateral tokens (cTokens), in native decimals
/// * if group rate limits are enabled, include the withdrawn bank's oracle group in
/// `remaining_accounts`
/// * withdraw_all - withdraw entire position if true
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn solend_withdraw(
    accounts: &SolendWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    solend_withdraw_with_program_id(crate::ID, accounts, amount, withdraw_all)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn solend_withdraw_with_program_id(
    program_id: Pubkey,
    accounts: &SolendWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    let mut data = SolendWithdraw::DISCRIMINATOR.to_vec();
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
