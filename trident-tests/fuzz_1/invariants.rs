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

pub fn token_balance(trident: &mut Trident, token_account: Pubkey) -> u64 {
    let token_account = trident.get_token_account(token_account);
    invariant!(token_account.is_ok());
    token_account.unwrap().account.amount
}

pub fn assert_no_balance_change(
    user_before: u64,
    user_after: u64,
    vault_before: u64,
    vault_after: u64,
) {
    invariant!(user_after == user_before);
    invariant!(vault_after == vault_before);
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
    invariant!(net_user + net_vault == 0);
}

pub fn assert_balance_unchanged(before: u64, after: u64) {
    invariant!(before == after);
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
        invariant!(after == *before);
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
    invariant!(user_after <= user_before);
    invariant!(vault_after >= vault_before);

    // For non-zero deposit attempts that succeed, enforce directional movement.
    if amount > 0 {
        invariant!(user_after < user_before);
        invariant!(vault_after > vault_before);
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
    invariant!(user_after >= user_before);
    invariant!(vault_after <= vault_before);

    // For non-zero withdrawal attempts that succeed, enforce directional movement.
    if amount > 0 {
        invariant!(user_after > user_before);
        invariant!(vault_after < vault_before);
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
        "{op}: zero-amount success may only change snapshot by opening an empty bank slot"
    );
    invariant!(
        after.asset_shares == [0u8; 16] && after.liability_shares == [0u8; 16],
        "{op}: newly opened slot must have zero asset and liability shares"
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
        "deposit: user token decrease should equal requested amount"
    );
    invariant!(
        vault_after - vault_before == amount,
        "deposit: vault increase should equal requested amount"
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
        "withdraw/borrow: user token increase should equal requested amount"
    );
    invariant!(
        vault_before - vault_after == amount,
        "withdraw/borrow: vault decrease should equal requested amount"
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
        "repay: user outflow should match vault inflow (no transfer fee in fuzz mints)"
    );
    invariant!(
        paid == amount,
        "repay: token leg should match post-fee repay amount when mint has no transfer fee"
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
        "deposit: bank balance should be active after success"
    );
    let a0 = i80_from_share_bytes(&before.asset_shares);
    let a1 = i80_from_share_bytes(&after.asset_shares);
    let l0 = i80_from_share_bytes(&before.liability_shares);
    let l1 = i80_from_share_bytes(&after.liability_shares);
    invariant!(l0 == l1, "deposit: liability shares must not change");
    invariant!(
        a1.cmp(&a0) == Ordering::Greater,
        "deposit: asset shares must increase"
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
            "withdraw: zero amount should not change lending shares"
        );
        return;
    }
    invariant!(
        before.had_active_balance,
        "withdraw: need an open position before withdraw"
    );
    let a0 = i80_from_share_bytes(&before.asset_shares);
    let a1 = i80_from_share_bytes(&after.asset_shares);
    let l0 = i80_from_share_bytes(&before.liability_shares);
    let l1 = i80_from_share_bytes(&after.liability_shares);
    invariant!(l0 == l1, "withdraw: liability shares must not change");
    if after.had_active_balance {
        invariant!(
            a1.cmp(&a0) == Ordering::Less,
            "withdraw: asset shares must decrease when balance stays open"
        );
    } else {
        invariant!(a0 > I80F48::ZERO, "withdraw: full close implies prior assets");
        invariant!(a1 == I80F48::ZERO && l1 == I80F48::ZERO, "withdraw: snapshot shows closed row");
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
        "borrow: bank balance should be active after success"
    );
    let a0 = i80_from_share_bytes(&before.asset_shares);
    let a1 = i80_from_share_bytes(&after.asset_shares);
    let l0 = i80_from_share_bytes(&before.liability_shares);
    let l1 = i80_from_share_bytes(&after.liability_shares);
    invariant!(a0 == a1, "borrow: asset shares on this bank must not change");
    invariant!(
        l1.cmp(&l0) == Ordering::Greater,
        "borrow: liability shares must increase"
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
            "repay: zero amount should not change lending shares"
        );
        return;
    }
    invariant!(
        before.had_active_balance,
        "repay: need an open position before repay"
    );
    let a0 = i80_from_share_bytes(&before.asset_shares);
    let a1 = i80_from_share_bytes(&after.asset_shares);
    let l0 = i80_from_share_bytes(&before.liability_shares);
    let l1 = i80_from_share_bytes(&after.liability_shares);
    invariant!(a0 == a1, "repay: asset shares on this bank must not change");
    if after.had_active_balance {
        invariant!(
            l1.cmp(&l0) == Ordering::Less,
            "repay: liability shares must decrease when balance stays open"
        );
    } else {
        invariant!(
            l0 > I80F48::ZERO,
            "repay: full close implies prior liabilities"
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
        "accrue: snapshot length mismatch"
    );
    for (&pk, &prev) in bank_pks.iter().zip(last_updates_before.iter()) {
        let now = bank_last_update_snapshot(trident, pk);
        invariant!(
            now > prev,
            "accrue: bank last_update should strictly increase after a time warp + accrue"
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
        "liquidation: ETH liq+ins vault token total should be conserved"
    );
    invariant!(
        after.liab_liquidity_vault <= snap.liab_liquidity_vault,
        "liquidation: liability liquidity vault should not increase"
    );
    invariant!(
        after.liab_insurance_vault >= snap.liab_insurance_vault,
        "liquidation: insurance vault should not decrease"
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
        "liquidation: liquidatee USDC asset shares should decrease"
    );
    invariant!(
        le_l1 < le_l0,
        "liquidation: liquidatee ETH liability shares should decrease"
    );
    invariant!(
        liq_a1 > liq_a0,
        "liquidation: liquidator USDC asset shares should increase"
    );
    invariant!(
        liq_l1 > liq_l0,
        "liquidation: liquidator ETH liability shares should increase"
    );
}

pub fn assert_liquidation_failure_state_unchanged(
    before: &LiquidationBalanceSnapshot,
    after: &LiquidationBalanceSnapshot,
) {
    invariant!(
        before == after,
        "liquidation: failed tx should not change vaults or relevant margin shares"
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
        "receivership: marginfi account must leave ACCOUNT_IN_RECEIVERSHIP after successful end"
    );
    invariant!(
        m.account_flags & ACCOUNT_IN_FLASHLOAN == 0,
        "receivership: must not leave ACCOUNT_IN_FLASHLOAN set"
    );
    invariant!(
        m.account_flags & ACCOUNT_IN_ORDER_EXECUTION == 0,
        "receivership: must not leave ACCOUNT_IN_ORDER_EXECUTION set"
    );
    let rec = trident
        .get_account_with_type::<LiquidationRecord>(&liquidation_record_pk, None)
        .expect("liquidation record");
    invariant!(
        rec.marginfi_account == marginfi_account_pk,
        "receivership: liquidation_record.marginfi_account must match this account"
    );
    invariant!(
        rec.liquidation_receiver == Pubkey::default(),
        "receivership: liquidation_record.liquidation_receiver must be cleared after end"
    );
    let newest = &rec.entries[3];
    invariant!(
        newest.timestamp != 0,
        "receivership: newest liquidation entry should record a timestamp after successful end"
    );
}

// --- Flashloan closed loop ---

/// When borrow and repay amounts match and the flashloan tx succeeds, the user’s token account is unchanged net.
pub fn assert_flashloan_closed_loop_user_unchanged(before: u64, after: u64) {
    invariant!(
        before == after,
        "flashloan: user token balance unchanged when borrow_amount == repay_amount and tx succeeds"
    );
}
