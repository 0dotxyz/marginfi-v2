use anchor_lang::{InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::{
    constants::INIT_BANK_ORIGINATION_FEE_DEFAULT,
    prelude::MarginfiError,
    state::bank::{BankImpl, BankVaultType},
};
use marginfi_type_crate::{
    constants::{
        CLOSE_ENABLED_FLAG, FREEZE_SETTINGS, PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG,
        TOKENLESS_REPAYMENTS_ALLOWED,
    },
    types::{
        make_points, u32_to_basis, Bank, BankCache, BankConfig, BankConfigOpt, EmodeEntry,
        InterestRateConfigOpt, MarginfiGroup, OracleSetup, RatePoint, EMODE_ON,
        INTEREST_CURVE_SEVEN_POINT,
    },
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{
    clock::Clock, instruction::Instruction, pubkey::Pubkey, transaction::Transaction,
};
use solana_sdk::{signature::Keypair, signer::Signer};
use test_case::test_case;

#[tokio::test]
async fn add_bank_success() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(None).await;

    let fee_wallet = test_f.marginfi_group.fee_wallet;

    let mints = vec![
        (
            MintFixture::new(test_f.context.clone(), None, None).await,
            *DEFAULT_USDC_TEST_BANK_CONFIG,
        ),
        (
            MintFixture::new(test_f.context.clone(), None, None).await,
            *DEFAULT_SOL_TEST_BANK_CONFIG,
        ),
        (
            MintFixture::new_from_file(&test_f.context.clone(), "src/fixtures/pyUSD.json"),
            *DEFAULT_PYUSD_TEST_BANK_CONFIG,
        ),
    ];

    let marginfi_group: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    let mut last_update = marginfi_group.fee_state_cache.last_update;
    assert_eq!(last_update, 0);

    for (mint_f, bank_config) in mints {
        // This is just to test that the group's last_update field is properly updated upon bank creation
        {
            let ctx = test_f.context.borrow_mut();
            let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
            // Advance clock by 1 sec
            clock.unix_timestamp += 1;
            ctx.set_sysvar(&clock);
        }

        // Load the fee state before the start of the test
        let fee_balance_before: u64;
        {
            let ctx = test_f.context.borrow_mut();
            fee_balance_before = ctx
                .banks_client
                .get_account(fee_wallet)
                .await
                .unwrap()
                .unwrap()
                .lamports;
        }

        let res = test_f
            .marginfi_group
            .try_lending_pool_add_bank(&mint_f, None, bank_config, None)
            .await;

        let marginfi_group: MarginfiGroup = test_f
            .load_and_deserialize(&test_f.marginfi_group.key)
            .await;
        assert_eq!(marginfi_group.fee_state_cache.last_update, last_update + 1);
        last_update = marginfi_group.fee_state_cache.last_update;

        // Check bank
        let bank_f = res.unwrap();
        let Bank {
            mint,
            mint_decimals,
            group,
            asset_share_value,
            liability_share_value,
            liquidity_vault,
            liquidity_vault_bump,
            liquidity_vault_authority_bump,
            insurance_vault,
            insurance_vault_bump,
            insurance_vault_authority_bump,
            collected_insurance_fees_outstanding,
            fee_vault,
            fee_vault_bump,
            fee_vault_authority_bump,
            collected_group_fees_outstanding,
            total_liability_shares,
            total_asset_shares,
            last_update,
            config,
            flags,
            emissions_rate,
            emissions_remaining,
            emissions_mint,
            collected_program_fees_outstanding,
            fees_destination_account,
            cache,
            lending_position_count,
            borrowing_position_count,
            _padding_0,
            integration_acc_1,
            integration_acc_2,
            integration_acc_3,
            _padding_1,
            .. // ignore internal padding
        } = bank_f.load().await;
        #[rustfmt::skip]
        let _ = {
            assert_eq!(mint, bank_f.mint.key);
            assert_eq!(mint_decimals, bank_f.mint.load_state().await.base.decimals);
            assert_eq!(group, test_f.marginfi_group.key);
            assert_eq!(asset_share_value, I80F48!(1.0).into());
            assert_eq!(liability_share_value, I80F48!(1.0).into());
            assert_eq!(liquidity_vault, bank_f.get_vault(BankVaultType::Liquidity).0);
            assert_eq!(liquidity_vault_bump, bank_f.get_vault(BankVaultType::Liquidity).1);
            assert_eq!(liquidity_vault_authority_bump, bank_f.get_vault_authority(BankVaultType::Liquidity).1);
            assert_eq!(insurance_vault, bank_f.get_vault(BankVaultType::Insurance).0);
            assert_eq!(insurance_vault_bump, bank_f.get_vault(BankVaultType::Insurance).1);
            assert_eq!(insurance_vault_authority_bump, bank_f.get_vault_authority(BankVaultType::Insurance).1);
            assert_eq!(fee_vault, bank_f.get_vault(BankVaultType::Fee).0);
            assert_eq!(fee_vault_bump, bank_f.get_vault(BankVaultType::Fee).1);
            assert_eq!(fee_vault_authority_bump, bank_f.get_vault_authority(BankVaultType::Fee).1);
            assert_eq!(collected_insurance_fees_outstanding, I80F48!(0.0).into());
            assert_eq!(collected_group_fees_outstanding, I80F48!(0.0).into());
            assert_eq!(total_liability_shares, I80F48!(0.0).into());
            assert_eq!(total_asset_shares, I80F48!(0.0).into());
            assert_eq!(config, bank_config);
            assert_eq!(flags, CLOSE_ENABLED_FLAG);
            assert_eq!(emissions_rate, 0);
            assert_eq!(emissions_mint, Pubkey::new_from_array([0; 32]));
            assert_eq!(emissions_remaining, I80F48!(0.0).into());
            assert_eq!(collected_program_fees_outstanding, I80F48!(0.0).into());
            assert_eq!(fees_destination_account, Pubkey::default());
            assert_eq!(cache, BankCache::default());

            assert_eq!(lending_position_count, 0);
            assert_eq!(borrowing_position_count, 0);
            assert_eq!(_padding_0, <[u8; 16] as Default>::default());
            assert_eq!(integration_acc_1, Pubkey::default());
            assert_eq!(integration_acc_2, Pubkey::default());
            assert_eq!(integration_acc_3, Pubkey::default());
            assert_eq!(_padding_1, <[[u64; 2]; 8] as Default>::default());

            // this is the only loosely checked field
            assert!(last_update >= 0 && last_update <= 5);
        };

        // Load the fee state after the test
        let fee_balance_after: u64;
        {
            let ctx = test_f.context.borrow_mut();
            fee_balance_after = ctx
                .banks_client
                .get_account(fee_wallet)
                .await
                .unwrap()
                .unwrap()
                .lamports;
        }
        let expected_fee_delta = INIT_BANK_ORIGINATION_FEE_DEFAULT as u64;
        let actual_fee_delta = fee_balance_after - fee_balance_before;
        assert_eq!(expected_fee_delta, actual_fee_delta);
    }

    Ok(())
}

#[tokio::test]
async fn add_bank_with_seed_success() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(None).await;

    let fee_wallet = test_f.marginfi_group.fee_wallet;

    let mints = vec![
        (
            MintFixture::new(test_f.context.clone(), None, None).await,
            *DEFAULT_USDC_TEST_BANK_CONFIG,
        ),
        (
            MintFixture::new(test_f.context.clone(), None, None).await,
            *DEFAULT_SOL_TEST_BANK_CONFIG,
        ),
        (
            MintFixture::new_from_file(&test_f.context.clone(), "src/fixtures/pyUSD.json"),
            *DEFAULT_PYUSD_TEST_BANK_CONFIG,
        ),
    ];

    for (mint_f, bank_config) in mints {
        let fee_balance_before: u64;
        {
            let ctx = test_f.context.borrow_mut();
            fee_balance_before = ctx
                .banks_client
                .get_account(fee_wallet)
                .await
                .unwrap()
                .unwrap()
                .lamports;
        }

        let bank_seed = 1200_u64;

        let res = test_f
            .marginfi_group
            .try_lending_pool_add_bank_with_seed(&mint_f, None, bank_config, bank_seed)
            .await;
        assert!(res.is_ok());

        // Check bank
        let bank_f = res.unwrap();
        let Bank {
            mint,
            mint_decimals,
            group,
            asset_share_value,
            liability_share_value,
            liquidity_vault,
            liquidity_vault_bump,
            liquidity_vault_authority_bump,
            insurance_vault,
            insurance_vault_bump,
            insurance_vault_authority_bump,
            collected_insurance_fees_outstanding,
            fee_vault,
            fee_vault_bump,
            fee_vault_authority_bump,
            collected_group_fees_outstanding,
            total_liability_shares,
            total_asset_shares,
            last_update,
            config,
            flags,
            emissions_rate,
            emissions_remaining,
            emissions_mint,
            collected_program_fees_outstanding,
            fees_destination_account,
            cache,
            lending_position_count,
            borrowing_position_count,
            _padding_0,
            integration_acc_1,
            integration_acc_2,
            integration_acc_3,
            _padding_1,
            .. // ignore internal padding
        } = bank_f.load().await;
        #[rustfmt::skip]
        let _ = {
            assert_eq!(mint, bank_f.mint.key);
            assert_eq!(mint_decimals, bank_f.mint.load_state().await.base.decimals);
            assert_eq!(group, test_f.marginfi_group.key);
            assert_eq!(asset_share_value, I80F48!(1.0).into());
            assert_eq!(liability_share_value, I80F48!(1.0).into());
            assert_eq!(liquidity_vault, bank_f.get_vault(BankVaultType::Liquidity).0);
            assert_eq!(liquidity_vault_bump, bank_f.get_vault(BankVaultType::Liquidity).1);
            assert_eq!(liquidity_vault_authority_bump, bank_f.get_vault_authority(BankVaultType::Liquidity).1);
            assert_eq!(insurance_vault, bank_f.get_vault(BankVaultType::Insurance).0);
            assert_eq!(insurance_vault_bump, bank_f.get_vault(BankVaultType::Insurance).1);
            assert_eq!(insurance_vault_authority_bump, bank_f.get_vault_authority(BankVaultType::Insurance).1);
            assert_eq!(fee_vault, bank_f.get_vault(BankVaultType::Fee).0);
            assert_eq!(fee_vault_bump, bank_f.get_vault(BankVaultType::Fee).1);
            assert_eq!(fee_vault_authority_bump, bank_f.get_vault_authority(BankVaultType::Fee).1);
            assert_eq!(collected_insurance_fees_outstanding, I80F48!(0.0).into());
            assert_eq!(collected_group_fees_outstanding, I80F48!(0.0).into());
            assert_eq!(total_liability_shares, I80F48!(0.0).into());
            assert_eq!(total_asset_shares, I80F48!(0.0).into());
            assert_eq!(config, bank_config);
            assert_eq!(flags, CLOSE_ENABLED_FLAG);
            assert_eq!(emissions_rate, 0);
            assert_eq!(emissions_mint, Pubkey::new_from_array([0; 32]));
            assert_eq!(emissions_remaining, I80F48!(0.0).into());
            assert_eq!(collected_program_fees_outstanding, I80F48!(0.0).into());
            assert_eq!(fees_destination_account, Pubkey::default());
            assert_eq!(cache, BankCache::default());

            assert_eq!(lending_position_count, 0);
            assert_eq!(borrowing_position_count, 0);
            assert_eq!(_padding_0, <[u8; 16] as Default>::default());
            assert_eq!(integration_acc_1, Pubkey::default());
            assert_eq!(integration_acc_2, Pubkey::default());
            assert_eq!(integration_acc_3, Pubkey::default());
            assert_eq!(_padding_1, <[[u64; 2]; 8] as Default>::default());

            // this is the only loosely checked field
            assert!(last_update >= 0 && last_update <= 5);
        };

        let fee_balance_after: u64;
        {
            let ctx = test_f.context.borrow_mut();
            fee_balance_after = ctx
                .banks_client
                .get_account(fee_wallet)
                .await
                .unwrap()
                .unwrap()
                .lamports;
        }
        let expected_fee_delta = INIT_BANK_ORIGINATION_FEE_DEFAULT as u64;
        let actual_fee_delta = fee_balance_after - fee_balance_before;
        assert_eq!(expected_fee_delta, actual_fee_delta);
    }

    Ok(())
}

