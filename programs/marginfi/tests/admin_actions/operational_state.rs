use fixtures::prelude::*;
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::types::BankConfigOpt;

#[tokio::test]
async fn reduce_only_transition_governance_admin_authorization() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Usdc);
    let governance_admin = solana_sdk::signature::Keypair::new();

    test_f
        .marginfi_group
        .try_set_governance_admin(&governance_admin)
        .await?;

    let config_reduce = BankConfigOpt {
        operational_state: Some(marginfi_type_crate::types::BankOperationalState::ReduceOnly),
        ..BankConfigOpt::default()
    };
    let result = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_with_signer(
            &governance_admin,
            &bank,
            config_reduce.clone(),
        )
        .await;
    assert!(
        result.is_err(),
        "governance_admin should NOT be able to enter ReduceOnly; only fast admin can"
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
        .try_lending_pool_configure_bank_with_signer(&governance_admin, &bank, config_operational)
        .await;
    assert!(
        result.is_ok(),
        "governance_admin should be able to exit ReduceOnly back to Operational"
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
