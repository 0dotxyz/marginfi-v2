//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`lending_account_borrow`].
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
    pub const DISCRIMINATOR: [u8; 8] = [4, 126, 116, 53, 48, 5, 212, 31];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for LendingAccountBorrow {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) Borrow assets from a bank. Accrues interest, records liability, applies
/// origination fee, transfers tokens, and runs a health check. If group rate limits are
/// enabled, `remaining_accounts` must include the borrowed bank's oracle group for USD
/// pricing.
///
/// For Token-2022, start with the bank mint. Include each active bank and its oracle accounts; group rate limits can require the borrowed bank oracle group.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_borrow(accounts: &LendingAccountBorrow, amount: u64) -> Instruction {
    lending_account_borrow_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_borrow_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountBorrow,
    amount: u64,
) -> Instruction {
    let mut data = LendingAccountBorrow::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_account_close_balance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountCloseBalance {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub bank: Pubkey,
}
impl LendingAccountCloseBalance {
    pub const DISCRIMINATOR: [u8; 8] = [245, 54, 41, 4, 243, 202, 31, 17];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.bank, false),
        ]
    }
}
impl ToAccountMetas for LendingAccountCloseBalance {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) Close a balance position with dust-level amounts.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_close_balance(accounts: &LendingAccountCloseBalance) -> Instruction {
    lending_account_close_balance_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_close_balance_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountCloseBalance,
) -> Instruction {
    let data = LendingAccountCloseBalance::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_account_deposit`].
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
    pub const DISCRIMINATOR: [u8; 8] = [171, 94, 235, 103, 82, 64, 212, 140];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for LendingAccountDeposit {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) Deposit assets into a bank. Accrues interest, records deposit, and
/// transfers tokens from the signer's token account to the bank's liquidity vault.
///
/// For Token-2022, start remaining accounts with the bank mint; append transfer-hook accounts if required.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_deposit(
    accounts: &LendingAccountDeposit,
    amount: u64,
    deposit_up_to_limit: Option<bool>,
) -> Instruction {
    lending_account_deposit_with_program_id(crate::ID, accounts, amount, deposit_up_to_limit)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_deposit_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountDeposit,
    amount: u64,
    deposit_up_to_limit: Option<bool>,
) -> Instruction {
    let mut data = LendingAccountDeposit::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    deposit_up_to_limit
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_account_end_flashloan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountEndFlashloan {
    pub marginfi_account: Pubkey,
    /// Needed for the same-asset emode checks and the premium snapshot recompute; validated by
    /// the `has_one = group` on `marginfi_account`.
    pub group: Pubkey,
    pub authority: Pubkey,
}
impl LendingAccountEndFlashloan {
    pub const DISCRIMINATOR: [u8; 8] = [105, 124, 201, 106, 153, 2, 8, 156];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.authority, true),
        ]
    }
}
impl ToAccountMetas for LendingAccountEndFlashloan {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) End a flash loan and run the health check.
///
/// Pass each active bank followed by its oracle and integration accounts for the health check.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_end_flashloan(accounts: &LendingAccountEndFlashloan) -> Instruction {
    lending_account_end_flashloan_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_end_flashloan_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountEndFlashloan,
) -> Instruction {
    let data = LendingAccountEndFlashloan::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_account_liquidate`].
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
    pub const DISCRIMINATOR: [u8; 8] = [214, 169, 151, 213, 251, 167, 86, 219];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for LendingAccountLiquidate {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Liquidate a lending account balance of an unhealthy marginfi account.
/// The liquidator takes on the liability and receives discounted collateral (2.5% liquidator
/// fee + 2.5% insurance fee).
/// * `asset_amount` - amount of collateral to liquidate
/// * `liquidatee_accounts` - number of remaining accounts for the liquidatee
/// * `liquidator_accounts` - number of remaining accounts for the liquidator
///
/// After the Token-2022 liability mint when required, pass the liquidatee and liquidator health-account partitions in that order.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_liquidate(
    accounts: &LendingAccountLiquidate,
    asset_amount: u64,
    liquidatee_accounts: u8,
    liquidator_accounts: u8,
) -> Instruction {
    lending_account_liquidate_with_program_id(
        crate::ID,
        accounts,
        asset_amount,
        liquidatee_accounts,
        liquidator_accounts,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_liquidate_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountLiquidate,
    asset_amount: u64,
    liquidatee_accounts: u8,
    liquidator_accounts: u8,
) -> Instruction {
    let mut data = LendingAccountLiquidate::DISCRIMINATOR.to_vec();
    asset_amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    liquidatee_accounts
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    liquidator_accounts
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_account_pulse_health`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountPulseHealth {
    pub marginfi_account: Pubkey,
    /// Needed for same-asset emode checks and the premium snapshot recompute
    pub group: Pubkey,
}
impl LendingAccountPulseHealth {
    pub const DISCRIMINATOR: [u8; 8] = [186, 52, 117, 97, 34, 74, 39, 253];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.group, false),
        ]
    }
}
impl ToAccountMetas for LendingAccountPulseHealth {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (Permissionless) Refresh the internal risk engine health cache. Useful for liquidators and
/// other consumers that want to see the internal risk state of a user account. This cache is
/// read-only and serves no purpose except being populated by this ix.
/// * remaining accounts expected in the same order as borrow, etc. I.e., for each balance the
/// user has, pass bank and oracle: <bank1, oracle1, bank2, oracle2>
///
/// Pass each active bank followed by its oracle and integration accounts for the health checks.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_pulse_health(accounts: &LendingAccountPulseHealth) -> Instruction {
    lending_account_pulse_health_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_pulse_health_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountPulseHealth,
) -> Instruction {
    let data = LendingAccountPulseHealth::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_account_repay`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountRepay {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    /// Must be marginfi_account's authority, unless in liquidation/deleverage receivership or order execution
    ///
    /// Note: during receivership and order execution, there are no signer checks whatsoever: any key can repay as
    /// long as the invariants checked at the end of execution are met.
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub signer_token_account: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}
impl LendingAccountRepay {
    pub const DISCRIMINATOR: [u8; 8] = [79, 209, 172, 177, 222, 51, 173, 151];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for LendingAccountRepay {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority, or any signer during receivership) Repay borrowed assets. Accrues
/// interest, records repayment, and transfers tokens to the bank's liquidity vault.
///
/// For Token-2022, start remaining accounts with the bank mint; append transfer-hook accounts if required.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_repay(
    accounts: &LendingAccountRepay,
    amount: u64,
    repay_all: Option<bool>,
) -> Instruction {
    lending_account_repay_with_program_id(crate::ID, accounts, amount, repay_all)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_repay_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountRepay,
    amount: u64,
    repay_all: Option<bool>,
) -> Instruction {
    let mut data = LendingAccountRepay::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    repay_all
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_account_start_flashloan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountStartFlashloan {
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub ixs_sysvar: Pubkey,
}
impl LendingAccountStartFlashloan {
    pub const DISCRIMINATOR: [u8; 8] = [14, 131, 33, 220, 81, 186, 180, 107];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new_readonly(self.ixs_sysvar, false),
        ]
    }
}
impl ToAccountMetas for LendingAccountStartFlashloan {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) Start a flash loan. Must have a corresponding `end_flashloan` ix in the
/// same tx. Health checks are skipped until the flash loan ends.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_start_flashloan(
    accounts: &LendingAccountStartFlashloan,
    end_index: u64,
) -> Instruction {
    lending_account_start_flashloan_with_program_id(crate::ID, accounts, end_index)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_start_flashloan_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountStartFlashloan,
    end_index: u64,
) -> Instruction {
    let mut data = LendingAccountStartFlashloan::DISCRIMINATOR.to_vec();
    end_index
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`lending_account_withdraw`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LendingAccountWithdraw {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    /// Must be marginfi_account's authority, unless in liquidation/deleverage receivership or order execution
    ///
    /// Note: during receivership and order execution, there are no signer checks whatsoever: any key can repay as
    /// long as the invariants checked at the end of execution are met.
    pub authority: Pubkey,
    pub bank: Pubkey,
    pub destination_token_account: Pubkey,
    pub bank_liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub token_program: Pubkey,
}
impl LendingAccountWithdraw {
    pub const DISCRIMINATOR: [u8; 8] = [36, 72, 74, 19, 210, 210, 192, 192];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for LendingAccountWithdraw {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority, or any signer during receivership) Withdraw assets from a bank. Accrues
/// interest, records withdrawal, transfers tokens, and runs a health check (skipped during
/// receivership). If group rate limits are enabled, `remaining_accounts` must include the
/// withdrawn bank's oracle group for USD pricing.
///
/// For Token-2022, start with the bank mint. Include each active bank and its oracle accounts; group rate limits can require the withdrawn bank oracle group.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_withdraw(
    accounts: &LendingAccountWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    lending_account_withdraw_with_program_id(crate::ID, accounts, amount, withdraw_all)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_withdraw_with_program_id(
    program_id: Pubkey,
    accounts: &LendingAccountWithdraw,
    amount: u64,
    withdraw_all: Option<bool>,
) -> Instruction {
    let mut data = LendingAccountWithdraw::DISCRIMINATOR.to_vec();
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