#[tokio::test]
async fn marginfi_group_add_bank_failure_inexistent_pyth_feed() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(None).await;

    let bank_asset_mint_fixture = MintFixture::new(test_f.context.clone(), None, None).await;

    let res = test_f
        .marginfi_group
        .try_lending_pool_add_bank(
            &bank_asset_mint_fixture,
            None,
            BankConfig {
                oracle_setup: OracleSetup::PythPushOracle,
                oracle_keys: create_oracle_key_array(INEXISTENT_PYTH_USDC_FEED),
                ..*DEFAULT_USDC_TEST_BANK_CONFIG
            },
            None,
        )
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::PythPushWrongAccountOwner);

    Ok(())
}

#[tokio::test]
async fn configure_bank_to_fixed_oracle() -> anyhow::Result<()> {
    let test_settings = TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            ..Default::default()
        }],
        ..Default::default()
    };
    let test_f = TestFixture::new(Some(test_settings)).await;

    let bank_f = test_f.get_bank(&BankMint::Usdc);
    let bank_before = bank_f.load().await;
    assert_ne!(bank_before.config.oracle_setup, OracleSetup::Fixed);

    let price_value = I80F48!(3.5);
    let price_wrapped = price_value.into();

    {
        let ctx = test_f.context.borrow_mut();
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolSetFixedOraclePrice {
                group: test_f.marginfi_group.key,
                admin: ctx.payer.pubkey(),
                bank: bank_f.key,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingPoolSetFixedOraclePrice {
                price: price_wrapped,
            }
            .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await?;
    }

    let bank_after = bank_f.load().await;
    assert_eq!(bank_after.config.oracle_setup, OracleSetup::Fixed);
    assert_eq!(I80F48::from(bank_after.config.fixed_price), price_value);
    assert_eq!(bank_after.config.oracle_keys[0], Pubkey::default());

    Ok(())
}

