#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

pub fn assert_juplend_deposit_success(
    amount: u64,
    user_before: u64,
    user_after: u64,
    liquidity_vault_before: u64,
    liquidity_vault_after: u64,
    f_token_vault_before: u64,
    f_token_vault_after: u64,
) {
    invariant!(
        liquidity_vault_after == liquidity_vault_before,
        "juplend deposit: liquidity_vault should be net unchanged. before: {}, after: {}, delta: {}",
        liquidity_vault_before,
        liquidity_vault_after,
        liquidity_vault_after as i128 - liquidity_vault_before as i128
    );

    if amount == 0 {
        invariant!(
            user_after == user_before,
            "juplend deposit: zero amount must not change user tokens. before: {}, after: {}, delta: {}",
            user_before,
            user_after,
            user_after as i128 - user_before as i128
        );
        invariant!(
            f_token_vault_after == f_token_vault_before,
            "juplend deposit: zero amount must not change fToken vault. before: {}, after: {}, delta: {}",
            f_token_vault_before,
            f_token_vault_after,
            f_token_vault_after as i128 - f_token_vault_before as i128
        );
        return;
    }

    invariant!(
        user_before - user_after == amount,
        "juplend deposit: user outflow should equal amount. requested: {}, user before: {}, after: {}, actual delta: {}",
        amount,
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        f_token_vault_after > f_token_vault_before,
        "juplend deposit: fToken vault should increase when amount > 0. before: {}, after: {}, delta: {}",
        f_token_vault_before,
        f_token_vault_after,
        f_token_vault_after as i128 - f_token_vault_before as i128
    );
}

pub fn assert_juplend_deposit_failure_balances_unchanged(
    amount: u64,
    user_before: u64,
    user_after: u64,
    liquidity_vault_before: u64,
    liquidity_vault_after: u64,
    f_token_vault_before: u64,
    f_token_vault_after: u64,
) {
    invariant!(
        user_after == user_before,
        "juplend deposit failure: user tokens changed (amount {}). before: {}, after: {}, delta: {}",
        amount,
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        liquidity_vault_after == liquidity_vault_before,
        "juplend deposit failure: liquidity_vault changed (amount {}). before: {}, after: {}, delta: {}",
        amount,
        liquidity_vault_before,
        liquidity_vault_after,
        liquidity_vault_after as i128 - liquidity_vault_before as i128
    );
    invariant!(
        f_token_vault_after == f_token_vault_before,
        "juplend deposit failure: fToken vault changed (amount {}). before: {}, after: {}, delta: {}",
        amount,
        f_token_vault_before,
        f_token_vault_after,
        f_token_vault_after as i128 - f_token_vault_before as i128
    );
}
