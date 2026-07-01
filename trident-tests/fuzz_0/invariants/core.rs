#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

pub fn token_balance(trident: &mut Trident, token_account_pk: Pubkey) -> u64 {
    let res = trident.get_token_account(token_account_pk);
    invariant!(
        res.is_ok(),
        "token_balance: get_token_account failed for {}, err: {:?}",
        token_account_pk,
        res.as_ref().err()
    );
    res.unwrap().account.amount
}

pub fn assert_no_balance_change(
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    invariant!(
        user_after == user_before,
        "no_balance_change: user token changed. before: {}, after: {}, delta: {}",
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        vault_after == vault_before,
        "no_balance_change: vault changed. before: {}, after: {}, delta: {}",
        vault_before,
        vault_after,
        vault_after as i128 - vault_before as i128
    );
}

pub fn assert_user_vault_token_conservation(
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    let net_user = user_after as i128 - user_before as i128;
    let net_vault = vault_after as i128 - vault_before as i128;
    invariant!(
        net_user + net_vault == 0,
        "user_vault_conservation: net_user {}, net_vault {}, sum {} (expected 0). user before/after: {}/{}, vault before/after: {}/{}",
        net_user,
        net_vault,
        net_user + net_vault,
        user_before,
        user_after,
        vault_before,
        vault_after
    );
}

pub fn assert_balance_unchanged(before: u64, after: u64) {
    invariant!(
        before == after,
        "balance_unchanged: before: {}, after: {}, delta: {}",
        before,
        after,
        after as i128 - before as i128
    );
}

pub fn flashloan_empty_balance_snapshot(
    trident: &mut Trident,
    liquidity_vaults: &[Pubkey],
    extra_token_accounts: &[Pubkey],
) -> Vec<(Pubkey, u64)> {
    let mut out = Vec::with_capacity(liquidity_vaults.len() + extra_token_accounts.len());
    for pk in liquidity_vaults {
        out.push((*pk, token_balance(trident, *pk)));
    }
    for pk in extra_token_accounts {
        out.push((*pk, token_balance(trident, *pk)));
    }
    out
}

pub fn assert_token_snapshot_unchanged(trident: &mut Trident, snap: &[(Pubkey, u64)]) {
    for (pk, before) in snap {
        let after = token_balance(trident, *pk);
        invariant!(
            after == *before,
            "token_snapshot_unchanged: account {} before: {}, after: {}, delta: {}",
            pk,
            before,
            after,
            after as i128 - *before as i128
        );
    }
}

pub fn assert_deposit_balance_invariants(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    invariant!(
        user_after <= user_before,
        "deposit_balance: user tokens should not increase. amount: {}, user before: {}, after: {}, delta: {}",
        amount,
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        vault_after >= vault_before,
        "deposit_balance: vault should not decrease. amount: {}, vault before: {}, after: {}, delta: {}",
        amount,
        vault_before,
        vault_after,
        vault_after as i128 - vault_before as i128
    );

    if amount > 0 {
        invariant!(
            user_after < user_before,
            "deposit_balance: non-zero deposit should decrease user. amount: {}, user before: {}, after: {}",
            amount,
            user_before,
            user_after
        );
        invariant!(
            vault_after > vault_before,
            "deposit_balance: non-zero deposit should increase vault. amount: {}, vault before: {}, after: {}",
            amount,
            vault_before,
            vault_after
        );
    }
    assert_user_vault_token_conservation(user_before, user_after, vault_before, vault_after);
}

pub fn assert_withdraw_balance_invariants(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    invariant!(
        user_after >= user_before,
        "withdraw_balance: user tokens should not decrease. amount: {}, user before: {}, after: {}, delta: {}",
        amount,
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        vault_after <= vault_before,
        "withdraw_balance: vault should not increase. amount: {}, vault before: {}, after: {}, delta: {}",
        amount,
        vault_before,
        vault_after,
        vault_after as i128 - vault_before as i128
    );

    if amount > 0 {
        invariant!(
            user_after > user_before,
            "withdraw_balance: non-zero withdraw should increase user. amount: {}, user before: {}, after: {}",
            amount,
            user_before,
            user_after
        );
        invariant!(
            vault_after < vault_before,
            "withdraw_balance: non-zero withdraw should decrease vault. amount: {}, vault before: {}, after: {}",
            amount,
            vault_before,
            vault_after
        );
    }
    assert_user_vault_token_conservation(user_before, user_after, vault_before, vault_after);
}

pub fn assert_borrow_balance_invariants(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    assert_withdraw_balance_invariants(amount, user_before, user_after, vault_before, vault_after);
}

pub fn assert_repay_balance_invariants(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    assert_deposit_balance_invariants(amount, user_before, user_after, vault_before, vault_after);
}
