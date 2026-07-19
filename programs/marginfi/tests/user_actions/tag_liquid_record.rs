use anchor_lang::prelude::Clock;
use fixed_macro::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, native, prelude::*};
use marginfi::constants::{LIQUIDATION_TAG_DELAY_SECS, LIQUIDATION_TAG_FULL_PREMIUM_SECS};
use marginfi::prelude::*;
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{BankConfigOpt, LiquidationRecord},
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

/// Sends `ixs` as a single payer-signed transaction.
async fn send_ixs(test_f: &TestFixture, ixs: &[Instruction]) -> Result<(), BanksClientError> {
    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    ctx.banks_client
        .process_transaction_with_preflight(tx)
        .await
}

async fn init_record(
    test_f: &TestFixture,
    liquidatee: &MarginfiAccountFixture,
    record_pk: Pubkey,
) -> anyhow::Result<()> {
    let payer = test_f.payer();
    let init_ix = liquidatee
        .make_init_liquidation_record_ix(record_pk, payer)
        .await;
    send_ixs(test_f, &[init_ix]).await?;
    Ok(())
}

/// `nonce` varies the compute budget so repeated tags are distinct txs (BanksClient replays
/// cached results for byte-identical txs).
async fn send_tag(
    test_f: &TestFixture,
    liquidatee: &MarginfiAccountFixture,
    record_pk: Pubkey,
    nonce: u32,
) -> Result<(), BanksClientError> {
    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000 + nonce);
    let tag_ix = liquidatee.make_tag_liquidation_record_ix(record_pk).await;
    send_ixs(test_f, &[cu_ix, tag_ix]).await
}

async fn load_record(test_f: &TestFixture, record_pk: Pubkey) -> LiquidationRecord {
    let ctx = test_f.context.borrow_mut();
    let account = ctx
        .banks_client
        .get_account(record_pk)
        .await
        .unwrap()
        .unwrap();
    *bytemuck::from_bytes::<LiquidationRecord>(&account.data[8..])
}

async fn current_time(test_f: &TestFixture) -> i64 {
    let clock: Clock = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_sysvar()
        .await
        .unwrap();
    clock.unix_timestamp
}

/// Sets the clock's unix timestamp without warping slots (the fixture's clock starts at 0, and
/// slot warps recompute the timestamp from genesis, clobbering manual values).
async fn set_timestamp(test_f: &TestFixture, timestamp: i64) {
    let mut clock: Clock = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_sysvar()
        .await
        .unwrap();
    clock.unix_timestamp = timestamp;
    test_f.context.borrow_mut().set_sysvar(&clock);
}

const T0: i64 = 1_000;

/// Re-publish the SOL and USDC feeds at the current clock time so warped tests don't hit oracle
/// staleness checks.
async fn refresh_oracles(test_f: &TestFixture) {
    let now = current_time(test_f).await;
    test_f.set_pyth_oracle_timestamp(PYTH_SOL_FEED, now).await;
    test_f.set_pyth_oracle_timestamp(PYTH_USDC_FEED, now).await;
}

/// Runs a full receivership liquidation in one tx: start, seize `withdraw_sol` SOL, repay
/// `repay_usdc` USDC, end.
async fn run_receivership_liquidation(
    test_f: &TestFixture,
    liquidatee: &MarginfiAccountFixture,
    record_pk: Pubkey,
    liquidator_usdc_acc: &TokenAccountFixture,
    withdraw_sol: f64,
    repay_usdc: f64,
) -> Result<TokenAccountFixture, BanksClientError> {
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let payer = test_f.payer();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, withdraw_sol, None)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, repay_usdc, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;
    send_ixs(test_f, &[start_ix, withdraw_ix, repay_ix, end_ix]).await?;
    Ok(liquidator_sol_acc)
}

/// Liquidatee deposits $20 of SOL and borrows $10 of USDC, then SOL weights are cut so the
/// account is maintenance-unhealthy. The record PDA is initialized. Returns the liquidator's
/// marginfi account (holding a 100 USDC deposit) and USDC token account (still holding 100
/// USDC) for use in repayments.
async fn setup_unhealthy_liquidatee() -> anyhow::Result<(
    TestFixture,
    MarginfiAccountFixture,
    MarginfiAccountFixture,
    Pubkey,
    TokenAccountFixture,
)> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
        .await?;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.4).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    init_record(&test_f, &liquidatee, record_pk).await?;

    Ok((
        test_f,
        liquidatee,
        liquidator,
        record_pk,
        liquidator_usdc_acc,
    ))
}

