use bytemuck::from_bytes_mut;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{
    assert_custom_error,
    bank::BankFixture,
    marginfi_account::MarginfiAccountFixture,
    marginfi_group::MarginfiGroupFixture,
    test::{BankMint, TestFixture, TestSettings},
};
use marginfi::errors::MarginfiError;
use marginfi_type_crate::types::{Bank, BankConfigOpt, BankOperationalState};
use solana_program_test::tokio;
use solana_sdk::clock::Clock;

#[tokio::test]
async fn lending_account_close_balance() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_eq_bank = test_f.get_bank(&BankMint::SolEquivalent);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_eq_bank, 1_000, None)
        .await?;

    let lender_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_bank, 1_000, None)
        .await?;

    let res = lender_mfi_account_f.try_balance_close(sol_bank).await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_f_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let borrower_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let borrower_token_account_f_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_f_usdc.key, usdc_bank, 1_000, None)
        .await?;

    // Borrow SOL EQ
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol_eq.key, sol_eq_bank, 0.01)
        .await;

    assert!(res.is_ok());

    // Borrow SOL
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_bank, 0.01)
        .await;

    assert!(res.is_ok());

    // This issue is not that bad, because the user can still borrow other assets (isolated liab < empty threshold)
    let res = borrower_mfi_account_f.try_balance_close(sol_bank).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);

    // Before v0.1.12, a partial repay could orphan dust of this shape, so a real balance may
    // still contain it. Since v0.1.12 prevents repay from creating orphaned dust, seed the
    // legacy state directly to retain coverage of the close path.
    let dust_liability_shares = I80F48!(0.00005);
    let mut borrower_account = borrower_mfi_account_f.load().await;
    let sol_eq_balance = borrower_account
        .lending_account
        .balances
        .iter_mut()
        .find(|balance| balance.is_active() && balance.bank_pk == sol_eq_bank.key)
        .unwrap();
    sol_eq_balance.liability_shares = dust_liability_shares.into();
    borrower_mfi_account_f
        .set_account(&borrower_account)
        .await?;

    {
        let mut bank_account = test_f
            .context
            .borrow_mut()
            .banks_client
            .get_account(sol_eq_bank.key)
            .await?
            .unwrap();
        let bank = from_bytes_mut::<Bank>(&mut bank_account.data.as_mut_slice()[8..]);
        bank.total_liability_shares = dust_liability_shares.into();
        // Dust below ZERO_AMOUNT_THRESHOLD does not count as an active borrow.
        bank.borrowing_position_count = 0;
        test_f
            .context
            .borrow_mut()
            .set_account(&sol_eq_bank.key, &bank_account.into());
    }

    // The dust is below the repayment tolerance, so it must be cleared through close_balance, repay
    // will fail. The borrower could also take on more debt, then repay.
    let last_update_before_failed_repay = borrower_mfi_account_f.load().await.last_update;
    let res = borrower_mfi_account_f
        .try_bank_repay(
            borrower_token_account_f_sol_eq.key,
            sol_eq_bank,
            1,
            Some(true),
        )
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::NoLiabilityFound);
    assert_eq!(
        last_update_before_failed_repay,
        borrower_mfi_account_f.load().await.last_update
    );

    // Let another second pass
    let last_update_before_close = borrower_mfi_account_f.load().await.last_update;
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 second
        clock.unix_timestamp += 1;
        ctx.set_sysvar(&clock);
    }

    let res = borrower_mfi_account_f.try_balance_close(sol_eq_bank).await;
    assert!(res.is_ok());
    let account = borrower_mfi_account_f.load().await;
    // Balance closing also updates last_update
    assert_eq!(account.last_update, last_update_before_close + 1);
    assert_eq!(account.indexer_flags.is_lending_only, 0);
    assert_eq!(account.indexer_flags.is_single_borrower, 1);

    Ok(())
}

async fn set_operational_state(
    group: &MarginfiGroupFixture,
    bank: &BankFixture,
    state: BankOperationalState,
) -> anyhow::Result<()> {
    group
        .try_lending_pool_configure_bank(
            bank,
            BankConfigOpt {
                operational_state: Some(state),
                ..BankConfigOpt::default()
            },
        )
        .await?;
    Ok(())
}

async fn has_active_balance(user: &MarginfiAccountFixture, bank: &BankFixture) -> bool {
    user.load()
        .await
        .lending_account
        .get_balance(&bank.key)
        .is_some()
}