#[tokio::test]
async fn update_fixed_bank_price() -> anyhow::Result<()> {
    let test_settings = TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Fixed,
            ..Default::default()
        }],
        ..Default::default()
    };
    let test_f = TestFixture::new(Some(test_settings)).await;

    let bank_f = test_f.get_bank(&BankMint::Fixed);
    let bank_before = bank_f.load().await;
    assert_eq!(bank_before.config.oracle_setup, OracleSetup::Fixed);
    assert_eq!(I80F48::from(bank_before.config.fixed_price), I80F48!(2.0));

    let new_price_value = I80F48!(4.2);
    let new_price_wrapped = new_price_value.into();

    {
        let ctx = test_f.context.borrow();
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::LendingPoolSetFixedOraclePrice {
                group: test_f.marginfi_group.key,
                admin: ctx.payer.pubkey(),
                bank: bank_f.key,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::LendingPoolSetFixedOraclePrice {
                price: new_price_wrapped,
            }
            .data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client.process_transaction(tx).await?;
    }

    let bank_after = bank_f.load().await;
    assert_eq!(bank_after.config.oracle_setup, OracleSetup::Fixed);
    assert_eq!(I80F48::from(bank_after.config.fixed_price), new_price_value);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::T22WithFee)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_success(bank_mint: BankMint) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let bank = test_f.get_bank(&bank_mint);
    let old_bank = bank.load().await;
    let exp_points = make_points(&vec![
        RatePoint::new(1234, 56789),
        RatePoint::new(2345, 67890),
    ]);

    let config_bank_opt = BankConfigOpt {
        interest_rate_config: Some(InterestRateConfigOpt {
            insurance_fee_fixed_apr: Some(I80F48::from_num(0.13).into()),
            insurance_ir_fee: Some(I80F48::from_num(0.11).into()),
            protocol_fixed_fee_apr: Some(I80F48::from_num(0.51).into()),
            protocol_ir_fee: Some(I80F48::from_num(0.011).into()),
            protocol_origination_fee: Some(I80F48::ZERO.into()),
            zero_util_rate: Some(123),
            hundred_util_rate: Some(1234567),
            points: Some(exp_points),
        }),
        ..BankConfigOpt::default()
    };
    let res = bank.update_config(config_bank_opt.clone(), None).await;
    assert!(res.is_ok());

    // Load bank and check each property in config matches
    // Ensure bank didn't change any other fields. Only need to check the opt fields

    let bank: Bank = test_f.load_and_deserialize(&bank.key).await;
    let BankConfigOpt {
        interest_rate_config,
        asset_weight_init,
        asset_weight_maint,
        liability_weight_init,
        liability_weight_maint,
        deposit_limit,
        borrow_limit,
        operational_state,
        risk_tier,
        asset_tag,
        total_asset_value_init_limit,
        oracle_max_age,
        oracle_max_confidence,
        permissionless_bad_debt_settlement,
        freeze_settings,
        tokenless_repayments_allowed,
    } = &config_bank_opt;
    // Compare bank field to opt field if Some, otherwise compare to old bank field
    macro_rules! check_bank_field {
        // Note: some nested fields (e.g. optimal_utilization_rate) don't exist on the config struct
        ($field:ident, $subfield:ident) => {
            assert_eq!(
                bank.config.$field.$subfield,
                $field
                    .as_ref()
                    .and_then(|opt| opt.$subfield.clone())
                    .unwrap_or(old_bank.config.$field.$subfield)
            );
        };

        // Top-level fields are always expected with the same name
        ($field:ident) => {
            assert_eq!(bank.config.$field, $field.unwrap_or(old_bank.config.$field));
        };
    }

    let _ = {
        check_bank_field!(interest_rate_config, insurance_fee_fixed_apr);
        check_bank_field!(interest_rate_config, insurance_ir_fee);
        check_bank_field!(interest_rate_config, protocol_fixed_fee_apr);
        check_bank_field!(interest_rate_config, protocol_ir_fee);
        check_bank_field!(interest_rate_config, protocol_origination_fee);
        check_bank_field!(interest_rate_config, zero_util_rate);
        check_bank_field!(interest_rate_config, hundred_util_rate);
        check_bank_field!(interest_rate_config, points);

        check_bank_field!(asset_weight_init);
        check_bank_field!(asset_weight_maint);
        check_bank_field!(liability_weight_init);
        check_bank_field!(liability_weight_maint);
        check_bank_field!(deposit_limit);
        check_bank_field!(borrow_limit);
        check_bank_field!(operational_state);
        check_bank_field!(risk_tier);
        check_bank_field!(asset_tag);
        check_bank_field!(total_asset_value_init_limit);
        check_bank_field!(oracle_max_age);
        check_bank_field!(oracle_max_confidence);

        assert!(permissionless_bad_debt_settlement
            // If Some(...) check flag set properly
            .map(|set| set == bank.get_flag(PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG))
            // If None check flag is unchanged
            .unwrap_or(
                bank.get_flag(PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG)
                    == old_bank.get_flag(PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG)
            ));

        assert!(freeze_settings
            // If Some(...) check flag set properly
            .map(|set| set == bank.get_flag(FREEZE_SETTINGS))
            // If None check flag is unchanged
            .unwrap_or(bank.get_flag(FREEZE_SETTINGS) == old_bank.get_flag(FREEZE_SETTINGS)));

        assert!(tokenless_repayments_allowed
            // If Some(...) check flag set properly
            .map(|set| set == bank.get_flag(TOKENLESS_REPAYMENTS_ALLOWED))
            // If None check flag is unchanged
            .unwrap_or(
                bank.get_flag(TOKENLESS_REPAYMENTS_ALLOWED)
                    == old_bank.get_flag(TOKENLESS_REPAYMENTS_ALLOWED)
            ));

        // Oracles no longer update in the standard config instruction
        assert_eq!(
            bank.config.oracle_keys, old_bank.config.oracle_keys,
            "The config does not update oracles, try config_oracle"
        );
        assert_eq!(
            bank.config.oracle_setup, old_bank.config.oracle_setup,
            "The config does not update oracles, try config_oracle"
        );
    };

    Ok(())
}