#[tokio::test]
async fn tag_sets_clears_and_rejects_double_tag() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, _liquidator_usdc_acc) =
        setup_unhealthy_liquidatee().await?;

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, record_pk, 0).await?;
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, T0);

    set_timestamp(&test_f, T0 + 60).await;
    refresh_oracles(&test_f).await;
    let res = send_tag(&test_f, &liquidatee, record_pk, 1).await;
    assert!(res.is_err());
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::LiquidationRecordAlreadyTagged
    );
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, T0);

    // Restore weights so the account is healthy again; the tag can now be cleared
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(1).into()),
                asset_weight_maint: Some(I80F48!(1).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    set_timestamp(&test_f, T0 + 120).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, record_pk, 2).await?;
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, 0);

    // Healthy and untagged: nothing to do
    set_timestamp(&test_f, T0 + 180).await;
    refresh_oracles(&test_f).await;
    let res = send_tag(&test_f, &liquidatee, record_pk, 3).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::HealthyAccount);
    Ok(())
}

/// Once the tag matures, a liquidation is allowed at the grown premium and any non-zero repayment
/// resets the tag, including a dust-sized repayment.
#[tokio::test]
async fn tag_grows_premium_and_liquidation_resets_tag() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, liquidator_usdc_acc) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, record_pk, 0).await?;
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, T0);

    set_timestamp(&test_f, T0 + LIQUIDATION_TAG_FULL_PREMIUM_SECS).await;
    refresh_oracles(&test_f).await;
    // Accrue the warped interval's interest now so the accrual jump doesn't land between the
    // liquidation's pre/post health snapshots
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    test_f.marginfi_group.try_accrue_interest(sol_bank).await?;

    // Repay $0.20 (2% of the $10 debt) and seize $0.20 of SOL. Even this dust-sized completed
    // liquidation resets the tag.
    let liquidator_sol_acc = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.02,
        0.2,
    )
    .await?;
    // Liquidator seized exactly 0.02 SOL against a 0.2 USDC repayment.
    assert_eq!(
        liquidator_sol_acc.balance().await,
        native!(0.02, "SOL", f64)
    );
    assert_eq!(
        liquidator_usdc_acc.balance().await,
        native!(99.8, "USDC", f64)
    );

    // The completed liquidation resets the tag, clears the receiver, and records the entry.
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, 0);
    assert_eq!(record.liquidation_receiver, Pubkey::default());
    assert_eq!(
        record.entries[3].timestamp,
        T0 + LIQUIDATION_TAG_FULL_PREMIUM_SECS
    );
    Ok(())
}

/// Walks the premium growth schedule on one fixture: within the delay the cap is still the 5%
/// base; halfway through the growth window the cap is 52.5% (halfway from 5% to 100%), so a $2
/// repayment allows at most $3.05 of collateral.
#[tokio::test]
async fn premium_growth_follows_schedule() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, liquidator_usdc_acc) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, record_pk, 0).await?;
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, T0);

    // Within the delay: seizing .3 * 10 = $3 exceeds the base 5% cap on a $2 repayment
    set_timestamp(&test_f, T0 + LIQUIDATION_TAG_DELAY_SECS / 2).await;
    refresh_oracles(&test_f).await;
    let res = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.3,
        2.0,
    )
    .await;
    assert!(res.is_err());
    assert_custom_error!(res.err().unwrap(), MarginfiError::LiquidationPremiumTooHigh);

    let growth_window = LIQUIDATION_TAG_FULL_PREMIUM_SECS - LIQUIDATION_TAG_DELAY_SECS;
    set_timestamp(&test_f, T0 + LIQUIDATION_TAG_DELAY_SECS + growth_window / 2).await;
    refresh_oracles(&test_f).await;
    // Accrue the warped interval's interest now so the accrual jump doesn't land between the
    // liquidation's pre/post health snapshots
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    test_f.marginfi_group.try_accrue_interest(sol_bank).await?;

    // Mid-window: $3.50 exceeds the $3.05 cap...
    let res = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.35,
        2.0,
    )
    .await;
    assert!(res.is_err());
    assert_custom_error!(res.err().unwrap(), MarginfiError::LiquidationPremiumTooHigh);

    // ...while $2.80 is allowed, and the 20% repayment resets the tag
    let liquidator_sol_acc = run_receivership_liquidation(
        &test_f,
        &liquidatee,
        record_pk,
        &liquidator_usdc_acc,
        0.28,
        2.0,
    )
    .await?;
    assert_eq!(
        liquidator_sol_acc.balance().await,
        native!(0.28, "SOL", f64)
    );
    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, 0);
    Ok(())
}

