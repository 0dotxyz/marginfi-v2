use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`marginfi_account_initialize`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountInitialize {
    pub marginfi_group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_payer: Pubkey,
    pub system_program: Pubkey,
}

impl MarginfiAccountInitialize {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_INITIALIZE;
}

impl ToAccountMetas for MarginfiAccountInitialize {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new(self.marginfi_account, true),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// Initialize a marginfi account for a given group. The account is a fresh keypair, and must
/// sign. If you are a CPI caller, consider using `marginfi_account_initialize_pda` instead, or
/// create the account manually and use `transfer_to_new_account` to gift it to the owner you
/// wish.
pub fn marginfi_account_initialize(accounts: &MarginfiAccountInitialize) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountInitialize::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`marginfi_account_initialize_pda`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountInitializePda {
    pub marginfi_group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_payer: Pubkey,
    pub instructions_sysvar: Pubkey,
    pub system_program: Pubkey,
}

impl MarginfiAccountInitializePda {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_INITIALIZE_PDA;
}

impl ToAccountMetas for MarginfiAccountInitializePda {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.instructions_sysvar, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// The same as `marginfi_account_initialize`, except the created marginfi account uses a PDA
/// (Program Derived Address)
///
/// seeds:
/// - marginfi_group
/// - authority: The account authority (owner)  
/// - account_index: A u16 value to allow multiple accounts per authority
/// - third_party_id: Optional u16 for third-party tagging. Seeds < PDA_FREE_THRESHOLD can be
///   used freely. For a dedicated seed used by just your program (via CPI), contact us.
pub fn marginfi_account_initialize_pda(
    accounts: &MarginfiAccountInitializePda,
    account_index: u16,
    third_party_id: Option<u16>,
) -> Instruction {
    let mut data = MarginfiAccountInitializePda::DISCRIMINATOR.to_vec();
    account_index.serialize(&mut data).unwrap();
    third_party_id.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`marginfi_account_close`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountClose {
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_payer: Pubkey,
}

impl MarginfiAccountClose {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_CLOSE;
}

impl ToAccountMetas for MarginfiAccountClose {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.fee_payer, true),
        ]
    }
}

/// (account authority) Close a marginfi account. Requires all balances to be empty and no
/// active flags (disabled, flashloan, receivership).
pub fn marginfi_account_close(accounts: &MarginfiAccountClose) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountClose::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`marginfi_account_set_freeze`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountSetFreeze {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub admin: Pubkey,
}

impl MarginfiAccountSetFreeze {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_SET_FREEZE;
}

impl ToAccountMetas for MarginfiAccountSetFreeze {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.admin, true),
        ]
    }
}

/// (admin only) Freeze or unfreeze a marginfi account. Frozen accounts can only be operated on
/// by the group admin.
pub fn marginfi_account_set_freeze(
    accounts: &MarginfiAccountSetFreeze,
    frozen: bool,
) -> Instruction {
    let mut data = MarginfiAccountSetFreeze::DISCRIMINATOR.to_vec();
    frozen.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`marginfi_account_update_emissions_destination_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountUpdateEmissionsDestinationAccount {
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub destination_account: Pubkey,
}

impl MarginfiAccountUpdateEmissionsDestinationAccount {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::MARGINFI_ACCOUNT_UPDATE_EMISSIONS_DESTINATION_ACCOUNT;
}

impl ToAccountMetas for MarginfiAccountUpdateEmissionsDestinationAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new_readonly(self.destination_account, false),
        ]
    }
}

/// (account authority) Set the wallet whose canonical ATA will receive off-chain emissions.
pub fn marginfi_account_update_emissions_destination_account(
    accounts: &MarginfiAccountUpdateEmissionsDestinationAccount,
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountUpdateEmissionsDestinationAccount::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`transfer_to_new_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferToNewAccount {
    pub group: Pubkey,
    pub old_marginfi_account: Pubkey,
    pub new_marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_payer: Pubkey,
    pub new_authority: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub fee_state: Pubkey,
    pub system_program: Pubkey,
}

impl TransferToNewAccount {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::TRANSFER_TO_NEW_ACCOUNT;
}

impl ToAccountMetas for TransferToNewAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.old_marginfi_account, false),
            AccountMeta::new(self.new_marginfi_account, true),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.new_authority, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (account authority) Transfer all positions to a new account under a new authority. The old
