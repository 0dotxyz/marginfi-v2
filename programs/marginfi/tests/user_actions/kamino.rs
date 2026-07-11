use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token::error::TokenError;
use fixtures::{assert_anchor_error, assert_custom_error, prelude::*};
use marginfi::{
    assert_eq_with_tolerance, errors::MarginfiError, state::rate_limiter::RateLimitWindowImpl,
};
use solana_program_test::*;
use solana_sdk::{
    clock::Clock, instruction::Instruction, signer::Signer, transaction::Transaction,
};
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

/// e2e: the bank rate limiter must debit a Kamino withdraw in underlying liquidity, not collateral
/// shares — so it nets against the deposit inflow (which is recorded in underlying).
#[tokio::test]
async fn kamino_withdraw_records_underlying_on_bank_rate_limiter() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10_000.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 5_000_000_000)
        .await?;

    // Enable a generous hourly limit so the withdraw is recorded but not blocked. The group admin
    // is the payer (default test settings).
    let hourly_limit = 1_000_000_000_000u64;
    let admin = setup.test_f.payer_keypair();
    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::ConfigureBankRateLimits {
            group: setup.test_f.marginfi_group.key,
            admin: admin.pubkey(),
            bank: setup.bank_f.key,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureBankRateLimits {
            hourly_max_outflow: Some(hourly_limit),
            daily_max_outflow: None,
        }
        .data(),
    };
    {
        let ctx = setup.test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&admin.pubkey()),
            &[&admin],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        ctx.banks_client.process_transaction(tx).await?;
    }

    let withdraw_amount = 1_000_000_000u64; // collateral shares
    let expected_underlying = setup
        .load_reserve()
        .await
        .collateral_to_liquidity(withdraw_amount)?;
    assert_ne!(
        expected_underlying, withdraw_amount,
        "fixture exchange rate must differ from 1 for this test to be meaningful"
    );

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

    let bank = setup.bank_f.load().await;
    let now = {
        let ctx = setup.test_f.context.borrow_mut();
        ctx.banks_client.get_sysvar::<Clock>().await?.unix_timestamp
    };
    assert_eq_with_tolerance!(
        bank.rate_limiter.hourly.remaining_capacity(now),
        hourly_limit as i64 - expected_underlying as i64,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i64
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