#[tokio::test]
async fn tag_fails_on_account_with_no_liabilities() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = test_f.create_marginfi_account().await;
    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), user.key.as_ref()],
        &marginfi::ID,
    );
    init_record(&test_f, &user, record_pk).await?;

    // An account with no balances has exactly zero health but nothing to liquidate, so it must
    // not be taggable
    let res = send_tag(&test_f, &user, record_pk, 0).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::HealthyAccount);
    Ok(())
}

#[tokio::test]
async fn close_record_fails_while_tagged() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, _liquidator_usdc_acc) =
        setup_unhealthy_liquidatee().await?;

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, record_pk, 0).await?;

    let close_ix = liquidatee
        .make_close_liquidation_record_ix(record_pk, test_f.payer())
        .await;
    let res = send_ixs(&test_f, &[close_ix.clone()]).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);

    // Restore weights and clear the tag; the never-liquidated record can then close immediately
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(1).into()),
                asset_weight_maint: Some(I80F48!(1).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    set_timestamp(&test_f, T0 + 60).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, record_pk, 1).await?;

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000);
    send_ixs(&test_f, &[cu_ix, close_ix]).await?;
    let record_account = {
        let ctx = test_f.context.borrow_mut();
        ctx.banks_client.get_account(record_pk).await?
    };
    assert!(record_account.is_none());
    Ok(())
}

#[tokio::test]
async fn deleverage_resets_tag() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, _liquidator_usdc_acc) =
        setup_unhealthy_liquidatee().await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, record_pk, 0).await?;

    // The risk admin repays $2 (20% of the debt), enough to reset the tag
    let risk_admin = test_f.payer();
    let risk_admin_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(10).await;
    let start_ix = liquidatee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 2.0, None)
        .await;
    let end_ix = liquidatee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;
    send_ixs(&test_f, &[start_ix, repay_ix, end_ix]).await?;

    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, 0);
    assert_eq!(record.liquidation_receiver, Pubkey::default());
    Ok(())
}

#[tokio::test]
async fn legacy_liquidate_resets_tag() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator, record_pk, _liquidator_usdc_acc) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    set_timestamp(&test_f, T0).await;
    refresh_oracles(&test_f).await;
    send_tag(&test_f, &liquidatee, record_pk, 0).await?;

    // Liquidate 0.2 SOL ($2) against the USDC debt via the legacy instruction; the ~$1.90
    // repaid (19% of the debt) is above the reset threshold
    liquidator
        .try_liquidate(&liquidatee, sol_bank, 0.2, usdc_bank)
        .await?;

    let record = load_record(&test_f, record_pk).await;
    assert_eq!(record.tagged_at, 0);
    Ok(())
}

/// Standard circuit-breaker config: 5%/10%/25% deviation tiers with 10m/1h/4h halt durations.
fn cb_config() -> BankConfigOpt {
    BankConfigOpt {
        circuit_breaker_enabled: Some(true),
        cb_deviation_bps_tiers: Some([500, 1000, 2500]),
        cb_tier_durations_seconds: Some([600, 3600, 14400]),
        cb_escalation_window_mult: Some(2),
        cb_ema_alpha_bps: Some(1000),
        ..Default::default()
    }
}

#[tokio::test]
async fn tag_blocked_while_cb_halted() -> anyhow::Result<()> {
    let (test_f, liquidatee, _liquidator, record_pk, _liquidator_usdc_acc) =
        setup_unhealthy_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Warm the SOL price cache, enable the breaker, then trip a halt with a +100% spike
    let warm_time: i64 = 100;
    let warm_slot: u64 = 1_000;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 10_000_000_000, 0, warm_time)
        .await;
    test_f.set_clock(warm_slot, warm_time).await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;
    sol_bank.update_config(cb_config(), None).await?;

    let trip_time = warm_time + 1;
    test_f.set_clock(warm_slot + 10, trip_time).await;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 20_000_000_000, 0, trip_time)
        .await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;

    // Restore the feed and refresh USDC so the tag's health check sees fresh, unremarkable prices
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 10_000_000_000, 0, trip_time)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, trip_time)
        .await;

    let res = send_tag(&test_f, &liquidatee, record_pk, 0).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::CircuitBreakerAdminOnly);
    Ok(())
}
