use fixed::types::I80F48;
use fixtures::{assert_custom_error, prelude::*, ui_to_native};
use marginfi::{
    assert_eq_with_tolerance,
    prelude::*,
    state::bank::{BankImpl, BankVaultType},
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::clock::Clock;
use test_case::test_case;

#[test_case(0.03, 0.012, BankMint::Usdc)]
#[test_case(128932.0, 9834.0, BankMint::PyUSD)]
#[test_case(0.5, 0.2, BankMint::Fixed)]
#[test_case(5_000., 2_000., BankMint::FixedLow)]
#[tokio::test]
async fn marginfi_account_withdraw_success(
    deposit_amount: f64,
    withdraw_amount: f64,
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // User

    let marginfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);
    let token_account_f = TokenAccountFixture::new(
        test_f.context.clone(),
        &test_f.get_bank(&bank_mint).mint,
        &test_f.payer(),
    )
    .await;
    test_f
        .get_bank_mut(&bank_mint)
        .mint
        .mint_to(&token_account_f.key, user_wallet_balance)
        .await;
    let bank_f = test_f.get_bank(&bank_mint);
    marginfi_account_f
        .try_bank_deposit(token_account_f.key, bank_f, deposit_amount, None)
        .await
        .unwrap();

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let marginfi_account = marginfi_account_f.load().await;
    // This is just to test that the account's last_update field is properly updated upon modification
    let pre_last_update = marginfi_account.last_update;
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 sec
        clock.unix_timestamp += 1;
        ctx.set_sysvar(&clock);
    }

    let pre_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let balance = marginfi_account
        .lending_account
        .get_balance(&bank_f.key)
        .unwrap();
    let pre_accounted = bank_f
        .load()
        .await
        .get_asset_amount(balance.asset_shares.into())
        .unwrap();

    let res = marginfi_account_f
        .try_bank_withdraw(token_account_f.key, bank_f, withdraw_amount, None)
        .await;
    assert!(res.is_ok());

    let post_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let marginfi_account = marginfi_account_f.load().await;
    assert_eq!(marginfi_account.last_update, pre_last_update + 1);
    let balance = marginfi_account
        .lending_account
        .get_balance(&bank_f.key)
        .unwrap();
    let post_accounted = bank_f
        .load()
        .await
        .get_asset_amount(balance.asset_shares.into())
        .unwrap();
    let post: I80F48 = post_accounted.into();
    let post: f64 = post.to_num();
    println!("post bal: {:?}", post);

    let active_balance_count = marginfi_account
        .lending_account
        .get_active_balances_iter()
        .count();
    assert_eq!(1, active_balance_count);

    let expected_liquidity_vault_delta =
        -I80F48::from(ui_to_native!(withdraw_amount, bank_f.mint.mint.decimals));
    let actual_liquidity_vault_delta =
        I80F48::from(post_vault_balance) - I80F48::from(pre_vault_balance);

    let accounted_user_balance_delta = post_accounted - pre_accounted;

    assert_eq!(expected_liquidity_vault_delta, actual_liquidity_vault_delta);
    assert_eq_with_tolerance!(
        expected_liquidity_vault_delta,
        accounted_user_balance_delta,
        1
    );

    let health_cache = marginfi_account.health_cache;
    let collateral_price_roughly = get_mint_price(bank_mint);
    // Apply a small discount to account for conf discounts, etc.
    let disc: f64 = 0.95;
    assert!(health_cache.is_engine_ok());
    assert!(health_cache.is_healthy());

    let asset_value: I80F48 = health_cache.asset_value.into();
    let asset_value: f64 = asset_value.to_num();
    let diff = deposit_amount - withdraw_amount;
    assert!(asset_value >= (diff) * collateral_price_roughly * disc);

    for (i, bal) in marginfi_account.lending_account.balances.iter().enumerate() {
        let shares: I80F48 = bal.asset_shares.into();
        if bal.is_active() {
            let price: f64 = f64::from_le_bytes(health_cache.prices[i]);
            if shares != I80F48::ZERO {
                assert!(price >= (collateral_price_roughly * disc));
            }
        }
    }

    Ok(())
}

