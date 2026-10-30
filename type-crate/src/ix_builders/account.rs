//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`admin_close_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminCloseAccount {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub global_fee_wallet: Pubkey,
    /// `withdraw_rebalance_fee_pool`) before close so its lamports are not orphaned.
    pub rebalance_fee_pool: Pubkey,
}
impl AdminCloseAccount {
    pub const DISCRIMINATOR: [u8; 8] = [131, 60, 75, 215, 109, 34, 157, 26];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.rebalance_fee_pool, false),
        ]
    }
}
impl ToAccountMetas for AdminCloseAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Close an account that is empty, inactive for >60 days, and has no
/// blocking state flags. Rent is returned to the group's global fee wallet.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn admin_close_account(accounts: &AdminCloseAccount) -> Instruction {
    admin_close_account_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn admin_close_account_with_program_id(
    program_id: Pubkey,
    accounts: &AdminCloseAccount,
) -> Instruction {
    let data = AdminCloseAccount::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_close`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountClose {
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_payer: Pubkey,
    /// `withdraw_rebalance_fee_pool`) before close so its lamports are not orphaned.
    pub rebalance_fee_pool: Pubkey,
}
impl MarginfiAccountClose {
    pub const DISCRIMINATOR: [u8; 8] = [186, 221, 93, 34, 50, 97, 194, 241];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.rebalance_fee_pool, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountClose {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) Close a marginfi account. Requires all balances to be empty and no
/// active flags (disabled, flashloan, receivership).
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close(accounts: &MarginfiAccountClose) -> Instruction {
    marginfi_account_close_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountClose,
) -> Instruction {
    let data = MarginfiAccountClose::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_close_liq_record`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountCloseLiqRecord {
    pub marginfi_account: Pubkey,
    pub liquidation_record: Pubkey,
    /// The wallet that originally paid to create this record.
    /// Rent is returned here via Anchor's `close` constraint.
    pub record_payer: Pubkey,
}
impl MarginfiAccountCloseLiqRecord {
    pub const DISCRIMINATOR: [u8; 8] = [187, 222, 41, 134, 102, 10, 112, 147];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new(self.record_payer, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountCloseLiqRecord {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Close a liquidation record PDA and return rent to the original payer.
/// Rent always goes to `record_payer`. Fails if the account is in receivership or deleverage.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close_liq_record(accounts: &MarginfiAccountCloseLiqRecord) -> Instruction {
    marginfi_account_close_liq_record_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close_liq_record_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountCloseLiqRecord,
) -> Instruction {
    let data = MarginfiAccountCloseLiqRecord::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_init_liq_record`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountInitLiqRecord {
    pub marginfi_account: Pubkey,
    pub fee_payer: Pubkey,
    pub liquidation_record: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountInitLiqRecord {
    pub const DISCRIMINATOR: [u8; 8] = [236, 213, 238, 126, 147, 251, 164, 8];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new(self.liquidation_record, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountInitLiqRecord {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Initialize a liquidation record PDA for a marginfi account. The fee_payer
/// pays rent; the record is required for receivership liquidation.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_init_liq_record(accounts: &MarginfiAccountInitLiqRecord) -> Instruction {
    marginfi_account_init_liq_record_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_init_liq_record_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountInitLiqRecord,
) -> Instruction {
    let data = MarginfiAccountInitLiqRecord::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_initialize`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountInitialize {
    pub marginfi_group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_payer: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountInitialize {
    pub const DISCRIMINATOR: [u8; 8] = [43, 78, 61, 255, 148, 52, 249, 154];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_group, false),
            AccountMeta::new(self.marginfi_account, true),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountInitialize {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// Initialize a marginfi account for a given group. The account is a fresh keypair, and must
/// sign. If you are a CPI caller, consider using `marginfi_account_initialize_pda` instead, or
/// create the account manually and use `transfer_to_new_account` to gift it to the owner you
/// wish.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_initialize(accounts: &MarginfiAccountInitialize) -> Instruction {
    marginfi_account_initialize_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_initialize_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountInitialize,
) -> Instruction {
    let data = MarginfiAccountInitialize::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_initialize_pda`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountInitializePda {
    pub marginfi_group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_payer: Pubkey,
    /// Instructions sysvar for CPI validation
    ///
    pub instructions_sysvar: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountInitializePda {
    pub const DISCRIMINATOR: [u8; 8] = [87, 177, 91, 80, 218, 119, 245, 31];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for MarginfiAccountInitializePda {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
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
/// used freely. For a dedicated seed used by just your program (via CPI), contact us.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_initialize_pda(
    accounts: &MarginfiAccountInitializePda,
    account_index: u16,
    third_party_id: Option<u16>,
) -> Instruction {
    marginfi_account_initialize_pda_with_program_id(
        crate::ID,
        accounts,
        account_index,
        third_party_id,
    )
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_initialize_pda_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountInitializePda,
    account_index: u16,
    third_party_id: Option<u16>,
) -> Instruction {
    let mut data = MarginfiAccountInitializePda::DISCRIMINATOR.to_vec();
    account_index
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    third_party_id
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_set_freeze`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountSetFreeze {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    /// Fast admin when freezing; governance admin when unfreezing. The legacy account name is
    /// retained for instruction compatibility.
    pub admin: Pubkey,
    pub instruction_sysvar: Pubkey,
}
impl MarginfiAccountSetFreeze {
    pub const DISCRIMINATOR: [u8; 8] = [199, 179, 231, 30, 138, 247, 110, 227];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.admin, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountSetFreeze {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// Freeze a marginfi account with the fast admin, or unfreeze it with the slow governance
/// admin. A frozen account can only be operated on by the slow governance admin.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_set_freeze(
    accounts: &MarginfiAccountSetFreeze,
    frozen: bool,
) -> Instruction {
    marginfi_account_set_freeze_with_program_id(crate::ID, accounts, frozen)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_set_freeze_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountSetFreeze,
    frozen: bool,
) -> Instruction {
    let mut data = MarginfiAccountSetFreeze::DISCRIMINATOR.to_vec();
    frozen
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_tag_liq_record`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountTagLiqRecord {
    pub marginfi_account: Pubkey,
    pub group: Pubkey,
}
impl MarginfiAccountTagLiqRecord {
    pub const DISCRIMINATOR: [u8; 8] = [12, 189, 215, 239, 231, 122, 220, 158];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.group, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountTagLiqRecord {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Tag an unhealthy account, growing the allowed liquidation premium over
/// time. Calling it on an account that is healthy again clears its existing tag.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_tag_liq_record(accounts: &MarginfiAccountTagLiqRecord) -> Instruction {
    marginfi_account_tag_liq_record_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_tag_liq_record_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountTagLiqRecord,
) -> Instruction {
    let data = MarginfiAccountTagLiqRecord::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_update_emissions_destination_account`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountUpdateEmissionsDestinationAccount {
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    /// the canonical ATA for each emissions mint.
    pub destination_account: Pubkey,
}
impl MarginfiAccountUpdateEmissionsDestinationAccount {
    pub const DISCRIMINATOR: [u8; 8] = [73, 185, 162, 201, 111, 24, 116, 185];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new_readonly(self.destination_account, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountUpdateEmissionsDestinationAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) Set the wallet whose canonical ATA will receive off-chain emissions.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_update_emissions_destination_account(
    accounts: &MarginfiAccountUpdateEmissionsDestinationAccount,
) -> Instruction {
    marginfi_account_update_emissions_destination_account_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_update_emissions_destination_account_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountUpdateEmissionsDestinationAccount,
) -> Instruction {
    let data = MarginfiAccountUpdateEmissionsDestinationAccount::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`transfer_to_new_account`].
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
    pub const DISCRIMINATOR: [u8; 8] = [28, 79, 129, 231, 169, 69, 69, 65];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for TransferToNewAccount {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) Transfer all positions to a new account under a new authority. The old
/// account is disabled. Pays a flat SOL fee to the protocol.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn transfer_to_new_account(accounts: &TransferToNewAccount) -> Instruction {
    transfer_to_new_account_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn transfer_to_new_account_with_program_id(
    program_id: Pubkey,
    accounts: &TransferToNewAccount,
) -> Instruction {
    let data = TransferToNewAccount::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`transfer_to_new_account_pda`].
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
    /// Instructions sysvar for CPI validation
    pub instructions_sysvar: Pubkey,
    pub system_program: Pubkey,
}
impl TransferToNewAccountPda {
    pub const DISCRIMINATOR: [u8; 8] = [172, 210, 224, 220, 146, 212, 253, 49];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
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
impl ToAccountMetas for TransferToNewAccountPda {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (account authority) Same as `transfer_to_new_account` except the resulting account is a PDA
///
/// seeds:
/// - marginfi_group
/// - authority: The account authority (owner)
/// - account_index: A u16 value to allow multiple accounts per authority
/// - third_party_id: Optional u16 for third-party tagging. Seeds < PDA_FREE_THRESHOLD can be
/// used freely. For a dedicated seed used by just your program (via CPI), contact us.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn transfer_to_new_account_pda(
    accounts: &TransferToNewAccountPda,
    account_index: u16,
    third_party_id: Option<u16>,
) -> Instruction {
    transfer_to_new_account_pda_with_program_id(crate::ID, accounts, account_index, third_party_id)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn transfer_to_new_account_pda_with_program_id(
    program_id: Pubkey,
    accounts: &TransferToNewAccountPda,
    account_index: u16,
    third_party_id: Option<u16>,
) -> Instruction {
    let mut data = TransferToNewAccountPda::DISCRIMINATOR.to_vec();
    account_index
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    third_party_id
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
