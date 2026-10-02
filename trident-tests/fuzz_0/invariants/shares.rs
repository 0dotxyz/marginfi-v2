#![allow(clippy::too_many_arguments)]

use std::cmp::Ordering;

use fixed::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::types::marginfi::MarginfiAccount;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankShareSnapshot {
    pub had_active_balance: bool,
    pub asset_shares: [u8; 16],
    pub liability_shares: [u8; 16],
    pub premium_outstanding: [u8; 16],
}

pub fn marginfi_bank_share_snapshot(
    trident: &mut Trident,
    marginfi_account: Pubkey,
    bank_pk: Pubkey,
) -> BankShareSnapshot {
    let acc = trident
        .get_account_with_type::<MarginfiAccount>(&marginfi_account, None)
        .expect("marginfi account");
    for b in &acc.lending_account.balances {
        if b.active != 0 && b.bank_pk == bank_pk {
            return BankShareSnapshot {
                had_active_balance: true,
                asset_shares: b.asset_shares.value,
                liability_shares: b.liability_shares.value,
                premium_outstanding: b.premium_outstanding.value,
            };
        }
    }
    BankShareSnapshot {
        had_active_balance: false,
        asset_shares: [0u8; 16],
        liability_shares: [0u8; 16],
        premium_outstanding: [0u8; 16],
    }
}

fn assert_zero_amount_find_or_create_shares_ok(
    before: &BankShareSnapshot,
    after: &BankShareSnapshot,
    op: &'static str,
) {
    // Note: any touch of an existing liability balance claims premium (materializing pending
    // interest into the receivable), so the receivable is deliberately NOT compared here.
    if before.had_active_balance == after.had_active_balance
        && before.asset_shares == after.asset_shares
        && before.liability_shares == after.liability_shares
    {
        return;
    }
    invariant!(
        !before.had_active_balance && after.had_active_balance,
        "{op}: zero-amount success may only open an empty bank slot. before.had_active: {}, after.had_active: {}",
        before.had_active_balance,
        after.had_active_balance
    );
    invariant!(
        after.asset_shares == [0u8; 16] && after.liability_shares == [0u8; 16],
        "{op}: newly opened slot must have zero asset and liability shares. after asset_shares: {:?}, liability_shares: {:?}",
        after.asset_shares,
        after.liability_shares
    );
}

fn i80_from_share_bytes(bytes: &[u8; 16]) -> I80F48 {
    I80F48::from_bits(i128::from_le_bytes(*bytes))
}

/// Bit-precision tolerance for "must not change" share assertions
/// (1e-10 in I80F48). Sized to land between two regimes:
///
/// * **Above** the I80F48 precision floor (≈ 2⁻⁴⁸ ≈ 3.55e-15) by ~5
///   orders of magnitude — absorbs the sub-bit residues the audit-
///   fixed `increase_balance_internal` / `decrease_balance_internal`
///   leave behind (e.g. `0 → 4e-15` on a 1-native-unit repay, `4e-15
///   → 0` on a borrow that touches a balance with residual dust).
/// * **Below** `ZERO_AMOUNT_THRESHOLD` (1e-4) by 6 orders of magnitude
///   — well under the M12 bug's typical 3e-6 dust, so the M12
///   reproduction in `#[init]` still detects a regression.
///
/// In bits: 1e-10 × 2⁴⁸ ≈ 28_147_497. Roughly 280× the precision
/// floor and 280×× looser than the M12 bug magnitude.
const SHARE_INVARIANCE_TOLERANCE_BITS: i128 = 28_147_497;

fn share_invariance_tolerance() -> I80F48 {
    I80F48::from_bits(SHARE_INVARIANCE_TOLERANCE_BITS)
}

/// Returns true iff `before` and `after` differ by at most the
/// bit-precision-residue tolerance — i.e. the change is "close enough
/// to no change" to be the audit-fix's sub-precision clean-up
/// rather than an M12-style dust injection.
fn dust_clear_allowed(before: I80F48, after: I80F48) -> bool {
    (after - before).abs() < share_invariance_tolerance()
}

