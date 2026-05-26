#![allow(clippy::too_many_arguments)]

//! Bank-level solvency invariant ported from the legacy libfuzzer harness
//! (`programs/marginfi/fuzz/fuzz_targets/lend.rs::verify_end_state`).
//!
//! For every bank, the SPL token balance held in the liquidity vault, minus
//! the bank's outstanding fee buckets, must equal `total_deposits −
//! total_liabilities` (where each is shares × share-value), within a 1-unit
//! tolerance. This is the on-chain analog of "the cash drawer reconciles
//! with the books" — interest accrual must mint the same value of shares it
//! changes the share-value by, and every deposit/withdraw/borrow/repay must
//! leave the equation unchanged.
//!
//! The libfuzzer original called `bank_data.accrue_interest(...)` directly
//! on the in-memory `Bank`; we instead submit a real
//! `LendingPoolAccrueBankInterest` ix before checking (see the call site in
//! `methods/core.rs::lending_pool_accrue_all_banks`).

use fixed::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::types::marginfi::Bank;

use super::token_balance;

/// Convert the IDL-mirrored 16-byte representation back into `I80F48`.
fn from_wrapped(bytes: [u8; 16]) -> I80F48 {
    I80F48::from_bits(i128::from_le_bytes(bytes))
}

/// Asserts the bank-solvency invariant for a single bank account.
///
/// Panics via `invariant!` if `|vault_balance − outstanding_fees − net_book|`
/// exceeds 1 native token unit (matching the libfuzzer original).
pub fn assert_bank_solvency(trident: &mut Trident, bank_pk: Pubkey) {
    let bank = trident
        .get_account_with_type::<Bank>(&bank_pk, None)
        .expect("bank account must deserialize");

    let asset_share_value = from_wrapped(bank.asset_share_value.value);
    let liability_share_value = from_wrapped(bank.liability_share_value.value);
    let total_asset_shares = from_wrapped(bank.total_asset_shares.value);
    let total_liability_shares = from_wrapped(bank.total_liability_shares.value);

    let total_deposits = total_asset_shares
        .checked_mul(asset_share_value)
        .expect("total_deposits overflow");
    let total_liabilities = total_liability_shares
        .checked_mul(liability_share_value)
        .expect("total_liabilities overflow");

    let outstanding_fees = from_wrapped(bank.collected_group_fees_outstanding.value)
        + from_wrapped(bank.collected_insurance_fees_outstanding.value)
        + from_wrapped(bank.collected_program_fees_outstanding.value);

    let vault_balance = I80F48::from_num(token_balance(trident, bank.liquidity_vault));
    let net_vault = vault_balance - outstanding_fees;
    let net_book = total_deposits - total_liabilities;

    let drift = (net_vault - net_book).abs();
    invariant!(
        drift <= I80F48::ONE,
        "bank solvency drift > 1 unit. bank: {bank_pk}\n  vault_balance: {vault_balance}\n  outstanding_fees: {outstanding_fees}\n  net_vault (vault - fees): {net_vault}\n  total_deposits: {total_deposits}\n  total_liabilities: {total_liabilities}\n  net_book (deposits - liabs): {net_book}\n  drift: {drift}"
    );
}
