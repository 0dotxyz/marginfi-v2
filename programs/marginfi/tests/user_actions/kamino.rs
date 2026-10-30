use anchor_lang::{InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token::error::TokenError;
use fixed::types::I80F48;
use fixtures::{assert_anchor_error, assert_custom_error, prelude::*};
use marginfi::state::bank::BankImpl;
use marginfi::{
    assert_eq_with_tolerance, errors::MarginfiError, state::rate_limiter::RateLimitWindowImpl,
};
use marginfi_type_crate::constants::KAMINO_MARKET_EMERGENCY;
use marginfi_type_crate::types::{Bank, BankOperationalState};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::*;
use solana_sdk::{
    account::AccountSharedData,
    clock::Clock,
    instruction::Instruction,
    pubkey::Pubkey,
    signer::Signer,
    transaction::{Transaction, TransactionError},
};
use test_case::test_case;

const KAMINO_ROUNDING_TOLERANCE_NATIVE: u64 = 1;
const SOLANA_SLOTS_PER_DAY: u64 = 216_000;
const WARP_DAYS: u64 = 30;
const KAMINO_TX_CU_LIMIT: u32 = 800_000;

// (wallet_funding_ui, deposit_native, internal_refresh)
#[test_case(10.0, 1_000_000, false; "1_usdc_external")]
#[test_case(500.0, 100_000_000, true; "100_usdc_internal")]
#[test_case(10_000.0, 5_000_000_000, false; "5000_usdc_external")]
#[test_case(100_000.0, 50_000_000_000, true; "50000_usdc_internal")]
#[tokio::test]
async fn kamino_deposit_local_instruction_call_success(
    wallet_funding: f64,
    deposit_amount: u64,
    internal_refresh: bool,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(wallet_funding).await;
    let pre_reserve = setup.load_reserve().await;
    let pre_accounted = setup.load_user_accounted_collateral(&user).await;
    assert!(pre_accounted.is_none());

    let pre = setup.load_state(&user_token).await;

    setup
        .test_f
        .run_kamino_deposit_with_options(
            &setup.bank_f,
            &user,
            user_token.key,
            deposit_amount,
            KaminoRefreshOptions {
                external: !internal_refresh,
                internal: internal_refresh,
                ..Default::default()
            },
        )
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

// (wallet_funding_ui, deposit_native, withdraw_collateral_native, internal_refresh)
#[test_case(10.0, 1_000_000, 100_000, false; "1_usdc_external")]
#[test_case(500.0, 100_000_000, 10_000_000, true; "100_usdc_internal")]
#[test_case(10_000.0, 5_000_000_000, 1_000_000_000, false; "5000_usdc_external")]
#[test_case(100_000.0, 50_000_000_000, 25_000_000_000, true; "50000_usdc_internal")]
#[tokio::test]
async fn kamino_withdraw_local_instruction_call_success(
    wallet_funding: f64,
    deposit_amount: u64,
    withdraw_amount: u64,
    internal_refresh: bool,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(wallet_funding).await;

    setup
        .test_f
        .run_kamino_deposit_with_options(
            &setup.bank_f,
            &user,
            user_token.key,
            deposit_amount,
            KaminoRefreshOptions {
                external: !internal_refresh,
                internal: internal_refresh,
                ..Default::default()
            },
        )
        .await?;

    let pre_reserve = setup.load_reserve().await;
    let pre = setup.load_state(&user_token).await;
    let pre_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino bank balance should be active after deposit");

    setup
        .test_f
        .run_kamino_withdraw_with_options(
            &setup.bank_f,
            &user,
            user_token.key,
            withdraw_amount,
            Some(false),
            KaminoRefreshOptions {
                external: !internal_refresh,
                internal: internal_refresh,
                ..Default::default()
            },
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
#[test_case(false; "caller_refresh")]
#[test_case(true; "internal_refresh_after_interest")]
#[tokio::test]
async fn kamino_withdraw_records_underlying_on_bank_rate_limiter(
    internal_refresh: bool,
) -> anyhow::Result<()> {
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
            instruction_sysvar: solana_sdk::sysvar::instructions::ID,
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
    let stale_underlying = setup
        .load_reserve()
        .await
        .collateral_to_liquidity(withdraw_amount)?;
    assert_ne!(
        stale_underlying, withdraw_amount,
        "fixture exchange rate must differ from 1 for this test to be meaningful"
    );

    if internal_refresh {
        warp_clock_forward(&setup).await?;
        setup
            .test_f
            .run_kamino_withdraw_with_options(
                &setup.bank_f,
                &user,
                user_token.key,
                withdraw_amount,
                Some(false),
                KaminoRefreshOptions {
                    external: false,
                    internal: true,
                    ..Default::default()
                },
            )
            .await?;
    } else {
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
    }

    let expected_underlying = setup
        .load_reserve()
        .await
        .collateral_to_liquidity(withdraw_amount)?;
    if internal_refresh {
        assert!(expected_underlying.abs_diff(stale_underlying) > KAMINO_ROUNDING_TOLERANCE_NATIVE);
    }

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
#[test_case(0.01, 1_000_000, false; "1_usdc_external")]
#[test_case(1.0, 100_000_000, true; "100_usdc_internal")]
#[test_case(100.0, 500_000_000_000, false; "500000_usdc_external")]
#[tokio::test]
async fn kamino_deposit_local_instruction_call_failure_insufficient_funds(
    wallet_funding: f64,
    deposit_amount: u64,
    internal_refresh: bool,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(wallet_funding).await;
    let pre_state = setup.load_state(&user_token).await;
    let pre_accounted = setup.load_user_accounted_collateral(&user).await;

    let res = setup
        .test_f
        .run_kamino_deposit_with_options(
            &setup.bank_f,
            &user,
            user_token.key,
            deposit_amount,
            KaminoRefreshOptions {
                external: !internal_refresh,
                internal: internal_refresh,
                ..Default::default()
            },
        )
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
#[test_case(10.0, 1_000_000, false; "1_usdc_external")]
#[test_case(10_000.0, 5_000_000_000, true; "5000_usdc_internal")]
#[tokio::test]
async fn kamino_withdraw_local_instruction_call_failure_oversized_amount(
    wallet_funding: f64,
    deposit_amount: u64,
    internal_refresh: bool,
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
        .run_kamino_withdraw_with_options(
            &setup.bank_f,
            &user,
            user_token.key,
            u64::MAX,
            Some(false),
            KaminoRefreshOptions {
                external: !internal_refresh,
                internal: internal_refresh,
                ..Default::default()
            },
        )
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

async fn warp_clock_forward(setup: &KaminoBankSetup) -> anyhow::Result<()> {
    let clock = setup.test_f.get_clock().await;
    let next_slot = clock.slot + WARP_DAYS * SOLANA_SLOTS_PER_DAY;
    let next_timestamp = clock.unix_timestamp + (WARP_DAYS * 24 * 60 * 60) as i64;
    // Kamino accrues interest using slots or timestamps depending on the reserve configuration.
    setup.test_f.context.borrow_mut().warp_to_slot(next_slot)?;
    setup.test_f.set_clock(next_slot, next_timestamp).await;
    Ok(())
}

async fn send_kamino_ixs(
    setup: &KaminoBankSetup,
    ixs: &[Instruction],
) -> anyhow::Result<(Result<(), TransactionError>, Vec<String>)> {
    let (mut banks, payer) = {
        let ctx = setup.test_f.context.borrow();
        (ctx.banks_client.clone(), ctx.payer.insecure_clone())
    };
    let blockhash = banks.get_latest_blockhash().await?;

    let mut all = vec![ComputeBudgetInstruction::set_compute_unit_limit(
        KAMINO_TX_CU_LIMIT,
    )];
    all.extend_from_slice(ixs);

    let tx = Transaction::new_signed_with_payer(&all, Some(&payer.pubkey()), &[&payer], blockhash);
    let res = banks.process_transaction_with_metadata(tx).await?;
    let logs = res.metadata.map(|m| m.log_messages).unwrap_or_default();
    Ok((res.result, logs))
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
    assert!(
        raw_price > I80F48::ZERO,
        "cache should be warm after withdraw"
    );
    assert_ne!(
        multiplier,
        I80F48::ONE,
        "the Kamino reserve must produce a non-unit multiplier for this test to be meaningful"
    );

    // Enable the breaker; the reference must be the effective (multiplier-adjusted) price.
    setup
        .bank_f
        .update_config(standard_cb_config(), None)
        .await?;

    let enabled = setup.bank_f.load().await;
    let seeded_ref: I80F48 = enabled.cb_reference_price.into();
    assert_eq!(seeded_ref, raw_price * multiplier);
    Ok(())
}

/// Kamino blocks its own instructions in emergency mode, but us lending against that collateral
/// needs no CPI, so nothing upstream stops it. We stop valuing it for Initial margin, and keep
/// Maintenance value so the position stays priced for repay and liquidation.
#[tokio::test]
async fn kamino_emergency_mode_zeroes_init_value_only() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10_000.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 5_000_000_000)
        .await?;

    // Sanity: a healthy reserve carries real Initial value.
    user.try_lending_account_pulse_health().await?;
    let healthy_cache = user.load().await.health_cache;
    assert!(I80F48::from(healthy_cache.asset_value) > I80F48::ZERO);

    setup.set_reserve_emergency_mode().await;

    user.try_lending_account_pulse_health().await?;
    let emergency_cache = user.load().await.health_cache;
    assert_eq!(
        I80F48::from(emergency_cache.asset_value),
        I80F48::ZERO,
        "emergency-mode collateral must back no new borrowing"
    );
    assert!(
        I80F48::from(emergency_cache.asset_value_maint) > I80F48::ZERO,
        "Maintenance value must survive"
    );

    // Kamino also refuses to move the collateral, so our withdraw fails inside the CPI.
    let res = setup
        .test_f
        .run_kamino_withdraw(&setup.bank_f, &user, user_token.key, 0, Some(true))
        .await;
    // Kamino's `ReserveEmergencyMode` (6175), not one of ours.
    let err = format!("{:?}", res.unwrap_err());
    assert!(
        err.contains("Custom(6175)"),
        "expected Kamino ReserveEmergencyMode, got: {err}"
    );

    Ok(())
}

/// Kamino halting a whole market stops its own instructions but keeps serving `refresh_reserve`,
/// so nothing upstream stops users borrowing against collateral parked there. The market account
/// never reaches our health path, so a permissionless ix caches the flag on the bank.
#[tokio::test]
async fn kamino_market_emergency_mode_zeroes_init_value_only() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10_000.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 5_000_000_000)
        .await?;

    setup.set_market_emergency_mode(true).await;

    // The market flag alone changes nothing, since health checks never load that account.
    user.try_lending_account_pulse_health().await?;
    let unpropagated = user.load().await.health_cache;
    assert!(I80F48::from(unpropagated.asset_value) > I80F48::ZERO);

    setup.try_propagate_market_emergency().await?;
    assert!(setup.bank_f.load().await.get_flag(KAMINO_MARKET_EMERGENCY));

    user.try_lending_account_pulse_health().await?;
    let halted = user.load().await.health_cache;
    assert_eq!(
        I80F48::from(halted.asset_value),
        I80F48::ZERO,
        "collateral in a halted market must back no new borrowing"
    );
    assert!(
        I80F48::from(halted.asset_value_maint) > I80F48::ZERO,
        "Maintenance value must survive"
    );

    // Kamino resuming the market restores borrowing power, through the same permissionless ix.
    setup.set_market_emergency_mode(false).await;
    setup.try_propagate_market_emergency().await?;
    assert!(!setup.bank_f.load().await.get_flag(KAMINO_MARKET_EMERGENCY));

    user.try_lending_account_pulse_health().await?;
    let resumed = user.load().await.health_cache;
    assert!(I80F48::from(resumed.asset_value) > I80F48::ZERO);

    Ok(())
}

#[tokio::test]
async fn kamino_deposit_refresh_flag_uses_refreshed_exchange_rate() -> anyhow::Result<()> {
    const DEPOSIT_AMOUNT: u64 = 100_000_000;

    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(500.0).await;

    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, DEPOSIT_AMOUNT)
        .await?;

    let pre = setup.load_state(&user_token).await;
    let pre_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("initial deposit must create an active balance");

    warp_clock_forward(&setup).await?;
    TestFixture::process_ixs(
        setup.test_f.context.clone(),
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(KAMINO_TX_CU_LIMIT),
            user.make_kamino_refresh_reserves_batch_ix(&setup.bank_f)
                .await,
        ],
    )
    .await?;

    let clock = setup.test_f.get_clock().await;
    let refreshed_reserve = setup.load_reserve().await;
    let obligation: kamino_mocks::state::MinimalObligation =
        setup.test_f.load_and_deserialize(&setup.obligation).await;

    assert_eq!(
        refreshed_reserve.slot, clock.slot,
        "reserve must be refreshed"
    );
    assert!(
        obligation.last_update_slot < clock.slot,
        "obligation must remain stale after reserve-only refresh"
    );

    let mut stale_ix = user
        .make_kamino_deposit_ix(user_token.key, &setup.bank_f, DEPOSIT_AMOUNT)
        .await;
    stale_ix.data = marginfi::instruction::KaminoDeposit {
        amount: DEPOSIT_AMOUNT,
        refresh_reserve: Some(false),
    }
    .data();

    let (result, logs) = send_kamino_ixs(&setup, &[stale_ix]).await?;
    assert!(result.is_err(), "deposit with stale obligation must fail");
    assert!(
        logs.iter().any(|l| l.contains("ObligationStale")),
        "expected klend ObligationStale, got:\n{logs:#?}"
    );

    let pre_internal_reserve = setup.load_reserve().await;
    warp_clock_forward(&setup).await?;

    setup
        .test_f
        .run_kamino_deposit_with_options(
            &setup.bank_f,
            &user,
            user_token.key,
            DEPOSIT_AMOUNT,
            KaminoRefreshOptions {
                external: false,
                internal: true,
                ..Default::default()
            },
        )
        .await?;

    let post_obligation: kamino_mocks::state::MinimalObligation =
        setup.test_f.load_and_deserialize(&setup.obligation).await;
    let post_clock = setup.test_f.get_clock().await;
    assert_eq!(
        post_obligation.last_update_slot, post_clock.slot,
        "obligation must have been refreshed in the current slot"
    );

    let post_reserve = setup.load_reserve().await;
    let expected_collateral = post_reserve.liquidity_to_collateral(DEPOSIT_AMOUNT)?;
    let stale_rate_collateral = pre_internal_reserve.liquidity_to_collateral(DEPOSIT_AMOUNT)?;
    assert!(
        expected_collateral.abs_diff(stale_rate_collateral) > KAMINO_ROUNDING_TOLERANCE_NATIVE,
        "{WARP_DAYS}-day interest must change the exchange rate"
    );

    let post = setup.load_state(&user_token).await;
    let actual_collateral =
        i128::from(post.obligation_collateral) - i128::from(pre.obligation_collateral);
    assert_eq_with_tolerance!(
        actual_collateral,
        expected_collateral as i128,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );

    let post_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("Kamino collateral balance must remain active");
    assert_eq_with_tolerance!(
        i128::from(post_accounted) - i128::from(pre_accounted),
        actual_collateral,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );

    Ok(())
}

#[test_case(false; "flag_off")]
#[test_case(true; "flag_on_already_fresh")]
#[tokio::test]
async fn kamino_deposit_with_caller_supplied_refresh_succeeds(
    internal_refresh: bool,
) -> anyhow::Result<()> {
    const DEPOSIT_AMOUNT: u64 = 100_000_000;

    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(500.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, DEPOSIT_AMOUNT)
        .await?;

    let pre = setup.load_state(&user_token).await;
    let pre_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("initial deposit must create an active balance");
    let pre_reserve = setup.load_reserve().await;

    warp_clock_forward(&setup).await?;

    setup
        .test_f
        .run_kamino_deposit_with_options(
            &setup.bank_f,
            &user,
            user_token.key,
            DEPOSIT_AMOUNT,
            KaminoRefreshOptions {
                external: true,
                internal: internal_refresh,
                skip_external_price_updates: true,
            },
        )
        .await?;

    let post_reserve = setup.load_reserve().await;
    let expected_collateral = post_reserve.liquidity_to_collateral(DEPOSIT_AMOUNT)?;
    let stale_rate_collateral = pre_reserve.liquidity_to_collateral(DEPOSIT_AMOUNT)?;
    assert!(
        expected_collateral.abs_diff(stale_rate_collateral) > KAMINO_ROUNDING_TOLERANCE_NATIVE,
        "{WARP_DAYS}-day interest must change the exchange rate"
    );

    let post = setup.load_state(&user_token).await;
    let actual_collateral =
        i128::from(post.obligation_collateral) - i128::from(pre.obligation_collateral);
    assert_eq_with_tolerance!(
        actual_collateral,
        expected_collateral as i128,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );

    let post_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("Kamino collateral balance must remain active");
    assert_eq_with_tolerance!(
        i128::from(post_accounted) - i128::from(pre_accounted),
        actual_collateral,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );

    Ok(())
}

#[test_case(0x02, false; "partial_internal_refresh")]
#[test_case(0x01, true; "withdraw_all_caller_refresh")]
#[test_case(0x03, true; "withdraw_all_internal_refresh_already_fresh")]
#[tokio::test]
async fn kamino_withdraw_uses_refreshed_exchange_rate(
    flags: u8,
    caller_refresh: bool,
) -> anyhow::Result<()> {
    const REQUESTED_COLLATERAL: u64 = 1_000_000_000;

    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10_000.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 5_000_000_000)
        .await?;

    let withdraw_all = flags & 0x01 != 0;
    let pre = setup.load_state(&user_token).await;
    let pre_accounted = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("Kamino balance must be active before withdrawing");
    let pre_reserve = setup.load_reserve().await;
    warp_clock_forward(&setup).await?;

    let amount = if withdraw_all {
        0
    } else {
        REQUESTED_COLLATERAL
    };
    setup
        .test_f
        .run_kamino_withdraw_with_options(
            &setup.bank_f,
            &user,
            user_token.key,
            amount,
            Some(withdraw_all),
            KaminoRefreshOptions {
                external: caller_refresh,
                internal: flags & 0x02 != 0,
                skip_external_price_updates: caller_refresh,
            },
        )
        .await?;

    let post = setup.load_state(&user_token).await;
    let post_reserve = setup.load_reserve().await;

    let redeemed = i128::from(pre.obligation_collateral) - i128::from(post.obligation_collateral);
    assert!(redeemed > 0, "withdraw must redeem collateral");
    let actual_collateral = redeemed as u64;
    if !withdraw_all {
        assert_eq!(actual_collateral, REQUESTED_COLLATERAL);
    }

    let expected_liquidity = post_reserve.collateral_to_liquidity(actual_collateral)?;
    let stale_rate_liquidity = pre_reserve.collateral_to_liquidity(actual_collateral)?;
    let liquidity_gap = expected_liquidity.abs_diff(stale_rate_liquidity);
    assert!(
        liquidity_gap > KAMINO_ROUNDING_TOLERANCE_NATIVE,
        "{WARP_DAYS}-day interest must change withdrawn liquidity beyond tolerance (gap={liquidity_gap})"
    );

    let actual_liquidity = i128::from(post.user_balance) - i128::from(pre.user_balance);
    assert_eq!(
        i128::from(pre.reserve_supply_balance) - i128::from(post.reserve_supply_balance),
        actual_liquidity
    );
    assert_eq_with_tolerance!(
        actual_liquidity,
        expected_liquidity as i128,
        KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
    );

    if withdraw_all {
        assert_eq_with_tolerance!(
            redeemed,
            pre_accounted as i128,
            KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
        );
        assert!(
            setup.load_user_accounted_collateral(&user).await.is_none(),
            "withdraw-all should close the user marginfi balance"
        );
    } else {
        let post_accounted = setup
            .load_user_accounted_collateral(&user)
            .await
            .expect("partial withdraw should keep a balance open");
        assert_eq_with_tolerance!(
            i128::from(pre_accounted) - i128::from(post_accounted),
            actual_collateral as i128,
            KAMINO_ROUNDING_TOLERANCE_NATIVE as i128
        );
    }
    Ok(())
}