#[tokio::test]
async fn config_group_admins() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let new_admin = Pubkey::new_unique();
    let new_emode_admin = Pubkey::new_unique();
    let new_curve_admin = Pubkey::new_unique();
    let new_limit_admin = Pubkey::new_unique();
    let new_emissions_admin = Pubkey::new_unique();
    let new_metadata_admin = Pubkey::new_unique();
    let new_risk_admin = Pubkey::new_unique();

    let res = test_f
        .marginfi_group
        .try_update(
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
        )
        .await;

    assert!(res.is_ok());
    let group_after = test_f.marginfi_group.load().await;
    assert_eq!(group_after.admin, new_admin);
    assert_eq!(group_after.emode_admin, new_emode_admin);
    assert_eq!(group_after.delegate_curve_admin, new_curve_admin);
    assert_eq!(group_after.delegate_limit_admin, new_limit_admin);
    assert_eq!(group_after.delegate_emissions_admin, new_emissions_admin);
    assert_eq!(group_after.metadata_admin, new_metadata_admin);
    assert_eq!(group_after.risk_admin, new_risk_admin);
    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::T22WithFee)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_emode_success(bank_mint: BankMint) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let bank = test_f.get_bank(&bank_mint);
    let old_bank = bank.load().await;

    assert_eq!(old_bank.emode.flags, 0u64);
    assert_eq!(old_bank.emode.emode_tag, 0u16);

    // First try to enable emode without any entries
    let empty_emode_tag = 1u16;

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, empty_emode_tag, &[])
        .await;
    assert!(res.is_ok());

    // Load bank and check that the emode settings got applied
    let loaded_bank: Bank = test_f.load_and_deserialize(&bank.key).await;
    let timestamp = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp
    };

    assert_eq!(loaded_bank.emode.flags, 0u64); // EMODE_ON is still not set because there are no entries
    assert_eq!(loaded_bank.emode.emode_tag, empty_emode_tag);
    assert_eq!(loaded_bank.emode.timestamp, timestamp);
    assert_eq!(old_bank.emode.emode_config, loaded_bank.emode.emode_config); // config stays the same
    assert_eq!(old_bank.config, loaded_bank.config); // everything else also stays the same

    // Use weights that are safe (CW < LW)
    let liab_init_w = I80F48::from(loaded_bank.config.liability_weight_init);
    let liab_maint_w = I80F48::from(loaded_bank.config.liability_weight_maint);
    let asset_init_w = liab_init_w * I80F48::from_num(0.7);
    let asset_maint_w = liab_maint_w * I80F48::from_num(0.9);

    let emode_tag = 2u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag, // sharing the same tag is allowed
        flags: 1,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await;
    assert!(res.is_ok());

    // Load bank and check that the emode settings got applied
    let loaded_bank: Bank = test_f.load_and_deserialize(&bank.key).await;
    let timestamp = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp
    };

    assert_eq!(loaded_bank.emode.flags, EMODE_ON);
    assert_eq!(loaded_bank.emode.emode_tag, emode_tag);
    assert_eq!(loaded_bank.emode.timestamp, timestamp);
    // Due to sorting by tag, the newly added entry is the last one
    let last_entry_index = loaded_bank.emode.emode_config.entries.len() - 1;
    assert_eq!(
        loaded_bank.emode.emode_config.entries[last_entry_index],
        emode_entries[0]
    );
    // All other entries are still "inactive"
    for i in 0..last_entry_index {
        assert_eq!(
            loaded_bank.emode.emode_config.entries[i].collateral_bank_emode_tag,
            0
        );
    }

    Ok(())
}

