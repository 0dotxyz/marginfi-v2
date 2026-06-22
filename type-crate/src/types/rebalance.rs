use crate::{assert_struct_align, assert_struct_size, constants::discriminators};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use bytemuck::{Pod, Zeroable};

#[cfg(not(feature = "anchor"))]
use super::Pubkey;
use super::{ExecuteOrderBalanceRecord, WrappedI80F48};

/// Maximum venues an auto-rebalance order may rotate across. Bounds the on-chain allowlist so the
/// order stays a fixed-size zero-copy account.
pub const MAX_ALLOWED_BANKS: usize = 8;

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
    /// The single SPL mint this order rotates across venues. src.mint == dst.mint == this.
    pub mint: Pubkey,
    /// Venue allowlist: the first `allowed_bank_count` entries are the banks this order may rotate
    /// across; the rest are zero. Stored in full (not a hash) so a keeper discovers the set from a
    /// single account read and `rebalance_start` validates src,dst ∈ list against on-chain state.
    pub allowed_banks: [Pubkey; MAX_ALLOWED_BANKS],
    /// Minimum required APR improvement (dst - src) to move, I80F48 (1.0 = 100%).
    pub min_improvement: WrappedI80F48,
    /// Minimum wall-clock seconds between executions (anti-ping-pong cooldown).
    pub cooldown_seconds: u64,
    /// Native token amount this order keeps in the best venue: each execution moves up to this much
    /// out of the current source bank into the destination. `0` means the entire source position
    /// (unlimited, up to what is deposited).
    pub amount: u64,
    /// Unix timestamp (seconds) of the last successful rebalance.
    pub last_exec_timestamp: u64,
    /// PDA bump.
    pub bump: u8,
    /// Number of populated entries in `allowed_banks` (2..=MAX_ALLOWED_BANKS).
    pub allowed_bank_count: u8,
    pub _pad0: [u8; 6],
    pub _reserved: [u8; 8],
}

impl RebalanceOrder {
    pub const LEN: usize = core::mem::size_of::<RebalanceOrder>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::REBALANCE_ORDER;
}

// Transient per-execution record: created in `rebalance_start`, closed in `rebalance_end` (rent ->
// executor). Captures the {src,dst} pre-value + a snapshot of every OTHER active balance so the end
// instruction can prove value conservation and that untouched balances are byte-identical.
assert_struct_size!(RebalanceRecord, 1000);
assert_struct_align!(RebalanceRecord, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct RebalanceRecord {
    pub order: Pubkey,
    pub executor: Pubkey,
    pub src_bank: Pubkey,
    pub dst_bank: Pubkey,
    /// Equity (weight-1) USD value of the src position at start. `pre_src + pre_dst` is the
    /// conservation baseline; the drop `pre_src - post_src` must match the order's `amount`
    /// (the whole position when the order is unlimited).
    pub pre_src_value: WrappedI80F48,
    /// Equity (weight-1) USD value of the dst position at start.
    pub pre_dst_value: WrappedI80F48,
    pub src_rate_pre: WrappedI80F48,
    pub dst_rate_pre: WrappedI80F48,
    /// Snapshot of every non-{src,dst} active balance; end verifies these unchanged (side+shares).
    pub balance_states: [ExecuteOrderBalanceRecord; 14],
    pub active_balance_count: u8,
    pub inactive_balance_count: u8,
    pub _pad0: [u8; 6],
    pub _reserved: [u8; 16],
}

impl RebalanceRecord {
    pub const LEN: usize = core::mem::size_of::<RebalanceRecord>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::REBALANCE_RECORD;
}