#[test_case(false, "ReserveStale"; "nothing_refreshed")]
#[test_case(true, "ObligationStale"; "reserve_only_refreshed")]
#[tokio::test]
async fn kamino_withdraw_without_refresh_flag_fails_on_stale_state(
    refresh_reserve_first: bool,
    expected_error: &str,
) -> anyhow::Result<()> {
    const REQUESTED_COLLATERAL: u64 = 1_000_000_000;

    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10_000.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 5_000_000_000)
        .await?;

    warp_clock_forward(&setup).await?;

    if refresh_reserve_first {
        TestFixture::process_ixs(
            setup.test_f.context.clone(),
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(KAMINO_TX_CU_LIMIT),
                user.make_kamino_refresh_reserves_batch_ix(&setup.bank_f)
                    .await,
            ],
        )
        .await?;
    }

    let mut ix = user
        .make_kamino_withdraw_ix(
            user_token.key,
            &setup.bank_f,
            REQUESTED_COLLATERAL,
            Some(false),
        )
        .await;
    ix.data = marginfi::instruction::KaminoWithdraw {
        amount: REQUESTED_COLLATERAL,
        flags: Some(0x00),
    }
    .data();

    let (result, logs) = send_kamino_ixs(&setup, &[ix]).await?;
    assert!(result.is_err(), "stale withdraw without refresh must fail");
    assert!(
        logs.iter().any(|l| l.contains(expected_error)),
        "expected {expected_error}, got:\n{logs:#?}"
    );
    Ok(())
}

