#![allow(clippy::too_many_arguments)]

//! Bank-state invariants: share-value monotonicity, fee-bucket
//! monotonicity, and cumulative-shares vs bank totals.
//!
//! These complement [`solvency`] (the equation
//! `vault − fees ≈ deposits − liabs` per bank) and [`position_counts`]
//! (balance-count consistency) by asserting global directional
//! properties that the libfuzzer-era harness never checked.

use std::collections::{HashMap, HashSet};

use fixed::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::types::marginfi::{Bank, MarginfiAccount};

fn from_wrapped(bytes: [u8; 16]) -> I80F48 {
    I80F48::from_bits(i128::from_le_bytes(bytes))
}

/// Snapshot of the per-bank values that must move in a known direction
/// across the lifetime of a fuzz sequence.
#[derive(Clone, Copy, Debug)]
pub struct BankBaseline {
    pub asset_share_value: I80F48,
    pub liability_share_value: I80F48,
    pub group_fees_outstanding: I80F48,
    pub insurance_fees_outstanding: I80F48,
    pub program_fees_outstanding: I80F48,
    pub last_update: i64,
}

/// Take a snapshot of every supplied bank's monotonicity-relevant fields.
/// Call at the end of `#[init]` once the foundation deposits have settled
/// so the baseline reflects the actual starting point of randomised flows.
pub fn snapshot_bank_baselines(
    trident: &mut Trident,
    bank_pks: &[Pubkey],
) -> HashMap<Pubkey, BankBaseline> {
    let mut out = HashMap::with_capacity(bank_pks.len());
    for &pk in bank_pks {
        let bank = trident
            .get_account_with_type::<Bank>(&pk, None)
            .expect("bank must deserialize");
        out.insert(
            pk,
            BankBaseline {
                asset_share_value: from_wrapped(bank.asset_share_value.value),
                liability_share_value: from_wrapped(bank.liability_share_value.value),
                group_fees_outstanding: from_wrapped(bank.collected_group_fees_outstanding.value),
                insurance_fees_outstanding: from_wrapped(
                    bank.collected_insurance_fees_outstanding.value,
                ),
                program_fees_outstanding: from_wrapped(
                    bank.collected_program_fees_outstanding.value,
                ),
                last_update: bank.last_update,
            },
        );
    }
    out
}

/// Per-bank directional invariants:
///
/// * `liability_share_value` is strictly monotonic — interest accrual only
///   grows it. No marginfi codepath writes it down.
/// * `asset_share_value` is monotonic *unless* a `handle_bankruptcy` ix
///   has succeeded on this bank (socialised loss reduces it).
/// * All three fee buckets (group / insurance / program) are monotonic
///   until a `withdraw_fees` ix runs — which the harness never invokes,
///   so the buckets must only grow.
pub fn assert_bank_directional_invariants(
    trident: &mut Trident,
    bank_pks: &[Pubkey],
    baselines: &HashMap<Pubkey, BankBaseline>,
    banks_with_bankruptcy: &HashSet<Pubkey>,
) {
    for &pk in bank_pks {
        let bank = trident
            .get_account_with_type::<Bank>(&pk, None)
            .expect("bank must deserialize");
        let base = baselines.get(&pk).expect("baseline missing for bank");

        let asset_share_value = from_wrapped(bank.asset_share_value.value);
        let liability_share_value = from_wrapped(bank.liability_share_value.value);

        invariant!(
            liability_share_value >= base.liability_share_value,
            "bank {pk} liability_share_value regressed: baseline {}, current {}",
            base.liability_share_value,
            liability_share_value
        );

        // Asset side: only enforce monotonicity if no bankruptcy fired
        // on this bank during the sequence. Bankruptcy is the one
        // codepath that legitimately reduces `asset_share_value` (loss
        // socialised across remaining depositors).
        if !banks_with_bankruptcy.contains(&pk) {
            invariant!(
                asset_share_value >= base.asset_share_value,
                "bank {pk} asset_share_value regressed without bankruptcy: baseline {}, current {}",
                base.asset_share_value,
                asset_share_value
            );
        }

        let group_fees = from_wrapped(bank.collected_group_fees_outstanding.value);
        let insurance_fees = from_wrapped(bank.collected_insurance_fees_outstanding.value);
        let program_fees = from_wrapped(bank.collected_program_fees_outstanding.value);

        invariant!(
            group_fees >= base.group_fees_outstanding,
            "bank {pk} collected_group_fees_outstanding regressed: baseline {}, current {}",
            base.group_fees_outstanding,
            group_fees
        );
        // Insurance fees can be drained by bankruptcy (insurance vault
        // covers bad debt). Skip the monotonicity check on banks that
        // saw a successful bankruptcy.
        if !banks_with_bankruptcy.contains(&pk) {
            invariant!(
                insurance_fees >= base.insurance_fees_outstanding,
                "bank {pk} collected_insurance_fees_outstanding regressed without bankruptcy: baseline {}, current {}",
                base.insurance_fees_outstanding,
                insurance_fees
            );
        }
        invariant!(
            program_fees >= base.program_fees_outstanding,
            "bank {pk} collected_program_fees_outstanding regressed: baseline {}, current {}",
            base.program_fees_outstanding,
            program_fees
        );

        // `last_update` must move forward across a sequence. The `#[end]`
        // hook explicitly accrues all banks before the invariant block,
        // so every bank has been touched at least once → strict `>`.
        invariant!(
            bank.last_update > base.last_update,
            "bank {pk} last_update did not advance: baseline {}, current {}",
            base.last_update,
            bank.last_update
        );
    }
}

