//! Variable-borrow premium engine.
//!
//! A per-user surcharge on top of base borrow interest, priced pairwise by the group's
//! (collateral `premium_tag`, liability `premium_tag`) matrix and weighted by each collateral's
//! share of the account's total collateral value.
//!
//! ## Lifecycle
//!
//! 1. **Claim (materialize)** — [`claim_premium`] folds `liability_amount × rate × elapsed /
//!    SECONDS_PER_YEAR` into `balance.premium_outstanding` and bumps
//!    `balance.last_update`. Mutates ONLY the balance: the premium is a receivable, not vault
//!    liquidity, so any balance close implicitly (and safely) writes it off.
//! 2. **Snapshot recompute** — [`update_premium_snapshots`] rewrites each liability's
//!    collateral-weighted rate from data collected during the health-check loop
//!    ([`PremiumScratch`]). It always claims at the OLD rate first, so a 0→nonzero snapshot
//!    transition can never retroactively charge the period before the rate existed.
//! 3. **Health projection** — the health loop adds `outstanding + pending` to each
//!    premium-active liability's weighted value, so dormant accounts degrade over time (and
//!    eventually become liquidatable — liquidation is the safety valve; there is no cap).
//! 4. **Settle** — only where real tokens enter the liquidity vault (repay):
//!    `bank.collected_premium_outstanding` is credited there and swept later to the protocol
//!    premium wallet. Bankruptcy / tokenless repayment / liability→asset flips write the
//!    receivable off without crediting the bank.

use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::SECONDS_PER_YEAR,
    types::{
        u32_to_milli, Balance, BalanceSide, MarginfiAccount, MarginfiGroup,
        MAX_LENDING_ACCOUNT_BALANCES,
    },
};

use crate::{math_error, prelude::MarginfiResult, state::marginfi_group::MarginfiGroupImpl};

/// Per-balance data collected during the health-check loop, enough to recompute premium
/// snapshots afterwards without reloading banks or oracles.
/// * Plain stack enum: survives the health loop's `heap_restore` checkpoints, and each
///   variant carries only the fields that are meaningful for it.
#[derive(Debug, Clone, Copy, Default)]
pub enum PremiumScratchEntry {
    /// Slot not in use (or a balance that contributes nothing to premium).
    #[default]
    Skip,
    /// A collateral leg: contributes `usd_value` of weight at `premium_tag`.
    /// * `usd_value` is UNWEIGHTED collateral USD; zero when the risk engine prices this
    ///   collateral at zero (isolated banks, skipped stale oracles).
    Asset { premium_tag: u16, usd_value: I80F48 },
    /// A liability leg: gets claimed at its old rate and receives a recomputed snapshot.
    /// Recorded for premium-INACTIVE banks too (`premium_active: false`) so the snapshot pass
    /// can WRITE OFF their receivable instead of accruing.
    Liability {
        /// Matches `Balance.bank_pk` — how the entry finds its balance again post-health-loop.
        bank_pk: Pubkey,
        premium_tag: u16,
        /// Liability amount in native token units.
        liability_amount: I80F48,
        /// The bank's `premium_activated_at` accrual clamp.
        activated_at: i64,
        /// Whether the bank has `PREMIUM_ACTIVE` set.
        premium_active: bool,
    },
}

/// Collector threaded through `get_health_components` (as `&mut Option<&mut PremiumScratch>`,
/// mirroring the `liq_cache` pattern). `complete` is set by the health loop only when every
/// active balance was processed successfully; snapshot writes must be skipped otherwise so a
/// partial pass can never produce garbage rates.
#[derive(Debug)]
pub struct PremiumScratch {
    pub entries: [PremiumScratchEntry; MAX_LENDING_ACCOUNT_BALANCES],
    pub count: usize,
    pub complete: bool,
}

impl Default for PremiumScratch {
    fn default() -> Self {
        Self {
            entries: [PremiumScratchEntry::Skip; MAX_LENDING_ACCOUNT_BALANCES],
            count: 0,
            complete: false,
        }
    }
}

impl PremiumScratch {
    pub fn push(&mut self, entry: PremiumScratchEntry) {
        if self.count < self.entries.len() {
            self.entries[self.count] = entry;
            self.count += 1;
        }
    }
}