// Snapshot all accounts whose data can change in a deposit or withdrawal. The payer's fee is
// intentionally excluded: transaction fees are charged even when an instruction fails.
async fn kamino_account_data(
    setup: &KaminoBankSetup,
    user: Pubkey,
    user_token: Pubkey,
) -> anyhow::Result<Vec<Vec<u8>>> {
    let bank = setup.bank_f.load().await;
    let addresses = [
        setup.bank_f.key,
        user,
        user_token,
        bank.liquidity_vault,
        bank.integration_acc_1,
        setup.obligation,
        setup.reserve_liquidity_supply,
    ];
    let mut data = Vec::with_capacity(addresses.len());
    for address in addresses {
        data.push(setup.test_f.try_load(&address).await?.unwrap().data);
    }
    Ok(data)
}

#[tokio::test]
async fn kamino_deposit_refreshes_before_bank_validation() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10.0).await;

    let mut bank = setup.test_f.try_load(&setup.bank_f.key).await?.unwrap();
    bytemuck::from_bytes_mut::<Bank>(&mut bank.data[8..])
        .config
        .operational_state = BankOperationalState::Paused;
    setup
        .test_f
        .context
        .borrow_mut()
        .set_account(&setup.bank_f.key, &AccountSharedData::from(bank));
    let before = kamino_account_data(&setup, user.key, user_token.key).await?;

    let mut ix = user
        .make_kamino_deposit_ix(user_token.key, &setup.bank_f, 1_000_000)
        .await;
    ix.data = marginfi::instruction::KaminoDeposit {
        amount: 1_000_000,
        refresh_reserve: Some(true),
    }
    .data();
    let (result, logs) = send_kamino_ixs(&setup, &[ix]).await?;
    assert!(result.is_err(), "deposit into a paused bank must fail");
    let reserve_refresh = logs
        .iter()
        .position(|l| l.contains("Instruction: RefreshReservesBatch"));
    let obligation_refresh = logs
        .iter()
        .position(|l| l.contains("Instruction: RefreshObligation"));
    let validation_error = logs.iter().position(|l| l.contains("BankPaused"));
    assert!(
        matches!((reserve_refresh, obligation_refresh, validation_error),
            (Some(r), Some(o), Some(e)) if r < o && o < e),
        "expected reserve refresh, obligation refresh, then bank validation: {logs:#?}"
    );
    assert_eq!(
        before,
        kamino_account_data(&setup, user.key, user_token.key).await?
    );
    Ok(())
}

