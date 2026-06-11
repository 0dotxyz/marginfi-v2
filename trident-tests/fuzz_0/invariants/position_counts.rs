#![allow(clippy::too_many_arguments)]

//! Bank position-counter consistency.
//!
//! Each `Bank` carries `lending_position_count: i32` and
//! `borrowing_position_count: i32` — totals of how many marginfi accounts
//! hold a non-zero asset / liability position in that bank. The program
//! maintains them by calling `increment_lending_position_count` /
//! `decrement_lending_position_count` (and the borrowing equivalents)
//! whenever a balance crosses the `ZERO_AMOUNT_THRESHOLD` boundary in
//! `increase_balance_internal` / `decrease_balance_internal`.
//!
//! Neither libfuzzer nor Trident asserts these counters against the ground
//! truth (iterate every marginfi account, count how many have a non-zero
//! position in this bank). This invariant does. It catches:
//!
//! - The exact wiring class touched by the M12 audit fix: if dust ever
//!   crossed the threshold without an `increment_*_count` call, the counter
//!   would drift by one and any future bank operation that reads it (close
//!   bank, accrual fee accounting) would misbehave.
//! - Any new balance-mutating instruction (e.g. liquidate, receivership,
//!   future integrations) that forgets to maintain the counter on either
//!   the source or destination side.

use fixed::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::types::marginfi::{Bank, MarginfiAccount};

/// `ZERO_AMOUNT_THRESHOLD` mirrors the marginfi constant used by the
/// program when classifying a position as "active enough to count":
/// see `marginfi_type_crate::constants::ZERO_AMOUNT_THRESHOLD`.
const ZERO_AMOUNT_THRESHOLD_BITS: i128 = {
    // I80F48!(0.0001).to_bits() — encoded directly to avoid pulling
    // fixed-macro into this module. 0.0001 × 2^48 ≈ 28147497671.
    28_147_497_671
};

fn zero_amount_threshold() -> I80F48 {
    I80F48::from_bits(ZERO_AMOUNT_THRESHOLD_BITS)
}

fn shares(bytes: [u8; 16]) -> I80F48 {
    I80F48::from_bits(i128::from_le_bytes(bytes))
}

/// Asserts each bank's `lending_position_count` and
/// `borrowing_position_count` matches the actual count of marginfi accounts
/// with a non-zero balance on that side.
///
/// `marginfi_accounts` is the full set of accounts the harness tracks
/// (users + seeder + liquidator) — the program counter sums positions
/// across the whole group, so a partial slice would always under-count.
pub fn assert_bank_position_counts(
    trident: &mut Trident,
    bank_pks: &[Pubkey],
    marginfi_accounts: &[Pubkey],
) {
    let threshold = zero_amount_threshold();

    for &bank_pk in bank_pks {
        let bank = trident
            .get_account_with_type::<Bank>(&bank_pk, None)
            .expect("bank must deserialize");

        let mut actual_lenders = 0i32;
        let mut actual_borrowers = 0i32;

        for &acc_pk in marginfi_accounts {
            let Some(acc) = trident.get_account_with_type::<MarginfiAccount>(&acc_pk, None) else {
                continue; // account not initialized in this run — skip
            };
            for balance in &acc.lending_account.balances {
                if balance.active == 0 || balance.bank_pk != bank_pk {
                    continue;
                }
                if shares(balance.asset_shares.value).abs() > threshold {
                    actual_lenders += 1;
                }
                if shares(balance.liability_shares.value).abs() > threshold {
                    actual_borrowers += 1;
                }
            }
        }

        invariant!(
            bank.lending_position_count == actual_lenders,
            "bank {bank_pk} lending_position_count drift: counter says {}, actual lenders {}",
            bank.lending_position_count,
            actual_lenders
        );
        invariant!(
            bank.borrowing_position_count == actual_borrowers,
            "bank {bank_pk} borrowing_position_count drift: counter says {}, actual borrowers {}",
            bank.borrowing_position_count,
            actual_borrowers
        );
    }
}
