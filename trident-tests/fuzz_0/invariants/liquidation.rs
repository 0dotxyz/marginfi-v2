#![allow(clippy::too_many_arguments)]

use fixed::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::types::marginfi::MarginfiAccount;

use super::token_balance;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiquidationBalanceSnapshot {
    pub liab_liquidity_vault: u64,
    pub liab_insurance_vault: u64,
    pub liquidatee_usdc_asset_shares: [u8; 16],
    pub liquidatee_eth_liability_shares: [u8; 16],
    pub liquidator_usdc_asset_shares: [u8; 16],
    pub liquidator_eth_liability_shares: [u8; 16],
}

fn i80_from_share_bytes(bytes: &[u8; 16]) -> I80F48 {
    I80F48::from_bits(i128::from_le_bytes(*bytes))
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