pub fn assert_exact_deposit_token_leg(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    if amount == 0 {
        return;
    }
    invariant!(
        user_before - user_after == amount,
        "deposit exact token leg: user decrease should equal amount. requested: {}, user before: {}, after: {}, actual decrease (signed): {}",
        amount,
        user_before,
        user_after,
        user_before as i128 - user_after as i128
    );
    invariant!(
        vault_after - vault_before == amount,
        "deposit exact token leg: vault increase should equal amount. requested: {}, vault before: {}, after: {}, actual increase (signed): {}",
        amount,
        vault_before,
        vault_after,
        vault_after as i128 - vault_before as i128
    );
}

pub fn assert_exact_user_vault_delta_withdraw(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    if amount == 0 {
        return;
    }
    invariant!(
        user_after - user_before == amount,
        "withdraw/borrow exact token leg: user increase should equal amount. requested: {}, user before: {}, after: {}, actual change (signed): {}",
        amount,
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        vault_before - vault_after == amount,
        "withdraw/borrow exact token leg: vault decrease should equal amount. requested: {}, vault before: {}, after: {}, actual change (signed): {}",
        amount,
        vault_before,
        vault_after,
        vault_after as i128 - vault_before as i128
    );
}

pub fn assert_repay_user_token_delta_matches_post_fee_amount(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    if amount == 0 {
        return;
    }
    let paid = user_before - user_after;
    let vault_in = vault_after - vault_before;
    invariant!(
        paid == vault_in,
        "repay: user outflow should match vault inflow. user before/after: {}/{}, paid: {}, vault before/after: {}/{}, vault_in: {}",
        user_before,
        user_after,
        paid,
        vault_before,
        vault_after,
        vault_in
    );
    invariant!(
        paid == amount,
        "repay: token leg should match post-fee amount (no transfer fee in fuzz). expected: {}, user before: {}, after: {}, paid: {}",
        amount,
        user_before,
        user_after,
        paid
    );
}

pub fn assert_deposit_success_share_invariants(
    before: &BankShareSnapshot,
    after: &BankShareSnapshot,
    amount: u64,
) {
    if amount == 0 {
        assert_zero_amount_find_or_create_shares_ok(before, after, "deposit");
        return;
    }
    invariant!(
        after.had_active_balance,
        "deposit shares: bank balance should be active after success. amount: {}, had_active after: {}",
        amount,
        after.had_active_balance
    );
    let a0 = i80_from_share_bytes(&before.asset_shares);
    let a1 = i80_from_share_bytes(&after.asset_shares);
    let l0 = i80_from_share_bytes(&before.liability_shares);
    let l1 = i80_from_share_bytes(&after.liability_shares);
    invariant!(
        l0 == l1 || dust_clear_allowed(l0, l1),
        "deposit shares: liability shares must not change. amount: {}, before: {}, after: {}",
        amount,
        l0,
        l1
    );
    invariant!(
        a1.cmp(&a0) == Ordering::Greater,
        "deposit shares: asset shares must increase. amount: {}, asset before: {}, after: {}, cmp: {:?}",
        amount,
        a0,
        a1,
        a1.cmp(&a0)
    );
}

pub fn assert_withdraw_success_share_invariants(
    before: &BankShareSnapshot,
    after: &BankShareSnapshot,
    amount: u64,
) {
    if amount == 0 {
        invariant!(
            before == after,
            "withdraw shares: zero amount should not change lending shares. before: {:?}, after: {:?}",
            before,
            after
        );
        return;
    }
    invariant!(
        before.had_active_balance,
        "withdraw shares: need an open position before withdraw. amount: {}, had_active before: {}",
        amount,
        before.had_active_balance
    );
    let a0 = i80_from_share_bytes(&before.asset_shares);
    let a1 = i80_from_share_bytes(&after.asset_shares);
    let l0 = i80_from_share_bytes(&before.liability_shares);
    let l1 = i80_from_share_bytes(&after.liability_shares);
    invariant!(
        l0 == l1 || dust_clear_allowed(l0, l1),
        "withdraw shares: liability shares must not change. amount: {}, before: {}, after: {}",
        amount,
        l0,
        l1
    );
    if after.had_active_balance {
        invariant!(
            a1.cmp(&a0) == Ordering::Less,
            "withdraw shares: asset shares must decrease when balance stays open. amount: {}, asset before: {}, after: {}, cmp: {:?}",
            amount,
            a0,
            a1,
            a1.cmp(&a0)
        );
    } else {
        invariant!(
            a0 > I80F48::ZERO,
            "withdraw shares: full close implies prior assets. amount: {}, asset before: {}",
            amount,
            a0
        );
        invariant!(
            a1 == I80F48::ZERO && l1 == I80F48::ZERO,
            "withdraw shares: closed row should zero shares. amount: {}, asset after: {}, liability after: {}",
            amount,
            a1,
            l1
        );
    }
}

