#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

pub fn assert_kamino_withdraw_liquidity_mint_conservation(
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
        "kamino withdraw liquidity conservation: sum before {} != sum after {} (diff {}). user before/after: {}/{}, vault: {}/{}, reserve_supply: {}/{}",
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

pub fn assert_kamino_withdraw_success_liquidity_leg(
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
    reserve_supply_before: u64,
    reserve_supply_after: u64,
) {
    assert_kamino_withdraw_liquidity_mint_conservation(
        user_before,
        user_after,
        vault_before,
        vault_after,
        reserve_supply_before,
        reserve_supply_after,
    );
    let liq_to_user = user_after as i128 - user_before as i128;
    if liq_to_user == 0 {
        invariant!(
            user_after == user_before
                && vault_after == vault_before
                && reserve_supply_after == reserve_supply_before,
            "kamino withdraw: zero liquidity to user must not move liquidity mint balances. user before/after: {}/{}, vault: {}/{}, reserve_supply: {}/{}",
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
        liq_to_user > 0,
        "kamino withdraw: user should receive liquidity. user before: {}, after: {}, delta: {}",
        user_before,
        user_after,
        liq_to_user
    );
    invariant!(
        vault_after == vault_before,
        "kamino withdraw: marginfi liquidity vault is only a hop; net change should be zero. before: {}, after: {}, delta: {}",
        vault_before,
        vault_after,
        vault_after as i128 - vault_before as i128
    );
    let liq_from_reserve = reserve_supply_before as i128 - reserve_supply_after as i128;
    invariant!(
        liq_from_reserve == liq_to_user,
        "kamino withdraw: reserve liquidity out should equal user in. user delta: {}, reserve before: {}, after: {}, reserve delta (out): {}",
        liq_to_user,
        reserve_supply_before,
        reserve_supply_after,
        liq_from_reserve
    );
}

pub fn assert_kamino_withdraw_success_collateral_source(
    withdraw_all: bool,
    amount_collateral: u64,
    src_before: u64,
    src_after: u64,
    liquidity_received: u64,
) {
    if !withdraw_all && amount_collateral == 0 {
        invariant!(
            src_after == src_before,
            "kamino withdraw: zero collateral request must not move collateral token balance. before: {}, after: {}, delta: {}",
            src_before,
            src_after,
            src_after as i128 - src_before as i128
        );
        return;
    }
    if !withdraw_all && amount_collateral > 0 {
        let burned = src_before as i128 - src_after as i128;
        invariant!(
            burned == amount_collateral as i128,
            "kamino withdraw: collateral redeemed should match requested collateral amount. requested: {}, src before: {}, after: {}, actual decrease (signed): {}",
            amount_collateral,
            src_before,
            src_after,
            burned
        );
        return;
    }
    if liquidity_received > 0 {
        invariant!(
            src_after < src_before,
            "kamino withdraw: withdraw_all with liquidity out must decrease collateral. src before: {}, after: {}, delta: {}",
            src_before,
            src_after,
            src_after as i128 - src_before as i128
        );
    } else {
        invariant!(
            src_after == src_before,
            "kamino withdraw: withdraw_all with no liquidity to user should leave collateral unchanged. before: {}, after: {}",
            src_before,
            src_after
        );
    }
}

pub fn assert_kamino_withdraw_failure_balances_unchanged(
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
    reserve_supply_before: u64,
    reserve_supply_after: u64,
    collateral_src_before: u64,
    collateral_src_after: u64,
) {
    invariant!(
        user_after == user_before,
        "kamino withdraw failure: user tokens changed. before: {}, after: {}, delta: {}",
        user_before,
        user_after,
        user_after as i128 - user_before as i128
    );
    invariant!(
        vault_after == vault_before,
        "kamino withdraw failure: liq vault changed. before: {}, after: {}, delta: {}",
        vault_before,
        vault_after,
        vault_after as i128 - vault_before as i128
    );
    invariant!(
        reserve_supply_after == reserve_supply_before,
        "kamino withdraw failure: reserve liquidity supply changed. before: {}, after: {}, delta: {}",
        reserve_supply_before,
        reserve_supply_after,
        reserve_supply_after as i128 - reserve_supply_before as i128
    );
    invariant!(
        collateral_src_after == collateral_src_before,
        "kamino withdraw failure: collateral source changed. before: {}, after: {}, delta: {}",
        collateral_src_before,
        collateral_src_after,
        collateral_src_after as i128 - collateral_src_before as i128
    );
}
