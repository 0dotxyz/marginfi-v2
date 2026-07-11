#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

pub fn assert_kamino_deposit_liquidity_mint_conservation(
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
    reserve_supply_before: u64,
    reserve_supply_after: u64,
) {
    let sum_before = user_before as i128 + vault_before as i128 + reserve_supply_before as i128;
    let sum_after = user_after as i128 + vault_after as i128 + reserve_supply_after as i128;
    invariant!(
        sum_before == sum_after,
        "kamino deposit liquidity conservation: sum before {} != sum after {} (diff {}). user before/after: {}/{}, vault: {}/{}, reserve_supply: {}/{}",
        sum_before,
        sum_after,
        sum_after - sum_before,
        user_before,
        user_after,
        vault_before,
        vault_after,
        reserve_supply_before,
        reserve_supply_after
    );
}

pub fn assert_kamino_deposit_success_liquidity_leg(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
    reserve_supply_before: u64,
    reserve_supply_after: u64,
) {
    assert_kamino_deposit_liquidity_mint_conservation(
        user_before,
        user_after,
        vault_before,
        vault_after,
        reserve_supply_before,
        reserve_supply_after,
    );
    if amount == 0 {
        invariant!(
            user_after == user_before
                && vault_after == vault_before
                && reserve_supply_after == reserve_supply_before,
            "kamino deposit: zero amount must not move liquidity mint balances. user before/after: {}/{}, vault: {}/{}, reserve_supply: {}/{}",
            user_before,
            user_after,
            vault_before,
            vault_after,
            reserve_supply_before,
            reserve_supply_after
        );
        return;
    }
    invariant!(
        user_before - user_after == amount,
        "kamino deposit: user liquidity outflow should equal requested amount. requested: {}, user before: {}, after: {}, actual decrease: {}",
        amount,
        user_before,
        user_after,
        user_before as i128 - user_after as i128
    );
}

pub fn assert_kamino_deposit_success_collateral_destination(
    amount: u64,
    dest_before: u64,
    dest_after: u64,
) {
    if amount == 0 {
        invariant!(
            dest_after == dest_before,
            "kamino deposit: zero amount must not change obligation collateral token balance. before: {}, after: {}, delta: {}",
            dest_before,
            dest_after,
            dest_after as i128 - dest_before as i128
        );
        return;
    }
    invariant!(
        dest_after > dest_before,
        "kamino deposit: obligation collateral token account should receive minted collateral. before: {}, after: {}, delta: {}",
        dest_before,
        dest_after,
        dest_after as i128 - dest_before as i128
    );
}

pub fn assert_kamino_deposit_failure_balances_unchanged(
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
    reserve_supply_before: u64,
    reserve_supply_after: u64,
    collateral_dest_before: u64,
    collateral_dest_after: u64,
) {
    invariant!(
        user_after == user_before,
        "kamino deposit failure: user tokens changed. before: {}, after: {}, delta: {}",
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        vault_after == vault_before,
        "kamino deposit failure: liq vault changed. before: {}, after: {}, delta: {}",
        vault_before,
        vault_after,
        vault_after as i128 - vault_before as i128
    );
    invariant!(
        reserve_supply_after == reserve_supply_before,
        "kamino deposit failure: reserve liquidity supply changed. before: {}, after: {}, delta: {}",
        reserve_supply_before,
        reserve_supply_after,
        reserve_supply_after as i128 - reserve_supply_before as i128
    );
    invariant!(
        collateral_dest_after == collateral_dest_before,
        "kamino deposit failure: collateral destination changed. before: {}, after: {}, delta: {}",
        collateral_dest_before,
        collateral_dest_after,
        collateral_dest_after as i128 - collateral_dest_before as i128
    );
}
