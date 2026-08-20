use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`lending_account_deposit`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountDeposit {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub signer_token_account: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}

impl LendingAccountDeposit {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_ACCOUNT_DEPOSIT;
}

impl ToAccountMetas for LendingAccountDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (account authority) Deposit assets into a bank. Accrues interest, records deposit, and
/// transfers tokens from the signer's token account to the bank's liquidity vault.
/// When the bank's mint is Token-2022, `remaining_accounts` must begin with that mint.
pub fn lending_account_deposit(
    accounts: &LendingAccountDeposit,
    amount: u64,
    deposit_up_to_limit: Option<bool>,
) -> Instruction {
    let mut data = LendingAccountDeposit::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    deposit_up_to_limit.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_account_repay`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountRepay {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub signer_token_account: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}

impl LendingAccountRepay {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_ACCOUNT_REPAY;
}

impl ToAccountMetas for LendingAccountRepay {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.signer_token_account, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (account authority, or any signer during receivership) Repay borrowed assets. Accrues
/// interest, records repayment, and transfers tokens to the bank's liquidity vault.
/// When the bank's mint is Token-2022, `remaining_accounts` must begin with that mint.
pub fn lending_account_repay(
    accounts: &LendingAccountRepay,
    amount: u64,
    repay_all: Option<bool>,
) -> Instruction {
    let mut data = LendingAccountRepay::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    repay_all.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_account_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub destination_token_account: Pubkey,
    pub bank_liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}

impl LendingAccountWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_ACCOUNT_WITHDRAW;
}

impl ToAccountMetas for LendingAccountWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.bank_liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (account authority, or any signer during receivership) Withdraw assets from a bank. Accrues
/// interest, records withdrawal, transfers tokens, and runs a health check (skipped during
/// receivership). `remaining_accounts` must hold a bank and its oracles for every active
/// balance, plus the withdrawn bank's oracle group when group rate limits are enabled.
/// When the bank's mint is Token-2022, `remaining_accounts` must begin with that mint.
pub fn lending_account_withdraw(
    accounts: &LendingAccountWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    let mut data = LendingAccountWithdraw::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    withdraw_all.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_account_borrow`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountBorrow {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub destination_token_account: Pubkey,
    pub bank_liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}

impl LendingAccountBorrow {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_ACCOUNT_BORROW;
}

impl ToAccountMetas for LendingAccountBorrow {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
            AccountMeta::new(self.destination_token_account, false),
            AccountMeta::new_readonly(self.bank_liquidity_vault_authority, false),
            AccountMeta::new(self.liquidity_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (account authority) Borrow assets from a bank. Accrues interest, records liability, applies
/// origination fee, transfers tokens, and runs a health check. `remaining_accounts` must hold
/// a bank and its oracles for every active balance, plus the borrowed bank's oracle group when
/// group rate limits are enabled.
/// When the bank's mint is Token-2022, `remaining_accounts` must begin with that mint.
pub fn lending_account_borrow(accounts: &LendingAccountBorrow, amount: u64) -> Instruction {
    let mut data = LendingAccountBorrow::DISCRIMINATOR.to_vec();
    amount.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_account_close_balance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountCloseBalance {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
}

impl LendingAccountCloseBalance {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_ACCOUNT_CLOSE_BALANCE;
}

impl ToAccountMetas for LendingAccountCloseBalance {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}

/// (account authority) Close a balance position with dust-level amounts.
pub fn lending_account_close_balance(accounts: &LendingAccountCloseBalance) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingAccountCloseBalance::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_account_liquidate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountLiquidate {
    pub group: Pubkey,
    pub asset_bank: Pubkey,
    pub liab_bank: Pubkey,
    pub liquidator_marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub liquidatee_marginfi_account: Pubkey,
    pub bank_liquidity_vault_authority: Pubkey,
    pub bank_liquidity_vault: Pubkey,
    pub bank_insurance_vault: Pubkey,
    pub token_program: Pubkey,
}

impl LendingAccountLiquidate {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_ACCOUNT_LIQUIDATE;
}

impl ToAccountMetas for LendingAccountLiquidate {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.asset_bank, false),
            AccountMeta::new(self.liab_bank, false),
            AccountMeta::new(self.liquidator_marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.liquidatee_marginfi_account, false),
            AccountMeta::new_readonly(self.bank_liquidity_vault_authority, false),
            AccountMeta::new(self.bank_liquidity_vault, false),
            AccountMeta::new(self.bank_insurance_vault, false),
            AccountMeta::new_readonly(self.token_program, false),
        ]
    }
}

/// (permissionless) Liquidate a lending account balance of an unhealthy marginfi account.
/// The liquidator takes on the liability and receives discounted collateral (2.5% liquidator
/// fee + 2.5% insurance fee).
/// * `asset_amount` - amount of collateral to liquidate
/// * `liquidatee_accounts` - number of remaining accounts for the liquidatee
/// * `liquidator_accounts` - number of remaining accounts for the liquidator
///
/// When the bank's mint is Token-2022, `remaining_accounts` must begin with that mint.
pub fn lending_account_liquidate(
    accounts: &LendingAccountLiquidate,
    asset_amount: u64,
    liquidatee_accounts: u8,
    liquidator_accounts: u8,
) -> Instruction {
    let mut data = LendingAccountLiquidate::DISCRIMINATOR.to_vec();
    asset_amount.serialize(&mut data).unwrap();
    liquidatee_accounts.serialize(&mut data).unwrap();
    liquidator_accounts.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_account_start_flashloan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountStartFlashloan {
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub ixs_sysvar: Pubkey,
}

impl LendingAccountStartFlashloan {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::START_FLASHLOAN;
}

impl ToAccountMetas for LendingAccountStartFlashloan {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new_readonly(self.ixs_sysvar, false),
        ]
    }
}

