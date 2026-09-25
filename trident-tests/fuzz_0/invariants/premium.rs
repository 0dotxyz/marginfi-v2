//! Variable-borrow premium invariants.
//!
//! The premium is a per-balance receivable (`premium_outstanding`) accrued lazily at the
//! `premium_rate_snapshot` and settled (with real tokens) only on repay, where it moves into
//! `bank.collected_premium_outstanding` pending sweep. These invariants pin the receivable's
//! sanity under arbitrary instruction sequences.

use fixed::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::types::marginfi::{Bank, MarginfiAccount};

fn from_wrapped(bytes: [u8; 16]) -> I80F48 {
    I80F48::from_le_bytes(bytes)
}

pub fn assert_premium_invariants(
    trident: &mut Trident,
    banks: &[Pubkey],
    marginfi_accounts: &[Pubkey],
) {
    for account_pk in marginfi_accounts {
        let Some(account) = trident.get_account_with_type::<MarginfiAccount>(account_pk, None)
        else {
            continue;
        };

        for balance in &account.lending_account.balances {
            if balance.active == 0 {
                continue;
            }
            let outstanding = from_wrapped(balance.premium_outstanding.value);
            let liability_shares = from_wrapped(balance.liability_shares.value);

            // Receivable is never negative.
            invariant!(
                outstanding >= I80F48::ZERO,
                "negative premium_outstanding {} on account {} bank {}",
                outstanding,
                account_pk,
                balance.bank_pk
            );

            // A balance with no liability carries no receivable: flips, closes, and repay_all
            // all settle or write it off.
            if liability_shares < I80F48::ONE {
                invariant!(
                    outstanding < I80F48::ONE,
                    "premium receivable {} stranded on non-liability balance (account {} bank {})",
                    outstanding,
                    account_pk,
                    balance.bank_pk
                );
                continue;
            }

        }
    }

    // Bank-side counter sanity: collected premium is never negative and, being realized-only,
    // never exceeds what the liquidity vault could back (checked more precisely by
    // `assert_bank_solvency`, which folds it into the fee buckets).
    for bank_pk in banks {
        let Some(bank) = trident.get_account_with_type::<Bank>(bank_pk, None) else {
            continue;
        };
        let collected = from_wrapped(bank.collected_premium_outstanding.value);
        invariant!(
            collected >= I80F48::ZERO,
            "negative collected_premium_outstanding {} on bank {}",
            collected,
            bank_pk
        );
    }
}
