//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::types::OrderTrigger;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`marginfi_account_close_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountCloseOrder {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub order: Pubkey,
    pub fee_recipient: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountCloseOrder {
    pub const DISCRIMINATOR: [u8; 8] = [212, 223, 79, 182, 172, 183, 205, 237];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.order, false),
            AccountMeta::new(self.fee_recipient, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountCloseOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Close an existing Order, returning rent to the user
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close_order(accounts: &MarginfiAccountCloseOrder) -> Instruction {
    marginfi_account_close_order_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close_order_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountCloseOrder,
) -> Instruction {
    let data = MarginfiAccountCloseOrder::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_end_execute_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountEndExecuteOrder {
    pub group: Pubkey,
    /// The account owning the order
    pub marginfi_account: Pubkey,
    /// The executioner ☠️
    pub executor: Pubkey,
    pub fee_recipient: Pubkey,
    pub order: Pubkey,
    /// This keeps track of the relevant state to be checked at the end of execution.
    pub execute_record: Pubkey,
    pub fee_state: Pubkey,
}
impl MarginfiAccountEndExecuteOrder {
    pub const DISCRIMINATOR: [u8; 8] = [115, 42, 20, 93, 121, 84, 178, 83];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.executor, true),
            AccountMeta::new(self.fee_recipient, false),
            AccountMeta::new(self.order, false),
            AccountMeta::new(self.execute_record, false),
            AccountMeta::new_readonly(self.fee_state, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountEndExecuteOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless keeper) End Order execution
/// * Closes the Order (keeper keeps the rent)
/// * Closes the borrow position involved in the Order, the lending position remains open
/// * User health must be "unchanged" (within Order requirements i.e. minus slippage). Keeper
/// may keep any slippage in excess of what was needed to complete the Order as profit.
/// * `StartExecuteOrder` must appear earlier in the tx
/// * Must appear last in the tx
/// * CPI is forbidden
/// * Returns rent for ephemeral accounts created during `StartExecuteOrder`
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_end_execute_order(
    accounts: &MarginfiAccountEndExecuteOrder,
) -> Instruction {
    marginfi_account_end_execute_order_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_end_execute_order_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountEndExecuteOrder,
) -> Instruction {
    let data = MarginfiAccountEndExecuteOrder::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_keeper_close_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountKeeperCloseOrder {
    /// marginfi account was closed.
    /// The ownership check is checked in the handler or/and type checks are made in the handler.
    pub marginfi_account: Pubkey,
    pub fee_recipient: Pubkey,
    pub order: Pubkey,
}
impl MarginfiAccountKeeperCloseOrder {
    pub const DISCRIMINATOR: [u8; 8] = [128, 114, 71, 46, 194, 71, 186, 106];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.fee_recipient, false),
            AccountMeta::new(self.order, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountKeeperCloseOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless keeper) Close an existing Order after the user account was closed, or it no
/// longer has the associated positions, or the user has executed
/// `marginfi_account_set_keeper_close_flags`. Keeper keeps the rent.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_keeper_close_order(
    accounts: &MarginfiAccountKeeperCloseOrder,
) -> Instruction {
    marginfi_account_keeper_close_order_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_keeper_close_order_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountKeeperCloseOrder,
) -> Instruction {
    let data = MarginfiAccountKeeperCloseOrder::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_place_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountPlaceOrder {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub fee_payer: Pubkey,
    pub authority: Pubkey,
    pub order: Pubkey,
    pub fee_state: Pubkey,
    pub global_fee_wallet: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountPlaceOrder {
    pub const DISCRIMINATOR: [u8; 8] = [244, 112, 75, 138, 143, 108, 7, 186];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.order, false),
            AccountMeta::new_readonly(self.fee_state, false),
            AccountMeta::new(self.global_fee_wallet, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountPlaceOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Create a new Order.
/// * bank_keys - Currently only two keys: the lending position and borrowing position in the
/// users's Balances for which the order is being placed
/// * trigger - the type of order (stop loss, take profit, or both), and the threshold at which
/// to trigger the order, in dollars
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_place_order(
    accounts: &MarginfiAccountPlaceOrder,
    bank_keys: Vec<Pubkey>,
    trigger: OrderTrigger,
) -> Instruction {
    marginfi_account_place_order_with_program_id(crate::ID, accounts, bank_keys, trigger)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_place_order_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountPlaceOrder,
    bank_keys: Vec<Pubkey>,
    trigger: OrderTrigger,
) -> Instruction {
    let mut data = MarginfiAccountPlaceOrder::DISCRIMINATOR.to_vec();
    bank_keys
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    trigger
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_set_keeper_close_flags`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountSetKeeperCloseFlags {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
}
impl MarginfiAccountSetKeeperCloseFlags {
    pub const DISCRIMINATOR: [u8; 8] = [82, 163, 165, 222, 212, 255, 33, 210];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountSetKeeperCloseFlags {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Purge flags from some balances, enabling a Keeper to call
/// `marginfi_account_keeper_close_order` on associated Orders. Typically, use
/// `marginfi_account_close_order` instead if trying to close an Order.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_set_keeper_close_flags(
    accounts: &MarginfiAccountSetKeeperCloseFlags,
    bank_keys_opt: Option<Vec<Pubkey>>,
) -> Instruction {
    marginfi_account_set_keeper_close_flags_with_program_id(crate::ID, accounts, bank_keys_opt)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_set_keeper_close_flags_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountSetKeeperCloseFlags,
    bank_keys_opt: Option<Vec<Pubkey>>,
) -> Instruction {
    let mut data = MarginfiAccountSetKeeperCloseFlags::DISCRIMINATOR.to_vec();
    bank_keys_opt
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_start_execute_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountStartExecuteOrder {
    pub group: Pubkey,
    /// The account owning the order
    pub marginfi_account: Pubkey,
    pub fee_payer: Pubkey,
    /// This account will have the authority to withdraw/repay as if they are the user authority
    /// until the end of the tx.
    ///
    pub executor: Pubkey,
    pub order: Pubkey,
    /// This keeps track of the relevant state to be checked at the end of execution.
    pub execute_record: Pubkey,
    pub instruction_sysvar: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountStartExecuteOrder {
    pub const DISCRIMINATOR: [u8; 8] = [1, 70, 140, 134, 183, 29, 208, 224];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.executor, false),
            AccountMeta::new(self.order, false),
            AccountMeta::new(self.execute_record, false),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountStartExecuteOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless keeper) Begin Order execution
/// * Enables the Keeper to withdraw/repay associated positions until the end of the tx
/// * Only one `StartExecuteOrder` is allowed per tx
/// * Must appear before `EndExecuteOrder` in the tx, and before any instructions except certain
/// allowed ones (compute budget, kamino refresh, etc)
/// * `EndExecuteOrder` must also appear in the tx
/// * CPI is forbidden
/// * Costs a small amount of rent, which is returned at the end of the tx, make sure you have
/// enough SOL to start the tx.
///
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_start_execute_order(
    accounts: &MarginfiAccountStartExecuteOrder,
) -> Instruction {
    marginfi_account_start_execute_order_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_start_execute_order_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountStartExecuteOrder,
) -> Instruction {
    let data = MarginfiAccountStartExecuteOrder::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
