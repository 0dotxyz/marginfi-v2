use fixed_macro::types::I80F48;
use fixtures::prelude::*;
use marginfi_type_crate::types::BankConfigOpt;
use solana_sdk::signature::Signer;

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
