use crate::{
    assert_struct_align, assert_struct_size,
    constants::{discriminators, BORROW_ORDER_FILL_GRANULE_BPS},
};
use fixed::types::I80F48;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use bytemuck::{Pod, Zeroable};

#[cfg(not(feature = "anchor"))]
use super::Pubkey;
use super::{u32_to_milli, ExecuteOrderBalanceRecord, WrappedI80F48, MAX_LENDING_ACCOUNT_BALANCES};

// A persistent intent to borrow `amount` from `bank` while the rate it has realized over the
// order's window sits under `open_below_apr`. Partial fills decrement and the order stays open.
assert_struct_size!(BorrowOrder, 256);
assert_struct_align!(BorrowOrder, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct BorrowOrder {
    pub marginfi_account: Pubkey,
    /// May modify or cancel the order.
    pub authority: Pubkey,
    /// The bank borrowed from, and whose rate the order watches.
    pub bank: Pubkey,
    /// Total to borrow across all fills, in native token units.
    pub amount: u64,
    /// Borrowed so far. `amount - filled` is what a further fill may take.
    pub filled: u64,
    /// Unix timestamp (seconds) of the last fill.
    pub last_exec_timestamp: i64,
    /// Borrow APR under which a fill may open, encoded via `milli_to_u32` (0-1000%).
    pub open_below_apr: u32,
    /// Borrow APR over which the position is repaid from the destination bank. Zero disables the
    /// close side; requires a destination bank.
    pub close_above_apr: u32,
    /// Minimum wall-clock seconds between fills.
    pub cooldown_seconds: u32,
    /// Shortest span the realized rate is measured over: the bank's youngest reading at least
    /// this old is the start of the span.
    pub window_seconds: u32,
    /// Bit 0 (`DESTINATION_WALLET`) - borrowed funds go to the authority's ATA.
    pub flags: u8,
    pub bump: u8,
    pub _pad0: [u8; 6],
    /// Same-mint bank the borrowed funds are deposited into, so they earn while the order holds
    /// them. Default when the funds go to the wallet.
    pub destination_bank: Pubkey,
    /// Liability shares in `bank` the order's fills hold, net of its closes. Interest included,
    /// this is the most a close may repay.
    pub liability_shares: WrappedI80F48,
    /// Lamports paid to the keeper from the account's fee pool for each open and each close, as far
    /// as the pool's spendable balance goes.
    pub keeper_tip: u64,
    _reserved0: [u64; 7],
}

impl BorrowOrder {
    pub const LEN: usize = core::mem::size_of::<BorrowOrder>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::BORROW_ORDER;
    /// `flags`: borrowed funds land in the authority's ATA.
    pub const DESTINATION_WALLET: u8 = 1 << 0;

    pub fn to_wallet(&self) -> bool {
        self.flags & Self::DESTINATION_WALLET != 0
    }

    /// Whether borrowed funds are redeployed into `destination_bank`.
    pub fn to_bank(&self) -> bool {
        self.destination_bank != Pubkey::default()
    }

    pub fn has_close_side(&self) -> bool {
        self.close_above_apr > 0
    }

    /// Native tokens a further fill may still take.
    pub fn remaining(&self) -> u64 {
        self.amount.saturating_sub(self.filled)
    }

    /// Whether `cooldown_seconds` has passed since the last fill.
    pub fn cooldown_elapsed(&self, now: i64) -> bool {
        self.last_exec_timestamp == 0
            || now.saturating_sub(self.last_exec_timestamp) >= i64::from(self.cooldown_seconds)
    }

    /// Whether `apr` sits under the level a fill may open at.
    pub fn opens_at(&self, apr: I80F48) -> bool {
        apr < u32_to_milli(self.open_below_apr)
    }

    /// Whether `apr` has risen over the level the position is repaid at.
    pub fn closes_at(&self, apr: I80F48) -> bool {
        self.has_close_side() && apr > u32_to_milli(self.close_above_apr)
    }

    /// The balances a fill may move: the borrow bank, and the destination when redeploying.
    pub fn banks(&self) -> Vec<Pubkey> {
        let mut banks = vec![self.bank];
        if self.to_bank() {
            banks.push(self.destination_bank);
        }
        banks
    }

    /// Liability shares a close may repay: what the order's fills hold, capped at what the account
    /// still holds in the bank.
    pub fn closable_shares(&self, account_shares: I80F48) -> I80F48 {
        I80F48::from(self.liability_shares).min(account_shares)
    }

    /// The slack a fill is allowed: `BORROW_ORDER_FILL_GRANULE_BPS` of `amount`, at least one unit.
    pub fn granule(&self) -> u64 {
        let granule = u128::from(self.amount) * u128::from(BORROW_ORDER_FILL_GRANULE_BPS) / 10_000;
        u64::try_from(granule).unwrap_or(u64::MAX).max(1)
    }

    /// The least a close may repay when `reachable` is what the destination can put toward the
    /// debt: all of it up to a granule, otherwise all but one granule.
    pub fn close_floor(&self, reachable: I80F48) -> u64 {
        let reachable: u64 = reachable.floor().to_num();
        let granule = self.granule();
        reachable
            .saturating_sub(granule)
            .max(reachable.min(granule))
    }
}

// Per-fill state a `start_borrow_order_*` records so the matching `end` can prove what actually
// moved. Created and closed inside one transaction, so none exist between them.
assert_struct_size!(BorrowOrderRecord, 1048);
assert_struct_align!(BorrowOrderRecord, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct BorrowOrderRecord {
    pub order: Pubkey,
    pub executor: Pubkey,
    /// Liability shares in the borrow bank before the fill.
    pub pre_liability_shares: WrappedI80F48,
    /// Asset shares in the destination bank before the fill. Zero when funds go to the wallet.
    pub pre_destination_shares: WrappedI80F48,
    /// The realized borrow APR that opened the fill.
    pub realized_apr: WrappedI80F48,
    /// The borrow bank's `collected_premium_outstanding` before the fill; a close's repay settles
    /// premium ahead of principal.
    pub pre_collected_premium: WrappedI80F48,
    /// Every active balance other than the order's two banks, as it stood at start. `end` requires
    /// each to be untouched and no balance outside the two banks to have been added.
    pub balance_states: [ExecuteOrderBalanceRecord; MAX_LENDING_ACCOUNT_BALANCES],
    pub active_balance_count: u8,
    /// `RECORD_OPEN` or `RECORD_CLOSE`: which sandwich this record belongs to.
    pub kind: u8,
    pub _pad0: [u8; 6],
    _reserved0: [u8; 16],
}

impl BorrowOrderRecord {
    pub const LEN: usize = core::mem::size_of::<BorrowOrderRecord>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::BORROW_ORDER_RECORD;
    pub const RECORD_OPEN: u8 = 0;
    pub const RECORD_CLOSE: u8 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::milli_to_u32;

    fn order(open_bps: u32, close_bps: Option<u32>) -> BorrowOrder {
        let milli = |bps: u32| milli_to_u32(I80F48::from_num(bps) / I80F48::from_num(10_000));
        let mut o = BorrowOrder::zeroed();
        o.amount = 1_000;
        o.open_below_apr = milli(open_bps);
        o.close_above_apr = close_bps.map(milli).unwrap_or(0);
        o
    }

    /// Both levels are strict, so a rate sitting exactly on one neither opens nor closes, and the
    /// gap between them is where the order rests.
    #[test]
    fn the_open_and_close_levels_are_strict() {
        let o = order(500, Some(1_500));
        let open = u32_to_milli(o.open_below_apr);
        let close = u32_to_milli(o.close_above_apr);
        assert!(!o.opens_at(open));
        assert!(o.opens_at(open - I80F48::DELTA));
        assert!(!o.closes_at(close));
        assert!(o.closes_at(close + I80F48::DELTA));
        assert!(!o.opens_at(open + I80F48::DELTA) && !o.closes_at(close - I80F48::DELTA));

        // No close level configured: no rate closes.
        assert!(!order(500, None).closes_at(I80F48::from_num(10)));
    }

    #[test]
    fn the_cooldown_gates_from_the_last_fill_only() {
        let mut o = order(500, None);
        o.cooldown_seconds = 600;
        assert!(o.cooldown_elapsed(5));
        o.last_exec_timestamp = 1_000;
        assert!(!o.cooldown_elapsed(1_599));
        assert!(o.cooldown_elapsed(1_600));
    }

    #[test]
    fn a_close_is_capped_by_the_order_and_by_the_account() {
        let mut o = order(500, None);
        o.liability_shares = I80F48::from_num(1_000).into();
        // The account owes more than the order opened: only the order's share may close.
        assert_eq!(
            o.closable_shares(I80F48::from_num(1_500)),
            I80F48::from_num(1_000)
        );
        // The user repaid some of it by hand: only what is left may close.
        assert_eq!(
            o.closable_shares(I80F48::from_num(250)),
            I80F48::from_num(250)
        );
    }

    #[test]
    fn the_close_floor_leaves_at_most_a_granule_and_never_admits_dust() {
        let mut o = order(500, None);
        o.amount = 100_000;
        assert_eq!(o.granule(), 1_000);
        let f = |reachable: u64| o.close_floor(I80F48::from_num(reachable));
        assert_eq!(f(50_000), 49_000);
        // Within two granules the floor is a whole granule; under one it is everything reachable.
        assert_eq!(f(1_500), 1_000);
        assert_eq!(f(500), 500);
        assert_eq!(f(0), 0);
        // The granule keeps the amount's full precision, and a tiny order still has one unit.
        o.amount = 19_999;
        assert_eq!(o.granule(), 199);
        o.amount = 50;
        assert_eq!(o.granule(), 1);
    }
}
