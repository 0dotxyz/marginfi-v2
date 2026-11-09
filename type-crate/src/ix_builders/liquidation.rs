//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`end_deleverage`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndDeleverage {
    pub marginfi_account: Pubkey,
    pub liquidation_record: Pubkey,
    pub group: Pubkey,
    pub risk_admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl EndDeleverage {
    pub const DISCRIMINATOR: [u8; 8] = [114, 14, 250, 143, 252, 104, 214, 209];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.risk_admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for EndDeleverage {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (risk_admin only) End forced deleverage. Validates health did not worsen.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn end_deleverage(accounts: &EndDeleverage) -> Instruction {
    end_deleverage_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn end_deleverage_with_program_id(program_id: Pubkey, accounts: &EndDeleverage) -> Instruction {
    let data = EndDeleverage::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`end_liquidation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndLiquidation {
    /// Account under liquidation
    pub marginfi_account: Pubkey,
    /// The associated liquidation record PDA for the given `marginfi_account`
    pub liquidation_record: Pubkey,
    pub group: Pubkey,
    pub liquidation_receiver: Pubkey,
    pub fee_state: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub system_program: Pubkey,
    /// Optional separate payer for the flat liquidation fee. When provided it must sign and pays the
    /// fee; when omitted, the `liquidation_receiver` pays (the default).
    pub fee_payer: Option<Pubkey>,
}
impl EndLiquidation {
    pub const DISCRIMINATOR: [u8; 8] = [110, 11, 244, 54, 229, 181, 22, 184];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.liquidation_receiver, true),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.system_program, false),
            match self.fee_payer {
                Some(key) => AccountMeta::new(key, true),
                None => AccountMeta::new_readonly(program_id, false),
            },
        ]
    }
}
impl ToAccountMetas for EndLiquidation {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (liquidation_receiver, set in start_liquidation) End receivership liquidation. Validates
/// health improved and seized assets are within fee limits. Charges a flat SOL fee.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn end_liquidation(accounts: &EndLiquidation) -> Instruction {
    end_liquidation_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn end_liquidation_with_program_id(
    program_id: Pubkey,
    accounts: &EndLiquidation,
) -> Instruction {
    let data = EndLiquidation::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`start_deleverage`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartDeleverage {
    /// Account to deleverage
    pub marginfi_account: Pubkey,
    /// The associated liquidation record PDA for the given `marginfi_account`
    pub liquidation_record: Pubkey,
    pub group: Pubkey,
    /// The risk admin will have the authority to withdraw/repay as if they are the user authority
    /// until the end of the tx.
    pub risk_admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl StartDeleverage {
    pub const DISCRIMINATOR: [u8; 8] = [10, 138, 10, 57, 40, 232, 182, 193];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.risk_admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for StartDeleverage {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (risk_admin only) Begin forced deleverage on an account. Similar to start_liquidation but
/// does not require the account to be unhealthy.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn start_deleverage(accounts: &StartDeleverage) -> Instruction {
    start_deleverage_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn start_deleverage_with_program_id(
    program_id: Pubkey,
    accounts: &StartDeleverage,
) -> Instruction {
    let data = StartDeleverage::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`start_liquidation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartLiquidation {
    /// Account under liquidation
    pub marginfi_account: Pubkey,
    /// The associated liquidation record PDA for the given `marginfi_account`
    pub liquidation_record: Pubkey,
    pub group: Pubkey,
    /// This account will have the authority to withdraw/repay as if they are the user authority
    /// until the end of the tx.
    ///
    pub liquidation_receiver: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl StartLiquidation {
    pub const DISCRIMINATOR: [u8; 8] = [244, 93, 90, 214, 192, 166, 191, 21];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.liquidation_receiver, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for StartLiquidation {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Begin receivership liquidation on an unhealthy account. Snapshots health
/// and marks the account in receivership. Must have `end_liquidation` as the last ix in the tx.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn start_liquidation(accounts: &StartLiquidation) -> Instruction {
    start_liquidation_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn start_liquidation_with_program_id(
    program_id: Pubkey,
    accounts: &StartLiquidation,
) -> Instruction {
    let data = StartLiquidation::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
