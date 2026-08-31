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
    StopLoss, // 0
    TakeProfit, // 1
    Both,       // 2
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

/// Optional carry-exit policy on an `Order`, orthogonal to its price trigger. `None` fields take
/// the matching `INTEREST_DEFAULT_*` constant.
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorSerialize, AnchorDeserialize))]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct InterestTriggerConfig {
    /// Shortest span the realized rates may be measured over.
    pub window_seconds: Option<u32>,
    /// The carry loss over this span is the budget the realized unwind cost must fit in.
    pub patience_seconds: Option<u32>,
    /// Annualized loss, against the lend leg, required to arm. `None` arms on any negative carry.
    pub min_negative_apr: Option<u32>,
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
    /// Unix timestamp (seconds) when the order was created. Reads 0 for orders created before this
    /// field existed (it was previously a reserved placeholder; same 8 bytes, so layout-compatible).
    pub created_at: i64,
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

    /// Asset-leg yield index when the anchor was taken: `asset_share_value` times the venue
    /// exchange multiplier (1 for native banks).
    pub interest_anchor_asset_index: WrappedI80F48,
    /// Liability-leg index when the anchor was taken: `liability_share_value`. Borrow banks are
    /// always native, so no multiplier applies.
    pub interest_anchor_debt_index: WrappedI80F48,
    /// Unix timestamp (seconds) the anchor indices were read at. Zero until the order is armed.
    pub interest_anchor_timestamp: i64,
    /// Shortest span the realized rates may be measured over, and the cadence at which the anchor
    /// may be advanced.
    pub interest_window_seconds: u32,
    /// The carry loss accrued over this span is the budget the realized unwind cost must fit in.
    pub interest_patience_seconds: u32,
    /// Annualized loss, against the lend leg, required to arm the trigger. Encoded like the
    /// interest-curve points via `milli_to_u32` (0-1000%). Zero arms on any negative carry.
    pub interest_min_negative_apr: u32,
    /// Bit 0 (`INTEREST_TRIGGER_ENABLED`) - the carry condition is active on this order.
    pub interest_flags: u8,
    pub pad3: [u8; 3],

    /// The anchor the current one displaced. Arming rotates, so a fresh arm cannot reset a
    /// measurement that had already come of age.
    pub interest_prev_asset_index: WrappedI80F48,
    pub interest_prev_debt_index: WrappedI80F48,
    /// Unix timestamp (seconds) of the displaced anchor. Zero before the first rotation.
    pub interest_prev_timestamp: i64,

    _reserved1: [[u8; 32]; 1],
}

impl Order {
    pub const LEN: usize = core::mem::size_of::<Order>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::ORDER;
    /// `interest_flags`: the carry condition is active alongside the price trigger.
    pub const INTEREST_TRIGGER_ENABLED: u8 = 1 << 0;

    pub fn interest_trigger_enabled(&self) -> bool {
        self.interest_flags & Self::INTEREST_TRIGGER_ENABLED != 0
    }
}

// The execution record does not store order balances and each order
// has at least 2 balances
pub const MAX_EXECUTE_RECORD_BALANCES: usize = MAX_LENDING_ACCOUNT_BALANCES - 2;

// Records key information about the account during order execution.
// It is closed after the order completes with funds returned to the executor.
assert_struct_size!(ExecuteOrderRecord, 888);
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
    /// `MET_*` bit flags: the order conditions satisfied at `start_execute_order`.
    pub met_conditions: u8,
    _reserved0: [u8; 5],
    pub order_start_health: WrappedI80F48,
    /// Net carry in USD per year at start, negative when the pair loses to interest. The end gate
    /// reads it because the closed liability leg makes it unrecomputable there.
    pub interest_carry: WrappedI80F48,
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
    /// `met_conditions`: the stop-loss or take-profit threshold was crossed.
    pub const MET_PRICE: u8 = 1 << 0;
    /// `met_conditions`: the carry condition was satisfied.
    pub const MET_INTEREST: u8 = 1 << 1;

    pub fn met_price(&self) -> bool {
        self.met_conditions & Self::MET_PRICE != 0
    }

    pub fn met_interest(&self) -> bool {
        self.met_conditions & Self::MET_INTEREST != 0
    }
}
