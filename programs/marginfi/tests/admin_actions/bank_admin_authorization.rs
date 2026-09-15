use fixtures::assert_custom_error;
use fixtures::prelude::*;
use marginfi::instruction::MarginfiGroupConfigureGov;
use marginfi::prelude::MarginfiError;
use marginfi::state::bank::BankImpl;
use marginfi_type_crate::{constants::TOKENLESS_REPAYMENTS_ALLOWED, types::BankConfigOpt};
use solana_program_test::BanksClientError;
use solana_sdk::{signature::Keypair, signer::Signer};

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
    assert_eq!(group_initial.admin, group_initial.governance_admin);
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
    assert_eq!(group_after.governance_admin, new_bank_admin.pubkey());
    assert_ne!(group_after.governance_admin, group_after.admin);

    let config = BankConfigOpt {
        asset_weight_init: Some(fixed_macro::types::I80F48!(0.5).into()),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(config, None).await;
    assert!(result.is_err());

    let tokenless_config = BankConfigOpt {
        tokenless_repayments_allowed: Some(true),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(tokenless_config.clone(), None).await;
    let err = result.unwrap_err().downcast::<BanksClientError>().unwrap();
    assert_custom_error!(err, MarginfiError::Unauthorized);

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_with_signer(&new_bank_admin, &bank, tokenless_config)
        .await?;
    assert!(
        bank.load().await.get_flag(TOKENLESS_REPAYMENTS_ALLOWED),
        "bank_admin should be able to enable tokenless repayments"
    );

    let fast_result = test_f
        .marginfi_group
        .try_group_configure_gov_with_signer(
            &test_f.payer_keypair(),
            MarginfiGroupConfigureGov {
                new_emode_admin: Some(solana_sdk::pubkey::Pubkey::new_unique()),
                new_risk_admin: Some(solana_sdk::pubkey::Pubkey::new_unique()),
                emode_max_init_leverage: None,
                emode_max_maint_leverage: None,
                same_asset_emode_init_leverage: None,
                same_asset_emode_maint_leverage: None,
            },
        )
        .await;
    assert_custom_error!(fast_result.unwrap_err(), MarginfiError::Unauthorized);

    let new_emode_admin = Keypair::new();
    let new_risk_admin = solana_sdk::pubkey::Pubkey::new_unique();
    test_f
        .marginfi_group
        .try_group_configure_gov_with_signer(
            &new_bank_admin,
            MarginfiGroupConfigureGov {
                new_emode_admin: Some(new_emode_admin.pubkey()),
                new_risk_admin: Some(new_risk_admin),
                emode_max_init_leverage: None,
                emode_max_maint_leverage: None,
                same_asset_emode_init_leverage: None,
                same_asset_emode_maint_leverage: None,
            },
        )
        .await?;
    let group_after_config = test_f.marginfi_group.load().await;
    assert_eq!(group_after_config.emode_admin, new_emode_admin.pubkey());
    assert_eq!(group_after_config.risk_admin, new_risk_admin);

    let fast_emode_result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode_with_signer(&bank, 0, &[], &test_f.payer_keypair())
        .await;
    assert_custom_error!(fast_emode_result.unwrap_err(), MarginfiError::Unauthorized);

    let emode_result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode_with_signer(&bank, 0, &[], &new_emode_admin)
        .await;
    assert_custom_error!(emode_result.unwrap_err(), MarginfiError::Unauthorized);

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode_with_signer(&bank, 0, &[], &new_bank_admin)
        .await?;

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
