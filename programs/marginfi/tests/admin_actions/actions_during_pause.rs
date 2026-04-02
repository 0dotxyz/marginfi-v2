use fixed_macro::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, native, prelude::*};
use marginfi::prelude::*;
use marginfi::state::{bank::BankVaultType, marginfi_account::MarginfiAccountImpl};
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{BankConfig, BankConfigOpt, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP},
};
use fixtures::test::{PYTH_SOL_FEED, PYTH_USDC_FEED};
use solana_program_test::*;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;

/// Deleverage with withdraw + repay succeeds while protocol is paused.
#[tokio::test]
async fn deleverage_succeeds_during_pause() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();
    let risk_admin = test_f.payer().clone();

    let lp = test_f.create_marginfi_account().await;
    let deleveragee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // LP provides liquidity
    let lp_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Setup deleveragee: deposit $30 SOL, borrow $20 USDC
    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&authority.pubkey())
        .await;

    deleveragee
        .try_bank_deposit_with_authority(user_token_sol.key, sol_bank, 3.0, None, &authority)
        .await?;
    deleveragee
        .try_bank_borrow_with_authority(user_token_usdc.key, usdc_bank, 20.0, 0, &authority)
        .await?;

    // Tweak weights so health can improve during deleverage
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.7).into()),
                asset_weight_maint: Some(I80F48!(0.8).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    // Pause the protocol
    test_f.marginfi_group.try_panic_pause().await?;
    test_f.marginfi_group.try_propagate_fee_state().await?;

    let marginfi_group = test_f.marginfi_group.load().await;
    assert!(marginfi_group.panic_state_cache.is_paused_flag());

    // Build deleverage tx
    let (record_pk, _) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );

    let risk_admin_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    let risk_admin_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;
    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(risk_admin_sol_acc.key, sol_bank, 1.0, None)
        .await;
    let repay_ix = deleveragee
        .make_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 10.0, None)
        .await;
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    // Execute deleverage while paused — should succeed
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Verify the deleverage worked
    let risk_admin_sol_tokens = risk_admin_sol_acc.balance().await;
    assert_eq!(risk_admin_sol_tokens, native!(1.0, "SOL", f64));

    let deleveragee_ma = deleveragee.load().await;
    assert!(!deleveragee_ma.get_flag(ACCOUNT_IN_RECEIVERSHIP));

    Ok(())
}

/// Normal user withdraw still fails during pause (not in deleverage).
#[tokio::test]
async fn normal_withdraw_still_blocked_during_pause() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let account_f = test_f.create_marginfi_account().await;
    let token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1000)
        .await;

    account_f
        .try_bank_deposit(token_account.key, usdc_bank, 500, None)
        .await?;

    // Pause
    test_f.marginfi_group.try_panic_pause().await?;
    test_f.marginfi_group.try_propagate_fee_state().await?;

    // Normal withdraw should fail
    let result = account_f
        .try_bank_withdraw(token_account.key, usdc_bank, 100, None)
        .await;

    assert_custom_error!(result.unwrap_err(), MarginfiError::ProtocolPaused);

    Ok(())
}

/// Normal user repay still fails during pause (not in deleverage).
#[tokio::test]
async fn normal_repay_still_blocked_during_pause() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Setup: LP deposits SOL, borrower deposits USDC and borrows SOL
    let lp = test_f.create_marginfi_account().await;
    let sol_acc = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(sol_acc.key, sol_bank, 10, None).await?;

    let borrower = test_f.create_marginfi_account().await;
    let usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1000)
        .await;
    borrower
        .try_bank_deposit(usdc_acc.key, usdc_bank, 1000, None)
        .await?;

    let borrow_acc = test_f.sol_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrow_acc.key, sol_bank, 1)
        .await?;

    // Pause
    test_f.marginfi_group.try_panic_pause().await?;
    test_f.marginfi_group.try_propagate_fee_state().await?;

    // Normal repay should fail
    let result = borrower
        .try_bank_repay(borrow_acc.key, sol_bank, 1, None)
        .await;

    assert_custom_error!(result.unwrap_err(), MarginfiError::ProtocolPaused);

    Ok(())
}

/// Permissionless liquidation still fails during pause.
#[tokio::test]
async fn liquidation_still_blocked_during_pause() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();
    let risk_admin = test_f.payer().clone();

    let lp = test_f.create_marginfi_account().await;
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // LP provides liquidity
    let lp_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Setup liquidatee: deposit SOL, borrow USDC
    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&authority.pubkey())
        .await;

    liquidatee
        .try_bank_deposit_with_authority(user_token_sol.key, sol_bank, 3.0, None, &authority)
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(user_token_usdc.key, usdc_bank, 20.0, 0, &authority)
        .await?;

    // Make account unhealthy
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.5).into()),
                asset_weight_maint: Some(I80F48!(0.6).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    // Pause
    test_f.marginfi_group.try_panic_pause().await?;
    test_f.marginfi_group.try_propagate_fee_state().await?;

    // Attempt liquidation — start_liquidation itself doesn't check pause,
    // but the withdraw/repay inside the tx will fail.
    // Actually, start_liquidation has no pause check, but the key question is
    // whether a liquidator can execute a full liquidation.
    // Since withdraw checks pause and liquidation sets ACCOUNT_IN_RECEIVERSHIP
    // (not ACCOUNT_IN_DELEVERAGE), the withdraw inside should fail.
    let (record_pk, _) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );

    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;

    let init_ix = liquidatee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;
    let start_ix = liquidatee
        .make_start_liquidation_ix(record_pk, risk_admin)
        .await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.5, None)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 5.0, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            risk_admin,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    let result = {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await
    };

    // Should fail because withdraw/repay are blocked during pause for liquidations
    assert!(result.is_err());

    Ok(())
}

