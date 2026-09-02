//! Variable-borrow premium engine.
//!
//! A per-user surcharge on top of base borrow interest, priced pairwise by the group's
//! (collateral `premium_tag`, liability `premium_tag`) matrix and weighted by each collateral's
//! share of the account's total collateral value.
//!
//! ## Terminology
//!
//! "Claim" is the canonical verb everywhere in code; "materialize" is its informal synonym in
//! prose (claiming materializes pending accrual into the stored receivable).
//!
//! ## Lifecycle
//!
//! 1. **Claim (materialize)** — [`BalancePremiumImpl::claim_premium`] folds
//!    `liability_amount × rate × elapsed / SECONDS_PER_YEAR` into
//!    `balance.premium_outstanding` and bumps
//!    `balance.last_update`. Mutates ONLY the balance: the premium is a receivable, not vault
//!    liquidity, so any balance close implicitly (and safely) writes it off.
//! 2. **Snapshot recompute** — [`MarginfiAccountPremiumImpl::update_premium_snapshots`]
//!    rewrites each liability's
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
//!
//! ## Scratch valuation reuses health prices (accepted approximation)
//!
//! Collateral weights reuse the health pass's biased, requirement-typed prices — zero extra
//! oracle work. The rate therefore wobbles with confidence bands (clamped at 5% of price;
//! uniform haircuts cancel in the average, so at most ~5% RELATIVE, symmetric, correctable
//! by any crank) and varies slightly by triggering instruction (Initial uses EMA for Pyth).
//! No risk weights; ReduceOnly collateral still counts (Initial zeroes it for health only).
//! Prices never scale the materialized amount (`debt_tokens × rate × time` is price-free),
//! so price quality is second-order here — health/liquidation pricing is untouched.
//!
//! ## Accepted approximations
//!
//! * **Deposit/repay staleness** — no oracles, so they claim at the old rate but can't
//!   re-weight; the stale rate persists until the next refreshing ix or `pulse_health`.
//!   Admin rate changes likewise apply per-account from each account's next refresh — after
//!   `configure_group_premium` / retags, crank `pulse_health` across active borrowers of the
//!   affected banks so the new rate takes effect promptly.
//! * **Crank-order variance** — [`BalancePremiumImpl::claim_premium`] uses the liability
//!   amount at claim time, so claiming before vs. after interest accrual differs by a
//!   second-order term.

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

/// `PremiumScratchEntry.flags`: entry is a collateral leg.
pub const SCRATCH_ASSET: u8 = 1 << 0;
/// `PremiumScratchEntry.flags`: entry is a liability leg. Liability entries are recorded for
/// premium-INACTIVE banks too (without [`SCRATCH_PREMIUM_ACTIVE`]) so the snapshot pass can
/// WRITE OFF their receivable instead of accruing.
pub const SCRATCH_LIABILITY: u8 = 1 << 1;
/// `PremiumScratchEntry.flags`: the liability's bank has `PREMIUM_ACTIVE` set.
pub const SCRATCH_PREMIUM_ACTIVE: u8 = 1 << 2;

/// Per-balance data collected during the health-check loop, enough to recompute premium
/// snapshots afterwards without reloading banks or oracles.
/// * Flat `#[repr(C)]` record (no enum): 16 of these live on the stack ([`PremiumScratch`]),
///   and the enum tag + per-variant padding was a meaningful share of the SBF frame budget.
/// * A zeroed entry (`flags == 0`) is an unused slot.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PremiumScratchEntry {
    /// Asset legs: UNWEIGHTED collateral USD (zero when the risk engine prices the collateral
    /// at zero — isolated banks, skipped stale oracles). Liability legs: liability amount in
    /// native token units.
    pub value: I80F48,
    /// Liability legs: the bank's `premium_activated_at` accrual clamp. Zero for asset legs.
    pub activated_at: i64,
    pub premium_tag: u16,
    /// Liability legs: RAW index into `lending_account.balances` (not the active-only
    /// ordinal, so inactive holes cannot misaddress). INVARIANT: no `sort_balances` between
    /// scratch collection and the snapshot write; closed balances are caught by write-time
    /// revalidation.
    pub balance_index: u8,
    /// `SCRATCH_*` bit flags; 0 = unused slot.
    pub flags: u8,
}

