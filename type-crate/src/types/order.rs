use crate::{
    assert_struct_align, assert_struct_size,
    constants::{discriminators, ORDER_ACTIVE_TAGS, ORDER_TAG_PADDING},
};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use bytemuck::{Pod, Zeroable};

#[cfg(not(feature = "anchor"))]
use super::Pubkey;
use super::{WrappedI80F48, MAX_LENDING_ACCOUNT_BALANCES};

#[repr(u8)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Default)]
pub enum OrderTriggerType {
    #[default]
    StopLoss = 0,
    TakeProfit = 1,
    Both = 2,
}

unsafe impl Zeroable for OrderTriggerType {}
unsafe impl Pod for OrderTriggerType {}

#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorSerialize, AnchorDeserialize))]
#[derive(Debug, PartialEq, Copy, Clone, Eq)]
pub enum OrderTrigger {
    StopLoss {
        threshold: WrappedI80F48,
        max_slippage: u32,
    },
    TakeProfit {
        threshold: WrappedI80F48,
        max_slippage: u32,
    },
    Both {
        stop_loss: WrappedI80F48,
        take_profit: WrappedI80F48,
        max_slippage: u32,
    },
}

assert_struct_size!(Order, 256);
assert_struct_align!(Order, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy), derive(Default, PartialEq, Eq))]
#[cfg_attr(not(feature = "anchor"), derive(Zeroable))]
#[derive(Debug)]
pub struct Order {
    pub marginfi_account: Pubkey,
    pub stop_loss: WrappedI80F48,
    pub take_profit: WrappedI80F48,
    /// Reserved for future use
    pub placeholder: u64,
    /// * a %, as u32, out of 100%, e.g. 50% = .5 * u32::MAX
    pub max_slippage: u32,
    pub pad0: [u8; 4],

    /// Active tags (currently 2). Remaining capacity is stored in padding for layout compatibility.
    /// Padding byte `ORDER_TAG_PADDING - 1` stores the tag count for forward compatibility. (u16 *
    /// 2 = 4 bytes)
    pub tags: [u16; ORDER_ACTIVE_TAGS],
    pub pad1: [u8; 4],
    // Note: if ever adding support for additional tags in the future, use this buffer space to
    // expand the tags slice, which should ensure older orders are backwards compatible.
    _tags_padding: [u8; ORDER_TAG_PADDING],

    /// Stop Loss (0), Take Profit (1), or Both (2)
    pub trigger: OrderTriggerType,
    /// Bump to derive this pda
    pub bump: u8,
    pub pad2: [u8; 6],
    _reserved1: [[u8; 32]; 4],
}

impl Order {
    pub const LEN: usize = core::mem::size_of::<Order>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::ORDER;
}

// The execution record does not store order balances and each order
// has at least 2 balances
pub const MAX_EXECUTE_RECORD_BALANCES: usize = MAX_LENDING_ACCOUNT_BALANCES - 2;

// Records key information about the account during order execution.
// It is closed after the order completes with funds returned to the executor.
assert_struct_size!(ExecuteOrderRecord, 872);
assert_struct_align!(ExecuteOrderRecord, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Default, Debug, PartialEq, Pod, Zeroable, Copy, Clone)
)]
pub struct ExecuteOrderRecord {
    pub order: Pubkey,
    pub executor: Pubkey,
    pub balance_states: [ExecuteOrderBalanceRecord; MAX_EXECUTE_RECORD_BALANCES],
    pub active_balance_count: u8,
    pub inactive_balance_count: u8,
    _reserved0: [u8; 6],
    pub order_start_health: WrappedI80F48,
}

// This is used to ensure the balance state after execution stays the same.
assert_struct_size!(ExecuteOrderBalanceRecord, 56);
assert_struct_align!(ExecuteOrderBalanceRecord, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct ExecuteOrderBalanceRecord {
    pub bank: Pubkey,
    pub is_asset: u8,
    _pad0: [u8; 5],
    pub tag: u16,
    pub shares: WrappedI80F48,
}

impl ExecuteOrderRecord {
    pub const LEN: usize = core::mem::size_of::<ExecuteOrderRecord>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::EXECUTE_ORDER_RECORD;
}