/// Admin can handle_bankruptcy while protocol is paused.
#[tokio::test]
async fn handle_bankruptcy_by_admin_succeeds_during_pause() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        ..Default::default()
    }))
    .await;

    // Setup: lender deposits USDC, borrower deposits SOL and borrows USDC
    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(
            lender_usdc.key,
            test_f.get_bank(&BankMint::Usdc),
            100_000,
            None,
        )
        .await?;

    let mut borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_001)
        .await;
    borrower
        .try_bank_deposit(
            borrower_sol.key,
            test_f.get_bank(&BankMint::Sol),
            1_001,
            None,
        )
        .await?;

    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, test_f.get_bank(&BankMint::Usdc), 10_000)
        .await?;

    // Make account bankrupt by zeroing collateral
    let collateral_bank = test_f.get_bank(&BankMint::Sol);
    borrower
        .nullify_assets_for_bank(collateral_bank.key)
        .await?;

    // Fund insurance vault
    {
        let (insurance_vault, _) = test_f
            .get_bank(&BankMint::Usdc)
            .get_vault(BankVaultType::Insurance);
        test_f
            .get_bank_mut(&BankMint::Usdc)
            .mint
            .mint_to(&insurance_vault, 10_000)
            .await;
    }

    // Pause the protocol
    test_f.marginfi_group.try_panic_pause().await?;
    test_f.marginfi_group.try_propagate_fee_state().await?;

    let marginfi_group = test_f.marginfi_group.load().await;
    assert!(marginfi_group.panic_state_cache.is_paused_flag());

    // Admin calls handle_bankruptcy while paused — should succeed
    let debt_bank = test_f.get_bank(&BankMint::Usdc);
    let res = test_f
        .marginfi_group
        .try_handle_bankruptcy(debt_bank, &borrower)
        .await;

    assert!(res.is_ok());

    // Verify account is disabled after bankruptcy
    let borrower_account = borrower.load().await;
    assert!(borrower_account.get_flag(ACCOUNT_DISABLED));

    Ok(())
}

/// Non-admin cannot handle_bankruptcy while protocol is paused (even with permissionless flag).
#[tokio::test]
async fn handle_bankruptcy_by_non_admin_fails_during_pause() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        ..Default::default()
    }))
    .await;

    // Setup: lender deposits USDC, borrower deposits SOL and borrows USDC
    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(
            lender_usdc.key,
            test_f.get_bank(&BankMint::Usdc),
            100_000,
            None,
        )
        .await?;

    let mut borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_001)
        .await;
    borrower
        .try_bank_deposit(
            borrower_sol.key,
            test_f.get_bank(&BankMint::Sol),
            1_001,
            None,
        )
        .await?;

    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, test_f.get_bank(&BankMint::Usdc), 10_000)
        .await?;

    // Make account bankrupt
    let collateral_bank = test_f.get_bank(&BankMint::Sol);
    borrower
        .nullify_assets_for_bank(collateral_bank.key)
        .await?;

    // Fund insurance vault
    {
        let (insurance_vault, _) = test_f
            .get_bank(&BankMint::Usdc)
            .get_vault(BankVaultType::Insurance);
        test_f
            .get_bank_mut(&BankMint::Usdc)
            .mint
            .mint_to(&insurance_vault, 10_000)
            .await;
    }

    // Enable permissionless bad debt settlement
    let debt_bank = test_f.get_bank(&BankMint::Usdc);
    debt_bank
        .update_config(
            BankConfigOpt {
                permissionless_bad_debt_settlement: Some(true),
                ..Default::default()
            },
            None,
        )
        .await?;

    // Change admin and risk_admin to random keys so payer is no longer admin
    test_f
        .marginfi_group
        .try_update(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        )
        .await?;

    // Pause the protocol
    test_f.marginfi_group.try_panic_pause().await?;
    test_f.marginfi_group.try_propagate_fee_state().await?;

    // Non-admin calls handle_bankruptcy while paused — should fail
    let res = test_f
        .marginfi_group
        .try_handle_bankruptcy(debt_bank, &borrower)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::ProtocolPaused);

    Ok(())
}