#[test_case(false; "partial")]
#[test_case(true; "withdraw_all")]
#[tokio::test]
async fn kamino_withdraw_refreshes_before_checking_balance(
    withdraw_all: bool,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10.0).await;
    let before = kamino_account_data(&setup, user.key, user_token.key).await?;

    // The account has no position, so withdrawal must fail after both refreshes.
    let mut ix = user
        .make_kamino_withdraw_ix(user_token.key, &setup.bank_f, 1, Some(withdraw_all))
        .await;
    ix.data = marginfi::instruction::KaminoWithdraw {
        amount: 1,
        flags: Some(if withdraw_all { 0x03 } else { 0x02 }),
    }
    .data();
    let (result, logs) = send_kamino_ixs(&setup, &[ix]).await?;
    assert!(result.is_err(), "withdraw without a position must fail");
    let reserve_refresh = logs
        .iter()
        .position(|l| l.contains("Instruction: RefreshReservesBatch"));
    let obligation_refresh = logs
        .iter()
        .position(|l| l.contains("Instruction: RefreshObligation"));
    let balance_error = logs.iter().position(|l| l.contains("BankAccountNotFound"));
    assert!(
        matches!((reserve_refresh, obligation_refresh, balance_error),
            (Some(r), Some(o), Some(e)) if r < o && o < e),
        "expected reserve refresh, obligation refresh, then balance error: {logs:#?}"
    );

    // The failed transaction must also roll back the otherwise successful refreshes.
    assert_eq!(
        before,
        kamino_account_data(&setup, user.key, user_token.key).await?
    );
    Ok(())
}

