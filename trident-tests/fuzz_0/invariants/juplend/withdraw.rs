#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

pub fn assert_juplend_withdraw_success(
    requested_amount: u64,
    withdraw_all: bool,
    user_before: u64,
    user_after: u64,
    withdraw_intermediary_before: u64,
    withdraw_intermediary_after: u64,
    liquidity_vault_before: u64,
    liquidity_vault_after: u64,
    f_token_vault_before: u64,
    f_token_vault_after: u64,
) {
    invariant!(
        liquidity_vault_after == liquidity_vault_before,
        "juplend withdraw: liquidity_vault should be unchanged. before: {}, after: {}, delta: {}",
        liquidity_vault_before,
        liquidity_vault_after,
        liquidity_vault_after as i128 - liquidity_vault_before as i128
    );

    invariant!(
        withdraw_intermediary_after == withdraw_intermediary_before,
        "juplend withdraw: withdraw intermediary ATA should be net unchanged. before: {}, after: {}, delta: {}",
        withdraw_intermediary_before,
        withdraw_intermediary_after,
        withdraw_intermediary_after as i128 - withdraw_intermediary_before as i128
    );

    let received = user_after as i128 - user_before as i128;
    if !withdraw_all {
        if requested_amount == 0 {
            invariant!(
                received == 0,
                "juplend withdraw: requested 0 but user changed. before: {}, after: {}, delta: {}",
                user_before,
                user_after,
                received
            );
        } else {
            invariant!(
                received == requested_amount as i128,
                "juplend withdraw: user inflow should equal requested amount. requested: {}, user before: {}, after: {}, delta: {}",
                requested_amount,
                user_before,
                user_after,
                received
            );
        }
    }

    invariant!(
        received > 0,
        "juplend withdraw: user should not lose tokens on success. before: {}, after: {}, delta: {}",
        user_before,
        user_after,
        received
    );

    invariant!(
        f_token_vault_after < f_token_vault_before,
        "juplend withdraw: fToken vault should decrease when user receives underlying. before: {}, after: {}, delta: {}",
        f_token_vault_before,
        f_token_vault_after,
        f_token_vault_after as i128 - f_token_vault_before as i128
    );
}

pub fn assert_juplend_withdraw_failure_balances_unchanged(
    requested_amount: u64,
    user_before: u64,
    user_after: u64,
    withdraw_intermediary_before: u64,
    withdraw_intermediary_after: u64,
    liquidity_vault_before: u64,
    liquidity_vault_after: u64,
    f_token_vault_before: u64,
    f_token_vault_after: u64,
) {
    invariant!(
        user_after == user_before,
        "juplend withdraw failure: user tokens changed (requested {}). before: {}, after: {}, delta: {}",
        requested_amount,
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        withdraw_intermediary_after == withdraw_intermediary_before,
        "juplend withdraw failure: withdraw intermediary ATA changed (requested {}). before: {}, after: {}, delta: {}",
        requested_amount,
        withdraw_intermediary_before,
        withdraw_intermediary_after,
        withdraw_intermediary_after as i128 - withdraw_intermediary_before as i128
    );
    invariant!(
        liquidity_vault_after == liquidity_vault_before,
        "juplend withdraw failure: liquidity_vault changed (requested {}). before: {}, after: {}, delta: {}",
        requested_amount,
        liquidity_vault_before,
        liquidity_vault_after,
        liquidity_vault_after as i128 - liquidity_vault_before as i128
    );
    invariant!(
        f_token_vault_after == f_token_vault_before,
        "juplend withdraw failure: fToken vault changed (requested {}). before: {}, after: {}, delta: {}",
        requested_amount,
        f_token_vault_before,
        f_token_vault_after,
        f_token_vault_after as i128 - f_token_vault_before as i128
    );
}
