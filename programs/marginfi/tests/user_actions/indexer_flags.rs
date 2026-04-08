use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::prelude::*;
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{clock::Clock, signature::Keypair};

#[tokio::test]
async fn indexer_flags_new_account_defaults() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let account_f = test_f.create_marginfi_account().await;
    let account = account_f.load().await;

    assert_eq!(account.indexer_flags.is_empty, 1);
    assert_eq!(account.indexer_flags.is_lending_only, 0);
    assert_eq!(account.indexer_flags.is_single_borrower, 0);
    assert_eq!(account.indexer_flags.has_ever_been_liquidated, 0);
    assert_eq!(account.indexer_flags.has_isolated, 0);
    assert_eq!(account.indexer_flags.has_staked, 0);
    assert_eq!(account.indexer_flags.has_kamino, 0);
    assert_eq!(account.indexer_flags.has_drift, 0);
    assert_eq!(account.indexer_flags.has_juplend, 0);

    Ok(())
}

// Pulse-derived flags

#[tokio::test]
async fn indexer_flags_pulse_sets_activity_and_trivial_balance() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let user_f = test_f.create_marginfi_account().await;
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;

    user_f
        .try_bank_deposit(user_usdc.key, usdc_bank_f, 0.000001, None)
        .await?;

    user_f.try_lending_account_pulse_health().await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.has_trivial_balance, 1);
    assert_eq!(account.indexer_flags.was_active_30d, 1);
    assert_eq!(account.indexer_flags.was_active_90d, 1);
    assert_eq!(account.indexer_flags.was_active_1y, 1);
    assert_eq!(account.indexer_flags.was_liquidatable, 0);
    assert_eq!(account.indexer_flags.was_underwater, 0);

    Ok(())
}

#[tokio::test]
async fn indexer_flags_pulse_stale_account_clears_activity() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let user_f = test_f.create_marginfi_account().await;
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    user_f
        .try_bank_deposit(user_usdc.key, usdc_bank_f, 100, None)
        .await?;

    // Advance time by > 1 year
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp += 400 * 24 * 60 * 60;
        ctx.set_sysvar(&clock);
    }

    user_f.try_lending_account_pulse_health().await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.was_active_30d, 0);
    assert_eq!(account.indexer_flags.was_active_90d, 0);
    assert_eq!(account.indexer_flags.was_active_1y, 0);

    Ok(())
}

// Sync instruction

#[tokio::test]
async fn indexer_flags_sync_instruction_recomputes_flags() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lp_f = test_f.create_marginfi_account().await;
    let lp_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lp_f.try_bank_deposit(lp_sol.key, sol_bank_f, 1_000, None)
        .await?;

    let user_f = test_f.create_marginfi_account().await;
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let user_sol = test_f.sol_mint.create_empty_token_account().await;

    user_f
        .try_bank_deposit(user_usdc.key, usdc_bank_f, 1_000, None)
        .await?;
    user_f.try_bank_borrow(user_sol.key, sol_bank_f, 10).await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.is_lending_only, 0);
    assert_eq!(account.indexer_flags.is_single_borrower, 1);

    // Calling sync should produce the same result
    user_f.try_sync_indexer_flags().await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.is_empty, 0);
    assert_eq!(account.indexer_flags.is_lending_only, 0);
    assert_eq!(account.indexer_flags.is_single_borrower, 1);

    Ok(())
}

// Pending closure / closeable flags

#[tokio::test]
async fn indexer_flags_pulse_sets_pending_closure_after_30d_empty() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user_f = test_f.create_marginfi_account().await;

    // Advance time by exactly 30 days (account is empty from creation)
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp += 30 * 24 * 60 * 60;
        ctx.set_sysvar(&clock);
    }

    user_f.try_lending_account_pulse_health().await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.is_empty, 1);
    assert_eq!(account.indexer_flags.pending_closure, 1);
    assert_eq!(account.indexer_flags.closeable, 0);

    Ok(())
}

#[tokio::test]
async fn indexer_flags_pulse_sets_closeable_after_60d_empty() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user_f = test_f.create_marginfi_account().await;

    // Advance time by exactly 60 days
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp += 60 * 24 * 60 * 60;
        ctx.set_sysvar(&clock);
    }

    user_f.try_lending_account_pulse_health().await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.pending_closure, 1);
    assert_eq!(account.indexer_flags.closeable, 1);

    Ok(())
}

#[tokio::test]
async fn indexer_flags_pending_closure_not_set_with_balances() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let user_f = test_f.create_marginfi_account().await;
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    user_f
        .try_bank_deposit(user_usdc.key, usdc_bank_f, 100, None)
        .await?;

    // Advance time by 60 days — but account has a balance
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp += 60 * 24 * 60 * 60;
        ctx.set_sysvar(&clock);
    }

    user_f.try_lending_account_pulse_health().await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.is_empty, 0);
    assert_eq!(account.indexer_flags.pending_closure, 0);
    assert_eq!(account.indexer_flags.closeable, 0);

    Ok(())
}

#[tokio::test]
async fn indexer_flags_deposit_clears_pending_closure() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let user_f = test_f.create_marginfi_account().await;

    // Advance time and pulse to set pending_closure
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp += 30 * 24 * 60 * 60;
        ctx.set_sysvar(&clock);
    }
    user_f.try_lending_account_pulse_health().await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.pending_closure, 1);

    // Deposit clears the flag via sync_from_balances
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    user_f
        .try_bank_deposit(user_usdc.key, usdc_bank_f, 100, None)
        .await?;

    let account = user_f.load().await;
    assert_eq!(account.indexer_flags.pending_closure, 0);
    assert_eq!(account.indexer_flags.closeable, 0);

    Ok(())
}

// Admin close

#[tokio::test]
async fn admin_close_account_succeeds_when_closeable() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let authority = Keypair::new();

    let user_f = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;

    // Set closeable flag directly (simulates pulse after 120d empty)
    let mut account = user_f.load().await;
    account.indexer_flags.closeable = 1;
    account.indexer_flags.is_empty = 1;
    user_f.set_account(&account).await?;

    let rent_destination = test_f.payer();
    user_f
        .try_admin_close_account(rent_destination)
        .await
        .unwrap();

    Ok(())
}

#[tokio::test]
async fn admin_close_account_fails_when_not_closeable() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let authority = Keypair::new();

    let user_f = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;

    // closeable is 0 (default)
    let rent_destination = test_f.payer();
    let res = user_f.try_admin_close_account(rent_destination).await;
    assert!(res.is_err());

    Ok(())
}

#[tokio::test]
async fn admin_close_account_fails_with_active_balances() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let user_f = test_f.create_marginfi_account().await;
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    user_f
        .try_bank_deposit(user_usdc.key, usdc_bank_f, 100, None)
        .await?;

    // Force closeable flag even though account has balances
    let mut account = user_f.load().await;
    account.indexer_flags.closeable = 1;
    user_f.set_account(&account).await?;

    let rent_destination = test_f.payer();
    let res = user_f.try_admin_close_account(rent_destination).await;
    assert!(res.is_err());

    Ok(())
}
