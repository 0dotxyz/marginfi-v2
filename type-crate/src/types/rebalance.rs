use crate::{assert_struct_align, assert_struct_size, constants::discriminators};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use bytemuck::{Pod, Zeroable};

#[cfg(not(feature = "anchor"))]
use super::Pubkey;
use super::{ExecuteOrderBalanceRecord, WrappedI80F48, MAX_LENDING_ACCOUNT_BALANCES};

/// Maximum venues an auto-rebalance order may rotate across. Bounds the on-chain allowlist so the
/// order stays a fixed-size zero-copy account.
pub const MAX_ALLOWED_BANKS: usize = 8;

/// Distinct banks one atomic rebalance may reference across all its moves (the union of sources and
/// destinations). Bounds the record size and the per-execution account/compute cost; wider spreads
/// fall back to sequential executions across cooldowns. Bounded by the order's allowlist.
pub const MAX_REBALANCE_BANKS: usize = MAX_ALLOWED_BANKS;

/// Declared moves one atomic rebalance may carry. Each move relocates value from one referenced bank
/// to another; `end_rebalance` reconciles the declared amounts against real per-bank value deltas.
pub const MAX_REBALANCE_MOVES: usize = 8;

/// Balances a `RebalanceRecord` snapshots for the untouched-balance proof: every active balance not in
/// the referenced set. At least one source is always an active balance, so at most
/// `MAX_LENDING_ACCOUNT_BALANCES - 1` others remain.
pub const MAX_REBALANCE_RECORD_BALANCES: usize = MAX_LENDING_ACCOUNT_BALANCES - 1;

// Persistent "auto-rebalance" intent: keep one asset (`mint`) in the highest-yield venue among an
// allowlisted set. Unlike `Order`, it is NOT consumed on execution — it persists until cancelled.
// PDA: [REBALANCE_ORDER_SEED, marginfi_account, mint] -> at most one per (account, asset).
assert_struct_size!(RebalanceOrder, 408);
assert_struct_align!(RebalanceOrder, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct RebalanceOrder {
    /// The marginfi account this order belongs to.
    pub marginfi_account: Pubkey,
    /// The account authority (may cancel the order).
    pub authority: Pubkey,
    /// The single SPL mint this order rotates across venues; every referenced bank must hold this mint.
    pub mint: Pubkey,
    /// Venue allowlist: the first `allowed_bank_count` entries are the banks this order may rotate
    /// across; the rest are zero. Stored in full (not a hash) so a keeper discovers the set from a
    /// single account read and `start_rebalance` validates that every referenced bank ∈ list against
    /// on-chain state.
    pub allowed_banks: [Pubkey; MAX_ALLOWED_BANKS],
    /// Minimum required APR improvement (dst - src) to move, I80F48 (1.0 = 100%).
    pub min_improvement: WrappedI80F48,
    /// Minimum wall-clock seconds between executions (anti-ping-pong cooldown).
    pub cooldown_seconds: u64,
    /// Per-execution total value budget: each execution may relocate at most this much native-token
    /// value (priced in USD) summed across all referenced banks. `0` means no cap (unlimited).
    pub amount: u64,
    /// Unix timestamp (seconds) of the last successful rebalance.
    pub last_exec_timestamp: u64,
    /// PDA bump.
    pub bump: u8,
    /// Number of populated entries in `allowed_banks` (2..=MAX_ALLOWED_BANKS).
    pub allowed_bank_count: u8,
    pub _pad0: [u8; 6],
    /// Lamport tip a keeper earns for relocating the order's full target, paid proportionally to the
    /// value actually moved and drawn from the account's rebalance fee pool. 0 = no tip.
    pub keeper_tip: u64,
}

impl RebalanceOrder {
    pub const LEN: usize = core::mem::size_of::<RebalanceOrder>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::REBALANCE_ORDER;
}

// A bank referenced by a rebalance execution, with its equity (weight-1) USD value at start.
// `end_rebalance` recomputes the post value and reconciles the delta against the declared moves.
assert_struct_size!(RebalanceRefBank, 48);
assert_struct_align!(RebalanceRefBank, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct RebalanceRefBank {
    pub bank: Pubkey,
    pub pre_value: WrappedI80F48,
}

// A declared value move from `src_index` to `dst_index` (indices into `RebalanceRecord.ref_banks`),
// of `amount` equity USD value. The keeper declares these; `start_rebalance` requires each move's
// destination rate to beat its source by the order's margin, and `end_rebalance` re-checks the
// destination is not worse after market impact and reconciles the amounts against the observed
// per-bank value deltas.
assert_struct_size!(RebalanceMove, 24);
assert_struct_align!(RebalanceMove, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct RebalanceMove {
    pub src_index: u8,
    pub dst_index: u8,
    pub _pad0: [u8; 6],
    pub amount: WrappedI80F48,
}

// Transient per-execution record: created in `start_rebalance`, closed in `end_rebalance` (rent ->
// executor). Captures every referenced bank's start value, the declared moves, and a snapshot of every
// OTHER active balance, so end can reconcile the moves against real deltas, prove value conservation,
// and prove untouched balances kept the same side and shares.
assert_struct_size!(RebalanceRecord, 1504);
assert_struct_align!(RebalanceRecord, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct RebalanceRecord {
    pub order: Pubkey,
    pub executor: Pubkey,
    /// The distinct banks this execution touches (first `ref_bank_count` entries), with start values.
    pub ref_banks: [RebalanceRefBank; MAX_REBALANCE_BANKS],
    /// The declared value moves (first `move_count` entries), referencing `ref_banks` by index.
    pub moves: [RebalanceMove; MAX_REBALANCE_MOVES],
    /// Snapshot of every active balance NOT in the referenced set; end verifies these unchanged.
    pub balance_states: [ExecuteOrderBalanceRecord; MAX_REBALANCE_RECORD_BALANCES],
    pub ref_bank_count: u8,
    pub move_count: u8,
    pub active_balance_count: u8,
    pub _pad0: [u8; 5],
    pub _reserved: [u8; 16],
}

impl RebalanceRecord {
    pub const LEN: usize = core::mem::size_of::<RebalanceRecord>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::REBALANCE_RECORD;
}