/// account is disabled. Pays a flat SOL fee to the protocol.
pub fn transfer_to_new_account(accounts: &TransferToNewAccount) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: TransferToNewAccount::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`transfer_to_new_account_pda`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferToNewAccountPda {
    pub group: Pubkey,
    pub old_marginfi_account: Pubkey,
    pub new_marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_payer: Pubkey,
    pub new_authority: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub fee_state: Pubkey,
    pub instructions_sysvar: Pubkey,
    pub system_program: Pubkey,
}

impl TransferToNewAccountPda {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::TRANSFER_TO_NEW_ACCOUNT_PDA;
}

impl ToAccountMetas for TransferToNewAccountPda {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.old_marginfi_account, false),
            AccountMeta::new(self.new_marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.new_authority, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new_readonly(self.instructions_sysvar, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (account authority) Same as `transfer_to_new_account` except the resulting account is a PDA
///
/// seeds:
/// - marginfi_group
/// - authority: The account authority (owner)  
/// - account_index: A u16 value to allow multiple accounts per authority
/// - third_party_id: Optional u16 for third-party tagging. Seeds < PDA_FREE_THRESHOLD can be
///   used freely. For a dedicated seed used by just your program (via CPI), contact us.
pub fn transfer_to_new_account_pda(
    accounts: &TransferToNewAccountPda,
    account_index: u16,
    third_party_id: Option<u16>,
) -> Instruction {
    let mut data = TransferToNewAccountPda::DISCRIMINATOR.to_vec();
    account_index.serialize(&mut data).unwrap();
    third_party_id.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`marginfi_account_init_liq_record`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountInitLiqRecord {
    pub marginfi_account: Pubkey,
    pub fee_payer: Pubkey,
    pub liquidation_record: Pubkey,
    pub system_program: Pubkey,
}

impl MarginfiAccountInitLiqRecord {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::INIT_LIQUIDATION_RECORD;
}

impl ToAccountMetas for MarginfiAccountInitLiqRecord {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}

/// (permissionless) Initialize a liquidation record PDA for a marginfi account. The fee_payer
/// pays rent; the record is required for receivership liquidation.
pub fn marginfi_account_init_liq_record(accounts: &MarginfiAccountInitLiqRecord) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountInitLiqRecord::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`marginfi_account_close_liq_record`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountCloseLiqRecord {
    pub marginfi_account: Pubkey,
    pub liquidation_record: Pubkey,
    pub record_payer: Pubkey,
}

impl MarginfiAccountCloseLiqRecord {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_CLOSE_LIQ_RECORD;
}

impl ToAccountMetas for MarginfiAccountCloseLiqRecord {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new(self.record_payer, false),
        ]
    }
}

/// (permissionless) Close a liquidation record PDA and return rent to the original payer.
/// Rent always goes to `record_payer`. Fails if the account is in receivership or deleverage.
pub fn marginfi_account_close_liq_record(accounts: &MarginfiAccountCloseLiqRecord) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountCloseLiqRecord::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`admin_close_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminCloseAccount {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub global_fee_wallet: Pubkey,
}

impl AdminCloseAccount {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::ADMIN_CLOSE_ACCOUNT;
}

impl ToAccountMetas for AdminCloseAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.global_fee_wallet, false),
        ]
    }
}

/// (permissionless) Close an account that is empty, inactive for >60 days, and has no
/// blocking state flags. Rent is returned to the group's global fee wallet.
pub fn admin_close_account(accounts: &AdminCloseAccount) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: AdminCloseAccount::DISCRIMINATOR.to_vec(),
    }
}
