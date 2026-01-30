use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::prelude::*;
use fixtures::{assert_custom_error, native};
use marginfi::prelude::*;
use marginfi::state::marginfi_account::MarginfiAccountImpl;
use marginfi_type_crate::types::ACCOUNT_FROZEN;
use solana_program_test::tokio;
use solana_sdk::{signature::Keypair, signer::Signer};

#[tokio::test]
async fn admin_can_toggle_account_freeze() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let authority = Keypair::new();

    let marginfi_account = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;

    let account = marginfi_account.load().await;
    assert!(!account.get_flag(ACCOUNT_FROZEN));

    marginfi_account.try_set_freeze(true).await?;
    assert!(marginfi_account.load().await.get_flag(ACCOUNT_FROZEN));

    marginfi_account.try_set_freeze(false).await?;
    assert!(!marginfi_account.load().await.get_flag(ACCOUNT_FROZEN));

    Ok(())
}

#[tokio::test]
async fn frozen_account_blocks_authority_allows_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();

    let marginfi_account = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;
    let mut usdc_bank = test_f.get_bank(&BankMint::Usdc).clone();

    let user_token_account =
        TokenAccountFixture::new(test_f.context.clone(), &usdc_bank.mint, &authority.pubkey())
            .await;
    usdc_bank
        .mint
        .mint_to(&user_token_account.key, native!(100, "USDC") as f64)
        .await;

    marginfi_account
        .try_bank_deposit_with_authority(user_token_account.key, &usdc_bank, 10.0, None, &authority)
        .await?;

    marginfi_account.try_set_freeze(true).await?;

    let res = marginfi_account
        .try_bank_deposit_with_authority(user_token_account.key, &usdc_bank, 1.0, None, &authority)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);

    let admin_token_account =
        TokenAccountFixture::new(test_f.context.clone(), &usdc_bank.mint, &test_f.payer()).await;
    usdc_bank
        .mint
        .mint_to(&admin_token_account.key, native!(5, "USDC") as f64)
        .await;
    let admin_res = marginfi_account
        .try_bank_deposit(admin_token_account.key, &usdc_bank, 2.0, None)
        .await;
    assert!(admin_res.is_ok());
    admin_res?;

    marginfi_account.try_set_freeze(false).await?;
    let authority_res = marginfi_account
        .try_bank_deposit_with_authority(user_token_account.key, &usdc_bank, 1.0, None, &authority)
        .await;

    assert!(authority_res.is_ok());
    Ok(())
}

#[tokio::test]
async fn frozen_account_blocks_borrow_allows_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();
    let payer = test_f.payer_keypair();

    let marginfi_account = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;
    let mut usdc_bank = test_f.get_bank(&BankMint::Usdc).clone();
    let mut sol_bank = test_f.get_bank(&BankMint::Sol).clone();

    // Seed SOL liquidity so borrows can succeed.
    let liquidity_provider = test_f.create_marginfi_account().await;
    let provider_sol_account =
        TokenAccountFixture::new(test_f.context.clone(), &sol_bank.mint, &test_f.payer()).await;
    sol_bank.mint.mint_to(&provider_sol_account.key, 50.0).await;
    liquidity_provider
        .try_bank_deposit(provider_sol_account.key, &sol_bank, 50.0, None)
        .await?;

    let user_token_account =
        TokenAccountFixture::new(test_f.context.clone(), &usdc_bank.mint, &authority.pubkey())
            .await;
    usdc_bank
        .mint
        .mint_to(&user_token_account.key, native!(200, "USDC") as f64)
        .await;
    marginfi_account
        .try_bank_deposit_with_authority(
            user_token_account.key,
            &usdc_bank,
            100.0,
            None,
            &authority,
        )
        .await?;

    marginfi_account.try_set_freeze(true).await?;

    let res = marginfi_account
        .try_bank_borrow_with_authority(
            TokenAccountFixture::new(test_f.context.clone(), &sol_bank.mint, &authority.pubkey())
                .await
                .key,
            &sol_bank,
            1.0,
            100,
            &authority,
        )
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);

    let admin_dest_account =
        TokenAccountFixture::new(test_f.context.clone(), &sol_bank.mint, &payer.pubkey()).await;
    let admin_res = marginfi_account
        .try_bank_borrow_with_authority(admin_dest_account.key, &sol_bank, 1.0, 100, &payer)
        .await;
    assert!(admin_res.is_ok());

    Ok(())
}

