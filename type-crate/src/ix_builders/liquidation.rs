use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`start_liquidation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartLiquidation {
    pub marginfi_account: Pubkey,
    pub liquidation_record: Pubkey,
    pub group: Pubkey,
    pub liquidation_receiver: Pubkey,
    pub instruction_sysvar: Pubkey,
}

impl StartLiquidation {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::START_LIQUIDATION;
}

impl ToAccountMetas for StartLiquidation {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.liquidation_receiver, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}

/// (permissionless) Begin receivership liquidation on an unhealthy account. Snapshots health
/// and marks the account in receivership. Must have `end_liquidation` as the last ix in the tx.
pub fn start_liquidation(accounts: &StartLiquidation) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: StartLiquidation::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`end_liquidation`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndLiquidation {
    pub marginfi_account: Pubkey,
    pub liquidation_record: Pubkey,
    pub group: Pubkey,
    pub liquidation_receiver: Pubkey,
    pub fee_state: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub system_program: Pubkey,
    pub fee_payer: Option<Pubkey>,
}

impl EndLiquidation {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::END_LIQUIDATION;
}

impl ToAccountMetas for EndLiquidation {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        let mut metas = Vec::with_capacity(8);
        metas.push(AccountMeta::new(self.marginfi_account, false));
        metas.push(AccountMeta::new(self.liquidation_record, false));
        metas.push(AccountMeta::new_readonly(self.group, false));
        metas.push(AccountMeta::new(self.liquidation_receiver, true));
        metas.push(AccountMeta::new_readonly(self.fee_state, false));
        metas.push(AccountMeta::new(self.global_fee_wallet, false));
        metas.push(AccountMeta::new_readonly(self.system_program, false));
        match self.fee_payer {
            Some(key) => metas.push(AccountMeta::new(key, true)),
            None => metas.push(AccountMeta::new_readonly(crate::ID, false)),
        }
        metas
    }
}

/// (liquidation_receiver, set in start_liquidation) End receivership liquidation. Validates
/// health improved and seized assets are within fee limits. Charges a flat SOL fee.
pub fn end_liquidation(accounts: &EndLiquidation) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: EndLiquidation::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`start_deleverage`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartDeleverage {
    pub marginfi_account: Pubkey,
    pub liquidation_record: Pubkey,
    pub group: Pubkey,
    pub risk_admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}

impl StartDeleverage {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::START_DELEVERAGE;
}

impl ToAccountMetas for StartDeleverage {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.risk_admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}

/// (risk_admin only) Begin forced deleverage on an account. Similar to start_liquidation but
/// does not require the account to be unhealthy.
pub fn start_deleverage(accounts: &StartDeleverage) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: StartDeleverage::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`end_deleverage`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndDeleverage {
    pub marginfi_account: Pubkey,
    pub liquidation_record: Pubkey,
    pub group: Pubkey,
    pub risk_admin: Pubkey,
}

impl EndDeleverage {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::END_DELEVERAGE;
}

impl ToAccountMetas for EndDeleverage {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.risk_admin, true),
        ]
    }
}

/// (risk_admin only) End forced deleverage. Validates health did not worsen.
pub fn end_deleverage(accounts: &EndDeleverage) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: EndDeleverage::DISCRIMINATOR.to_vec(),
    }
}
