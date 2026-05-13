#![allow(clippy::too_many_arguments)]

use std::cmp::Ordering;

use fixed::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::types::marginfi::Bank;
use crate::types::marginfi::LiquidationRecord;
use crate::types::marginfi::MarginfiAccount;

/// Matches `marginfi_type_crate` account flag bits.
const ACCOUNT_IN_RECEIVERSHIP: u64 = 1 << 4;
const ACCOUNT_IN_FLASHLOAN: u64 = 1 << 1;
const ACCOUNT_IN_ORDER_EXECUTION: u64 = 1 << 7;

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

/// SPL leg: tokens only move between the user account and this bank's liquidity vault.
/// Does not reimplement protocol pricing; pure conservation.
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

/// Snapshot liquidity vaults and any extra token accounts (user ATAs) for empty-body flashloan checks.
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
    // Deposits should never increase user tokens or decrease the vault.
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

    // For non-zero deposit attempts that succeed, enforce directional movement.
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
    // Withdrawals should never decrease user tokens or increase the vault.
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

    // For non-zero withdrawal attempts that succeed, enforce directional movement.
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
    // Borrowing should increase the user's token balance and decrease the vault.
    assert_withdraw_balance_invariants(amount, user_before, user_after, vault_before, vault_after);
}

pub fn assert_repay_balance_invariants(
    amount: u64,
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    // Repaying should decrease the user's token balance and increase the vault.
    assert_deposit_balance_invariants(amount, user_before, user_after, vault_before, vault_after);
}

// --- Lending share snapshots (`MarginfiAccount` balance row per bank) ---

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BankShareSnapshot {
    pub had_active_balance: bool,
    pub asset_shares: [u8; 16],
    pub liability_shares: [u8; 16],
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
            };
        }
    }
    BankShareSnapshot {
        had_active_balance: false,
        asset_shares: [0u8; 16],
        liability_shares: [0u8; 16],
    }
}

