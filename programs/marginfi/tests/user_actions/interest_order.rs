use fixed::types::I80F48;
use fixed_macro::types::I80F48 as fp;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::constants::INTEREST_MAX_EXIT_BUDGET_SECONDS;
use marginfi_type_crate::types::{milli_to_u32, PremiumEntry};
use solana_program_test::tokio;

use super::interest_order_common::*;

#[tokio::test]
async fn interest_order_fires_once_the_pair_has_bled_for_a_window() -> anyhow::Result<()> {
    // The default stop-loss sits far below the pair's value, so only carry can fire this.
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW).await;
    fx.unwind(1.0).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "order should be consumed by the execution"
    );
    let account = fx.account_f.load().await;
    let sol = fx.test_f.get_bank(&BankMint::Sol);
    assert!(
        !account
            .lending_account
            .balances
            .iter()
            .any(|b| b.is_active() && b.bank_pk == sol.key),
        "the borrow leg should be closed"
    );
    let usdc = fx.test_f.get_bank(&BankMint::Usdc);
    assert!(
        account
            .lending_account
            .balances
            .iter()
            .any(|b| b.is_active() && b.bank_pk == usdc.key),
        "the lend leg should survive"
    );
    Ok(())
}

#[tokio::test]
async fn a_carry_order_fires_on_history_recorded_before_it_was_placed() -> anyhow::Result<()> {
    let fx = setup(Params {
        history_before_placement: TEST_WINDOW,
        ..Default::default()
    })
    .await?;

    // Placed a full window after the banks' first readings, so it is executable at once.
    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the order should execute on history older than itself"
    );
    Ok(())
}

#[tokio::test]
async fn interest_order_cannot_execute_before_its_window_elapses() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW - 1).await;
    let res = fx.unwind(1.0).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderInterestHistoryTooShort
    );
    Ok(())
}

#[tokio::test]
async fn an_unwind_costlier_than_the_carry_budget_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        interest: Some(interest_config(TEST_WINDOW_SECONDS, 3_600)),
        ..Default::default()
    })
    .await?;

    fx.advance(TEST_WINDOW).await;
    let res = fx.unwind(1.04).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderInterestCostExceedsCarry
    );
    Ok(())
}

#[tokio::test]
async fn a_price_trigger_still_fires_before_the_carry_window_elapses() -> anyhow::Result<()> {
    // The pair is worth ~$900, so this stop-loss is already breached at placement.
    let mut fx = setup(Params {
        stop_loss: fp!(5000),
        ..Default::default()
    })
    .await?;

    fx.advance(3_600).await;
    fx.unwind(1.0).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the price trigger should execute despite the carry window being short"
    );
    Ok(())
}

#[tokio::test]
async fn a_brief_spike_inside_the_window_does_not_fire() -> anyhow::Result<()> {
    let mut fx = setup(spike_params()).await?;

    fx.settle_borrow_rate().await?;

    let driver = drive_sol_rate(&fx, SPIKE_BORROW, SPIKE_COLLATERAL).await?;
    fx.advance(600).await;
    fx.settle_borrow_rate().await?;
    driver.release(&fx).await?;

    fx.advance(TEST_WINDOW).await;
    fx.settle_borrow_rate().await?;

    let res = fx.unwind(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestNotNegative);
    Ok(())
}

#[tokio::test]
async fn the_same_rate_sustained_across_the_window_does_fire() -> anyhow::Result<()> {
    let mut fx = setup(spike_params()).await?;

    fx.settle_borrow_rate().await?;

    let _driver = drive_sol_rate(&fx, SPIKE_BORROW, SPIKE_COLLATERAL).await?;
    fx.advance(TEST_WINDOW).await;
    fx.settle_borrow_rate().await?;

    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "a rate held for the whole window should fire the exit"
    );
    Ok(())
}

#[tokio::test]
async fn both_conditions_met_lets_either_cost_bound_carry_the_execution() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        // Already breached at placement, so the price condition fires too.
        stop_loss: fp!(5000),
        // An hour's worth of loss is almost no budget at all.
        interest: Some(interest_config(TEST_WINDOW_SECONDS, 3_600)),
        ..Default::default()
    })
    .await?;

    fx.advance(TEST_WINDOW).await;
    fx.unwind(1.04).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the price bound should carry an execution the carry budget cannot"
    );
    Ok(())
}