pub fn assert_borrow_success_share_invariants(
    before: &BankShareSnapshot,
    after: &BankShareSnapshot,
    amount: u64,
) {
    if amount == 0 {
        assert_zero_amount_find_or_create_shares_ok(before, after, "borrow");
        return;
    }
    invariant!(
        after.had_active_balance,
        "borrow shares: bank balance should be active after success. amount: {}, had_active after: {}",
        amount,
        after.had_active_balance
    );
    let a0 = i80_from_share_bytes(&before.asset_shares);
    let a1 = i80_from_share_bytes(&after.asset_shares);
    let l0 = i80_from_share_bytes(&before.liability_shares);
    let l1 = i80_from_share_bytes(&after.liability_shares);
    invariant!(
        a0 == a1 || dust_clear_allowed(a0, a1),
        "borrow shares: asset shares must not change. amount: {}, before: {}, after: {}",
        amount,
        a0,
        a1
    );
    invariant!(
        l1.cmp(&l0) == Ordering::Greater,
        "borrow shares: liability shares must increase. amount: {}, liability before: {}, after: {}, cmp: {:?}",
        amount,
        l0,
        l1,
        l1.cmp(&l0)
    );
}

pub fn assert_repay_success_share_invariants(
    before: &BankShareSnapshot,
    after: &BankShareSnapshot,
    amount: u64,
) {
    if amount == 0 {
        // Note: a zero-amount repay still claims premium (materializing pending interest into
        // the receivable), so only the share fields are compared.
        invariant!(
            before.had_active_balance == after.had_active_balance
                && before.asset_shares == after.asset_shares
                && before.liability_shares == after.liability_shares,
            "repay shares: zero amount should not change lending shares. before: {:?}, after: {:?}",
            before,
            after
        );
        return;
    }
    invariant!(
        before.had_active_balance,
        "repay shares: need an open position before repay. amount: {}, had_active before: {}",
        amount,
        before.had_active_balance
    );
    let a0 = i80_from_share_bytes(&before.asset_shares);
    let a1 = i80_from_share_bytes(&after.asset_shares);
    let l0 = i80_from_share_bytes(&before.liability_shares);
    let l1 = i80_from_share_bytes(&after.liability_shares);
    invariant!(
        a0 == a1 || dust_clear_allowed(a0, a1),
        "repay shares: asset shares must not change. amount: {}, before: {}, after: {}",
        amount,
        a0,
        a1
    );
    if after.had_active_balance {
        // Premium settles BEFORE principal: a repay smaller than the premium receivable leaves
        // liability shares untouched and only moves the receivable (which the claim that runs in
        // the same ix may simultaneously top up with newly-materialized pending premium).
        let premium_moved = before.premium_outstanding != after.premium_outstanding;
        invariant!(
            l1.cmp(&l0) == Ordering::Less || premium_moved,
            "repay shares: liability shares must decrease (or the premium receivable must move) when balance stays open. amount: {}, liability before: {}, after: {}, premium before: {}, after: {}",
            amount,
            l0,
            l1,
            i80_from_share_bytes(&before.premium_outstanding),
            i80_from_share_bytes(&after.premium_outstanding)
        );
    } else {
        invariant!(
            l0 > I80F48::ZERO,
            "repay shares: full close implies prior liabilities. amount: {}, liability before: {}",
            amount,
            l0
        );
    }
}