/// Total recognized premium for a position: already-materialized `outstanding` plus simple
/// interest accrued at the snapshot rate since `last_update`. Uncapped: liquidation via the
/// health projection is the safety valve for unbounded dormant accrual.
pub fn accrued_premium_total(
    liability_amount: I80F48,
    rate_snapshot: u32,
    outstanding: I80F48,
    elapsed_seconds: u64,
) -> MarginfiResult<I80F48> {
    let pending = if rate_snapshot == 0 || elapsed_seconds == 0 || liability_amount <= I80F48::ZERO
    {
        I80F48::ZERO
    } else {
        // Divide elapsed by the year FIRST: `liability × rate × elapsed_seconds` can overflow
        // I80F48 for mega-positions dormant for years (which would brick repay/liquidation via
        // the checked-math revert), while `elapsed/year` stays tiny.
        let years = I80F48::from_num(elapsed_seconds)
            .checked_div(SECONDS_PER_YEAR)
            .ok_or_else(math_error!())?;
        liability_amount
            .checked_mul(u32_to_milli(rate_snapshot))
            .ok_or_else(math_error!())?
            .checked_mul(years)
            .ok_or_else(math_error!())?
    };

    Ok(outstanding.checked_add(pending).ok_or_else(math_error!())?)
}

/// Elapsed seconds of ACTIVE premium accrual: since the balance's last claim, but never
/// earlier than the bank's most recent `PREMIUM_ACTIVE` activation — so an off->on flag cycle
/// can never charge for the deactivated window (accrual in an earlier active window that was
/// never claimed is forgiven, the safe direction). Also clamped to zero for clock skew
/// (`now < start`) and for uninitialized (`last_update == 0`) balances.
pub fn premium_elapsed_seconds(balance: &Balance, activated_at: i64, now: u64) -> u64 {
    if balance.last_update == 0 {
        return 0;
    }
    let start = balance.last_update.max(activated_at.max(0) as u64);
    now.saturating_sub(start)
}

/// Materialize pending premium into `balance.premium_outstanding` at the CURRENT snapshot rate
/// and advance `last_update`.
///
/// Always bumps `last_update`, including when the snapshot rate is zero — otherwise a later
/// 0→nonzero snapshot write would retroactively charge the zero-rate period.
/// * Mutates only the balance (realized-only accounting): bank counters are credited at
///   settlement, where real tokens arrive.
pub fn claim_premium(
    balance: &mut Balance,
    liability_amount: I80F48,
    activated_at: i64,
    now: u64,
) -> MarginfiResult {
    let elapsed = premium_elapsed_seconds(balance, activated_at, now);
    let total = accrued_premium_total(
        liability_amount,
        balance.premium_rate_snapshot,
        balance.premium_outstanding.into(),
        elapsed,
    )?;
    balance.premium_outstanding = total.into();
    balance.last_update = now;
    Ok(())
}