/// Permissionless handle_bankruptcy still works when protocol is NOT paused (regression).
#[tokio::test]
async fn handle_bankruptcy_without_pause_permissionless_still_works() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        ..Default::default()
    }))
    .await;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(
            lender_usdc.key,
            test_f.get_bank(&BankMint::Usdc),
            100_000,
            None,
        )
        .await?;

    let mut borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_001)
        .await;
    borrower
        .try_bank_deposit(
            borrower_sol.key,
            test_f.get_bank(&BankMint::Sol),
            1_001,
            None,
        )
        .await?;

    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, test_f.get_bank(&BankMint::Usdc), 10_000)
        .await?;

    let collateral_bank = test_f.get_bank(&BankMint::Sol);
    borrower
        .nullify_assets_for_bank(collateral_bank.key)
        .await?;

    {
        let (insurance_vault, _) = test_f
            .get_bank(&BankMint::Usdc)
            .get_vault(BankVaultType::Insurance);
        test_f
            .get_bank_mut(&BankMint::Usdc)
            .mint
            .mint_to(&insurance_vault, 10_000)
            .await;
    }

    let debt_bank = test_f.get_bank(&BankMint::Usdc);
    debt_bank
        .update_config(
            BankConfigOpt {
                permissionless_bad_debt_settlement: Some(true),
                ..Default::default()
            },
            None,
        )
        .await?;

    // Change admin/risk_admin so payer is no longer admin
    test_f
        .marginfi_group
        .try_update(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        )
        .await?;

    // Protocol is NOT paused — permissionless caller should succeed
    let marginfi_group = test_f.marginfi_group.load().await;
    assert!(!marginfi_group.panic_state_cache.is_paused_flag());

    let res = test_f
        .marginfi_group
        .try_handle_bankruptcy(debt_bank, &borrower)
        .await;

    assert!(res.is_ok());

    let borrower_account = borrower.load().await;
    assert!(borrower_account.get_flag(ACCOUNT_DISABLED));

    Ok(())
}

/// Non-admin can handle_bankruptcy after pause expires.
#[tokio::test]
async fn handle_bankruptcy_non_admin_succeeds_after_unpause() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        ..Default::default()
    }))
    .await;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(
            lender_usdc.key,
            test_f.get_bank(&BankMint::Usdc),
            100_000,
            None,
        )
        .await?;

    let mut borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_001)
        .await;
    borrower
        .try_bank_deposit(
            borrower_sol.key,
            test_f.get_bank(&BankMint::Sol),
            1_001,
            None,
        )
        .await?;

    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, test_f.get_bank(&BankMint::Usdc), 10_000)
        .await?;

    let collateral_bank = test_f.get_bank(&BankMint::Sol);
    borrower
        .nullify_assets_for_bank(collateral_bank.key)
        .await?;

    {
        let (insurance_vault, _) = test_f
            .get_bank(&BankMint::Usdc)
            .get_vault(BankVaultType::Insurance);
        test_f
            .get_bank_mut(&BankMint::Usdc)
            .mint
            .mint_to(&insurance_vault, 10_000)
            .await;
    }

    let debt_bank = test_f.get_bank(&BankMint::Usdc);
    debt_bank
        .update_config(
            BankConfigOpt {
                permissionless_bad_debt_settlement: Some(true),
                ..Default::default()
            },
            None,
        )
        .await?;

    // Change admin/risk_admin so payer is no longer admin
    test_f
        .marginfi_group
        .try_update(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        )
        .await?;

    // Pause the protocol
    test_f.marginfi_group.try_panic_pause().await?;
    test_f.marginfi_group.try_propagate_fee_state().await?;

    // Verify paused
    let marginfi_group = test_f.marginfi_group.load().await;
    assert!(marginfi_group.panic_state_cache.is_paused_flag());

    // Advance clock past pause expiry
    let new_timestamp = {
        let start_timestamp = {
            let ctx = test_f.context.borrow_mut();
            let clock: anchor_lang::prelude::Clock = ctx.banks_client.get_sysvar().await?;
            clock.unix_timestamp + marginfi_type_crate::types::PanicState::PAUSE_DURATION_SECONDS + 60
        };

        let ctx = test_f.context.borrow_mut();
        let mut clock: anchor_lang::prelude::Clock = ctx.banks_client.get_sysvar().await?;
        let time =
            start_timestamp + marginfi_type_crate::types::PanicState::PAUSE_DURATION_SECONDS + 60;
        clock.unix_timestamp = time;
        ctx.set_sysvar(&clock);
        time
    };

    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, new_timestamp)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_SOL_FEED, new_timestamp)
        .await;

    // Propagate expired state
    test_f.marginfi_group.try_propagate_fee_state().await?;

    // Non-admin calls handle_bankruptcy after pause expired — should succeed
    let res = test_f
        .marginfi_group
        .try_handle_bankruptcy(debt_bank, &borrower)
        .await;

    assert!(res.is_ok());

    let borrower_account = borrower.load().await;
    assert!(borrower_account.get_flag(ACCOUNT_DISABLED));

    Ok(())
}