/// **Deposit** and **borrow** use `BankAccountWrapper::find_or_create`. With `amount == 0` the token
/// transfer is a no-op, but the program may still **allocate an active balance row** (zero asset and
/// liability shares) when this bank had no row yet.
///
/// Bank `accrue_interest` runs first and updates **bank** share *prices*; it does **not** change the
/// user’s stored share **counts** on that path. A failing `before == after` on zero-amount success is
/// therefore almost always **new empty slot**, not interest rounding.
fn assert_zero_amount_find_or_create_shares_ok(
    before: &BankShareSnapshot,
    after: &BankShareSnapshot,
    op: &'static str,
) {
    if before == after {
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

/// SPL: successful deposit / withdraw / borrow move exactly `amount` for Tokenkeg and our fuzz
/// mints (no transfer-fee extension). Repay uses post-fee `amount` then may transfer
/// `pre_fee >= amount` for Token-2022; fuzz ETH/BTC mints have no fee, so equality holds.
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
        l0 == l1,
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
        l0 == l1,
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
        a0 == a1,
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
        invariant!(
            before == after,
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
        a0 == a1,
        "repay shares: asset shares must not change. amount: {}, before: {}, after: {}",
        amount,
        a0,
        a1
    );
    if after.had_active_balance {
        invariant!(
            l1.cmp(&l0) == Ordering::Less,
            "repay shares: liability shares must decrease when balance stays open. amount: {}, liability before: {}, after: {}, cmp: {:?}",
            amount,
            l0,
            l1,
            l1.cmp(&l0)
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

// --- Accrue interest (`LendingPoolAccrueBankInterest`) ---

pub fn bank_last_update_snapshot(trident: &mut Trident, bank_pk: Pubkey) -> i64 {
    trident
        .get_account_with_type::<Bank>(&bank_pk, None)
        .expect("bank")
        .last_update
}

/// After `forward_in_time` + successful accrue, each bank’s `last_update` must advance to the
/// current clock (see `Bank::accrue_interest`: `time_delta == 0` early-returns without updating).
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

// --- Liquidation (`LendingAccountLiquidate`) ---

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiquidationBalanceSnapshot {
    pub liab_liquidity_vault: u64,
    pub liab_insurance_vault: u64,
    pub liquidatee_usdc_asset_shares: [u8; 16],
    pub liquidatee_eth_liability_shares: [u8; 16],
    pub liquidator_usdc_asset_shares: [u8; 16],
    pub liquidator_eth_liability_shares: [u8; 16],
}

fn active_balance_shares(acc: &MarginfiAccount, bank: Pubkey, asset_side: bool) -> [u8; 16] {
    for b in &acc.lending_account.balances {
        if b.active != 0 && b.bank_pk == bank {
            return if asset_side {
                b.asset_shares.value
            } else {
                b.liability_shares.value
            };
        }
    }
    [0u8; 16]
}

pub fn liquidation_balance_snapshot(
    trident: &mut Trident,
    liab_bank_liquidity_vault: Pubkey,
    liab_bank_insurance_vault: Pubkey,
    liquidatee_marginfi: Pubkey,
    liquidator_marginfi: Pubkey,
    asset_bank: Pubkey,
    liab_bank: Pubkey,
) -> LiquidationBalanceSnapshot {
    let le = trident
        .get_account_with_type::<MarginfiAccount>(&liquidatee_marginfi, None)
        .expect("liquidatee marginfi account");
    let liq = trident
        .get_account_with_type::<MarginfiAccount>(&liquidator_marginfi, None)
        .expect("liquidator marginfi account");
    LiquidationBalanceSnapshot {
        liab_liquidity_vault: token_balance(trident, liab_bank_liquidity_vault),
        liab_insurance_vault: token_balance(trident, liab_bank_insurance_vault),
        liquidatee_usdc_asset_shares: active_balance_shares(&le, asset_bank, true),
        liquidatee_eth_liability_shares: active_balance_shares(&le, liab_bank, false),
        liquidator_usdc_asset_shares: active_balance_shares(&liq, asset_bank, true),
        liquidator_eth_liability_shares: active_balance_shares(&liq, liab_bank, false),
    }
}

/// Insurance fee moves liability tokens liquidity_vault → insurance_vault; total stays constant.
pub fn assert_liquidation_liab_vault_token_conservation(
    snap: &LiquidationBalanceSnapshot,
    after: &LiquidationBalanceSnapshot,
) {
    let sum_before = snap.liab_liquidity_vault as i128 + snap.liab_insurance_vault as i128;
    let sum_after = after.liab_liquidity_vault as i128 + after.liab_insurance_vault as i128;
    invariant!(
        sum_before == sum_after,
        "liquidation liab vaults: liq+ins total not conserved. sum before: {} (liq {} + ins {}), sum after: {} (liq {} + ins {}), diff: {}",
        sum_before,
        snap.liab_liquidity_vault,
        snap.liab_insurance_vault,
        sum_after,
        after.liab_liquidity_vault,
        after.liab_insurance_vault,
        sum_after - sum_before
    );
    invariant!(
        after.liab_liquidity_vault <= snap.liab_liquidity_vault,
        "liquidation: liability liquidity vault should not increase. before: {}, after: {}, delta: {}",
        snap.liab_liquidity_vault,
        after.liab_liquidity_vault,
        after.liab_liquidity_vault as i128 - snap.liab_liquidity_vault as i128
    );
    invariant!(
        after.liab_insurance_vault >= snap.liab_insurance_vault,
        "liquidation: insurance vault should not decrease. before: {}, after: {}, delta: {}",
        snap.liab_insurance_vault,
        after.liab_insurance_vault,
        after.liab_insurance_vault as i128 - snap.liab_insurance_vault as i128
    );
}

pub fn assert_liquidation_success_share_invariants(
    before: &LiquidationBalanceSnapshot,
    after: &LiquidationBalanceSnapshot,
    asset_amount: u64,
) {
    assert_liquidation_liab_vault_token_conservation(before, after);

    if asset_amount == 0 {
        return;
    }

    let le_a0 = i80_from_share_bytes(&before.liquidatee_usdc_asset_shares);
    let le_a1 = i80_from_share_bytes(&after.liquidatee_usdc_asset_shares);
    let le_l0 = i80_from_share_bytes(&before.liquidatee_eth_liability_shares);
    let le_l1 = i80_from_share_bytes(&after.liquidatee_eth_liability_shares);
    let liq_a0 = i80_from_share_bytes(&before.liquidator_usdc_asset_shares);
    let liq_a1 = i80_from_share_bytes(&after.liquidator_usdc_asset_shares);
    let liq_l0 = i80_from_share_bytes(&before.liquidator_eth_liability_shares);
    let liq_l1 = i80_from_share_bytes(&after.liquidator_eth_liability_shares);

    invariant!(
        le_a1 < le_a0,
        "liquidation shares: liquidatee USDC asset should decrease. asset_amount: {}, before: {}, after: {}",
        asset_amount,
        le_a0,
        le_a1
    );
    invariant!(
        le_l1 < le_l0,
        "liquidation shares: liquidatee ETH liability should decrease. asset_amount: {}, before: {}, after: {}",
        asset_amount,
        le_l0,
        le_l1
    );
    invariant!(
        liq_a1 > liq_a0,
        "liquidation shares: liquidator USDC asset should increase. asset_amount: {}, before: {}, after: {}",
        asset_amount,
        liq_a0,
        liq_a1
    );
    invariant!(
        liq_l1 > liq_l0,
        "liquidation shares: liquidator ETH liability should increase. asset_amount: {}, before: {}, after: {}",
        asset_amount,
        liq_l0,
        liq_l1
    );
}

pub fn assert_liquidation_failure_state_unchanged(
    before: &LiquidationBalanceSnapshot,
    after: &LiquidationBalanceSnapshot,
) {
    invariant!(
        before == after,
        "liquidation failure: state should be unchanged. before: {:?}, after: {:?}",
        before,
        after
    );
}

// --- Receivership (`StartLiquidation` / `EndLiquidation`) ---

/// After a **successful** start→…→end bundle, `end_receivership` clears receivership and the
/// receiver pubkey; it **appends** a history row in `entries[3]` (rotate + write) — the record is
/// not all-zeroes by design.
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

// --- Flashloan closed loop ---

/// When borrow and repay amounts match and the flashloan tx succeeds, the user’s token account is unchanged net.
pub fn assert_flashloan_closed_loop_user_unchanged(before: u64, after: u64) {
    invariant!(
        before == after,
        "flashloan: user token balance should be unchanged when borrow_amount == repay_amount and tx succeeds. before: {}, after: {}, delta: {}",
        before,
        after,
        after as i128 - before as i128
    );
}

// --- Kamino (`KaminoDeposit`) ---
//
// On-chain path (see `programs/marginfi/.../kamino/deposit.rs`): SPL transfer user → bank
// `liquidity_vault`, then Kamino CPI pulls from that vault into `reserve_liquidity_supply`. The
// marginfi bank row is updated via `deposit_no_repay(obligation_collateral_change)` (collateral
// units), so we reuse the same **share** invariants as `LendingAccountDeposit` for the Kamino bank.

/// Liquidity mint: only the user ATA, marginfi liquidity vault, and Kamino reserve supply participate
/// in the deposit leg; their SPL total must be conserved.
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
    // invariant!(
    //     vault_after == vault_before,
    //     "kamino deposit: marginfi liquidity vault is only a hop; net change should be zero. before: {}, after: {}, delta: {}",
    //     vault_before,
    //     vault_after,
    //     vault_after as i128 - vault_before as i128
    // );
    // invariant!(
    //     reserve_supply_after - reserve_supply_before == amount,
    //     "kamino deposit: reserve liquidity supply should increase by amount. requested: {}, reserve_supply before: {}, after: {}, actual increase: {}",
    //     amount,
    //     reserve_supply_before,
    //     reserve_supply_after,
    //     reserve_supply_after as i128 - reserve_supply_before as i128
    // );
}

/// Kamino mints obligation collateral into `reserve_destination_deposit_collateral` when `amount > 0`.
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

// --- Kamino (`KaminoWithdraw`) ---
//
// On-chain path (see `programs/marginfi/.../kamino/withdraw.rs`): Kamino CPI redeems collateral
// into the bank `liquidity_vault` (`user_destination_liquidity`), then SPL transfer
// liquidity_vault → user. `amount` in the instruction is **collateral** token units unless
// `withdraw_all` is set. The three liquidity SPL accounts (user, marginfi vault, reserve supply)
// still form a closed system for the **liquidity** mint.

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

/// Obligation collateral is redeemed from `reserve_source_collateral` (same account the fuzz passes
/// as `reserve_collateral_supply_vault` on deposit/withdraw).
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