#[tokio::test]
async fn the_slippage_ceiling_binds_even_when_the_budget_would_allow_more() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        // A year's worth of loss makes the budget far larger than this unwind costs.
        interest: Some(interest_config(
            TEST_WINDOW_SECONDS,
            INTEREST_MAX_EXIT_BUDGET_SECONDS,
        )),
        max_slippage_pct: 1.0,
        // A float the rate driver below can actually borrow against.
        lender_sol: SPIKE_LENDER_SOL,
        ..Default::default()
    })
    .await?;

    fx.settle_borrow_rate().await?;

    let _driver = drive_sol_rate(&fx, SPIKE_BORROW, SPIKE_COLLATERAL).await?;
    fx.advance(TEST_WINDOW).await;
    fx.settle_borrow_rate().await?;

    // This pulls ~$35 more than the ~$100 liability was worth: past the 1% ceiling, and nowhere
    // near the year of driven-rate loss the carry budget allows.
    let res = fx.unwind(1.35).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderExecutionOverWithdrawal
    );
    Ok(())
}

#[tokio::test]
async fn read_only_order_banks_are_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW).await;

    let res = fx.unwind_with_readonly_banks().await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderInterestBankNotWritable
    );
    Ok(())
}

#[tokio::test]
async fn an_unreadable_carry_leg_does_not_block_a_price_trigger() -> anyhow::Result<()> {
    // The pair is worth ~$900, so this stop-loss is breached from the start.
    let mut fx = setup(Params {
        stop_loss: fp!(5000),
        ..Default::default()
    })
    .await?;

    fx.advance(TEST_WINDOW).await;

    fx.unwind_readonly_full(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the price trigger should execute despite the carry leg being unreadable"
    );
    Ok(())
}

#[tokio::test]
async fn execution_accrues_both_order_banks() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW).await;
    assert!(
        fx.bank_last_update(&BankMint::Usdc).await < fx.now,
        "the lend leg should be stale going in, or this proves nothing"
    );

    fx.unwind(1.0).await?;

    assert_eq!(fx.bank_last_update(&BankMint::Usdc).await, fx.now);
    assert_eq!(fx.bank_last_update(&BankMint::Sol).await, fx.now);
    Ok(())
}

#[tokio::test]
async fn the_variable_borrow_premium_counts_toward_the_carry_cost() -> anyhow::Result<()> {
    let mut fx = setup(premium_params()).await?;

    fx.advance(TEST_WINDOW).await;

    // Base rates alone leave the near-idle borrow leg well short of the trigger margin.
    let res = fx.unwind(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestNotNegative);

    // A 25% premium on the SOL liability, collateralised by the USDC lend leg.
    let group_f = &fx.test_f.marginfi_group;
    group_f
        .try_configure_group_premium(PremiumEntry {
            collateral_tag: TAG_COLLATERAL,
            liability_tag: TAG_LIABILITY,
            rate: milli_to_u32(I80F48::from_num(0.25)),
        })
        .await?;
    group_f
        .try_configure_bank_premium(fx.test_f.get_bank(&BankMint::Usdc), TAG_COLLATERAL, true)
        .await?;
    group_f
        .try_configure_bank_premium(fx.test_f.get_bank(&BankMint::Sol), TAG_LIABILITY, true)
        .await?;
    // The snapshot is written by an oracle-carrying instruction, not by the config change itself.
    fx.account_f.try_lending_account_pulse_health().await?;

    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the premium should carry the pair past the trigger margin"
    );
    Ok(())
}

#[tokio::test]
async fn a_pulse_at_the_moment_of_maturity_cannot_displace_the_measurement() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW).await;

    // A third party prices both banks the instant the order became executable.
    fx.pulse(&BankMint::Usdc).await?;
    fx.pulse(&BankMint::Sol).await?;

    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the older readings should still carry the execution"
    );
    Ok(())
}

#[tokio::test]
async fn execution_holds_up_with_the_account_near_max_balances() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    // Unrelated deposits, none of which the order touches.
    for mint in [
        BankMint::Fixed,
        BankMint::FixedLow,
        BankMint::SolSwbPull,
        BankMint::SolSwbOrigFee,
        BankMint::SolEquivalent,
        BankMint::PyUSD,
    ] {
        let bank = fx.test_f.get_bank(&mint);
        let funded = bank.mint.create_token_account_and_mint_to(10.0).await;
        fx.account_f
            .try_bank_deposit(funded.key, bank, 10.0, None)
            .await?;
    }

    let active = fx
        .account_f
        .load()
        .await
        .lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
        .count();
    assert_eq!(active, 8, "six pads plus the order's own two legs");

    fx.advance(TEST_WINDOW).await;
    fx.unwind_with_budget(1.0, 1_400_000).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the order should execute with the account nearly full"
    );
    // Every unrelated deposit is left exactly as it was.
    let after = fx.account_f.load().await;
    assert_eq!(
        after
            .lending_account
            .balances
            .iter()
            .filter(|b| b.is_active())
            .count(),
        active - 1,
        "only the borrow leg should have closed"
    );
    Ok(())
}