#[tokio::test]
async fn lending_pool_clone_emode_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let copy_from_bank = test_f.get_bank(&BankMint::Usdc);
    let copy_to_bank_admin = test_f.get_bank(&BankMint::Sol);
    let copy_to_bank_emode_admin = test_f.get_bank(&BankMint::PyUSD);

    let copy_to_before = copy_to_bank_admin.load().await;
    assert_eq!(copy_to_before.emode.flags, 0);
    assert_eq!(copy_to_before.emode.emode_tag, 0);

    let copy_from_before = copy_from_bank.load().await;
    let liab_init_w = I80F48::from(copy_from_before.config.liability_weight_init);
    let liab_maint_w = I80F48::from(copy_from_before.config.liability_weight_maint);
    let asset_init_w = liab_init_w * I80F48::from_num(0.7);
    let asset_maint_w = liab_maint_w * I80F48::from_num(0.9);

    let emode_tag = 2u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 1,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&copy_from_bank, emode_tag, &emode_entries)
        .await?;

    // Admin can clone emode settings.
    test_f
        .marginfi_group
        .try_lending_pool_clone_emode(&copy_from_bank, &copy_to_bank_admin)
        .await?;

    let copy_from_after = copy_from_bank.load().await;
    let copy_to_after = copy_to_bank_admin.load().await;

    assert_eq!(copy_to_after.emode, copy_from_after.emode);
    assert_eq!(copy_to_after.config, copy_to_before.config);

    // A dedicated emode admin can also clone emode settings.
    let group_before = test_f.marginfi_group.load().await;
    let new_emode_admin = Keypair::new();
    test_f
        .marginfi_group
        .try_update(
            group_before.admin,
            new_emode_admin.pubkey(),
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
        )
        .await?;

    test_f
        .marginfi_group
        .try_lending_pool_clone_emode_with_signer(
            &new_emode_admin,
            &copy_from_bank,
            &copy_to_bank_emode_admin,
        )
        .await?;

    let copy_to_after = copy_to_bank_emode_admin.load().await;
    assert_eq!(copy_to_after.emode, copy_from_after.emode);

    Ok(())
}

