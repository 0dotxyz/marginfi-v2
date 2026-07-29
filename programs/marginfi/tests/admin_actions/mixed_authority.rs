use fixed_macro::types::I80F48;
use fixtures::assert_custom_error;
use fixtures::prelude::*;
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::types::BankConfigOpt;
use solana_program_test::BanksClientError;

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