/// Global-consistency check: for each bank, the sum of `asset_shares` (and
/// `liability_shares`) across every marginfi account must not exceed the
/// bank's totals — anything else means the bank's bookkeeping is out of
/// sync with the per-account ledger.
///
/// The `position_counts` invariant checks balance *count* matches; this
/// checks balance *sum*. A bug that miscredits a balance (e.g. adds shares
/// without bumping `total_asset_shares`) would pass position_counts but
/// fail here. Conversely, a bug that increments totals without crediting
/// any user would pass here but produce phantom shares that solvency
/// might absorb — both invariants together close the gap.
///
/// Tolerance: `position_counts`'s `ZERO_AMOUNT_THRESHOLD` (1e-4 in I80F48)
/// covers per-balance dust; for the sum we allow `n_accounts ×
/// tolerance` in addition since each balance can carry its own dust.
pub fn assert_cumulative_shares_within_totals(
    trident: &mut Trident,
    bank_pks: &[Pubkey],
    marginfi_accounts: &[Pubkey],
) {
    // 1e-4 per balance × N balances per bank tolerance.
    let per_balance_tolerance = I80F48::from_bits(28_147_497_671);
    let total_tolerance =
        per_balance_tolerance.saturating_mul(I80F48::from_num(marginfi_accounts.len() as i64));

    for &bank_pk in bank_pks {
        let bank = trident
            .get_account_with_type::<Bank>(&bank_pk, None)
            .expect("bank must deserialize");

        let bank_total_assets = from_wrapped(bank.total_asset_shares.value);
        let bank_total_liabs = from_wrapped(bank.total_liability_shares.value);

        let mut sum_assets = I80F48::ZERO;
        let mut sum_liabs = I80F48::ZERO;

        for &acc_pk in marginfi_accounts {
            let Some(acc) = trident.get_account_with_type::<MarginfiAccount>(&acc_pk, None) else {
                continue;
            };
            for balance in &acc.lending_account.balances {
                if balance.active == 0 || balance.bank_pk != bank_pk {
                    continue;
                }
                sum_assets += from_wrapped(balance.asset_shares.value);
                sum_liabs += from_wrapped(balance.liability_shares.value);
            }
        }

        invariant!(
            sum_assets <= bank_total_assets + total_tolerance,
            "bank {bank_pk}: cumulative user asset_shares {sum_assets} > bank.total_asset_shares {bank_total_assets} (tol {total_tolerance})"
        );
        invariant!(
            sum_liabs <= bank_total_liabs + total_tolerance,
            "bank {bank_pk}: cumulative user liability_shares {sum_liabs} > bank.total_liability_shares {bank_total_liabs} (tol {total_tolerance})"
        );
    }
}