#[tokio::test]
async fn frozen_account_blocks_withdraw_allows_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();
    let payer = test_f.payer_keypair();

    let marginfi_account = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;
    let mut usdc_bank = test_f.get_bank(&BankMint::Usdc).clone();

    let user_token_account =
        TokenAccountFixture::new(test_f.context.clone(), &usdc_bank.mint, &authority.pubkey())
            .await;
    usdc_bank
        .mint
        .mint_to(&user_token_account.key, native!(200, "USDC") as f64)
        .await;
    marginfi_account
        .try_bank_deposit_with_authority(
            user_token_account.key,
            &usdc_bank,
            100.0,
            None,
            &authority,
        )
        .await?;

    marginfi_account.try_set_freeze(true).await?;

    let res = marginfi_account
        .try_bank_withdraw_with_authority(user_token_account.key, &usdc_bank, 1.0, None, &authority)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);

    let admin_dest_account =
        TokenAccountFixture::new(test_f.context.clone(), &usdc_bank.mint, &payer.pubkey()).await;
    let admin_res = marginfi_account
        .try_bank_withdraw_with_authority(admin_dest_account.key, &usdc_bank, 1.0, None, &payer)
        .await;
    assert!(admin_res.is_ok());

    Ok(())
}

#[tokio::test]
async fn frozen_account_blocks_repay_allows_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();
    let payer = test_f.payer_keypair();

    let marginfi_account = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;
    let mut usdc_bank = test_f.get_bank(&BankMint::Usdc).clone();
    let mut sol_bank = test_f.get_bank(&BankMint::Sol).clone();

    let liquidity_provider = test_f.create_marginfi_account().await;
    let provider_sol_account =
        TokenAccountFixture::new(test_f.context.clone(), &sol_bank.mint, &test_f.payer()).await;
    sol_bank.mint.mint_to(&provider_sol_account.key, 50.0).await;
    liquidity_provider
        .try_bank_deposit(provider_sol_account.key, &sol_bank, 50.0, None)
        .await?;

    let user_usdc_account =
        TokenAccountFixture::new(test_f.context.clone(), &usdc_bank.mint, &authority.pubkey())
            .await;
    usdc_bank
        .mint
        .mint_to(&user_usdc_account.key, native!(300, "USDC") as f64)
        .await;
    marginfi_account
        .try_bank_deposit_with_authority(user_usdc_account.key, &usdc_bank, 150.0, None, &authority)
        .await?;
    let user_sol_account =
        TokenAccountFixture::new(test_f.context.clone(), &sol_bank.mint, &authority.pubkey()).await;
    marginfi_account
        .try_bank_borrow_with_authority(user_sol_account.key, &sol_bank, 10.0, 100, &authority)
        .await?;

    marginfi_account.try_set_freeze(true).await?;

    let res = marginfi_account
        .try_bank_repay_with_authority(user_sol_account.key, &sol_bank, 5.0, None, &authority)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);

    let admin_sol_account =
        TokenAccountFixture::new(test_f.context.clone(), &sol_bank.mint, &payer.pubkey()).await;
    sol_bank.mint.mint_to(&admin_sol_account.key, 20.0).await;
    let admin_res = marginfi_account
        .try_bank_repay_with_authority(admin_sol_account.key, &sol_bank, 5.0, None, &payer)
        .await;
    assert!(admin_res.is_ok());

    Ok(())
}