/// (account authority) Start a flash loan. Must have a corresponding `end_flashloan` ix in the
/// same tx. Health checks are skipped until the flash loan ends.
pub fn lending_account_start_flashloan(
    accounts: &LendingAccountStartFlashloan,
    end_index: u64,
) -> Instruction {
    let mut data = LendingAccountStartFlashloan::DISCRIMINATOR.to_vec();
    end_index.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`lending_account_end_flashloan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountEndFlashloan {
    pub marginfi_account: Pubkey,
    pub group: Pubkey,
    pub authority: Pubkey,
}

impl LendingAccountEndFlashloan {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::END_FLASHLOAN;
}

impl ToAccountMetas for LendingAccountEndFlashloan {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.authority, true),
        ]
    }
}

/// (account authority) End a flash loan and run the health check.
///
/// `remaining_accounts` must hold a bank and its oracles for every active balance on the
/// account; the init-health check prices all of them.
pub fn lending_account_end_flashloan(accounts: &LendingAccountEndFlashloan) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingAccountEndFlashloan::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`lending_account_pulse_health`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountPulseHealth {
    pub marginfi_account: Pubkey,
    pub group: Pubkey,
}

impl LendingAccountPulseHealth {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::LENDING_ACCOUNT_PULSE_HEALTH;
}

impl ToAccountMetas for LendingAccountPulseHealth {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.group, false),
        ]
    }
}

/// (Permissionless) Refresh the internal risk engine health cache. Useful for liquidators and
/// other consumers that want to see the internal risk state of a user account. This cache is
/// read-only and serves no purpose except being populated by this ix.
/// `remaining_accounts` must contain each active bank followed by that bank's oracle accounts,
/// repeating this group for every active balance. Use [`crate::pdas::bank_observation_keys`] for
/// the oracle order within each group.
pub fn lending_account_pulse_health(accounts: &LendingAccountPulseHealth) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: LendingAccountPulseHealth::DISCRIMINATOR.to_vec(),
    }
}
