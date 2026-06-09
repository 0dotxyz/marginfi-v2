#![allow(clippy::too_many_arguments)]

//! Global-state invariants: account flag-context consistency and
//! group/fee-state immutability across a fuzz sequence.
//!
//! Marginfi defines several account-state flags. The harness must
//! never see these set at the end of a sequence:
//!   - `ACCOUNT_IN_FLASHLOAN`     (bit 1, `1 << 1 = 0x02`) — transient
//!   - `ACCOUNT_IN_RECEIVERSHIP`  (bit 4, `1 << 4 = 0x10`) — transient
//!   - `ACCOUNT_IN_DELEVERAGE`    (bit 5, `1 << 5 = 0x20`) — transient
//!   - `ACCOUNT_FROZEN`           (bit 6, `1 << 6 = 0x40`) — admin-set
//!   - `ACCOUNT_IN_ORDER_EXECUTION` (bit 7, `1 << 7 = 0x80`) — transient
//!
//! Transients (`FLASHLOAN`/`RECEIVERSHIP`/`DELEVERAGE`/`ORDER_EXECUTION`)
//! are cleared by their respective `*End*` ixs; `FROZEN` is admin-set
//! and the harness never invokes the freeze ix. Stuck bits indicate
//! either a forgotten end-ix or a backdoor flag set.
//!
//! `ACCOUNT_DISABLED` (bit 0) is *intentionally excluded* — marginfi
//! sets it as part of `handle_bankruptcy` and `TransferToNewAccount`,
//! so the engineered bankruptcy flow correctly leaves it on User D
//! roughly once per sequence.
//!
//! `MarginfiGroup` and `FeeState` are never modified by the lending ixs
//! the harness exercises (no admin / config / pause ixs in flows). Raw-
//! bytes equality between an end-of-`#[init]` snapshot and the value at
//! `#[end]` catches any backdoor that mutates either account.

use trident_fuzz::fuzzing::*;

use crate::types::marginfi::MarginfiAccount;

/// Combined mask of the bits that must never be set at the end of a
/// fuzz sequence. Includes genuine transients (cleared by their
/// `*End*` ix) plus `FROZEN` (admin-set, the harness never freezes).
/// Bit 0 (`ACCOUNT_DISABLED`) is excluded because `handle_bankruptcy`
/// sets it legitimately. Bits 2 and 3 are deprecated.
pub const TRANSIENT_FLAGS_MASK: u64 =
    (1 << 1) | (1 << 4) | (1 << 5) | (1 << 6) | (1 << 7);

/// Snapshot of an account's raw data, for immutability checking.
#[derive(Clone, Debug)]
pub struct AccountDataSnapshot {
    pub key: Pubkey,
    pub data: Vec<u8>,
}

/// Capture an account's full data buffer. Use at end-of-`#[init]` to
/// pin the reference state; `assert_account_data_unchanged` re-reads
/// and compares.
pub fn snapshot_account_data(trident: &mut Trident, key: Pubkey) -> AccountDataSnapshot {
    let acc = trident.get_account(&key);
    AccountDataSnapshot {
        key,
        data: acc.data().to_vec(),
    }
}

/// Assert the account's current data matches the snapshot byte-for-byte.
/// On mismatch the panic message points to the first differing index
/// for fast triage.
pub fn assert_account_data_unchanged(trident: &mut Trident, snap: &AccountDataSnapshot) {
    let acc = trident.get_account(&snap.key);
    let current = acc.data();
    invariant!(
        current == snap.data.as_slice(),
        "account {} data changed during fuzz (len before {}, after {}, first diff @ {})",
        snap.key,
        snap.data.len(),
        current.len(),
        snap.data
            .iter()
            .zip(current.iter())
            .position(|(a, b)| a != b)
            .map(|i| i.to_string())
            .unwrap_or_else(|| "length differs".to_string())
    );
}

/// Assert no marginfi account ended the sequence with a transient flag
/// stuck. Any `1` bit at the masked positions means a `*End*` ix
/// failed to clear its corresponding flag — a real bug, since the
/// harness's `#[end]` runs after every flow completes.
pub fn assert_marginfi_accounts_have_no_transient_flags(
    trident: &mut Trident,
    marginfi_accounts: &[Pubkey],
) {
    for &pk in marginfi_accounts {
        let Some(acc) = trident.get_account_with_type::<MarginfiAccount>(&pk, None) else {
            continue;
        };
        let stuck = acc.account_flags & TRANSIENT_FLAGS_MASK;
        invariant!(
            stuck == 0,
            "marginfi account {pk} has stuck transient flags 0x{stuck:x} (full account_flags 0x{:x})",
            acc.account_flags
        );
    }
}

/// Assert every account in the harness's tracked set still references
/// the expected `marginfi_group`. A re-parented account would be a
/// silent ownership-transfer bug — possible if an admin or transfer ix
/// were to incorrectly mutate the field.
pub fn assert_marginfi_accounts_group_unchanged(
    trident: &mut Trident,
    marginfi_accounts: &[Pubkey],
    expected_group: Pubkey,
) {
    for &pk in marginfi_accounts {
        let Some(acc) = trident.get_account_with_type::<MarginfiAccount>(&pk, None) else {
            continue;
        };
        invariant!(
            acc.group == expected_group,
            "marginfi account {pk} group changed: expected {expected_group}, found {}",
            acc.group
        );
    }
}

/// Assert no marginfi account holds two active balances pointing at the
/// same bank. `BankAccountWrapper::find_or_create` is the only path
/// that should open a balance, and it re-uses an existing slot for the
/// same `bank_pk`. Two active slots for the same bank means the
/// program leaked a balance — a bookkeeping bug invisible to per-bank
/// totals but visible here.
pub fn assert_no_duplicate_bank_balances(trident: &mut Trident, marginfi_accounts: &[Pubkey]) {
    for &pk in marginfi_accounts {
        let Some(acc) = trident.get_account_with_type::<MarginfiAccount>(&pk, None) else {
            continue;
        };
        let mut seen: Vec<Pubkey> = Vec::new();
        for balance in &acc.lending_account.balances {
            if balance.active == 0 {
                continue;
            }
            invariant!(
                !seen.contains(&balance.bank_pk),
                "marginfi account {pk} has duplicate active balances for bank {} ({} total active so far)",
                balance.bank_pk,
                seen.len() + 1
            );
            seen.push(balance.bank_pk);
        }
    }
}
