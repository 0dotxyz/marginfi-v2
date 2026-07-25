use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::types::BankConfigOpt;
use pretty_assertions::assert_eq;
use solana_program_test::{tokio, BanksClientError};
use solana_sdk::signer::Signer;

#[tokio::test]
async fn bank_admin_rotation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let group_before = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_before.admin, group_before.bank_admin);

    let new_bank_admin_1 = solana_sdk::signature::Keypair::new();
    let new_bank_admin_2 = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&new_bank_admin_1)
        .await?;

    let group_after_rotate_1 = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_after_rotate_1.bank_admin, new_bank_admin_1.pubkey());

    test_f
        .marginfi_group
        .try_set_bank_admin_with_signer(&new_bank_admin_1, new_bank_admin_2.pubkey())
        .await?;

    let group_after_rotate_2 = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_after_rotate_2.bank_admin, new_bank_admin_2.pubkey());

    let new_bank_admin_3 = solana_sdk::signature::Keypair::new();
    let result = test_f
        .marginfi_group
        .try_set_bank_admin_with_signer(&new_bank_admin_1, new_bank_admin_3.pubkey())
        .await;
    assert!(result.is_err());

    Ok(())
}

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
        asset_weight_init: Some(I80F48!(0.5).into()),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(config, None).await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn configure_bank_authority_split() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Usdc);
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let config = BankConfigOpt {
        deposit_limit: Some(2_000_000u64),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(config, None).await;
    assert!(result.is_ok());

    let config = BankConfigOpt {
        deposit_limit: Some(3_000_000u64),
        ..BankConfigOpt::default()
    };
    let result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_with_signer(&bank_admin_kp, bank, config)
        .await;
    assert!(result.is_err());

    let config = BankConfigOpt {
        asset_weight_init: Some(I80F48!(0.8).into()),
        ..BankConfigOpt::default()
    };
    let result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_with_signer(&bank_admin_kp, bank, config)
        .await;
    assert!(result.is_ok());

    let config_reduce = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::ReduceOnly),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(config_reduce, None).await;
    assert!(result.is_ok());

    let bank_state = bank.load().await;
    assert_eq!(
        bank_state.config.operational_state,
        marginfi_type_crate::types::BankOperationalState::ReduceOnly
    );

    let config_operational = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::Operational),
        ..BankConfigOpt::default()
    };
    let result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_with_signer(
            &bank_admin_kp,
            bank,
            config_operational.clone(),
        )
        .await;
    assert!(result.is_ok());

    let bank_state = bank.load().await;
    assert_eq!(
        bank_state.config.operational_state,
        marginfi_type_crate::types::BankOperationalState::Operational
    );

    let config = BankConfigOpt {
        asset_weight_maint: Some(I80F48!(0.75).into()),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(config, None).await;
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn mixed_config_rejected() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Usdc);

    let config = BankConfigOpt {
        deposit_limit: Some(1_000_000u64),
        asset_weight_init: Some(I80F48!(0.8).into()),
        ..BankConfigOpt::default()
    };

    let result = bank.update_config(config, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().downcast::<BanksClientError>().unwrap();
    assert_custom_error!(err, MarginfiError::MixedBankConfigAuthority);

    let config = BankConfigOpt {
        deposit_limit: Some(1_000_000u64),
        oracle_max_confidence: Some(100u32),
        ..BankConfigOpt::default()
    };

    let result = bank.update_config(config, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().downcast::<BanksClientError>().unwrap();
    assert_custom_error!(err, MarginfiError::MixedBankConfigAuthority);

    Ok(())
}

#[tokio::test]
async fn legacy_fallback_behavior() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let payer_key = test_f.context.borrow().payer.pubkey();

    let group = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group.bank_admin, payer_key);

    let bank = test_f.get_bank(&BankMint::Usdc);

    let config = BankConfigOpt {
        deposit_limit: Some(5_000_000u64),
        ..BankConfigOpt::default()
    };

    let result = bank.update_config(config, None).await;
    assert!(result.is_ok());

    let config = BankConfigOpt {
        asset_weight_init: Some(I80F48!(0.8).into()),
        ..BankConfigOpt::default()
    };

    let result = bank.update_config(config, None).await;
    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn set_bank_admin_authorization_after_divergence() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let original_admin = test_f.context.borrow().payer.pubkey();
    let new_bank_admin = solana_sdk::signature::Keypair::new();
    let another_admin = solana_sdk::signature::Keypair::new();

    let group_before = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_before.admin, original_admin);
    assert_eq!(group_before.bank_admin, original_admin);

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

    let result = test_f
        .marginfi_group
        .try_set_bank_admin(&another_admin)
        .await;

    assert!(
        result.is_err(),
        "Original admin should NOT be able to set_bank_admin after bank_admin diverged"
    );

    Ok(())
}

