//! Instruction builders generated from the marginfi IDL.
#![allow(clippy::doc_lazy_continuation)]

use super::ToAccountMetas;
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
    pub const DISCRIMINATOR: [u8; 8] = [218, 239, 199, 221, 92, 158, 134, 62];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
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
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_close_rebalance_order(
    accounts: &MarginfiAccountCloseRebalanceOrder,
) -> Instruction {
    marginfi_account_close_rebalance_order_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
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
    pub const DISCRIMINATOR: [u8; 8] = [47, 225, 163, 216, 213, 214, 225, 155];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
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
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_end_rebalance(accounts: &MarginfiAccountEndRebalance) -> Instruction {
    marginfi_account_end_rebalance_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
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
    pub const DISCRIMINATOR: [u8; 8] = [161, 48, 11, 214, 229, 132, 192, 149];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
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
/// Append any required remaining accounts after the fixed account metas.
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

/// Build the instruction for a program ID selected at runtime.
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
    pub const DISCRIMINATOR: [u8; 8] = [198, 13, 185, 127, 7, 105, 88, 182];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
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
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_settle_rebalance_tip(
    accounts: &MarginfiAccountSettleRebalanceTip,
) -> Instruction {
    marginfi_account_settle_rebalance_tip_with_program_id(crate::ID, accounts)
}

/// Build the instruction for a program ID selected at runtime.
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
    pub const DISCRIMINATOR: [u8; 8] = [251, 122, 91, 161, 219, 98, 5, 236];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
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
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_start_rebalance(
    accounts: &MarginfiAccountStartRebalance,
    moves: Vec<RebalanceMove>,
    execution_seq: u64,
) -> Instruction {
    marginfi_account_start_rebalance_with_program_id(crate::ID, accounts, moves, execution_seq)
}

/// Build the instruction for a program ID selected at runtime.
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
    pub const DISCRIMINATOR: [u8; 8] = [83, 123, 125, 204, 200, 119, 247, 91];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
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
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_top_up_rebalance_fee_pool(
    accounts: &MarginfiAccountTopUpRebalanceFeePool,
    amount: u64,
) -> Instruction {
    marginfi_account_top_up_rebalance_fee_pool_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
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
    pub const DISCRIMINATOR: [u8; 8] = [175, 230, 251, 122, 168, 128, 207, 142];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
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
/// Append any required remaining accounts after the fixed account metas.
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

/// Build the instruction for a program ID selected at runtime.
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
    pub const DISCRIMINATOR: [u8; 8] = [121, 157, 246, 242, 240, 63, 254, 16];

    /// Construct account metas for a specific program ID, including optional-account sentinels.
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
/// Append any required remaining accounts after the fixed account metas.
#[allow(clippy::too_many_arguments)]
pub fn marginfi_account_withdraw_rebalance_fee_pool(
    accounts: &MarginfiAccountWithdrawRebalanceFeePool,
    amount: u64,
) -> Instruction {
    marginfi_account_withdraw_rebalance_fee_pool_with_program_id(crate::ID, accounts, amount)
}

/// Build the instruction for a program ID selected at runtime.
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
