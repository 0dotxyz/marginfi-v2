#![allow(clippy::too_many_arguments)]

//! Per-account layout invariants.
//!
//! `LendingAccount::balances` is a fixed-size `[Balance; 16]`. The program
//! maintains a packed-active-first ordering: any slot with `active == 0` is
//! considered free, and the program's iteration / `find_or_create` /
//! `decrement_*_position_count` paths assume that once you encounter an
//! inactive slot, every subsequent slot is also inactive.
//!
//! Inspired by the libfuzzer harness's defensive `sort_balances` call before
//! every action: that call only made sense because the property *might* not
//! always hold. Asserting it after every successful op turns the assumption
//! into a checked invariant — catches any bug in the
//! `increment/decrement_*_position_count` wiring (the same code path the M12
//! audit finding touched) that could leave an active hole in the middle of
//! the array.

use trident_fuzz::fuzzing::*;

use crate::types::marginfi::MarginfiAccount;

/// Asserts the marginfi account's lending balances are packed: no `active=1`
/// slot follows an `active=0` slot.
///
/// Cheap: one account read + a 16-element scan. Safe to call after every
/// successful `lending_account_*` ix; meaningless on failure paths (the tx
/// rolled back, so the layout from the prior successful op already held).
pub fn assert_balances_packed(trident: &mut Trident, marginfi_account_pk: Pubkey) {
    let acc = trident
        .get_account_with_type::<MarginfiAccount>(&marginfi_account_pk, None)
        .expect("marginfi account must deserialize");

    let mut seen_inactive = false;
    for (idx, balance) in acc.lending_account.balances.iter().enumerate() {
        let active = balance.active != 0;
        if active && seen_inactive {
            invariant!(
                false,
                "balance layout: active slot at index {idx} follows an inactive slot in marginfi_account {marginfi_account_pk}. Layout: [{}]",
                acc.lending_account
                    .balances
                    .iter()
                    .map(|b| if b.active != 0 { "A" } else { "_" })
                    .collect::<Vec<_>>()
                    .join("")
            );
        }
        if !active {
            seen_inactive = true;
        }
    }
}
