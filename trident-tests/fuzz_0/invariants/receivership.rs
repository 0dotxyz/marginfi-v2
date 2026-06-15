#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

use crate::types::marginfi::LiquidationRecord;
use crate::types::marginfi::MarginfiAccount;

const ACCOUNT_IN_RECEIVERSHIP: u64 = 1 << 4;
const ACCOUNT_IN_FLASHLOAN: u64 = 1 << 1;
const ACCOUNT_IN_ORDER_EXECUTION: u64 = 1 << 7;

pub fn assert_receivership_cleared_after_success(
    trident: &mut Trident,
    marginfi_account_pk: Pubkey,
    liquidation_record_pk: Pubkey,
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
}
