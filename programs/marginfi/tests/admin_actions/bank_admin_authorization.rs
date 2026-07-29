use fixtures::prelude::*;
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::types::BankConfigOpt;
use solana_sdk::signer::Signer;

#[tokio::test]
async fn governance_actions_require_bank_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Usdc);
    let new_bank_admin = solana_sdk::signature::Keypair::new();
    let payer_key = test_f.context.borrow().payer.pubkey();

    let group_initial = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_initial.admin, group_initial.bank_admin);
    assert_eq!(group_initial.admin, payer_key);

    test_f
        .marginfi_group
        .try_set_bank_admin(&new_bank_admin)
        .await?;

    let group_after = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_after.bank_admin, new_bank_admin.pubkey());
    assert_ne!(group_after.bank_admin, group_after.admin);

    let config = BankConfigOpt {
        asset_weight_init: Some(fixed_macro::types::I80F48!(0.5).into()),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(config, None).await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn add_bank_requires_bank_admin_authorization() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let mint_f = MintFixture::new(test_f.context.clone(), None, None).await;

    let result = test_f
        .marginfi_group
        .try_lending_pool_add_bank(&mint_f, None, *DEFAULT_USDC_TEST_BANK_CONFIG, None)
        .await;
    assert!(
        result.is_err(),
        "admin should NOT be able to add_bank when bank_admin != admin"
    );
    assert_custom_error!(result.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}

#[tokio::test]
async fn add_bank_with_seed_requires_bank_admin_authorization() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let mint_f = MintFixture::new(test_f.context.clone(), None, None).await;
    let bank_seed = 1234u64;

    let result = test_f
        .marginfi_group
        .try_lending_pool_add_bank_with_seed(
            &mint_f,
            None,
            *DEFAULT_USDC_TEST_BANK_CONFIG,
            bank_seed,
        )
        .await;
    assert!(
        result.is_err(),
        "admin should NOT be able to add_bank_with_seed when bank_admin != admin"
    );
    assert_custom_error!(result.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}

#[tokio::test]
async fn add_bank_both_directions_after_rotation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let mint_f = MintFixture::new(test_f.context.clone(), None, None).await;
    let compact_config = (*DEFAULT_USDC_TEST_BANK_CONFIG).into();

    let result = test_f
        .marginfi_group
        .try_lending_pool_add_bank(&mint_f, None, *DEFAULT_USDC_TEST_BANK_CONFIG, None)
        .await;
    assert!(
        result.is_err(),
        "admin should NOT be able to add_bank when bank_admin != admin"
    );
    assert_custom_error!(result.unwrap_err(), MarginfiError::Unauthorized);

    let result = test_f
        .marginfi_group
        .try_lending_pool_add_bank_with_signer(&bank_admin_kp, &mint_f, compact_config)
        .await;
    assert!(result.is_ok(), "bank_admin should be able to add_bank");

    Ok(())
}

#[tokio::test]
async fn add_bank_with_seed_both_directions_after_rotation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let mint_f = MintFixture::new(test_f.context.clone(), None, None).await;
    let bank_seed = 1234u64;

    let result = test_f
        .marginfi_group
        .try_lending_pool_add_bank_with_seed(
            &mint_f,
            None,
            *DEFAULT_USDC_TEST_BANK_CONFIG,
            bank_seed,
        )
        .await;
    assert!(
        result.is_err(),
        "admin should NOT be able to add_bank_with_seed when bank_admin != admin"
    );
    assert_custom_error!(result.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}