#[tokio::test]
async fn lending_pool_clone_emode_unauthorized_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let copy_from_bank = test_f.get_bank(&BankMint::Usdc);
    let copy_to_bank = test_f.get_bank(&BankMint::Sol);

    let copy_to_before = copy_to_bank.load().await;

    let copy_from_before = copy_from_bank.load().await;
    let liab_init_w = I80F48::from(copy_from_before.config.liability_weight_init);
    let liab_maint_w = I80F48::from(copy_from_before.config.liability_weight_maint);
    let asset_init_w = liab_init_w * I80F48::from_num(0.7);
    let asset_maint_w = liab_maint_w * I80F48::from_num(0.9);

    let emode_tag = 2u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 1,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&copy_from_bank, emode_tag, &emode_entries)
        .await?;

    // Other group roles must not be able to clone emode settings.
    let group_before = test_f.marginfi_group.load().await;
    let new_risk_admin = Keypair::new();
    test_f
        .marginfi_group
        .try_update(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            new_risk_admin.pubkey(),
        )
        .await?;

    let err = test_f
        .marginfi_group
        .try_lending_pool_clone_emode_with_signer(&new_risk_admin, &copy_from_bank, &copy_to_bank)
        .await
        .unwrap_err();
    assert_custom_error!(err, MarginfiError::Unauthorized);

    let copy_to_after = copy_to_bank.load().await;
    assert_eq!(copy_to_after.emode, copy_to_before.emode);
    assert_eq!(copy_to_after.config, copy_to_before.config);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::T22WithFee)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_emode_invalid_args(bank_mint: BankMint) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&bank_mint);

    // Try to set an emode config with invalid weight params -> should fail
    let emode_tag = 1u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 1,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: I80F48!(1.0).into(),
        asset_weight_maint: I80F48!(0.9).into(),
    }];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await;
    assert!(res.is_err());

    // Try to set an emode config with duplicate entries -> should fail
    let emode_tag = 2u16;
    let emode_entries = vec![
        EmodeEntry {
            collateral_bank_emode_tag: emode_tag,
            flags: 1,
            pad0: [0, 0, 0, 0, 0],
            asset_weight_init: I80F48!(0.9).into(),
            asset_weight_maint: I80F48!(1.0).into(),
        },
        EmodeEntry {
            collateral_bank_emode_tag: emode_tag,
            flags: 0,
            pad0: [0, 0, 0, 0, 0],
            asset_weight_init: I80F48!(0.5).into(),
            asset_weight_maint: I80F48!(0.9).into(),
        },
    ];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await;
    assert!(res.is_err());

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_emode_valid_leverage(bank_mint: BankMint) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&bank_mint);
    let loaded_bank = bank.load().await;

    let group_before = test_f.marginfi_group.load().await;
    let max_init_leverage = I80F48::from_num(19);
    let max_maint_leverage = I80F48::from_num(20);
    test_f
        .marginfi_group
        .try_update_with_emode_leverage(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
            Some(max_init_leverage.into()),
            Some(max_maint_leverage.into()),
        )
        .await?;

    let liab_init_w = loaded_bank.config.liability_weight_init;
    let liab_maint_w = loaded_bank.config.liability_weight_maint;

    // liab_init = 1.2, liab_maint = 1.0, asset_init = 0.84, asset_maint = 0.9
    // This gives: L_init = 1/(1-0.84/1.2) = 1/(1-0.7) = 3.33x
    //             L_maint = 1/(1-0.9/1.0) = 1/0.1 = 10x
    let asset_init_w = I80F48::from(liab_init_w) * I80F48::from_num(0.7);
    let asset_maint_w = I80F48::from(liab_maint_w) * I80F48::from_num(0.9);

    let emode_tag = 1u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 0,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await;

    assert!(
        res.is_ok(),
        "Valid emode config with safe leverage should succeed"
    );

    let loaded_bank: Bank = test_f.load_and_deserialize(&bank.key).await;
    assert_eq!(loaded_bank.emode.flags, EMODE_ON);
    assert_eq!(loaded_bank.emode.emode_tag, emode_tag);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_emode_invalid_cw_exceeds_lw_init(
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&bank_mint);
    let loaded_bank = bank.load().await;

    let group_before = test_f.marginfi_group.load().await;
    let max_init_leverage = I80F48::from_num(19);
    let max_maint_leverage = I80F48::from_num(20);
    test_f
        .marginfi_group
        .try_update_with_emode_leverage(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
            Some(max_init_leverage.into()),
            Some(max_maint_leverage.into()),
        )
        .await?;

    let liab_init_w = I80F48::from(loaded_bank.config.liability_weight_init);
    let liab_maint_w = I80F48::from(loaded_bank.config.liability_weight_maint);

    // Set asset_weight_init >= liability_weight_init (invalid!)
    let asset_init_w = liab_init_w + I80F48::from_num(0.1); // Exceeds liability weight
    let asset_maint_w = liab_maint_w * I80F48::from_num(0.9); // This is fine

    let emode_tag = 1u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 0,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await;

    assert!(
        res.is_err(),
        "Emode config with CW_init >= LW_init should fail"
    );
    assert_custom_error!(res.unwrap_err(), MarginfiError::BadEmodeConfig);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_emode_invalid_cw_exceeds_lw_maint(
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&bank_mint);
    let loaded_bank = bank.load().await;

    let group_before = test_f.marginfi_group.load().await;
    let max_init_leverage = I80F48::from_num(19);
    let max_maint_leverage = I80F48::from_num(20);
    test_f
        .marginfi_group
        .try_update_with_emode_leverage(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
            Some(max_init_leverage.into()),
            Some(max_maint_leverage.into()),
        )
        .await?;

    let liab_init_w = I80F48::from(loaded_bank.config.liability_weight_init);
    let liab_maint_w = I80F48::from(loaded_bank.config.liability_weight_maint);

    let asset_init_w = liab_init_w * I80F48::from_num(0.8); // This is fine
    let asset_maint_w = liab_maint_w + I80F48::from_num(0.05); // Exceeds liability weight

    let emode_tag = 1u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 0,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await;

    assert!(
        res.is_err(),
        "Emode config with CW_maint >= LW_maint should fail"
    );
    assert_custom_error!(res.unwrap_err(), MarginfiError::BadEmodeConfig);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_emode_invalid_excessive_leverage(
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&bank_mint);
    let loaded_bank = bank.load().await;

    let group_before = test_f.marginfi_group.load().await;
    let max_init_leverage = I80F48::from_num(19);
    let max_maint_leverage = I80F48::from_num(20);
    test_f
        .marginfi_group
        .try_update_with_emode_leverage(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
            Some(max_init_leverage.into()),
            Some(max_maint_leverage.into()),
        )
        .await?;

    let liab_init_w = I80F48::from(loaded_bank.config.liability_weight_init);
    let liab_maint_w = I80F48::from(loaded_bank.config.liability_weight_maint);

    // Set weights that result in >20x leverage
    // For init: CW/LW = 0.96 => L = 1/(1-0.96) = 1/0.04 = 25x > 20x (invalid!)
    let asset_init_w = liab_init_w * I80F48::from_num(0.96);
    // For maint: CW/LW = 0.95 => L = 1/(1-0.95) = 1/0.05 = 20x (at the limit, should be ok)
    let asset_maint_w = liab_maint_w * I80F48::from_num(0.95);

    let emode_tag = 1u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 0,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await;

    assert!(res.is_err(), "Emode config with leverage > 20x should fail");
    assert_custom_error!(res.unwrap_err(), MarginfiError::BadEmodeConfig);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_rejects_invalid_emode_leverage_on_weight_update(
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&bank_mint);

    let liab_init_w = I80F48::from_num(1.5);
    let liab_maint_w = I80F48::from_num(1.5);
    bank.update_config(
        BankConfigOpt {
            liability_weight_init: Some(liab_init_w.into()),
            liability_weight_maint: Some(liab_maint_w.into()),
            ..BankConfigOpt::default()
        },
        None,
    )
    .await?;

    let asset_init_w = liab_init_w * I80F48::from_num(0.9);
    let asset_maint_w = liab_maint_w * I80F48::from_num(0.9);

    let emode_tag = 1u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 0,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await?;

    // Tighten base weights to push emode leverage above max while remaining a valid bank config.
    let new_liab_init_w = asset_init_w / I80F48::from_num(0.96);
    let new_liab_maint_w = asset_maint_w / I80F48::from_num(0.96);

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank(
            &bank,
            BankConfigOpt {
                liability_weight_init: Some(new_liab_init_w.into()),
                liability_weight_maint: Some(new_liab_maint_w.into()),
                ..BankConfigOpt::default()
            },
        )
        .await;

    assert!(
        res.is_err(),
        "Updating base weights that push emode leverage above limits should fail"
    );
    assert_custom_error!(res.unwrap_err(), MarginfiError::BadEmodeConfig);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_bank_emode_max_leverage_boundary(bank_mint: BankMint) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&bank_mint);
    let loaded_bank = bank.load().await;

    let group_before = test_f.marginfi_group.load().await;
    let max_init_leverage = I80F48::from_num(19);
    let max_maint_leverage = I80F48::from_num(20);
    test_f
        .marginfi_group
        .try_update_with_emode_leverage(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
            Some(max_init_leverage.into()),
            Some(max_maint_leverage.into()),
        )
        .await?;

    let liab_init_w = I80F48::from(loaded_bank.config.liability_weight_init);
    let liab_maint_w = I80F48::from(loaded_bank.config.liability_weight_maint);

    // Set weights that result in slightly below the limits to account for floating point precision
    // For init: targeting ~18.9x leverage (safely below 19x limit)
    // For maint: targeting ~19.9x leverage (safely below 20x limit)
    // CW/LW = 1 - 1/L, so for L=18.9: CW/LW = 1 - 1/18.9 ≈ 0.9471
    // For L=19.9: CW/LW = 1 - 1/19.9 ≈ 0.9497
    let asset_init_w = liab_init_w * I80F48::from_num(0.947);
    let asset_maint_w = liab_maint_w * I80F48::from_num(0.9497);

    let emode_tag = 1u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: emode_tag,
        flags: 0,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: asset_init_w.into(),
        asset_weight_maint: asset_maint_w.into(),
    }];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&bank, emode_tag, &emode_entries)
        .await;

    assert!(
        res.is_ok(),
        "Emode config with init<19x and maint<20x leverage (below limits) should succeed"
    );

    let loaded_bank: Bank = test_f.load_and_deserialize(&bank.key).await;
    assert_eq!(loaded_bank.emode.flags, EMODE_ON);
    assert_eq!(loaded_bank.emode.emode_tag, emode_tag);

    Ok(())
}

