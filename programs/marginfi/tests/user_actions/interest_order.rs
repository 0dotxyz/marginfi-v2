use drift_mocks::{constants::SPOT_CUMULATIVE_INTEREST_PRECISION, state::MinimalSpotMarket};
use fixed::types::I80F48;
use fixed_macro::types::I80F48 as fp;
use fixtures::bank::BankFixture;
use fixtures::{assert_custom_error, prelude::*};
use juplend_mocks::state::{Lending as JuplendLending, EXCHANGE_PRICES_PRECISION};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::constants::{
    BANK_RATE_READING_SPACING_SECONDS, INTEREST_MAX_EXIT_BUDGET_SECONDS,
};
use marginfi_type_crate::types::{milli_to_u32, PremiumEntry, RateReading};
use solana_program_test::tokio;
use solana_sdk::account::AccountSharedData;

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
async fn readings_inside_the_spacing_are_not_recorded() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;
    assert_eq!(fx.recorded_readings(&BankMint::Usdc).await, 1);

    fx.advance(BANK_RATE_READING_SPACING_SECONDS - 1).await;
    fx.pulse(&BankMint::Usdc).await?;
    assert_eq!(fx.recorded_readings(&BankMint::Usdc).await, 1);

    fx.advance(1).await;
    fx.pulse(&BankMint::Usdc).await?;
    let bank = fx.load_bank(&BankMint::Usdc).await;
    assert_eq!(bank.recorded_rate_readings().count(), 2);
    // A native bank has no venue multiplier, so its reading is its share values alone.
    assert_eq!(
        *bank.newest_rate_reading().unwrap(),
        RateReading::new(
            bank.asset_share_value.into(),
            bank.liability_share_value.into(),
            fx.now
        )
        .unwrap()
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

/// The Drift and JupLend fixtures boot at timestamp 0, which a reading treats as never written.
/// Their venue state is stale once it falls behind the clock, so the mocks are stamped to match.
const VENUE_READING_TS: i64 = 1;

async fn start_clock(test_f: &TestFixture) {
    let slot = test_f.get_clock().await.slot;
    test_f.set_clock(slot, VENUE_READING_TS).await;
}

/// Assert the newest reading on `bank_f` carries the venue's own exchange rate. A native bank's
/// multiplier is 1 and cannot distinguish the two; every integration can.
async fn assert_venue_reading_carries_multiplier(
    test_f: &TestFixture,
    bank_f: &BankFixture,
    multiplier: I80F48,
) {
    assert_ne!(
        multiplier,
        I80F48::ONE,
        "the venue should price its position away from 1, or this proves nothing"
    );
    let bank = bank_f.load().await;
    let reading = bank
        .newest_rate_reading()
        .expect("pricing the bank should have taken a reading");
    let expected = RateReading::new(
        I80F48::from(bank.asset_share_value) * multiplier,
        I80F48::from(bank.liability_share_value) * multiplier,
        test_f.get_clock().await.unix_timestamp,
    )
    .unwrap();
    assert_eq!(*reading, expected);
}

#[tokio::test]
async fn a_kamino_bank_reads_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;
    setup
        .test_f
        .marginfi_group
        .try_pulse_bank_price_cache(&setup.bank_f)
        .await?;

    // klend's collateral exchange rate: liquidity per collateral token.
    let (total_liq, total_col) = setup.load_reserve().await.scaled_supplies()?;
    assert_venue_reading_carries_multiplier(&setup.test_f, &setup.bank_f, total_liq / total_col)
        .await;
    Ok(())
}

#[tokio::test]
async fn a_drift_bank_reads_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_drift_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_drift_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;

    // The mock market boots with no accrued interest, so its multiplier is exactly 1. Advance it,
    // as the drift deposit/withdraw tests do, so the reading has something to carry.
    {
        let spot_market_key = setup.bank_f.load().await.integration_acc_1;
        let mut account = setup.test_f.try_load(&spot_market_key).await?.unwrap();
        let spot_market = bytemuck::from_bytes_mut::<MinimalSpotMarket>(
            &mut account.data[8..8 + std::mem::size_of::<MinimalSpotMarket>()],
        );
        spot_market.cumulative_deposit_interest =
            (SPOT_CUMULATIVE_INTEREST_PRECISION * 3 / 2).to_le_bytes();
        spot_market.last_interest_ts = VENUE_READING_TS as u64;
        setup
            .test_f
            .context
            .borrow_mut()
            .set_account(&spot_market_key, &AccountSharedData::from(account));
    }
    start_clock(&setup.test_f).await;
    setup
        .test_f
        .marginfi_group
        .try_pulse_bank_price_cache(&setup.bank_f)
        .await?;

    // Drift's scaled balances grow by the market's cumulative deposit interest.
    let cumulative =
        u128::from_le_bytes(setup.load_spot_market().await.cumulative_deposit_interest);
    let multiplier = I80F48::from_num(cumulative)
        / I80F48::from_num(drift_mocks::constants::SPOT_CUMULATIVE_INTEREST_PRECISION);
    assert_venue_reading_carries_multiplier(&setup.test_f, &setup.bank_f, multiplier).await;
    Ok(())
}

#[tokio::test]
async fn a_juplend_bank_reads_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_juplend_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;

    // The mock lending state boots at parity, so advance its exchange price the way the juplend
    // withdraw tests do, leaving the multiplier something the reading must actually carry.
    {
        let mut account = setup.test_f.try_load(&setup.lending).await?.unwrap();
        let lending = bytemuck::from_bytes_mut::<JuplendLending>(
            &mut account.data[8..8 + std::mem::size_of::<JuplendLending>()],
        );
        lending.token_exchange_price = (EXCHANGE_PRICES_PRECISION * 3 / 2) as u64;
        lending.last_update_timestamp = VENUE_READING_TS as u64;
        setup
            .test_f
            .context
            .borrow_mut()
            .set_account(&setup.lending, &AccountSharedData::from(account));
    }
    start_clock(&setup.test_f).await;
    setup
        .test_f
        .marginfi_group
        .try_pulse_bank_price_cache(&setup.bank_f)
        .await?;

    // JupLend's fToken exchange price, which the liquidity layer advances as it earns.
    let multiplier = I80F48::from_num(setup.load_lending().await.token_exchange_price)
        / I80F48::from_num(EXCHANGE_PRICES_PRECISION);
    assert_venue_reading_carries_multiplier(&setup.test_f, &setup.bank_f, multiplier).await;
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
