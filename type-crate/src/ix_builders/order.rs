use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::OrderTrigger;
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Accounts for [`marginfi_account_place_order`].
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
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_PLACE_ORDER;
}

impl ToAccountMetas for MarginfiAccountPlaceOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
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

/// (user) Create a new Order.
/// * bank_keys - Currently only two keys: the lending position and borrowing position in the
///   users's Balances for which the order is being placed
/// * trigger - the type of order (stop loss, take profit, or both), and the threshold at which
///   to trigger the order, in dollars
pub fn marginfi_account_place_order(
    accounts: &MarginfiAccountPlaceOrder,
    bank_keys: Vec<Pubkey>,
    trigger: OrderTrigger,
) -> Instruction {
    let mut data = MarginfiAccountPlaceOrder::DISCRIMINATOR.to_vec();
    bank_keys.serialize(&mut data).unwrap();
    trigger.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`marginfi_account_close_order`].
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
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_CLOSE_ORDER;
}

impl ToAccountMetas for MarginfiAccountCloseOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
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

/// (user) Close an existing Order, returning rent to the user
pub fn marginfi_account_close_order(accounts: &MarginfiAccountCloseOrder) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountCloseOrder::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`marginfi_account_keeper_close_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountKeeperCloseOrder {
    pub marginfi_account: Pubkey,
    pub fee_recipient: Pubkey,
    pub order: Pubkey,
}

impl MarginfiAccountKeeperCloseOrder {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_KEEPER_CLOSE_ORDER;
}

impl ToAccountMetas for MarginfiAccountKeeperCloseOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.fee_recipient, false),
            AccountMeta::new(self.order, false),
        ]
    }
}

/// (permissionless keeper) Close an existing Order after the user account was closed, or it no
/// longer has the associated positions, or the user has executed
/// `marginfi_account_set_keeper_close_flags`. Keeper keeps the rent.
pub fn marginfi_account_keeper_close_order(
    accounts: &MarginfiAccountKeeperCloseOrder,
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountKeeperCloseOrder::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`marginfi_account_set_keeper_close_flags`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountSetKeeperCloseFlags {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
}

impl MarginfiAccountSetKeeperCloseFlags {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::MARGINFI_ACCOUNT_SET_KEEPER_CLOSE_FLAGS;
}

impl ToAccountMetas for MarginfiAccountSetKeeperCloseFlags {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
        ]
    }
}

/// (user) Purge flags from some balances, enabling a Keeper to call
/// `marginfi_account_keeper_close_order` on associated Orders. Typically, use
/// `marginfi_account_close_order` instead if trying to close an Order.
pub fn marginfi_account_set_keeper_close_flags(
    accounts: &MarginfiAccountSetKeeperCloseFlags,
    bank_keys_opt: Option<Vec<Pubkey>>,
) -> Instruction {
    let mut data = MarginfiAccountSetKeeperCloseFlags::DISCRIMINATOR.to_vec();
    bank_keys_opt.serialize(&mut data).unwrap();
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data,
    }
}

/// Accounts for [`marginfi_account_start_execute_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountStartExecuteOrder {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub fee_payer: Pubkey,
    pub executor: Pubkey,
    pub order: Pubkey,
    pub execute_record: Pubkey,
    pub instruction_sysvar: Pubkey,
    pub system_program: Pubkey,
}

impl MarginfiAccountStartExecuteOrder {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::START_EXECUTE_ORDER;
}

impl ToAccountMetas for MarginfiAccountStartExecuteOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
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

/// (permissionless keeper) Begin Order execution
/// * Enables the Keeper to withdraw/repay associated positions until the end of the tx
/// * Only one `StartExecuteOrder` is allowed per tx
/// * Must appear before `EndExecuteOrder` in the tx, and before any instructions except certain
///   allowed ones (compute budget, kamino refresh, etc)
/// * `EndExecuteOrder` must also appear in the tx
/// * CPI is forbidden
/// * Costs a small amount of rent, which is returned at the end of the tx, make sure you have
///   enough SOL to start the tx.
pub fn marginfi_account_start_execute_order(
    accounts: &MarginfiAccountStartExecuteOrder,
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountStartExecuteOrder::DISCRIMINATOR.to_vec(),
    }
}

/// Accounts for [`marginfi_account_end_execute_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountEndExecuteOrder {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub executor: Pubkey,
    pub fee_recipient: Pubkey,
    pub order: Pubkey,
    pub execute_record: Pubkey,
    pub fee_state: Pubkey,
}

impl MarginfiAccountEndExecuteOrder {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::END_EXECUTE_ORDER;
}

impl ToAccountMetas for MarginfiAccountEndExecuteOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
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

/// (permissionless keeper) End Order execution
/// * Closes the Order (keeper keeps the rent)
/// * Closes the borrow position involved in the Order, the lending position remains open
/// * User health must be "unchanged" (within Order requirements i.e. minus slippage). Keeper
///   may keep any slippage in excess of what was needed to complete the Order as profit.
/// * `StartExecuteOrder` must appear earlier in the tx
/// * Must appear last in the tx
/// * CPI is forbidden
/// * Returns rent for ephemeral accounts created during `StartExecuteOrder`
pub fn marginfi_account_end_execute_order(
    accounts: &MarginfiAccountEndExecuteOrder,
) -> Instruction {
    Instruction {
        program_id: crate::ID,
        accounts: accounts.to_account_metas(),
        data: MarginfiAccountEndExecuteOrder::DISCRIMINATOR.to_vec(),
    }
}
