//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
use crate::constants::ix_discriminators;
use crate::types::{RebalanceMove, WrappedI80F48};
use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Fixed accounts for [`marginfi_account_close_rebalance_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountCloseRebalanceOrder {
    /// are validated in the handler.
    pub marginfi_account: Pubkey,
    /// Signs to close an order that still holds a position; omitted for the permissionless close of a
    /// dead order.
    pub authority: Option<Pubkey>,
    pub fee_recipient: Pubkey,
    pub rebalance_order: Pubkey,
}
impl MarginfiAccountCloseRebalanceOrder {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::native::MARGINFI_ACCOUNT_CLOSE_REBALANCE_ORDER;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(self.marginfi_account, false),
            match self.authority {
                Some(key) => AccountMeta::new_readonly(key, true),
                None => AccountMeta::new_readonly(program_id, false),
            },
            AccountMeta::new(self.fee_recipient, false),
            AccountMeta::new(self.rebalance_order, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountCloseRebalanceOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// Close an auto-rebalance order. The authority may cancel their own order at any time; anyone
/// may permissionlessly close a stale order once the account was closed or it no longer holds a
/// position in any allowed venue. Rent goes to `fee_recipient`.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close_rebalance_order(
    accounts: &MarginfiAccountCloseRebalanceOrder,
) -> Instruction {
    marginfi_account_close_rebalance_order_with_program_id(crate::ID, accounts)
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close_rebalance_order_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountCloseRebalanceOrder,
) -> Instruction {
    let data = MarginfiAccountCloseRebalanceOrder::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_end_rebalance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountEndRebalance {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub rebalance_order: Pubkey,
    pub rebalance_record: Pubkey,
    pub executor: Pubkey,
    pub fee_pool: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountEndRebalance {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::native::END_REBALANCE;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new(self.rebalance_order, false),
            AccountMeta::new(self.rebalance_record, false),
            AccountMeta::new(self.executor, true),
            AccountMeta::new(self.fee_pool, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountEndRebalance {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless keeper) End an auto-rebalance. Re-checks dst >= src post-move, value
/// conservation, untouched balances, and health; the order persists. The keeper tip is escrowed
/// into the record and paid later via `settle_rebalance_tip`.
///
/// Remaining accounts: the same bank blocks as `marginfi_account_start_rebalance`, in the same order, then each active bank followed by its oracle and integration accounts, for the health check.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_end_rebalance(accounts: &MarginfiAccountEndRebalance) -> Instruction {
    marginfi_account_end_rebalance_with_program_id(crate::ID, accounts)
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_end_rebalance_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountEndRebalance,
) -> Instruction {
    let data = MarginfiAccountEndRebalance::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_place_rebalance_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountPlaceRebalanceOrder {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub rebalance_order: Pubkey,
    pub fee_payer: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountPlaceRebalanceOrder {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::native::MARGINFI_ACCOUNT_PLACE_REBALANCE_ORDER;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new_readonly(self.mint, false),
            AccountMeta::new(self.rebalance_order, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountPlaceRebalanceOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Create a persistent same-mint auto-rebalance order: keep `mint` in the highest-yield
/// bank among `allowed_banks`. Persists until cancelled.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_place_rebalance_order(
    accounts: &MarginfiAccountPlaceRebalanceOrder,
    allowed_banks: Vec<Pubkey>,
    min_improvement: Option<WrappedI80F48>,
    cooldown_seconds: Option<u64>,
    amount: Option<u64>,
    keeper_tip: Option<u64>,
) -> Instruction {
    marginfi_account_place_rebalance_order_with_program_id(
        crate::ID,
        accounts,
        allowed_banks,
        min_improvement,
        cooldown_seconds,
        amount,
        keeper_tip,
    )
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_place_rebalance_order_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountPlaceRebalanceOrder,
    allowed_banks: Vec<Pubkey>,
    min_improvement: Option<WrappedI80F48>,
    cooldown_seconds: Option<u64>,
    amount: Option<u64>,
    keeper_tip: Option<u64>,
) -> Instruction {
    let mut data = MarginfiAccountPlaceRebalanceOrder::DISCRIMINATOR.to_vec();
    allowed_banks
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    min_improvement
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    cooldown_seconds
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    keeper_tip
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_settle_rebalance_tip`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountSettleRebalanceTip {
    pub group: Pubkey,
    /// record's `has_one` pins this key; `group` is pinned by the recorded banks, which must belong
    /// to it.
    pub marginfi_account: Pubkey,
    pub rebalance_record: Pubkey,
    /// `record.executor`.
    pub executor: Pubkey,
    pub fee_pool: Pubkey,
    /// The permissionless caller; pays the tx.
    pub caller: Pubkey,
}
impl MarginfiAccountSettleRebalanceTip {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::native::MARGINFI_ACCOUNT_SETTLE_REBALANCE_TIP;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new_readonly(self.marginfi_account, false),
            AccountMeta::new(self.rebalance_record, false),
            AccountMeta::new(self.executor, false),
            AccountMeta::new(self.fee_pool, false),
            AccountMeta::new(self.caller, true),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountSettleRebalanceTip {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Settle a rebalance's escrowed keeper tip after the settlement delay. Pays the
/// recorded keeper only if the destinations realized more yield than the sources over the window
/// (defeats cross-tx rate manipulation); otherwise refunds the tip to the fee pool. Closes the
/// record, returning its rent to the recorded keeper.
///
/// Remaining accounts: the same bank blocks as `marginfi_account_start_rebalance`, in the same order, without the reward accounts.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_settle_rebalance_tip(
    accounts: &MarginfiAccountSettleRebalanceTip,
) -> Instruction {
    marginfi_account_settle_rebalance_tip_with_program_id(crate::ID, accounts)
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_settle_rebalance_tip_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountSettleRebalanceTip,
) -> Instruction {
    let data = MarginfiAccountSettleRebalanceTip::DISCRIMINATOR.to_vec();
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_start_rebalance`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountStartRebalance {
    pub group: Pubkey,
    pub marginfi_account: Pubkey,
    pub rebalance_order: Pubkey,
    pub executor: Pubkey,
    pub rebalance_record: Pubkey,
    pub fee_payer: Pubkey,
    pub instruction_sysvar: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountStartRebalance {
    pub const DISCRIMINATOR: [u8; 8] = ix_discriminators::native::START_REBALANCE;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.group, false),
            AccountMeta::new(self.marginfi_account, false),
            AccountMeta::new_readonly(self.rebalance_order, false),
            AccountMeta::new_readonly(self.executor, false),
            AccountMeta::new(self.rebalance_record, false),
            AccountMeta::new(self.fee_payer, true),
            AccountMeta::new_readonly(self.instruction_sysvar, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountStartRebalance {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless keeper) Begin an auto-rebalance. `moves` declares each value relocation as
/// `(src_index, dst_index, amount)` over the banks passed in remaining_accounts; validates
/// same-mint, allowed venues, and every move's dst APR > src + min_improvement. Opens the
/// start/end sandwich; `end_rebalance` must be the last ix; CPI forbidden.
///
/// Remaining accounts: one block per bank in the order's allowlist; `moves` refer to blocks by index. Each block is the bank, then JupLend's `TokenReserve` for JupLend banks, then the venue's reward accounts, then the bank's oracle and integration accounts.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_start_rebalance(
    accounts: &MarginfiAccountStartRebalance,
    moves: Vec<RebalanceMove>,
    execution_seq: u64,
) -> Instruction {
    marginfi_account_start_rebalance_with_program_id(crate::ID, accounts, moves, execution_seq)
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_start_rebalance_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountStartRebalance,
    moves: Vec<RebalanceMove>,
    execution_seq: u64,
) -> Instruction {
    let mut data = MarginfiAccountStartRebalance::DISCRIMINATOR.to_vec();
    moves
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    execution_seq
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_top_up_rebalance_fee_pool`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountTopUpRebalanceFeePool {
    pub marginfi_account: Pubkey,
    pub fee_pool: Pubkey,
    pub payer: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountTopUpRebalanceFeePool {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::native::MARGINFI_ACCOUNT_TOP_UP_REBALANCE_FEE_POOL;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_account, false),
            AccountMeta::new(self.fee_pool, false),
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountTopUpRebalanceFeePool {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (permissionless) Fund an account's rebalance fee pool with SOL, used to pay keeper tips.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_top_up_rebalance_fee_pool(
    accounts: &MarginfiAccountTopUpRebalanceFeePool,
    amount: u64,
) -> Instruction {
    marginfi_account_top_up_rebalance_fee_pool_with_program_id(crate::ID, accounts, amount)
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_top_up_rebalance_fee_pool_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountTopUpRebalanceFeePool,
    amount: u64,
) -> Instruction {
    let mut data = MarginfiAccountTopUpRebalanceFeePool::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_update_rebalance_order`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountUpdateRebalanceOrder {
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub rebalance_order: Pubkey,
}
impl MarginfiAccountUpdateRebalanceOrder {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::native::MARGINFI_ACCOUNT_UPDATE_REBALANCE_ORDER;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.rebalance_order, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountUpdateRebalanceOrder {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Update an existing auto-rebalance order's allowlist and/or policy in place; `None`
/// fields are left unchanged.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_update_rebalance_order(
    accounts: &MarginfiAccountUpdateRebalanceOrder,
    allowed_banks: Option<Vec<Pubkey>>,
    min_improvement: Option<WrappedI80F48>,
    cooldown_seconds: Option<u64>,
    amount: Option<u64>,
    keeper_tip: Option<u64>,
) -> Instruction {
    marginfi_account_update_rebalance_order_with_program_id(
        crate::ID,
        accounts,
        allowed_banks,
        min_improvement,
        cooldown_seconds,
        amount,
        keeper_tip,
    )
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_update_rebalance_order_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountUpdateRebalanceOrder,
    allowed_banks: Option<Vec<Pubkey>>,
    min_improvement: Option<WrappedI80F48>,
    cooldown_seconds: Option<u64>,
    amount: Option<u64>,
    keeper_tip: Option<u64>,
) -> Instruction {
    let mut data = MarginfiAccountUpdateRebalanceOrder::DISCRIMINATOR.to_vec();
    allowed_banks
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    min_improvement
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    cooldown_seconds
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    keeper_tip
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}

/// Fixed accounts for [`marginfi_account_withdraw_rebalance_fee_pool`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarginfiAccountWithdrawRebalanceFeePool {
    pub marginfi_account: Pubkey,
    pub authority: Pubkey,
    pub fee_pool: Pubkey,
    pub destination: Pubkey,
    pub system_program: Pubkey,
}
impl MarginfiAccountWithdrawRebalanceFeePool {
    pub const DISCRIMINATOR: [u8; 8] =
        ix_discriminators::native::MARGINFI_ACCOUNT_WITHDRAW_REBALANCE_FEE_POOL;

    /// Account metas for `program_id`. A missing optional account is passed as `program_id`.
    pub fn to_account_metas_with_program_id(&self, _program_id: Pubkey) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.marginfi_account, false),
            AccountMeta::new_readonly(self.authority, true),
            AccountMeta::new(self.fee_pool, false),
            AccountMeta::new(self.destination, false),
            AccountMeta::new_readonly(self.system_program, false),
        ]
    }
}
impl ToAccountMetas for MarginfiAccountWithdrawRebalanceFeePool {
    fn to_account_metas(&self) -> Vec<AccountMeta> {
        self.to_account_metas_with_program_id(crate::ID)
    }
}
/// (user) Withdraw SOL from an account's rebalance fee pool back to the authority.
///
/// The returned instruction does not include remaining accounts.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_withdraw_rebalance_fee_pool(
    accounts: &MarginfiAccountWithdrawRebalanceFeePool,
    amount: u64,
) -> Instruction {
    marginfi_account_withdraw_rebalance_fee_pool_with_program_id(crate::ID, accounts, amount)
}

/// Builds the instruction for a program ID chosen at runtime.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_withdraw_rebalance_fee_pool_with_program_id(
    program_id: Pubkey,
    accounts: &MarginfiAccountWithdrawRebalanceFeePool,
    amount: u64,
) -> Instruction {
    let mut data = MarginfiAccountWithdrawRebalanceFeePool::DISCRIMINATOR.to_vec();
    amount
        .serialize(&mut data)
        .expect("serializing into Vec cannot fail");
    Instruction {
        program_id,
        accounts: accounts.to_account_metas_with_program_id(program_id),
        data,
    }
}
