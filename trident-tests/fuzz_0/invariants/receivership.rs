#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

use crate::types::marginfi::LiquidationRecord;
use crate::types::marginfi::MarginfiAccount;

const ACCOUNT_IN_RECEIVERSHIP: u64 = 1 << 4;
const ACCOUNT_IN_FLASHLOAN: u64 = 1 << 1;
const ACCOUNT_IN_ORDER_EXECUTION: u64 = 1 << 7;

/// Reads the account's premium-growth tag, or 0 when the account doesn't exist.
pub fn read_tagged_at(trident: &mut Trident, marginfi_account_pk: Pubkey) -> i64 {
    trident
        .get_account_with_type::<MarginfiAccount>(&marginfi_account_pk, None)
        .map(|a| a.liquidation_tagged_at)
        .unwrap_or(0)
}

/// A completed liquidation may clear the premium-growth tag, restart it at the current time, or
/// leave it untouched, but must never tag an untagged record nor move a tag backwards.
pub fn assert_tag_cleared_or_advanced(
    trident: &mut Trident,
    marginfi_account_pk: Pubkey,
    tagged_at_before: i64,
    ctx: &str,
) {
    let after = read_tagged_at(trident, marginfi_account_pk);
    invariant!(
        after == 0 || (tagged_at_before != 0 && after >= tagged_at_before),
        "{}: a liquidation must clear or advance tagged_at, never tag an untagged record. before: {}, after: {}",
        ctx,
        tagged_at_before,
        after
    );
}

pub fn assert_receivership_cleared_after_success(
    trident: &mut Trident,
    marginfi_account_pk: Pubkey,
    liquidation_record_pk: Pubkey,
    tagged_at_before: i64,
) {
    let m = trident
        .get_account_with_type::<MarginfiAccount>(&marginfi_account_pk, None)
        .expect("marginfi account");
    invariant!(
        m.account_flags & ACCOUNT_IN_RECEIVERSHIP == 0,
        "receivership: marginfi account must leave ACCOUNT_IN_RECEIVERSHIP. account_flags: {:#x}, expected bit clear",
        m.account_flags
    );
    invariant!(
        m.account_flags & ACCOUNT_IN_FLASHLOAN == 0,
        "receivership: must not leave ACCOUNT_IN_FLASHLOAN set. account_flags: {:#x}",
        m.account_flags
    );
    invariant!(
        m.account_flags & ACCOUNT_IN_ORDER_EXECUTION == 0,
        "receivership: must not leave ACCOUNT_IN_ORDER_EXECUTION set. account_flags: {:#x}",
        m.account_flags
    );
    let rec = trident
        .get_account_with_type::<LiquidationRecord>(&liquidation_record_pk, None)
        .expect("liquidation record");
    invariant!(
        rec.marginfi_account == marginfi_account_pk,
        "receivership: liquidation_record.marginfi_account mismatch. expected: {}, got: {}",
        marginfi_account_pk,
        rec.marginfi_account
    );
    invariant!(
        rec.liquidation_receiver == Pubkey::default(),
        "receivership: liquidation_receiver should be cleared. got: {}",
        rec.liquidation_receiver
    );
    let newest = &rec.entries[3];
    invariant!(
        newest.timestamp != 0,
        "receivership: newest liquidation entry should record a timestamp. entries[3].timestamp: {}",
        newest.timestamp
    );
    invariant!(
        m.liquidation_tagged_at == 0
            || (tagged_at_before != 0 && m.liquidation_tagged_at >= tagged_at_before),
        "receivership: a liquidation must clear or advance tagged_at, never tag an untagged record. before: {}, after: {}",
        tagged_at_before,
        m.liquidation_tagged_at
    );
}
