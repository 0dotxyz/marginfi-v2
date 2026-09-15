use bytemuck::{Pod, Zeroable};
use fixed::types::I80F48;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use crate::{
    assert_struct_align, assert_struct_size,
    constants::SECONDS_PER_YEAR,
    types::{u32_to_milli, Balance, MarginfiGroup},
};

/// Maximum pairwise premium entries storable in `MarginfiGroup.premium_entries`. Future
/// capacity growth carves from the group's `_padding_2` region and raises
/// `PremiumSettings.entry_capacity`.
pub const MAX_PREMIUM_ENTRIES: usize = 64;
/// A `premium_tag` of 0 is untagged: it never matches any premium entry.
pub const PREMIUM_TAG_EMPTY: u16 = 0;
/// Maximum configurable premium APR for a pair: 100%, in the `milli_to_u32` encoding
/// (`u32::MAX` = 1000%, so 100% = `u32::MAX / 10`). The encoding ceiling of 1000% is
/// deliberately not exposed to the emode admin.
pub const MAX_PREMIUM_RATE: u32 = u32::MAX / 10;

assert_struct_size!(PremiumSettings, 32);
assert_struct_align!(PremiumSettings, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone, Eq)]
/// Header for the group's pairwise variable-borrow premium matrix.
/// * `entry_count > 0` is the single source of truth for whether the matrix is configured.
pub struct PremiumSettings {
    /// Unix timestamp from the system clock when the premium matrix was last updated.
    pub timestamp: i64,
    /// Number of live entries at the start of `premium_entries`. 0 = matrix off.
    pub entry_count: u16,
    /// Storage capacity for entries. `MAX_PREMIUM_ENTRIES` for groups at the current account
    /// size; a future group-account resize may raise this.
    pub entry_capacity: u16,
    // Pad to next 8-byte multiple
    pub _pad0: [u8; 4],
    /// Reserved for future use
    pub _reserved0: [u64; 2],
}

impl Default for PremiumSettings {
    fn default() -> Self {
        Self::zeroed()
    }
}

assert_struct_size!(PremiumEntry, 8);
assert_struct_align!(PremiumEntry, 4);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone, Eq)]
/// One pairwise variable-borrow premium rate: accounts lending collateral tagged
/// `collateral_tag` pay an extra `rate` APR (proportional to that collateral's share of their
/// total collateral) when borrowing from banks tagged `liability_tag`.
pub struct PremiumEntry {
    /// `premium_tag` of the collateral bank(s) this surcharge applies to. 0 = empty slot.
    pub collateral_tag: u16,
    /// `premium_tag` of the liability bank(s) this surcharge applies to. 0 = empty slot.
    pub liability_tag: u16,
    /// Premium APR for this pair, encoded like interest-curve points via `milli_to_u32`
    /// (0-1000%).
    pub rate: u32,
}

impl PremiumEntry {
    pub fn is_empty(&self) -> bool {
        self.collateral_tag == PREMIUM_TAG_EMPTY || self.liability_tag == PREMIUM_TAG_EMPTY
    }
}

impl MarginfiGroup {
    /// Look up the variable-borrow premium rate (milli-u32 encoding) for a (collateral tag,
    /// liability tag) pair. Missing pairs and untagged (0) banks pay no premium.
    /// * Entries are stored sorted by (collateral_tag, liability_tag) — every config path
    ///   preserves this.
    pub fn find_premium_rate(&self, collateral_tag: u16, liability_tag: u16) -> u32 {
        if collateral_tag == PREMIUM_TAG_EMPTY || liability_tag == PREMIUM_TAG_EMPTY {
            return 0;
        }
        let n = (self.premium_settings.entry_count as usize).min(MAX_PREMIUM_ENTRIES);
        self.premium_entries[..n]
            .binary_search_by_key(&(collateral_tag, liability_tag), |e| {
                (e.collateral_tag, e.liability_tag)
            })
            .map(|i| self.premium_entries[i].rate)
            .unwrap_or(0)
    }
}

