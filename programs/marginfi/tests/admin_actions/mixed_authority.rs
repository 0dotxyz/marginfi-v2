use anchor_lang::{InstructionData, ToAccountMetas};
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::types::{
    BankConfigFast, BankConfigGov, BankConfigOpt, BankOperationalState,
};
use solana_sdk::instruction::Instruction;
use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};

#[tokio::test]
async fn aggregate_test_config_dispatches_fast_and_governance_instructions() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;

    let bank = test_f.get_bank(&BankMint::Usdc);
    bank.update_config(
        BankConfigOpt {
            deposit_limit: Some(1_000_000),
            asset_weight_init: Some(I80F48!(0.8).into()),
            oracle_max_confidence: Some(100),
            ..BankConfigOpt::default()
        },
        None,
    )
    .await?;

    let bank_state = bank.load().await;
    assert_eq!(bank_state.config.deposit_limit, 1_000_000);
    assert_eq!(bank_state.config.asset_weight_init, I80F48!(0.8).into());
    assert_eq!(bank_state.config.oracle_max_confidence, 100);
    Ok(())
}

#[tokio::test]
async fn group_configuration_has_explicit_fast_and_governance_entry_points() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let bank_admin = Keypair::new();
    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin)
        .await?;

    let metadata_admin = solana_sdk::pubkey::Pubkey::new_unique();
    test_f
        .marginfi_group
        .try_group_configure_with_signer(
            &test_f.payer_keypair(),
            marginfi::instruction::MarginfiGroupConfigure {
                new_admin: None,
                new_curve_admin: None,
                new_limit_admin: None,
                new_flow_admin: None,
                new_emissions_admin: None,
                new_metadata_admin: Some(metadata_admin),
            },
        )
        .await?;

    let risk_admin = solana_sdk::pubkey::Pubkey::new_unique();
    test_f
        .marginfi_group
        .try_group_configure_gov_with_signer(
            &bank_admin,
            marginfi::instruction::MarginfiGroupConfigureGov {
                new_emode_admin: None,
                new_risk_admin: Some(risk_admin),
                emode_max_init_leverage: None,
                emode_max_maint_leverage: None,
                same_asset_emode_init_leverage: None,
                same_asset_emode_maint_leverage: None,
            },
        )
        .await?;

    let group = test_f.marginfi_group.load().await;
    assert_eq!(group.metadata_admin, metadata_admin);
    assert_eq!(group.risk_admin, risk_admin);
    Ok(())
}

#[tokio::test]
async fn bank_configuration_entry_points_reject_wrong_operational_state_class() -> anyhow::Result<()>
{
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    }))
    .await;
    let bank = test_f.get_bank(&BankMint::Usdc);
    let bank_admin = Keypair::new();
    test_f
        .marginfi_group
        .try_set_bank_admin(&bank_admin)
        .await?;

    let fast_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::LendingPoolConfigureBank {
            group: test_f.marginfi_group.key,
            admin: test_f.payer_keypair().pubkey(),
            bank: bank.key,
            instruction_sysvar: solana_sdk::sysvar::instructions::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBank {
            bank_config_opt: BankConfigFast {
                operational_state: Some(BankOperationalState::Operational),
                ..BankConfigFast::default()
            },
        }
        .data(),
    };
    let ctx = test_f.context.borrow();
    let tx = Transaction::new_signed_with_payer(
        &[fast_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await?,
    );
    let err = ctx.banks_client.process_transaction(tx).await.unwrap_err();
    assert_custom_error!(err, MarginfiError::InvalidFastBankOperationalState);
    drop(ctx);

    let gov_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::LendingPoolConfigureBankGov {
            group: test_f.marginfi_group.key,
            governance_admin: bank_admin.pubkey(),
            bank: bank.key,
            instruction_sysvar: solana_sdk::sysvar::instructions::ID,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBankGov {
            bank_config_opt: BankConfigGov {
                operational_state: Some(BankOperationalState::Paused),
                ..BankConfigGov::default()
            },
        }
        .data(),
    };
    let ctx = test_f.context.borrow();
    let tx = Transaction::new_signed_with_payer(
        &[gov_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer, &bank_admin],
        ctx.banks_client.get_latest_blockhash().await?,
    );
    let err = ctx.banks_client.process_transaction(tx).await.unwrap_err();
    assert_custom_error!(err, MarginfiError::InvalidGovernanceBankOperationalState);

    Ok(())
}