impl PremiumScratchEntry {
    pub fn is_asset(&self) -> bool {
        self.flags & SCRATCH_ASSET != 0
    }
    pub fn is_liability(&self) -> bool {
        self.flags & SCRATCH_LIABILITY != 0
    }
    pub fn is_premium_active(&self) -> bool {
        self.flags & SCRATCH_PREMIUM_ACTIVE != 0
    }
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
    /// A countable (non-isolated) collateral leg could not be priced. Tracked separately from
    /// `err_code` because health may not flag the leg at all (ReduceOnly + Initial soft-zeroes
    /// before inspecting the oracle) while the premium weighting still counts it — such a
    /// pass must never be marked complete.
    pub unpriceable_leg: bool,
}

impl Default for PremiumScratch {
    fn default() -> Self {
        Self {
            entries: [PremiumScratchEntry::default(); MAX_LENDING_ACCOUNT_BALANCES],
            count: 0,
            complete: false,
            unpriceable_leg: false,
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

    /// Incomplete pass + premium-active debt: refreshing here would drop the stale leg and skew
    /// the rate, so debt-changing handlers revert (passive ones just skip).
    pub fn refresh_unavailable(&self) -> bool {
        !self.complete
            && self.entries[..self.count]
                .iter()
                .any(|e| e.is_liability() && e.is_premium_active())
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

/// Highest configured pair rate against `liability_tag` across all collateral tags (0 when
/// none). Seeds a conservative snapshot when liquidation assumes premium-active debt during
/// an incomplete health pass — worst rate instead of 0%, so a permanently-stale leg is never
/// a standing exemption; the next complete pass reprices to the true weighted rate.
pub fn max_premium_rate_for_liability_tag(group: &MarginfiGroup, liability_tag: u16) -> u32 {
    let count = (group.premium_settings.entry_count as usize).min(group.premium_entries.len());
    let mut max_rate = 0u32;
    for entry in &group.premium_entries[..count] {
        if entry.liability_tag == liability_tag && entry.rate > max_rate {
            max_rate = entry.rate;
        }
    }
    max_rate
}

/// Premium methods on [`Balance`] (extension trait: `Balance` lives in the type-crate, so an
/// inherent impl is not possible here — same pattern as `BalanceImpl`).
pub trait BalancePremiumImpl {
    /// Materialize pending premium into `premium_outstanding` at the CURRENT snapshot rate
    /// and advance `last_update`.
    ///
    /// Always bumps `last_update`, including when the snapshot rate is zero — otherwise a
    /// later 0→nonzero snapshot write would retroactively charge the zero-rate period.
    /// * Mutates only the balance (realized-only accounting): bank counters are credited at
    ///   settlement, where real tokens arrive.
    fn claim_premium(
        &mut self,
        liability_amount: I80F48,
        activated_at: i64,
        now: u64,
    ) -> MarginfiResult;

    /// Forgive the receivable: zero `premium_outstanding` AND `premium_rate_snapshot`,
    /// without crediting the bank. The single clear used by every write-off site
    /// (premium-inactive banks, non-liability balances, dust-closed liabilities) so no site
    /// can clear one field and leave a stale value in the other.
    fn write_off_premium(&mut self);
}

impl BalancePremiumImpl for Balance {
    fn claim_premium(
        &mut self,
        liability_amount: I80F48,
        activated_at: i64,
        now: u64,
    ) -> MarginfiResult {
        let elapsed = premium_elapsed_seconds(self, activated_at, now);
        let total = accrued_premium_total(
            liability_amount,
            self.premium_rate_snapshot,
            self.premium_outstanding.into(),
            elapsed,
        )?;
        self.premium_outstanding = total.into();
        self.last_update = now;
        Ok(())
    }

    fn write_off_premium(&mut self) {
        self.premium_outstanding = I80F48::ZERO.into();
        self.premium_rate_snapshot = 0;
    }
}

/// Premium methods on [`MarginfiAccount`] (extension trait, same rationale as
/// [`BalancePremiumImpl`]).
pub trait MarginfiAccountPremiumImpl {
    /// Claim every premium-active liability at its OLD snapshot rate, then overwrite each
    /// snapshot with the freshly computed collateral-weighted pair rate. Runs in instruction
    /// handlers after a successful health check, using the scratch that check collected.
    ///
    /// The weighted rate for a liability is
    /// `Σ(collateral_usd_i × pair_rate(collateral_tag_i, liability_tag)) / Σ(collateral_usd_i)`,
    /// or zero when the account has no priced collateral or the matrix is disabled.
    /// * No-op when the scratch is incomplete (partial health pass must never write rates).
    fn update_premium_snapshots(
        &mut self,
        group: &MarginfiGroup,
        scratch: &PremiumScratch,
        now: u64,
    ) -> MarginfiResult;
}

impl MarginfiAccountPremiumImpl for MarginfiAccount {
    fn update_premium_snapshots(
        &mut self,
        group: &MarginfiGroup,
        scratch: &PremiumScratch,
        now: u64,
    ) -> MarginfiResult {
        update_premium_snapshots_internal(self, group, scratch, now)
    }
}

fn update_premium_snapshots_internal(
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
        if entry.is_asset() {
            total_collateral_usd = total_collateral_usd
                .checked_add(entry.value)
                .ok_or_else(math_error!())?;
        }
    }

    for entry in entries {
        if !entry.is_liability() {
            continue;
        }
        let liability_tag = entry.premium_tag;

        // Address the balance by the RAW slot recorded at collection time, then revalidate:
        // the balance may have closed between the health pass and this write (e.g. repay_all
        // in the same handler). Reordering between collect and write is forbidden (see
        // `PremiumScratchEntry::balance_index`).
        let balance = marginfi_account
            .lending_account
            .balances
            .get_mut(entry.balance_index as usize);
        let Some(balance) = balance else {
            continue;
        };
        if !balance.is_active() || balance.is_empty(BalanceSide::Liabilities) {
            continue;
        }

        // Premium-inactive liability: write off any receivable (premium from a
        // since-deactivated bank) and clear the snapshot; never accrue.
        if !entry.is_premium_active() {
            balance.write_off_premium();
            balance.last_update = now;
            continue;
        }

        // Materialize at the OLD rate before overwriting it. This also bumps `last_update`,
        // making a 0 -> nonzero rate transition safe (no retroactive projection).
        balance.claim_premium(entry.value, entry.activated_at, now)?;

        let new_rate: u32 = if total_collateral_usd > I80F48::ZERO {
            let mut weighted = I80F48::ZERO;
            for collateral in entries {
                if !collateral.is_asset() || collateral.value <= I80F48::ZERO {
                    continue;
                }
                let pair_rate = group.find_premium_rate(collateral.premium_tag, liability_tag);
                if pair_rate == 0 {
                    continue;
                }
                weighted = weighted
                    .checked_add(
                        collateral
                            .value
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
    use marginfi_type_crate::types::{
        milli_to_u32, MarginfiGroup, PremiumEntry, MAX_PREMIUM_ENTRIES, PREMIUM_TAG_EMPTY,
    };

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
    fn max_premium_rate_is_the_encoders_100_percent() {
        // MAX_PREMIUM_RATE is defined as `u32::MAX / 10` (integer division). This must agree
        // EXACTLY with what the encoder produces for 100% APR, or an admin configuring
        // exactly 100% via `milli_to_u32(1.0)` would be rejected by the cap check.
        use marginfi_type_crate::types::MAX_PREMIUM_RATE;
        assert_eq!(milli_to_u32(I80F48::ONE), MAX_PREMIUM_RATE);
        // Decodes to at most 100% (in fact a hair under, from fixed-point truncation).
        assert!(u32_to_milli(MAX_PREMIUM_RATE) <= I80F48::ONE);
        assert_approx(u32_to_milli(MAX_PREMIUM_RATE), I80F48::ONE, TOL);
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
        balance
            .claim_premium(I80F48!(100), 0, 1_000 + YEAR)
            .unwrap();
        assert_eq!(balance.last_update, 1_000 + YEAR);
        assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);
    }

    #[test]
    fn claim_materializes_and_advances_clock() {
        let mut balance = Balance::empty_deactivated();
        balance.last_update = 1_000;
        balance.premium_rate_snapshot = rate(1.0);
        balance
            .claim_premium(I80F48!(100), 0, 1_000 + YEAR)
            .unwrap();
        assert_eq!(balance.last_update, 1_000 + YEAR);
        assert_approx(
            balance.premium_outstanding.into(),
            I80F48!(1.0),
            I80F48!(0.0001),
        );

        // Double-claim at the same timestamp is a no-op
        balance
            .claim_premium(I80F48!(100), 0, 1_000 + YEAR)
            .unwrap();
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
        PremiumScratchEntry {
            value: I80F48::from_num(usd),
            activated_at: 0,
            premium_tag: tag,
            balance_index: 0,
            flags: SCRATCH_ASSET,
        }
    }

    fn liab_entry(balance_index: u8, amount: f64, tag: u16) -> PremiumScratchEntry {
        PremiumScratchEntry {
            value: I80F48::from_num(amount),
            activated_at: 0,
            premium_tag: tag,
            balance_index,
            flags: SCRATCH_LIABILITY | SCRATCH_PREMIUM_ACTIVE,
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
        scratch.push(liab_entry(0, 50.0, 100));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();
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
        scratch.push(liab_entry(0, 50.0, 300));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();
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
        scratch.push(liab_entry(0, 100.0, 10));
        scratch.push(liab_entry(1, 40.0, 11));
        scratch.push(liab_entry(2, 25.0, 12));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();

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
        scratch.push(liab_entry(0, 50.0, 100));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();
        assert_eq!(snapshot_rate(&account), I80F48::ZERO);
    }

    #[test]
    fn snapshot_incomplete_scratch_is_a_noop() {
        let group = group_with(&[(200, 100, 1.0)]);
        let liab_pk = Pubkey::new_unique();
        let mut account = account_with_liability(liab_pk, 0, 500);

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(liab_entry(0, 50.0, 100));
        // complete deliberately left false (partial health pass)

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();
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
        scratch.push(liab_entry(0, 100.0, 100));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, t0 + YEAR)
            .unwrap();

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
        scratch.push(inactive_liab_entry(0, 50.0, 100));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();
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
        let _liab_pk = Pubkey::new_unique();
        // Balance closed between health pass and write: no panic, no write
        let mut account = MarginfiAccount::zeroed();

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(liab_entry(0, 50.0, 100));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();
        assert_eq!(account.lending_account.balances[0].premium_rate_snapshot, 0);
    }

    // ---------------- scratch layout / addressing ----------------

    #[test]
    fn scratch_entry_is_flat_and_small() {
        // The whole point of the flat record + balance_index: 16 entries must fit comfortably
        // in the SBF stack frame. 32 bytes/entry, ~530 bytes total (vs ~1.2KB with the old
        // enum + bank_pk).
        assert_eq!(core::mem::size_of::<PremiumScratchEntry>(), 32);
        assert!(core::mem::size_of::<PremiumScratch>() <= 552);
    }

    #[test]
    fn snapshot_addresses_raw_slot_across_inactive_holes() {
        // Balances: [0] inactive hole, [1] active liability. The scratch records RAW index 1;
        // an active-ordinal scheme would have addressed slot 0 and misfired.
        let group = group_with(&[(200, 100, 1.0)]);
        let mut account = MarginfiAccount::zeroed();
        let balance = &mut account.lending_account.balances[1];
        balance.active = 1;
        balance.bank_pk = Pubkey::new_unique();
        balance.liability_shares = I80F48!(50).into();
        balance.last_update = 500;

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(liab_entry(1, 50.0, 100));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();
        assert_eq!(account.lending_account.balances[0].premium_rate_snapshot, 0);
        assert_approx(
            u32_to_milli(account.lending_account.balances[1].premium_rate_snapshot),
            I80F48!(0.01),
            TOL,
        );
    }

    #[test]
    fn snapshot_revalidates_balance_closed_between_pass_and_write() {
        // The indexed balance deactivated between collect and write (repay_all in the same
        // handler): revalidation must skip it, not write to a dead slot.
        let group = group_with(&[(200, 100, 1.0)]);
        let liab_pk = Pubkey::new_unique();
        let mut account = account_with_liability(liab_pk, rate(1.0), 500);
        account.lending_account.balances[0].active = 0;

        let mut scratch = PremiumScratch::default();
        scratch.push(asset_entry(100.0, 200));
        scratch.push(liab_entry(0, 50.0, 100));
        scratch.complete = true;

        account
            .update_premium_snapshots(&group, &scratch, 1_000)
            .unwrap();
        let balance = &account.lending_account.balances[0];
        assert_approx(
            u32_to_milli(balance.premium_rate_snapshot),
            I80F48!(0.01),
            TOL,
        );
        assert_eq!(balance.last_update, 500, "closed slot must not be touched");
    }

    // ---------------- max_premium_rate_for_liability_tag (liquidator seeding) ----------------

    #[test]
    fn max_pair_rate_picks_worst_rate_for_tag() {
        let group = group_with(&[(1, 10, 1.0), (2, 10, 5.0), (4, 10, 0.2), (4, 11, 9.0)]);
        assert_eq!(
            max_premium_rate_for_liability_tag(&group, 10),
            rate(5.0),
            "highest rate across all collateral tags for liability tag 10"
        );
        assert_eq!(max_premium_rate_for_liability_tag(&group, 11), rate(9.0));
        // No pairs targeting the tag (or untagged): seeds zero — harmless.
        assert_eq!(max_premium_rate_for_liability_tag(&group, 12), 0);
        assert_eq!(
            max_premium_rate_for_liability_tag(&group, PREMIUM_TAG_EMPTY),
            0
        );
    }

    // ---------------- refresh_unavailable (the debt-origination gate) ----------------

    fn inactive_liab_entry(balance_index: u8, amount: f64, tag: u16) -> PremiumScratchEntry {
        PremiumScratchEntry {
            value: I80F48::from_num(amount),
            activated_at: 0,
            premium_tag: tag,
            balance_index,
            flags: SCRATCH_LIABILITY,
        }
    }

    #[test]
    fn refresh_unavailable_only_blocks_incomplete_pass_with_active_liability() {
        let _pk = Pubkey::new_unique();

        // Incomplete pass + a premium-active liability => refresh needed but untrustworthy.
        let mut s = PremiumScratch::default();
        s.push(asset_entry(100.0, 200));
        s.push(liab_entry(0, 50.0, 100));
        assert!(!s.complete);
        assert!(
            s.refresh_unavailable(),
            "must block: active debt, incomplete pass"
        );

        // Same incomplete pass, but the only liability is on a premium-INACTIVE bank: nothing
        // to mis-price (it gets written off regardless), so the gate must NOT block.
        let mut s = PremiumScratch::default();
        s.push(asset_entry(100.0, 200));
        s.push(inactive_liab_entry(0, 50.0, 100));
        assert!(
            !s.refresh_unavailable(),
            "inactive-only debt must not block"
        );

        // Incomplete pass with no liability at all (pure collateral): nothing to refresh.
        let mut s = PremiumScratch::default();
        s.push(asset_entry(100.0, 200));
        assert!(!s.refresh_unavailable(), "no debt must not block");

        // A COMPLETE pass is always trustworthy, even with active debt.
        let mut s = PremiumScratch::default();
        s.push(asset_entry(100.0, 200));
        s.push(liab_entry(0, 50.0, 100));
        s.complete = true;
        assert!(!s.refresh_unavailable(), "complete pass must never block");

        // Default (empty, incomplete) scratch: no entries, no block.
        assert!(!PremiumScratch::default().refresh_unavailable());
    }
}