#[tokio::test]
async fn configure_bank_interest_only_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&BankMint::Usdc);
    let old_bank = bank.load().await;

    let exp_points = make_points(&vec![
        RatePoint::new(1234, 56789),
        RatePoint::new(2345, 67890),
    ]);

    let ir_config = InterestRateConfigOpt {
        // TODO deprecate in 1.7
        // optimal_utilization_rate: Some(I80F48::from_num(0.9).into()),
        // plateau_interest_rate: Some(I80F48::from_num(0.5).into()),
        // max_interest_rate: Some(I80F48::from_num(1.5).into()),
        insurance_fee_fixed_apr: Some(I80F48::from_num(0.01).into()),
        insurance_ir_fee: Some(I80F48::from_num(0.02).into()),
        protocol_fixed_fee_apr: Some(I80F48::from_num(0.03).into()),
        protocol_ir_fee: Some(I80F48::from_num(0.04).into()),
        protocol_origination_fee: Some(I80F48::from_num(0.05).into()),
        zero_util_rate: Some(123),
        hundred_util_rate: Some(1234567),
        points: Some(exp_points),
    };

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_interest_only(&bank, ir_config.clone())
        .await?;

    let bank_after: Bank = test_f.load_and_deserialize(&bank.key).await;

    // TODO deprecate in 1.7
    assert_eq!(
        bank_after
            .config
            .interest_rate_config
            .optimal_utilization_rate,
        I80F48::ZERO.into()
    );
    // TODO deprecate in 1.7
    assert_eq!(
        bank_after.config.interest_rate_config.plateau_interest_rate,
        I80F48::ZERO.into()
    );
    // TODO deprecate in 1.7
    assert_eq!(
        bank_after.config.interest_rate_config.max_interest_rate,
        I80F48::ZERO.into()
    );
    assert_eq!(
        bank_after
            .config
            .interest_rate_config
            .insurance_fee_fixed_apr,
        ir_config.insurance_fee_fixed_apr.unwrap()
    );
    assert_eq!(
        bank_after.config.interest_rate_config.insurance_ir_fee,
        ir_config.insurance_ir_fee.unwrap()
    );
    assert_eq!(
        bank_after
            .config
            .interest_rate_config
            .protocol_fixed_fee_apr,
        ir_config.protocol_fixed_fee_apr.unwrap()
    );
    assert_eq!(
        bank_after.config.interest_rate_config.protocol_ir_fee,
        ir_config.protocol_ir_fee.unwrap()
    );
    assert_eq!(
        bank_after
            .config
            .interest_rate_config
            .protocol_origination_fee,
        ir_config.protocol_origination_fee.unwrap()
    );
    assert_eq!(
        bank_after.config.interest_rate_config.zero_util_rate,
        ir_config.zero_util_rate.unwrap()
    );
    assert_eq!(
        bank_after.config.interest_rate_config.hundred_util_rate,
        ir_config.hundred_util_rate.unwrap()
    );
    assert_eq!(bank_after.config.interest_rate_config.points, exp_points);
    assert_eq!(
        bank_after.config.interest_rate_config.curve_type,
        INTEREST_CURVE_SEVEN_POINT
    );

    // No change
    assert_eq!(
        bank_after.config.deposit_limit,
        old_bank.config.deposit_limit
    );
    assert_eq!(bank_after.config.borrow_limit, old_bank.config.borrow_limit);
    assert_eq!(
        bank_after.config.total_asset_value_init_limit,
        old_bank.config.total_asset_value_init_limit
    );

    Ok(())
}

#[tokio::test]
async fn configure_bank_interest_only_not_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&BankMint::Usdc);
    let group_before = test_f.marginfi_group.load().await;
    test_f
        .marginfi_group
        .try_update(
            group_before.admin,
            group_before.emode_admin,
            Pubkey::new_unique(),
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
        )
        .await?;

    let ir_config = InterestRateConfigOpt {
        hundred_util_rate: Some(1234567),
        ..Default::default()
    };

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_interest_only(&bank, ir_config)
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}