#[tokio::test]
async fn set_fixed_oracle_price_bank_admin_authorization() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Fixed,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Fixed);
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    let original_admin = test_f.context.borrow().payer.insecure_clone();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let new_price = I80F48!(5.5).into();

    let result = test_f
        .marginfi_group
        .try_lending_pool_set_fixed_oracle_price_with_signer(&bank_admin_kp, &bank, new_price)
        .await;
    assert!(
        result.is_ok(),
        "bank_admin should be able to set_fixed_oracle_price"
    );

    let another_price = I80F48!(7.2).into();

    let result = test_f
        .marginfi_group
        .try_lending_pool_set_fixed_oracle_price_with_signer(&original_admin, &bank, another_price)
        .await;
    assert!(
        result.is_err(),
        "admin should NOT be able to set_fixed_oracle_price when bank_admin != admin"
    );
    let err = result.unwrap_err();
    assert_custom_error!(err, MarginfiError::Unauthorized);

    Ok(())
}

#[tokio::test]
async fn reduce_only_transition_bank_admin_authorization() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Usdc);
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let config_reduce = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::ReduceOnly),
        ..BankConfigOpt::default()
    };
    let result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_with_signer(&bank_admin_kp, &bank, config_reduce.clone())
        .await;
    assert!(
        result.is_err(),
        "bank_admin should NOT be able to enter ReduceOnly; only admin can"
    );
    let err = result.unwrap_err();
    assert_custom_error!(err, MarginfiError::Unauthorized);

    let result = bank.update_config(config_reduce, None).await;
    assert!(result.is_ok(), "admin should be able to enter ReduceOnly");

    let bank_state = bank.load().await;
    assert_eq!(
        bank_state.config.operational_state,
        marginfi_type_crate::types::BankOperationalState::ReduceOnly
    );

    let config_operational = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::Operational),
        ..BankConfigOpt::default()
    };
    let result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_with_signer(&bank_admin_kp, &bank, config_operational)
        .await;
    assert!(
        result.is_ok(),
        "bank_admin should be able to exit ReduceOnly back to Operational"
    );

    let bank_state = bank.load().await;
    assert_eq!(
        bank_state.config.operational_state,
        marginfi_type_crate::types::BankOperationalState::Operational
    );

    let config_reduce = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::ReduceOnly),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(config_reduce, None).await;
    assert!(
        result.is_ok(),
        "admin SHOULD be able to enter ReduceOnly (not exiting to Operational)"
    );

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
async fn set_bank_admin_rejects_default_pubkey() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let new_bank_admin = solana_sdk::signature::Keypair::new();

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

    let result = test_f
        .marginfi_group
        .try_set_bank_admin_with_signer(&new_bank_admin, solana_sdk::pubkey::Pubkey::default())
        .await;

    assert!(
        result.is_err(),
        "set_bank_admin should reject Pubkey::default()"
    );
    let err = result.unwrap_err().downcast::<BanksClientError>().unwrap();
    assert_custom_error!(err, MarginfiError::InvalidBankAdmin);

    let group_final = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiGroup>(
            &test_f.marginfi_group.key,
        )
        .await;
    assert_eq!(group_final.bank_admin, new_bank_admin.pubkey());

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

#[tokio::test]
async fn mixed_operational_and_admin_rejected() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Usdc);
    let bank_admin_kp = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin_kp)
        .await?;

    let config = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::ReduceOnly),
        ..BankConfigOpt::default()
    };
    bank.update_config(config, None).await?;

    let config_bypass = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::Operational),
        deposit_limit: Some(1_000_000u64),
        ..BankConfigOpt::default()
    };
    let result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_with_signer(&bank_admin_kp, &bank, config_bypass)
        .await;
    assert!(
        result.is_err(),
        "bank_admin should NOT be able to mix operational_state + deposit_limit"
    );
    assert_custom_error!(result.unwrap_err(), MarginfiError::MixedBankConfigAuthority);

    let config_bypass2 = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::ReduceOnly),
        asset_weight_init: Some(I80F48!(0.8).into()),
        ..BankConfigOpt::default()
    };
    let result = bank.update_config(config_bypass2, None).await;
    assert!(
        result.is_err(),
        "admin should NOT be able to mix operational_state + asset_weight"
    );
    let err = result.unwrap_err().downcast::<BanksClientError>().unwrap();
    assert_custom_error!(err, MarginfiError::MixedBankConfigAuthority);

    Ok(())
}