#[test_case(0.03, BankMint::Usdc)]
#[test_case(100.0, BankMint::Usdc)]
#[test_case(100.0, BankMint::Sol)]
#[test_case(128932.0, BankMint::PyUSD)]
#[test_case(0.5, BankMint::Fixed)]
#[test_case(5_000., BankMint::FixedLow)]
#[tokio::test]
async fn marginfi_account_withdraw_all_success(
    deposit_amount: f64,
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // User

    let marginfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);
    let token_account_f = TokenAccountFixture::new(
        test_f.context.clone(),
        &test_f.get_bank(&bank_mint).mint,
        &test_f.payer(),
    )
    .await;
    test_f
        .get_bank_mut(&bank_mint)
        .mint
        .mint_to(&token_account_f.key, user_wallet_balance)
        .await;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let bank_f = test_f.get_bank(&bank_mint);

    marginfi_account_f
        .try_bank_deposit(token_account_f.key, bank_f, deposit_amount, None)
        .await
        .unwrap();

    let marginfi_account = marginfi_account_f.load().await;
    let pre_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let balance = marginfi_account
        .lending_account
        .get_balance(&bank_f.key)
        .unwrap();
    let pre_accounted = bank_f
        .load()
        .await
        .get_asset_amount(balance.asset_shares.into())
        .unwrap();

    let res = marginfi_account_f
        .try_bank_withdraw(token_account_f.key, bank_f, 0, Some(true))
        .await;
    assert!(res.is_ok());

    let marginfi_account = marginfi_account_f.load().await;

    let active_balance_count = marginfi_account
        .lending_account
        .get_active_balances_iter()
        .count();
    assert_eq!(0, active_balance_count);

    let post_vault_balance = bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    assert!(marginfi_account
        .lending_account
        .get_balance(&bank_f.key)
        .is_none());
    let post_accounted = I80F48::ZERO;

    let deposit_amount_native = ui_to_native!(deposit_amount, bank_f.mint.mint.decimals);

    let expected_liquidity_vault_delta = -I80F48::from(deposit_amount_native);
    let actual_liquidity_vault_delta =
        I80F48::from(post_vault_balance) - I80F48::from(pre_vault_balance);
    let accounted_user_balance_delta = post_accounted - pre_accounted;

    assert_eq!(expected_liquidity_vault_delta, actual_liquidity_vault_delta);
    assert_eq_with_tolerance!(
        expected_liquidity_vault_delta,
        accounted_user_balance_delta,
        1
    );

    Ok(())
}

#[test_case(0.03, 0.030001, BankMint::Usdc)]
#[test_case(100., 102., BankMint::Sol)]
#[test_case(109247394., 109247394.000001, BankMint::PyUSD)]
#[tokio::test]
async fn marginfi_account_withdraw_failure_withdrawing_too_much(
    deposit_amount: f64,
    withdraw_amount: f64,
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // User

    let marginfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);
    let token_account_f = TokenAccountFixture::new(
        test_f.context.clone(),
        &test_f.get_bank(&bank_mint).mint,
        &test_f.payer(),
    )
    .await;
    test_f
        .get_bank_mut(&bank_mint)
        .mint
        .mint_to(&token_account_f.key, user_wallet_balance)
        .await;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let bank_f = test_f.get_bank(&bank_mint);

    marginfi_account_f
        .try_bank_deposit(token_account_f.key, bank_f, deposit_amount, None)
        .await?;

    let res = marginfi_account_f
        .try_bank_withdraw(token_account_f.key, bank_f, withdraw_amount, None)
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::OperationWithdrawOnly);

    Ok(())
}