/// Elapsed seconds of ACTIVE premium accrual: since the balance's last claim, but never
/// earlier than the bank's most recent `PREMIUM_ACTIVE` activation (`activated_at`) — so an
/// off->on flag cycle can never charge for the deactivated window (accrual in an earlier active
/// window that was never claimed is forgiven, the safe direction). Also clamped to zero for
/// clock skew (`now < start`) and for uninitialized (`last_update == 0`) balances.
pub fn premium_elapsed_seconds(balance: &Balance, activated_at: i64, now: u64) -> u64 {
    if balance.last_update == 0 {
        return 0;
    }
    let start = balance.last_update.max(activated_at.max(0) as u64);
    now.saturating_sub(start)
}

/// Total recognized premium for a position: already-materialized `outstanding` plus simple
/// interest accrued at the snapshot rate since `last_update`:
/// `outstanding + liability_amount × rate × elapsed_seconds / SECONDS_PER_YEAR`.
/// Uncapped: liquidation via the health projection is the safety valve for unbounded dormant
/// accrual. Returns `None` on fixed-point overflow.
pub fn accrued_premium_total(
    liability_amount: I80F48,
    rate_snapshot: u32,
    outstanding: I80F48,
    elapsed_seconds: u64,
) -> Option<I80F48> {
    let pending = if rate_snapshot == 0 || elapsed_seconds == 0 || liability_amount <= I80F48::ZERO
    {
        I80F48::ZERO
    } else {
        // Divide elapsed by the year FIRST: `liability × rate × elapsed_seconds` can overflow
        // I80F48 for mega-positions dormant for years (which would brick repay/liquidation via
        // the checked-math revert), while `elapsed/year` stays tiny.
        let years = I80F48::from_num(elapsed_seconds).checked_div(SECONDS_PER_YEAR)?;
        liability_amount
            .checked_mul(u32_to_milli(rate_snapshot))?
            .checked_mul(years)?
    };
    outstanding.checked_add(pending)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::milli_to_u32;
    use fixed_macro::types::I80F48;

    const YEAR: u64 = 31_536_000;

    fn rate(percent: f64) -> u32 {
        milli_to_u32(I80F48::from_num(percent / 100.0))
    }

    fn assert_approx(actual: I80F48, expected: I80F48, tol: I80F48) {
        assert!(
            (actual - expected).abs() <= tol,
            "actual {} != expected {} (tol {})",
            actual,
            expected,
            tol
        );
    }

    fn group_with_entries(entries: &[(u16, u16, u32)]) -> MarginfiGroup {
        let mut group = MarginfiGroup::zeroed();
        for (i, (c, l, r)) in entries.iter().enumerate() {
            group.premium_entries[i] = PremiumEntry {
                collateral_tag: *c,
                liability_tag: *l,
                rate: *r,
            };
        }
        group.premium_settings.entry_count = entries.len() as u16;
        group
    }

    // ---------------- find_premium_rate ----------------

    #[test]
    fn find_premium_rate_zeroed_group_is_off() {
        // Zeroed (= any pre-0.1.10 mainnet group) reads as matrix off.
        let group = MarginfiGroup::zeroed();
        assert_eq!(group.premium_settings.entry_count, 0);
        assert_eq!(group.find_premium_rate(100, 200), 0);
    }

    #[test]
    fn find_premium_rate_hit_and_miss() {
        let group = group_with_entries(&[(100, 200, 7), (100, 300, 9), (150, 200, 11)]);
        assert_eq!(group.find_premium_rate(100, 200), 7);
        assert_eq!(group.find_premium_rate(100, 300), 9);
        assert_eq!(group.find_premium_rate(150, 200), 11);
        // Missing pair defaults to 0
        assert_eq!(group.find_premium_rate(150, 300), 0);
        assert_eq!(group.find_premium_rate(999, 999), 0);
    }

    #[test]
    fn find_premium_rate_tag_zero_never_matches() {
        // A pathological entry with tag 0 (rejected by config validation, but belt-and-braces)
        let group = group_with_entries(&[(0, 200, 7), (100, 0, 9)]);
        assert_eq!(group.find_premium_rate(0, 200), 0);
        assert_eq!(group.find_premium_rate(100, 0), 0);
        assert_eq!(group.find_premium_rate(0, 0), 0);
    }

    #[test]
    fn find_premium_rate_respects_count() {
        let mut group = group_with_entries(&[(100, 200, 7), (100, 300, 9)]);
        // Entries past entry_count are ignored
        group.premium_settings.entry_count = 1;
        assert_eq!(group.find_premium_rate(100, 300), 0);
        assert_eq!(group.find_premium_rate(100, 200), 7);
        // count 0 => matrix off => everything is 0
        group.premium_settings.entry_count = 0;
        assert_eq!(group.find_premium_rate(100, 200), 0);
    }

    // ---------------- accrued_premium_total ----------------

    #[test]
    fn accrual_story6_numbers() {
        // Story 6: 50.41 debt x 1% APR x 60 days
        let total =
            accrued_premium_total(I80F48!(50.41), rate(1.0), I80F48::ZERO, 60 * 24 * 60 * 60)
                .unwrap();
        assert_approx(total, I80F48!(0.082866), I80F48!(0.0001));
    }

    #[test]
    fn accrual_short_circuits() {
        // elapsed 0
        let t = accrued_premium_total(I80F48!(100), rate(1.0), I80F48!(5), 0).unwrap();
        assert_eq!(t, I80F48!(5));
        // zero rate
        let t = accrued_premium_total(I80F48!(100), 0, I80F48!(5), YEAR).unwrap();
        assert_eq!(t, I80F48!(5));
        // zero debt
        let t = accrued_premium_total(I80F48::ZERO, rate(1.0), I80F48!(5), YEAR).unwrap();
        assert_eq!(t, I80F48!(5));
    }

    #[test]
    fn accrual_is_uncapped_simple_interest() {
        // 2% APR on 100 for 1 year on top of 3 already outstanding = 5 total; no ceiling
        let total = accrued_premium_total(I80F48!(100), rate(2.0), I80F48!(3.0), YEAR).unwrap();
        assert_approx(total, I80F48!(5.0), I80F48!(0.0001));
    }

    #[test]
    fn accrual_overflow_is_none_not_panic() {
        assert_eq!(
            accrued_premium_total(I80F48::MAX, rate(1000.0), I80F48::MAX, YEAR * 1000),
            None
        );
    }

    // ---------------- premium_elapsed_seconds ----------------

    #[test]
    fn elapsed_clamps_zero_last_update_and_clock_skew() {
        let mut balance = Balance::empty_deactivated();
        // last_update == 0 must never charge ~55 years of premium
        balance.last_update = 0;
        assert_eq!(premium_elapsed_seconds(&balance, 0, 1_750_000_000), 0);
        // clock skew: now < last_update
        balance.last_update = 2_000_000_000;
        assert_eq!(premium_elapsed_seconds(&balance, 0, 1_750_000_000), 0);
        // normal
        balance.last_update = 1_000;
        assert_eq!(premium_elapsed_seconds(&balance, 0, 2_000), 1_000);
    }

    #[test]
    fn elapsed_clamps_to_bank_activation() {
        // Accrual never starts before the bank's latest inactive->active transition: an
        // off->on flag cycle cannot charge for the deactivated window.
        let mut balance = Balance::empty_deactivated();
        balance.last_update = 1_000;
        // Re-activated at 5_000: only [5_000, 6_000] accrues, not [1_000, 6_000]
        assert_eq!(premium_elapsed_seconds(&balance, 5_000, 6_000), 1_000);
        // Activation older than the last claim: no effect
        assert_eq!(premium_elapsed_seconds(&balance, 500, 6_000), 5_000);
        // Activation in the future of `now` (same-slot config): clamps to zero
        assert_eq!(premium_elapsed_seconds(&balance, 7_000, 6_000), 0);
        // Never-activated sentinel (0) behaves as no clamp
        assert_eq!(premium_elapsed_seconds(&balance, 0, 6_000), 5_000);
        // Negative activation (garbage) is treated as 0
        assert_eq!(premium_elapsed_seconds(&balance, -5, 6_000), 5_000);
    }
}
