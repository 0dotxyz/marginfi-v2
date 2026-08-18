use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::SolendConfigCompact;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`solend_init_obligation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolendInitObligation {
    pub fee_payer: Pubkey,
    pub bank: Pubkey,
    pub signer_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub integration_acc_1: Pubkey,
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_collateral_supply: Pubkey,
    pub user_collateral: Pubkey,
    pub pyth_price: Pubkey,
    pub switchboard_feed: Pubkey,
    pub solend_program: Pubkey,
    pub token_program: Pubkey,
    pub rent: Pubkey,
    pub system_program: Pubkey,
}

impl SolendInitObligation {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::SOLEND_INIT_OBLIGATION;
}

impl ToAccountMetas for SolendInitObligation {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
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

/// (permissionless) Initialize a Solend obligation for a marginfi bank
/// Requires a minimum deposit to ensure the obligation remains active
/// * amount - minimum deposit amount (at least 10 units) in native decimals
pub fn solend_init_obligation(accounts: &SolendInitObligation, amount: u64) -> Instruction {
    let mut data = SolendInitObligation::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`solend_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolendDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub signer_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub integration_acc_1: Pubkey,
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_collateral_supply: Pubkey,
    pub user_collateral: Pubkey,
    pub pyth_price: Pubkey,
    pub switchboard_feed: Pubkey,
    pub solend_program: Pubkey,
    pub token_program: Pubkey,
}

impl SolendDeposit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::SOLEND_DEPOSIT;
}

impl ToAccountMetas for SolendDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
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

/// (user) Deposit into a Solend reserve through a marginfi account
/// * amount - in the underlying token (e.g., USDC), in native decimals
pub fn solend_deposit(accounts: &SolendDeposit, amount: u64) -> Instruction {
    let mut data = SolendDeposit::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`solend_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolendWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub destination_token_account: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub integration_acc_2: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub integration_acc_1: Pubkey,
    pub mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_collateral_supply: Pubkey,
    pub user_collateral: Pubkey,
    pub solend_program: Pubkey,
    pub token_program: Pubkey,
}

impl SolendWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::SOLEND_WITHDRAW;
}

impl ToAccountMetas for SolendWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
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

/// (user) Withdraw from a Solend reserve through a marginfi account
/// * amount - in collateral tokens (cTokens), in native decimals  
/// * if group rate limits are enabled, include the withdrawn bank's oracle group in
///   `remaining_accounts`
/// * withdraw_all - withdraw entire position if true
pub fn solend_withdraw(
    accounts: &SolendWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    let mut data = SolendWithdraw::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    withdraw_all.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_pool_add_bank_solend`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingPoolAddBankSolend {
    pub group: Pubkey,
    pub admin: Pubkey,
    pub fee_payer: Pubkey,
    pub bank_mint: Pubkey,
    pub bank: Pubkey,
    pub integration_acc_1: Pubkey,
    pub integration_acc_2: Pubkey,
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
    pub token_program: Pubkey,
    pub system_program: Pubkey,
}

impl LendingPoolAddBankSolend {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_POOL_ADD_BANK_SOLEND;
}

impl ToAccountMetas for LendingPoolAddBankSolend {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.group, false),
            AccountMeta::new_readonly(self.admin, true),
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
        ]
    }
}

/// (admin) Add a Solend bank to the marginfi group
pub fn lending_pool_add_bank_solend(
    accounts: &LendingPoolAddBankSolend,
    bank_config: SolendConfigCompact,
    bank_seed: u64,
) -> Instruction {
    let mut data = LendingPoolAddBankSolend::DISCRIMINATOR.to_vec();
    bank_config.serialize(&mut data).unwrap();
    bank_seed.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}
