use anchor_lang::prelude::Pubkey;
use fixtures::assert_custom_error;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::prelude::*;
use marginfi::prelude::MarginfiError;
use marginfi::state::marginfi_account::MarginfiAccountImpl;
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