#[test_case(false; "partial")]
#[test_case(true; "withdraw_all")]
#[tokio::test]
async fn kamino_withdraw_rolls_back_when_obligation_refresh_fails(
    withdraw_all: bool,
) -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(500.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 100_000_000)
        .await?;
    warp_clock_forward(&setup).await?;

    // Keep the reserve valid, but make Kamino reject the obligation's market after the reserve
    // refresh. Marginfi's own deposit-reserve constraints still pass.
    let mut obligation = setup.test_f.try_load(&setup.obligation).await?.unwrap();
    let data = bytemuck::from_bytes_mut::<kamino_mocks::state::MinimalObligation>(
        &mut obligation.data[8..],
    );
    data.lending_market = Pubkey::default();
    setup
        .test_f
        .context
        .borrow_mut()
        .set_account(&setup.obligation, &AccountSharedData::from(obligation));
    let before = kamino_account_data(&setup, user.key, user_token.key).await?;

    let mut ix = user
        .make_kamino_withdraw_ix(user_token.key, &setup.bank_f, 1_000_000, Some(withdraw_all))
        .await;
    ix.data = marginfi::instruction::KaminoWithdraw {
        amount: 1_000_000,
        flags: Some(if withdraw_all { 0x03 } else { 0x02 }),
    }
    .data();
    let (result, logs) = send_kamino_ixs(&setup, &[ix]).await?;
    assert!(
        result.is_err(),
        "mismatched obligation market must fail: {logs:#?}"
    );
    assert!(
        logs.iter()
            .any(|l| l.contains("Instruction: RefreshObligation")),
        "expected obligation refresh failure: {logs:#?}"
    );
    assert_eq!(
        before,
        kamino_account_data(&setup, user.key, user_token.key).await?
    );
    Ok(())
}

#[tokio::test]
async fn kamino_deposit_rolls_back_refresh_when_transfer_fails() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10.0).await;
    warp_clock_forward(&setup).await?;
    let before = kamino_account_data(&setup, user.key, user_token.key).await?;

    let mut ix = user
        .make_kamino_deposit_ix(user_token.key, &setup.bank_f, 100_000_000)
        .await;
    ix.data = marginfi::instruction::KaminoDeposit {
        amount: 100_000_000,
        refresh_reserve: Some(true),
    }
    .data();
    let (result, logs) = send_kamino_ixs(&setup, &[ix]).await?;
    assert!(
        result.is_err(),
        "deposit exceeding the token balance must fail"
    );
    assert!(
        logs.iter()
            .any(|l| l.contains("Instruction: RefreshObligation")),
        "expected refresh before transfer failure: {logs:#?}"
    );
    assert!(
        logs.iter().any(|l| l.contains("insufficient funds")),
        "expected token transfer failure: {logs:#?}"
    );
    assert_eq!(
        before,
        kamino_account_data(&setup, user.key, user_token.key).await?
    );
    Ok(())
}