#[tokio::test]
async fn configure_bank_limits_only_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&BankMint::Usdc);
    let old_bank = bank.load().await;

    let new_deposit_limit = old_bank.config.deposit_limit + 100;
    let new_borrow_limit = old_bank.config.borrow_limit + 200;
    let new_tavl = old_bank.config.total_asset_value_init_limit + 50;

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank_limits_only(
            &bank,
            Some(new_deposit_limit),
            Some(new_borrow_limit),
            Some(new_tavl),
        )
        .await?;

    let bank_after: Bank = test_f.load_and_deserialize(&bank.key).await;

    assert_eq!(bank_after.config.deposit_limit, new_deposit_limit);
    assert_eq!(bank_after.config.borrow_limit, new_borrow_limit);
    assert_eq!(bank_after.config.total_asset_value_init_limit, new_tavl);
    assert_eq!(
        bank_after.config.interest_rate_config,
        old_bank.config.interest_rate_config
    );

    Ok(())
}

#[tokio::test]
async fn configure_bank_limits_only_not_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&BankMint::Usdc);
    let group_before = test_f.marginfi_group.load().await;
    test_f
        .marginfi_group
        .try_update(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            Pubkey::new_unique(),
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
        )
        .await?;

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_limits_only(&bank, Some(1), Some(1), Some(1))
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::SolSwbPull)]
#[tokio::test]
async fn configure_group_max_emode_leverage_propagates_to_bank_cache(
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let bank = test_f.get_bank(&bank_mint);
    let group_before = test_f.marginfi_group.load().await;

    let custom_max_init_leverage = I80F48::from_num(14);
    let custom_max_maint_leverage = I80F48::from_num(15);
    let res = test_f
        .marginfi_group
        .try_update_with_emode_leverage(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
            Some(custom_max_init_leverage.into()),
            Some(custom_max_maint_leverage.into()),
        )
        .await;

    assert!(res.is_ok(), "Setting max_emode_leverage should succeed");

    let group_after: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    let actual_init = u32_to_basis(group_after.emode_max_init_leverage);
    assert!(
        (actual_init - custom_max_init_leverage).abs() < I80F48::from_num(0.001),
        "Group's emode_max_init_leverage should be approximately {} (got {})",
        custom_max_init_leverage,
        actual_init
    );
    let actual_maint = u32_to_basis(group_after.emode_max_maint_leverage);
    assert!(
        (actual_maint - custom_max_maint_leverage).abs() < I80F48::from_num(0.001),
        "Group's emode_max_maint_leverage should be approximately {} (got {})",
        custom_max_maint_leverage,
        actual_maint
    );

    let config_bank_opt = BankConfigOpt {
        deposit_limit: Some(1000000),
        ..BankConfigOpt::default()
    };
    let res = bank.update_config(config_bank_opt, None).await;
    assert!(res.is_ok());

    let res = test_f
        .marginfi_group
        .try_update_with_emode_leverage(
            group_before.admin,
            group_before.emode_admin,
            group_before.delegate_curve_admin,
            group_before.delegate_limit_admin,
            group_before.delegate_emissions_admin,
            group_before.metadata_admin,
            group_before.risk_admin,
            None, // Should default to 15
            None, // Should default to 20
        )
        .await;

    assert!(
        res.is_ok(),
        "Setting max_emode_leverage to default should succeed"
    );

    let group_default: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    let default_init_max_leverage = I80F48::from_num(15); // DEFAULT_INIT_MAX_EMODE_LEVERAGE
    let default_maint_max_leverage = I80F48::from_num(20); // DEFAULT_MAINT_MAX_EMODE_LEVERAGE
    let actual_default_init = u32_to_basis(group_default.emode_max_init_leverage);
    assert!(
        (actual_default_init - default_init_max_leverage).abs() < I80F48::from_num(0.001),
        "Group's emode_max_init_leverage should be approximately {} (got {})",
        default_init_max_leverage,
        actual_default_init
    );
    let actual_default_maint = u32_to_basis(group_default.emode_max_maint_leverage);
    assert!(
        (actual_default_maint - default_maint_max_leverage).abs() < I80F48::from_num(0.001),
        "Group's emode_max_maint_leverage should be approximately {} (got {})",
        default_maint_max_leverage,
        actual_default_maint
    );

    let config_bank_opt2 = BankConfigOpt {
        borrow_limit: Some(500000),
        ..BankConfigOpt::default()
    };
    let res = bank.update_config(config_bank_opt2, None).await;
    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn configure_bank_oracle_min_age_validation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&BankMint::Usdc);

    // Try to set oracle_max_age below ORACLE_MIN_AGE (10 seconds) - should fail
    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank(
            &bank,
            BankConfigOpt {
                oracle_max_age: Some(9),
                ..Default::default()
            },
        )
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidOracleSetup);

    // Try to set oracle_max_age exactly at ORACLE_MIN_AGE (10 seconds) - should succeed
    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank(
            &bank,
            BankConfigOpt {
                oracle_max_age: Some(10),
                ..Default::default()
            },
        )
        .await;

    assert!(res.is_ok());

    let bank_after: Bank = test_f.load_and_deserialize(&bank.key).await;
    assert_eq!(bank_after.config.oracle_max_age, 10);

    // Try to set oracle_max_age above ORACLE_MIN_AGE - should succeed
    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank(
            &bank,
            BankConfigOpt {
                oracle_max_age: Some(30),
                ..Default::default()
            },
        )
        .await;

    assert!(res.is_ok());

    let bank_after: Bank = test_f.load_and_deserialize(&bank.key).await;
    assert_eq!(bank_after.config.oracle_max_age, 30);

    Ok(())
}
