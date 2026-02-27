use anchor_lang::prelude::Pubkey;
use fixed::types::I80F48;
use fixtures::assert_custom_error;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::prelude::*;
use marginfi::prelude::MarginfiError;
use marginfi::state::marginfi_account::MarginfiAccountImpl;
use marginfi_type_crate::constants::EMISSIONS_FLAG_LENDING_ACTIVE;
use marginfi_type_crate::types::ACCOUNT_FROZEN;
use solana_program_test::tokio;
use solana_sdk::signature::Keypair;

// ─── marginfi_account_update_emissions_destination_account ──────────

#[tokio::test]
async fn set_emissions_destination_frozen_account_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();

    let account_f = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;

    // Freeze the account
    account_f.try_set_freeze(true).await?;
    assert!(account_f.load().await.get_flag(ACCOUNT_FROZEN));

    // Attempt to set emissions destination on a frozen account
    let destination = Pubkey::new_unique();
    let res = account_f
        .try_set_emissions_destination_with_authority(destination, &authority)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);

    // Unfreeze and verify it works again
    account_f.try_set_freeze(false).await?;
    account_f
        .try_set_emissions_destination_with_authority(destination, &authority)
        .await?;

    let account = account_f.load().await;
    assert_eq!(account.emissions_destination_account, destination);

    Ok(())
}

#[tokio::test]
async fn set_emissions_destination_wrong_authority_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();

    let account_f = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;

    // Try with a random keypair that is not the authority
    let wrong_authority = Keypair::new();
    let destination = Pubkey::new_unique();
    let res = account_f
        .try_set_emissions_destination_with_authority(destination, &wrong_authority)
        .await;

    assert!(res.is_err());

    Ok(())
}

// ─── lending_account_clear_emissions ────────────────────────────────

#[tokio::test]
async fn clear_emissions_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    // Create an account and deposit to get an active balance
    let account_f = test_f.create_marginfi_account().await;
    let token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    account_f
        .try_bank_deposit(token_account.key, usdc_bank_f, 100, None)
        .await?;

    // Directly set emissions_outstanding on the balance via account mutation
    {
        let mut mfi_account = account_f.load().await;
        let balance = mfi_account
            .lending_account
            .balances
            .iter_mut()
            .find(|b| b.is_active() && b.bank_pk == usdc_bank_f.key)
            .unwrap();
        balance.emissions_outstanding = I80F48::from_num(500_000).into();
        account_f.set_account(&mfi_account).await.unwrap();
    }

    // Verify emissions_outstanding is set
    let mfi_account = account_f.load().await;
    let balance = mfi_account
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == usdc_bank_f.key)
        .unwrap();
    assert_ne!(I80F48::from(balance.emissions_outstanding), I80F48::ZERO);

    // Bank already has emissions_rate == 0 and no emission flags by default,
    // so clear_emissions should succeed.
    account_f.try_clear_emissions(usdc_bank_f).await?;

    // Verify emissions_outstanding is now zero
    let mfi_account = account_f.load().await;
    let balance = mfi_account
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == usdc_bank_f.key)
        .unwrap();
    assert_eq!(I80F48::from(balance.emissions_outstanding), I80F48::ZERO);

    Ok(())
}

#[tokio::test]
async fn clear_emissions_fails_when_emissions_active() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    // Create an account and deposit
    let account_f = test_f.create_marginfi_account().await;
    let token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    account_f
        .try_bank_deposit(token_account.key, usdc_bank_f, 100, None)
        .await?;

    // Set emissions_outstanding on the balance
    {
        let mut mfi_account = account_f.load().await;
        let balance = mfi_account
            .lending_account
            .balances
            .iter_mut()
            .find(|b| b.is_active() && b.bank_pk == usdc_bank_f.key)
            .unwrap();
        balance.emissions_outstanding = I80F48::from_num(500_000).into();
        account_f.set_account(&mfi_account).await.unwrap();
    }

    // Set emissions as active on the bank (rate > 0)
    let emissions_mint = MintFixture::new(test_f.context.clone(), None, None).await;
    usdc_bank_f
        .set_emissions(
            emissions_mint.key,
            1_000_000,
            I80F48::from_num(1_000_000),
            EMISSIONS_FLAG_LENDING_ACTIVE,
        )
        .await;

    // clear_emissions should fail because emissions are still active
    let res = account_f.try_clear_emissions(usdc_bank_f).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidConfig);

    Ok(())
}

#[tokio::test]
async fn clear_emissions_no_balance_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    // Create an account but do NOT deposit (no active balance for this bank)
    let account_f = test_f.create_marginfi_account().await;

    let res = account_f.try_clear_emissions(usdc_bank_f).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankAccountNotFound);

    Ok(())
}
