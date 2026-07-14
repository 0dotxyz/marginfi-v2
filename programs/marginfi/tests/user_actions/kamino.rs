use anchor_spl::token::spl_token::error::TokenError;
use fixed::types::I80F48;
use fixtures::{assert_anchor_error, assert_custom_error, prelude::*};
use marginfi::{assert_eq_with_tolerance, errors::MarginfiError};
use solana_program_test::*;
use test_case::test_case;

const KAMINO_ROUNDING_TOLERANCE_NATIVE: u64 = 1;

// (wallet_funding_ui, deposit_native)
#[test_case(10.0, 1_000_000)] // 1 USDC
#[test_case(500.0, 100_000_000)] // 100 USDC
#[test_case(10_000.0, 5_000_000_000)] // 5,000 USDC
#[test_case(100_000.0, 50_000_000_000)] // 50,000 USDC
#[tokio::test]
async fn kamino_deposit_local_instruction_call_success(
    wallet_funding: f64,
    deposit_amount: u64,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(wallet_funding).await;
    let pre_reserve = setup.load_reserve().await;
    let pre_accounted = setup.load_user_accounted_collateral(&user).await;
    assert!(pre_accounted.is_none());

    let pre = setup.load_state(&user_token).await;

    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, deposit_amount)
        .await?;

    let post = setup.load_state(&user_token).await;
    let post_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino bank balance should be active after deposit");

    let expected_collateral_delta = pre_reserve.liquidity_to_collateral(deposit_amount)? as i128;
    let actual_obligation_delta =
        post.obligation_collateral as i128 - pre.obligation_collateral as i128;
    let actual_accounted_delta = post_accounted as i128;

    assert_eq!(pre.user_balance - post.user_balance, deposit_amount);
    assert_eq!(
        post.reserve_supply_balance - pre.reserve_supply_balance,
        deposit_amount
    );
    assert_eq_with_tolerance!(
        actual_obligation_delta,
        expected_collateral_delta,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );
    assert_eq_with_tolerance!(
        actual_accounted_delta,
        expected_collateral_delta,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );

    Ok(())
}

// (wallet_funding_ui, deposit_native, withdraw_collateral_native)
#[test_case(10.0, 1_000_000, 100_000)] // deposit 1 USDC, withdraw 0.1 collateral
#[test_case(500.0, 100_000_000, 10_000_000)] // deposit 100 USDC, withdraw 10 collateral
#[test_case(10_000.0, 5_000_000_000, 1_000_000_000)] // deposit 5,000 USDC, withdraw 1,000 collateral
#[test_case(100_000.0, 50_000_000_000, 25_000_000_000)] // deposit 50,000 USDC, withdraw 25,000 collateral
#[tokio::test]
async fn kamino_withdraw_local_instruction_call_success(
    wallet_funding: f64,
    deposit_amount: u64,
    withdraw_amount: u64,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(wallet_funding).await;

    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, deposit_amount)
        .await?;

    let pre_reserve = setup.load_reserve().await;
    let pre = setup.load_state(&user_token).await;
    let pre_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino bank balance should be active after deposit");

    setup
        .test_f
        .run_kamino_withdraw(
            &setup.bank_f,
            &user,
            user_token.key,
            withdraw_amount,
            Some(false),
        )
        .await?;

    let post = setup.load_state(&user_token).await;
    let post_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino bank balance should remain active after partial withdraw");

    let expected_liquidity_delta = pre_reserve.collateral_to_liquidity(withdraw_amount)? as i128;
    let actual_user_liquidity_delta = post.user_balance as i128 - pre.user_balance as i128;
    let actual_reserve_liquidity_delta =
        pre.reserve_supply_balance as i128 - post.reserve_supply_balance as i128;
    let actual_obligation_delta =
        pre.obligation_collateral as i128 - post.obligation_collateral as i128;
    let actual_accounted_delta = pre_accounted as i128 - post_accounted as i128;

    assert_eq_with_tolerance!(
        actual_user_liquidity_delta,
        expected_liquidity_delta,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );
    assert_eq_with_tolerance!(
        actual_reserve_liquidity_delta,
        expected_liquidity_delta,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );
    assert_eq!(
        actual_obligation_delta, withdraw_amount as i128,
        "obligation collateral burn should match requested collateral amount"
    );
    assert_eq_with_tolerance!(
        actual_accounted_delta,
        withdraw_amount as i128,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );

    Ok(())
}

