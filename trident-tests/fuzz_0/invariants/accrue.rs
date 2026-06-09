#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

use crate::types::marginfi::Bank;

pub fn bank_last_update_snapshot(trident: &mut Trident, bank_pk: Pubkey) -> i64 {
    trident
        .get_account_with_type::<Bank>(&bank_pk, None)
        .expect("bank")
        .last_update
}

pub fn assert_accrue_advanced_bank_last_updates(
    trident: &mut Trident,
    bank_pks: &[Pubkey],
    last_updates_before: &[i64],
) {
    invariant!(
        bank_pks.len() == last_updates_before.len(),
        "accrue: snapshot length mismatch. bank_pks len: {}, last_updates_before len: {}",
        bank_pks.len(),
        last_updates_before.len()
    );
    for (&pk, &prev) in bank_pks.iter().zip(last_updates_before.iter()) {
        let now = bank_last_update_snapshot(trident, pk);
        invariant!(
            now > prev,
            "accrue: bank last_update should strictly increase. bank: {}, before: {}, after: {}, delta: {}",
            pk,
            prev,
            now,
            now as i128 - prev as i128
        );
    }
}