/// Claim every premium-active liability at its OLD snapshot rate, then overwrite each snapshot
/// with the freshly computed collateral-weighted pair rate. Runs in instruction handlers after
/// a successful health check, using the scratch that check collected.
///
/// The weighted rate for a liability is
/// `Σ(collateral_usd_i × pair_rate(collateral_tag_i, liability_tag)) / Σ(collateral_usd_i)`,
/// or zero when the account has no priced collateral or the matrix is disabled.
/// * No-op when the scratch is incomplete (partial health pass must never write rates).
pub fn update_premium_snapshots(
    marginfi_account: &mut MarginfiAccount,
    group: &MarginfiGroup,
    scratch: &PremiumScratch,
    now: u64,
) -> MarginfiResult {
    if !scratch.complete {
        return Ok(());
    }

    let entries = &scratch.entries[..scratch.count];

    let mut total_collateral_usd = I80F48::ZERO;
    for entry in entries {
        if let PremiumScratchEntry::Asset { usd_value, .. } = entry {
            total_collateral_usd = total_collateral_usd
                .checked_add(*usd_value)
                .ok_or_else(math_error!())?;
        }
    }

    for entry in entries {
        let PremiumScratchEntry::Liability {
            bank_pk,
            premium_tag: liability_tag,
            liability_amount,
            activated_at,
            premium_active,
        } = entry
        else {
            continue;
        };

        let balance = marginfi_account
            .lending_account
            .balances
            .iter_mut()
            .find(|b| b.is_active() && b.bank_pk == *bank_pk);
        let Some(balance) = balance else {
            // The balance closed between the health pass and this write (e.g. repay_all in the
            // same handler); nothing to update.
            continue;
        };
        if balance.is_empty(BalanceSide::Liabilities) {
            continue;
        }

        // Premium-inactive liability: write off any receivable (premium from a
        // since-deactivated bank) and clear the snapshot; never accrue.
        if !premium_active {
            balance.premium_outstanding = I80F48::ZERO.into();
            balance.premium_rate_snapshot = 0;
            balance.last_update = now;
            continue;
        }

        // Materialize at the OLD rate before overwriting it. This also bumps `last_update`,
        // making a 0 -> nonzero rate transition safe (no retroactive projection).
        claim_premium(balance, *liability_amount, *activated_at, now)?;

        let new_rate: u32 = if total_collateral_usd > I80F48::ZERO {
            let mut weighted = I80F48::ZERO;
            for collateral in entries {
                let PremiumScratchEntry::Asset {
                    premium_tag: collateral_tag,
                    usd_value,
                } = collateral
                else {
                    continue;
                };
                if *usd_value <= I80F48::ZERO {
                    continue;
                }
                let pair_rate = group.find_premium_rate(*collateral_tag, *liability_tag);
                if pair_rate == 0 {
                    continue;
                }
                weighted = weighted
                    .checked_add(
                        usd_value
                            .checked_mul(u32_to_milli(pair_rate))
                            .ok_or_else(math_error!())?,
                    )
                    .ok_or_else(math_error!())?;
            }
            let rate = weighted
                .checked_div(total_collateral_usd)
                .ok_or_else(math_error!())?;
            marginfi_type_crate::types::milli_to_u32(rate)
        } else {
            0
        };

        balance.premium_rate_snapshot = new_rate;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;
    use fixed_macro::types::I80F48;
    use marginfi_type_crate::types::{milli_to_u32, PremiumEntry, MAX_PREMIUM_ENTRIES};

    const YEAR: u64 = 31_536_000;
    const TOL: I80F48 = I80F48!(0.000001);

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

    // ---------------- rate encoding ----------------

    #[test]
    fn rate_encoding_roundtrip() {
        for percent in [0.0, 0.001, 0.2, 1.0, 5.5, 100.0, 1000.0] {
            let encoded = rate(percent);
            let decoded = u32_to_milli(encoded);
            assert_approx(decoded, I80F48::from_num(percent / 100.0), TOL);
        }
    }

    #[test]
    fn rate_encoding_saturates_not_wraps() {
        // Above the 1000% ceiling and negative inputs clamp, never wrap
        assert_eq!(milli_to_u32(I80F48::from_num(20.0)), u32::MAX);
        assert_eq!(milli_to_u32(I80F48::from_num(-1.0)), 0);
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

    // ---------------- elapsed / claim ----------------

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
    }

    #[test]
    fn claim_zero_rate_still_advances_clock() {
        // THE retroactivity guard: claiming at rate 0 must bump last_update, otherwise a later
        // 0 -> nonzero snapshot write would charge the entire zero-rate period.
        let mut balance = Balance::empty_deactivated();
        balance.last_update = 1_000;
        balance.premium_rate_snapshot = 0;
        claim_premium(&mut balance, I80F48!(100), 0, 1_000 + YEAR).unwrap();
        assert_eq!(balance.last_update, 1_000 + YEAR);
        assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);
    }

    #[test]
    fn claim_materializes_and_advances_clock() {
        let mut balance = Balance::empty_deactivated();
        balance.last_update = 1_000;
        balance.premium_rate_snapshot = rate(1.0);
        claim_premium(&mut balance, I80F48!(100), 0, 1_000 + YEAR).unwrap();
        assert_eq!(balance.last_update, 1_000 + YEAR);
        assert_approx(
            balance.premium_outstanding.into(),
            I80F48!(1.0),
            I80F48!(0.0001),
        );

        // Double-claim at the same timestamp is a no-op
        claim_premium(&mut balance, I80F48!(100), 0, 1_000 + YEAR).unwrap();
        assert_approx(
            balance.premium_outstanding.into(),
            I80F48!(1.0),
            I80F48!(0.0001),
        );
    }

    // ---------------- update_premium_snapshots ----------------

    fn group_with(entries: &[(u16, u16, f64)]) -> MarginfiGroup {
        let mut group = MarginfiGroup::zeroed();
        for (i, (c, l, pct)) in entries.iter().enumerate() {
            group.premium_entries[i] = PremiumEntry {
                collateral_tag: *c,
                liability_tag: *l,
                rate: rate(*pct),
            };
        }
        group.premium_settings.entry_count = entries.len() as u16;
        group.premium_settings.entry_capacity = MAX_PREMIUM_ENTRIES as u16;
        group
    }

    fn account_with_liability(bank_pk: Pubkey, snapshot: u32, last_update: u64) -> MarginfiAccount {
        let mut account = MarginfiAccount::zeroed();
        let balance = &mut account.lending_account.balances[0];
        balance.active = 1;
        balance.bank_pk = bank_pk;
        balance.liability_shares = I80F48!(50).into();
        balance.premium_rate_snapshot = snapshot;
        balance.last_update = last_update;
        account
    }

    fn asset_entry(usd: f64, tag: u16) -> PremiumScratchEntry {
        PremiumScratchEntry::Asset {
            premium_tag: tag,
            usd_value: I80F48::from_num(usd),
        }
    }

    fn liab_entry(bank_pk: Pubkey, amount: f64, tag: u16) -> PremiumScratchEntry {
        PremiumScratchEntry::Liability {
            bank_pk,
            premium_tag: tag,
            liability_amount: I80F48::from_num(amount),
            activated_at: 0,
            premium_active: true,
        }
    }

    fn snapshot_rate(account: &MarginfiAccount) -> I80F48 {
        u32_to_milli(account.lending_account.balances[0].premium_rate_snapshot)
    }

    #[test]
    fn snapshot_story1_single_collateral() {
        // 100% BONK collateral, borrow stable at (meme, stable) = 1%
        let group = group_with(&[(200, 100, 1.0)]);
        let liab_pk = Pubkey::new_unique();
        let mut account = account_with_liability(liab_pk, 0, 500);

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(liab_entry(liab_pk, 50.0, 100));
        scratch.complete = true;

        update_premium_snapshots(&mut account, &group, &scratch, 1_000).unwrap();
        assert_approx(snapshot_rate(&account), I80F48!(0.01), TOL);
        // 0 -> nonzero transition bumped the accrual clock (no retroactive projection)
        assert_eq!(account.lending_account.balances[0].last_update, 1_000);
        assert_eq!(
            I80F48::from(account.lending_account.balances[0].premium_outstanding),
            I80F48::ZERO
        );
    }

    #[test]
    fn snapshot_story2_mixed_collateral_weighted() {
        // $90 stable (no pair) + $10 meme at 2% vs the borrowed asset => 0.2%
        let group = group_with(&[(200, 300, 2.0)]);
        let liab_pk = Pubkey::new_unique();
        let mut account = account_with_liability(liab_pk, 0, 500);

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(90.0, 100));
        scratch.push(asset_entry(10.0, 200));
        scratch.push(liab_entry(liab_pk, 50.0, 300));
        scratch.complete = true;

        update_premium_snapshots(&mut account, &group, &scratch, 1_000).unwrap();
        assert_approx(snapshot_rate(&account), I80F48!(0.002), TOL);
    }

    #[test]
    fn snapshot_story4_5_missing_pairs_default_zero() {
        // Tags: BONK=1 SOL=2 USDC=3 ETH=4; liabilities: stable=10 LST=11 major=12
        let group = group_with(&[
            (1, 10, 1.0),
            (1, 11, 2.0),
            (2, 10, 0.1),
            (4, 10, 0.2),
            (4, 11, 0.3),
        ]);
        let stable_pk = Pubkey::new_unique();
        let lst_pk = Pubkey::new_unique();
        let major_pk = Pubkey::new_unique();

        let mut account = MarginfiAccount::zeroed();
        for (i, pk) in [stable_pk, lst_pk, major_pk].iter().enumerate() {
            let balance = &mut account.lending_account.balances[i];
            balance.active = 1;
            balance.bank_pk = *pk;
            balance.liability_shares = I80F48!(10).into();
            balance.last_update = 500;
        }

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 1));
        scratch.push(asset_entry(100.0, 2));
        scratch.push(asset_entry(50.0, 3));
        scratch.push(asset_entry(50.0, 4));
        scratch.push(liab_entry(stable_pk, 100.0, 10));
        scratch.push(liab_entry(lst_pk, 40.0, 11));
        scratch.push(liab_entry(major_pk, 25.0, 12));
        scratch.complete = true;

        update_premium_snapshots(&mut account, &group, &scratch, 1_000).unwrap();

        let rate_of = |i: usize| -> I80F48 {
            u32_to_milli(account.lending_account.balances[i].premium_rate_snapshot)
        };
        // Spec-exact expected values: 0.4000%, 0.7167%, 0.0000%
        assert_approx(rate_of(0), I80F48!(0.004), TOL);
        assert_approx(rate_of(1), I80F48::from_num(0.0071666), I80F48!(0.00001));
        assert_eq!(rate_of(2), I80F48::ZERO);
    }

    #[test]
    fn snapshot_zero_collateral_means_zero_rate() {
        let group = group_with(&[(200, 100, 1.0)]);
        let liab_pk = Pubkey::new_unique();
        let mut account = account_with_liability(liab_pk, rate(1.0), 500);

        let mut scratch = PremiumScratch::default();
        scratch.push(liab_entry(liab_pk, 50.0, 100));
        scratch.complete = true;

        update_premium_snapshots(&mut account, &group, &scratch, 1_000).unwrap();
        assert_eq!(snapshot_rate(&account), I80F48::ZERO);
    }

    #[test]
    fn snapshot_incomplete_scratch_is_a_noop() {
        let group = group_with(&[(200, 100, 1.0)]);
        let liab_pk = Pubkey::new_unique();
        let mut account = account_with_liability(liab_pk, 0, 500);

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(liab_entry(liab_pk, 50.0, 100));
        // complete deliberately left false (partial health pass)

        update_premium_snapshots(&mut account, &group, &scratch, 1_000).unwrap();
        assert_eq!(account.lending_account.balances[0].premium_rate_snapshot, 0);
        assert_eq!(account.lending_account.balances[0].last_update, 500);
    }

    #[test]
    fn snapshot_write_claims_at_old_rate_first() {
        // Balance accruing at 1% for a year; the recompute changes the rate to 2% but the
        // elapsed year must be charged at the OLD 1% rate.
        let group = group_with(&[(200, 100, 2.0)]);
        let liab_pk = Pubkey::new_unique();
        let t0 = 1_000u64;
        let mut account = account_with_liability(liab_pk, rate(1.0), t0);

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(liab_entry(liab_pk, 100.0, 100));
        scratch.complete = true;

        update_premium_snapshots(&mut account, &group, &scratch, t0 + YEAR).unwrap();

        let balance = &account.lending_account.balances[0];
        assert_approx(
            balance.premium_outstanding.into(),
            I80F48!(1.0), // 100 x 1% x 1yr, NOT 2%
            I80F48!(0.0001),
        );
        assert_approx(
            u32_to_milli(balance.premium_rate_snapshot),
            I80F48!(0.02),
            TOL,
        );
        assert_eq!(balance.last_update, t0 + YEAR);
    }

    #[test]
    fn snapshot_pass_writes_off_inactive_liability_receivable() {
        // A liability on a premium-INACTIVE bank carrying a receivable (premium from a
        // since-deactivated bank) must be written off and its snapshot cleared — never
        // accrued or settled.
        let group = group_with(&[(200, 100, 1.0)]);
        let liab_pk = Pubkey::new_unique();
        let mut account = account_with_liability(liab_pk, rate(1.0), 500);
        account.lending_account.balances[0].premium_outstanding = I80F48!(26891413).into();

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(PremiumScratchEntry::Liability {
            bank_pk: liab_pk,
            premium_tag: 100,
            liability_amount: I80F48!(50),
            activated_at: 0,
            premium_active: false,
        });
        scratch.complete = true;

        update_premium_snapshots(&mut account, &group, &scratch, 1_000).unwrap();
        let balance = &account.lending_account.balances[0];
        assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);
        assert_eq!(balance.premium_rate_snapshot, 0);
        assert_eq!(balance.last_update, 1_000);
    }

    #[test]
    fn accrual_no_overflow_for_mega_position_dormant_decades() {
        // 1e15 native units at the 1000% rate ceiling, dormant 50 years: must compute, not
        // revert (a checked-math revert here would brick repay/liquidation).
        let total = accrued_premium_total(
            I80F48::from_num(1_000_000_000_000_000u64),
            u32::MAX,
            I80F48::ZERO,
            50 * 31_536_000,
        )
        .unwrap();
        assert!(total > I80F48::ZERO);
    }

    #[test]
    fn snapshot_skips_closed_and_asset_side_balances() {
        let group = group_with(&[(200, 100, 1.0)]);
        let liab_pk = Pubkey::new_unique();
        // Balance closed between health pass and write: no panic, no write
        let mut account = MarginfiAccount::zeroed();

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(liab_entry(liab_pk, 50.0, 100));
        scratch.complete = true;

        update_premium_snapshots(&mut account, &group, &scratch, 1_000).unwrap();
        assert_eq!(account.lending_account.balances[0].premium_rate_snapshot, 0);
    }
}