// (wallet_funding_ui, deposit_native)
#[test_case(0.01, 1_000_000)] // try deposit 1 USDC with 0.01 wallet
#[test_case(1.0, 100_000_000)] // try deposit 100 USDC with 1 wallet
#[test_case(100.0, 500_000_000_000)] // try deposit 500,000 USDC with 100 wallet
#[tokio::test]
async fn kamino_deposit_local_instruction_call_failure_insufficient_funds(
    wallet_funding: f64,
    deposit_amount: u64,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(wallet_funding).await;
    let pre_state = setup.load_state(&user_token).await;
    let pre_accounted = setup.load_user_accounted_collateral(&user).await;

    let res = setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, deposit_amount)
        .await;
    let err = res.expect_err("deposit should fail with insufficient user funds");
    assert_anchor_error!(err, TokenError::InsufficientFunds);

    let post_state = setup.load_state(&user_token).await;
    let post_accounted = setup.load_user_accounted_collateral(&user).await;

    assert_eq!(
        pre_state, post_state,
        "state should be unchanged on failed deposit"
    );
    assert_eq!(
        pre_accounted, post_accounted,
        "marginfi accounted collateral should be unchanged on failed deposit"
    );

    Ok(())
}

// (wallet_funding_ui, deposit_native)
#[test_case(10.0, 1_000_000)] // deposit 1 USDC then withdraw u64::MAX
#[test_case(10_000.0, 5_000_000_000)] // deposit 5,000 USDC then withdraw u64::MAX
#[tokio::test]
async fn kamino_withdraw_local_instruction_call_failure_oversized_amount(
    wallet_funding: f64,
    deposit_amount: u64,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(wallet_funding).await;

    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, deposit_amount)
        .await?;
    let pre_state = setup.load_state(&user_token).await;
    let pre_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino bank balance should be active after deposit");

    let res = setup
        .test_f
        .run_kamino_withdraw(&setup.bank_f, &user, user_token.key, u64::MAX, Some(false))
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OperationWithdrawOnly);

    let post_state = setup.load_state(&user_token).await;
    let post_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino bank balance should remain active after failed withdraw");

    assert_eq!(
        pre_state, post_state,
        "state should be unchanged on oversized withdraw failure"
    );
    assert_eq!(
        pre_accounted, post_accounted,
        "marginfi accounted collateral should be unchanged on oversized withdraw failure"
    );

    Ok(())
}

/// The circuit breaker tracks the multiplier-adjusted effective price on integration banks: for a
/// Kamino bank the risk price is base_oracle_price x reserve_exchange_rate. Enabling the breaker
/// must seed the reference at that adjusted price, not the raw base, or an integration bank with a
/// non-unit multiplier would spuriously trip on its first observation.
#[tokio::test]
async fn kamino_cb_enable_seeds_multiplier_adjusted_reference() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(500.0).await;

    // Deposit then withdraw: the Kamino withdraw path warms the bank cache with the raw base price
    // and the reserve exchange-rate multiplier (deposit only refreshes interest rates).
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 100_000_000)
        .await?;
    setup
        .test_f
        .run_kamino_withdraw(
            &setup.bank_f,
            &user,
            user_token.key,
            10_000_000,
            Some(false),
        )
        .await?;

    let warmed = setup.bank_f.load().await;
    let raw_price: I80F48 = warmed.cache.last_oracle_price.into();
    let multiplier: I80F48 = warmed.cache.price_multiplier.into();
    assert!(raw_price > I80F48::ZERO, "cache should be warm after withdraw");
    assert_ne!(
        multiplier,
        I80F48::ONE,
        "the Kamino reserve must produce a non-unit multiplier for this test to be meaningful"
    );

    // Enable the breaker; the reference must be the effective (multiplier-adjusted) price.
    setup.bank_f.update_config(standard_cb_config(), None).await?;

    let enabled = setup.bank_f.load().await;
    let seeded_ref: I80F48 = enabled.cb_reference_price.into();
    assert_eq!(seeded_ref, raw_price * multiplier);
    Ok(())
}