/// Withdrawing an integration position's exact amount without `withdraw_all` leaves an active
/// balance with zero shares. Its own `withdraw_all` rejects it and a Paused bank blocks the
/// deposit-then-withdraw_all workaround, so `close_balance` is the only way to free the slot.
#[tokio::test]
async fn close_balance_frees_empty_kamino_balance_on_paused_bank() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let test_f = &setup.test_f;
    let bank_f = &setup.bank_f;
    let (user, user_token) = setup.create_user_with_liquidity(10.0).await;

    test_f
        .run_kamino_deposit(bank_f, &user, user_token.key, 1_000_000)
        .await?;
    let collateral = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino balance should be active after deposit");
    test_f
        .run_kamino_withdraw(bank_f, &user, user_token.key, collateral, None)
        .await?;

    let account = user.load().await;
    let balance = account
        .lending_account
        .get_balance(&bank_f.key)
        .expect("partial-path withdraw of the full amount leaves the balance active");
    assert_eq!(I80F48::from(balance.asset_shares), I80F48::ZERO);
    assert_eq!(I80F48::from(balance.liability_shares), I80F48::ZERO);

    let res = test_f
        .run_kamino_withdraw(bank_f, &user, user_token.key, 0, Some(true))
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::NoAssetFound);

    set_operational_state(&test_f.marginfi_group, bank_f, BankOperationalState::Paused).await?;
    let res = test_f
        .run_kamino_deposit(bank_f, &user, user_token.key, 1_000_000)
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankPaused);

    let bank_before = bank_f.load().await;
    user.try_balance_close(bank_f).await?;

    assert!(!has_active_balance(&user, bank_f).await);
    let bank_after = bank_f.load().await;
    assert_eq!(
        bank_after.lending_position_count, bank_before.lending_position_count,
        "the withdraw already released the position count; close must not decrement it again"
    );
    assert_eq!(
        I80F48::from(bank_after.total_asset_shares),
        I80F48::from(bank_before.total_asset_shares)
    );

    Ok(())
}

#[tokio::test]
async fn close_balance_rejects_integration_balance_with_assets() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 1_000_000)
        .await?;

    let res = user.try_balance_close(&setup.bank_f).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);
    assert!(has_active_balance(&user, &setup.bank_f).await);

    Ok(())
}

/// Drift and JupLend withdraws take token amounts that convert to venue shares with rounding, so
/// an exact zero is not reliably reachable through their withdraw. Seed the empty-but-active state
/// directly to cover the asset-tag gate for these venues.
async fn zero_out_balance(user: &MarginfiAccountFixture, bank: &BankFixture) -> anyhow::Result<()> {
    let mut account = user.load().await;
    let balance = account
        .lending_account
        .balances
        .iter_mut()
        .find(|b| b.is_active() && b.bank_pk == bank.key)
        .expect("balance should be active after deposit");
    balance.asset_shares = I80F48::ZERO.into();
    user.set_account(&account).await
}

#[tokio::test]
async fn close_balance_frees_empty_drift_balance_on_reduce_only_bank() -> anyhow::Result<()> {
    let setup = TestFixture::setup_drift_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10.0).await;
    setup
        .test_f
        .run_drift_deposit(&setup.bank_f, &user, user_token.key, 1_000_000)
        .await?;
    zero_out_balance(&user, &setup.bank_f).await?;
    set_operational_state(
        &setup.test_f.marginfi_group,
        &setup.bank_f,
        BankOperationalState::ReduceOnly,
    )
    .await?;

    user.try_balance_close(&setup.bank_f).await?;
    assert!(!has_active_balance(&user, &setup.bank_f).await);

    Ok(())
}

#[tokio::test]
async fn close_balance_frees_empty_juplend_balance_on_reduce_only_bank() -> anyhow::Result<()> {
    let setup = TestFixture::setup_juplend_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(10.0).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &user, user_token.key, 1_000_000)
        .await?;
    zero_out_balance(&user, &setup.bank_f).await?;
    set_operational_state(
        &setup.test_f.marginfi_group,
        &setup.bank_f,
        BankOperationalState::ReduceOnly,
    )
    .await?;

    user.try_balance_close(&setup.bank_f).await?;
    assert!(!has_active_balance(&user, &setup.bank_f).await);

    Ok(())
}
