use super::common::*;
use fixed::types::I80F48 as FixedI80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, native, prelude::*};
use marginfi::{assert_eq_with_tolerance, errors::MarginfiError};
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED, types::compute_same_asset_emode_weight,
};
use solana_program_test::*;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, pubkey::Pubkey, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

fn midpoint(left: FixedI80F48, right: FixedI80F48) -> FixedI80F48 {
    (left + right) / FixedI80F48::from_num(2)
}

fn usdc_native_to_ui(amount: u64) -> FixedI80F48 {
    FixedI80F48::from_num(amount) / FixedI80F48::from_num(native!(1, "USDC"))
}

#[tokio::test]
async fn same_asset_emode_juplend_same_mint_position_is_healthy_then_turns_unhealthy_when_leverage_tightens(
) -> anyhow::Result<()> {
    let deposit_ui = 89.0;
    let deposit_native = native!(89, "USDC");
    let healthy_init_leverage = 76;
    let healthy_maint_leverage = 79;
    let tightened_init_leverage = 72;
    let tightened_maint_leverage = 75;

    let setup = TestFixture::setup_juplend_bank(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    configure_same_asset_pair(
        &setup.test_f,
        setup.test_f.get_bank(&BankMint::Usdc),
        &setup.bank_f,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;

    let usdc_bank = setup.test_f.get_bank(&BankMint::Usdc);

    let lp = setup.test_f.create_marginfi_account().await;
    let lp_usdc = setup
        .test_f
        .usdc_mint
        .create_token_account_and_mint_to(220.0)
        .await;
    lp.try_bank_deposit(lp_usdc.key, usdc_bank, 220.0, None)
        .await?;

    let lending_before_deposit = setup.load_lending().await;
    let (user, user_usdc) = setup.create_user_with_liquidity(deposit_ui).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &user, user_usdc.key, deposit_native)
        .await?;
    let accounted_shares = setup
        .load_user_accounted_shares(&user)
        .await
        .expect("juplend collateral should be active after deposit");
    let expected_shares = lending_before_deposit
        .expected_shares_for_deposit(deposit_native)
        .expect("expected shares for deposit should be computable");
    let accounted_underlying = lending_before_deposit
        .expected_assets_for_redeem(accounted_shares)
        .expect("expected assets for redeem should be computable");
    assert_eq_with_tolerance!(
        accounted_shares as i128,
        expected_shares as i128,
        JUPLEND_ROUNDING_TOLERANCE_NATIVE
    );
    assert_eq_with_tolerance!(
        accounted_underlying as i128,
        deposit_native as i128,
        JUPLEND_ROUNDING_TOLERANCE_NATIVE
    );

    // Deposit = 89 underlying USDC into JupLend.
    // JupLend first mints `expected_shares` shares, then health redeems the accounted shares back
    // into `accounted_underlying` underlying USDC. The assertions above pin that redeemed amount
    // to the nominal 89 USDC within the 30-native-unit JupLend share-rounding tolerance.
    // On the nominal 89 USDC deposit:
    // - healthy init weight = 75 / 76 ~= 0.986842, so the healthy init limit is
    //   89 * 75 / 76 ~= 87.828947 USDC
    // - tightened maint weight = 74 / 75 ~= 0.986667, so the tightened maint limit is
    //   89 * 74 / 75 ~= 87.813333 USDC
    // Borrow is the midpoint between those two values after using the live redeemed underlying
    // amount, so the position starts healthy and flips unhealthy after the tighten.
    let accounted_underlying_ui = usdc_native_to_ui(accounted_underlying);
    let healthy_init_limit = accounted_underlying_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(healthy_init_leverage),
            FixedI80F48::ONE,
        );
    let tightened_maint_limit = accounted_underlying_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(tightened_maint_leverage),
            FixedI80F48::ONE,
        );
    let borrow_ui = midpoint(healthy_init_limit, tightened_maint_limit).to_num::<f64>();

    let borrow_destination = setup.test_f.usdc_mint.create_empty_token_account().await;
    user.try_bank_borrow(borrow_destination.key, usdc_bank, borrow_ui)
        .await?;

    reconfigure_same_asset_leverage(
        &setup.test_f,
        tightened_init_leverage,
        tightened_maint_leverage,
    )
    .await?;
    user.try_lending_account_pulse_health().await?;

    let tightened = user.load().await;
    let tightened_init_health = FixedI80F48::from(tightened.health_cache.asset_value)
        - FixedI80F48::from(tightened.health_cache.liability_value);
    let tightened_maint_health = FixedI80F48::from(tightened.health_cache.asset_value_maint)
        - FixedI80F48::from(tightened.health_cache.liability_value_maint);
    assert!(tightened_init_health < FixedI80F48::ZERO);
    assert!(tightened_maint_health < FixedI80F48::ZERO);

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_juplend_same_value_borrow_fails_once_the_liability_mint_changes(
) -> anyhow::Result<()> {
    let deposit_ui = 97.0;
    let deposit_native = native!(97, "USDC");
    let same_asset_init_leverage = 63;
    let same_asset_maint_leverage = 66;

    let setup = TestFixture::setup_juplend_bank(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: None,
            },
        ],
        protocol_fees: false,
    }))
    .await;
    configure_same_asset_pair(
        &setup.test_f,
        setup.test_f.get_bank(&BankMint::Usdc),
        &setup.bank_f,
        0.5,
        0.5,
        same_asset_init_leverage,
        same_asset_maint_leverage,
    )
    .await?;

    let usdc_bank = setup.test_f.get_bank(&BankMint::Usdc);
    let sol_bank = setup.test_f.get_bank(&BankMint::Sol);

    let lp_usdc_account = setup.test_f.create_marginfi_account().await;
    let lp_usdc = setup
        .test_f
        .usdc_mint
        .create_token_account_and_mint_to(240.0)
        .await;
    lp_usdc_account
        .try_bank_deposit(lp_usdc.key, usdc_bank, 240.0, None)
        .await?;

    let lp_sol_account = setup.test_f.create_marginfi_account().await;
    let lp_sol = setup
        .test_f
        .sol_mint
        .create_token_account_and_mint_to(24.0)
        .await;
    lp_sol_account
        .try_bank_deposit(lp_sol.key, sol_bank, 24.0, None)
        .await?;

    let lending_before_deposit = setup.load_lending().await;
    let (user, user_usdc) = setup.create_user_with_liquidity(deposit_ui).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &user, user_usdc.key, deposit_native)
        .await?;
    let accounted_shares = setup
        .load_user_accounted_shares(&user)
        .await
        .expect("juplend collateral should be active after deposit");
    let expected_shares = lending_before_deposit
        .expected_shares_for_deposit(deposit_native)
        .expect("expected shares for deposit should be computable");
    let accounted_underlying = lending_before_deposit
        .expected_assets_for_redeem(accounted_shares)
        .expect("expected assets for redeem should be computable");
    assert_eq_with_tolerance!(
        accounted_shares as i128,
        expected_shares as i128,
        JUPLEND_ROUNDING_TOLERANCE_NATIVE
    );
    assert_eq_with_tolerance!(
        accounted_underlying as i128,
        deposit_native as i128,
        JUPLEND_ROUNDING_TOLERANCE_NATIVE
    );

    // Deposit = 97 underlying USDC into JupLend.
    // The redeemed underlying amount is again pinned to the nominal deposit within the share
    // rounding tolerance.
    // On the nominal 97 USDC deposit:
    // - same-asset init weight = 62 / 63 ~= 0.984127, so the same-mint USDC limit is
    //   97 * 62 / 63 ~= 95.460317 USDC
    // - plain regular limit after the liability mint changes is only 97 * 0.5 = 48.5 USDC
    // Converting that same debt notional into SOL at $10/SOL gives roughly 9.545 SOL, but with
    // the same-asset lift gone the account could support only about 4.85 SOL of debt, so the
    // equal-value SOL borrow must fail.
    let accounted_underlying_ui = usdc_native_to_ui(accounted_underlying);
    let same_asset_limit = accounted_underlying_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(same_asset_init_leverage),
            FixedI80F48::ONE,
        );
    let borrow_ui = same_asset_limit.to_num::<f64>();
    let equivalent_sol_borrow_ui =
        (FixedI80F48::from_num(borrow_ui) / FixedI80F48::from_num(10)).to_num::<f64>();

    let user_usdc_borrow = setup.test_f.usdc_mint.create_empty_token_account().await;
    user.try_bank_borrow(user_usdc_borrow.key, usdc_bank, borrow_ui)
        .await?;

    user.try_bank_repay(user_usdc_borrow.key, usdc_bank, 0.0, Some(true))
        .await?;

    let user_sol_borrow = setup.test_f.sol_mint.create_empty_token_account().await;
    let res = user
        .try_bank_borrow(user_sol_borrow.key, sol_bank, equivalent_sol_borrow_ui)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_position_can_be_liquidated_after_juplend_leverage_tightens(
) -> anyhow::Result<()> {
    let deposit_ui = 108.0;
    let deposit_native = native!(108, "USDC");
    let healthy_init_leverage = 20;
    let healthy_maint_leverage = 21;
    let tightened_init_leverage = 18;
    let tightened_maint_leverage = 19;
    let partial_liquidation_native = 250_000;
    let partial_repay_ui = 0.25;

    let setup = TestFixture::setup_juplend_bank(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    configure_same_asset_pair(
        &setup.test_f,
        setup.test_f.get_bank(&BankMint::Usdc),
        &setup.bank_f,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;

    let usdc_bank = setup.test_f.get_bank(&BankMint::Usdc);

    let lp = setup.test_f.create_marginfi_account().await;
    let lp_usdc = setup
        .test_f
        .usdc_mint
        .create_token_account_and_mint_to(260.0)
        .await;
    lp.try_bank_deposit(lp_usdc.key, usdc_bank, 170.0, None)
        .await?;

    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        setup.test_f.context.clone(),
        &setup.test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let liquidatee_usdc = setup
        .test_f
        .usdc_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), deposit_ui)
        .await;
    {
        let deposit_ix = liquidatee
            .make_juplend_deposit_ix_with_authority(
                liquidatee_usdc.key,
                &setup.bank_f,
                deposit_native,
                liquidatee_authority.pubkey(),
            )
            .await;
        let ctx = setup.test_f.context.borrow_mut();
        let deposit_tx = Transaction::new_signed_with_payer(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                deposit_ix,
            ],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &liquidatee_authority],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(deposit_tx)
            .await?;
    }

    // Deposit = 108 underlying USDC into JupLend.
    // Using the redeemed underlying amount from the accounted shares, the nominal liability window
    // is:
    // - healthy init limit = 108 * 19 / 20 = 102.6 USDC
    // - tightened maint limit = 108 * 18 / 19 ~= 102.315789 USDC
    // Borrow is the midpoint between those two values, so the account starts healthy and becomes
    // liquidatable only after the small 20/21 -> 18/19 tighten.
    let lending = setup.load_lending().await;
    let accounted_shares = setup
        .load_user_accounted_shares(&liquidatee)
        .await
        .expect("juplend collateral should be active after deposit");
    let accounted_underlying = lending
        .expected_assets_for_redeem(accounted_shares)
        .expect("expected assets for redeem should be computable");
    let accounted_underlying_ui = usdc_native_to_ui(accounted_underlying);
    let healthy_init_limit = accounted_underlying_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(healthy_init_leverage),
            FixedI80F48::ONE,
        );
    let tightened_maint_limit = accounted_underlying_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(tightened_maint_leverage),
            FixedI80F48::ONE,
        );
    let borrow_ui = midpoint(healthy_init_limit, tightened_maint_limit).to_num::<f64>();

    let borrow_destination = setup.test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow_with_authority(
            borrow_destination.key,
            usdc_bank,
            borrow_ui,
            0,
            &liquidatee_authority,
        )
        .await?;

    reconfigure_same_asset_leverage(
        &setup.test_f,
        tightened_init_leverage,
        tightened_maint_leverage,
    )
    .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    let payer = setup.test_f.payer().clone();
    let init_ix = liquidatee
        .make_init_liquidation_record_ix(record_pk, payer)
        .await;
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_usdc_acc = setup.test_f.usdc_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_juplend_withdraw_ix(
            liquidator_usdc_acc.key,
            &setup.bank_f,
            partial_liquidation_native,
            Some(false),
        )
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, partial_repay_ui, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            setup.test_f.marginfi_group.fee_state,
            setup.test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    {
        let ctx = setup.test_f.context.borrow_mut();

        let liquidation_tx = Transaction::new_signed_with_payer(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                init_ix,
                start_ix,
                withdraw_ix,
                repay_ix,
                end_ix,
            ],
            Some(&payer),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(liquidation_tx)
            .await?;
    }

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_position_can_be_deleveraged_after_juplend_leverage_tightens(
) -> anyhow::Result<()> {
    let deposit_ui = 99.0;
    let deposit_native = native!(99, "USDC");
    let healthy_init_leverage = 88;
    let healthy_maint_leverage = 91;
    let tightened_init_leverage = 84;
    let tightened_maint_leverage = 87;
    let partial_withdraw_native = native!(10, "USDC");
    let partial_repay_ui = 10.0;

    let setup = TestFixture::setup_juplend_bank(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    configure_same_asset_pair(
        &setup.test_f,
        setup.test_f.get_bank(&BankMint::Usdc),
        &setup.bank_f,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;
    let usdc_bank = setup.test_f.get_bank(&BankMint::Usdc);

    let risk_admin = setup.test_f.payer().clone();

    let lp = setup.test_f.create_marginfi_account().await;
    let lp_usdc_acc = setup
        .test_f
        .usdc_mint
        .create_token_account_and_mint_to(320.0)
        .await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 180.0, None)
        .await?;

    let deleveragee_authority = Keypair::new();
    let deleveragee = MarginfiAccountFixture::new_with_authority(
        setup.test_f.context.clone(),
        &setup.test_f.marginfi_group.key,
        &deleveragee_authority,
    )
    .await;
    let deleveragee_usdc = setup
        .test_f
        .usdc_mint
        .create_token_account_and_mint_to_with_owner(&deleveragee_authority.pubkey(), deposit_ui)
        .await;
    {
        let deposit_ix = deleveragee
            .make_juplend_deposit_ix_with_authority(
                deleveragee_usdc.key,
                &setup.bank_f,
                deposit_native,
                deleveragee_authority.pubkey(),
            )
            .await;
        let ctx = setup.test_f.context.borrow_mut();
        let deposit_tx = Transaction::new_signed_with_payer(
            &[
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                deposit_ix,
            ],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &deleveragee_authority],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(deposit_tx)
            .await?;
    }

    // Deposit = 99 underlying USDC into JupLend.
    // Using the redeemed underlying amount from the accounted shares, the nominal liability window
    // is:
    // - healthy init limit = 99 * 87 / 88 = 97.875 USDC
    // - tightened maint limit = 99 * 86 / 87 ~= 97.862069 USDC
    // Borrow is the midpoint between those two values, so the position is barely healthy before
    // the tighten and barely unhealthy afterward.
    let lending = setup.load_lending().await;
    let accounted_shares = setup
        .load_user_accounted_shares(&deleveragee)
        .await
        .expect("juplend collateral should be active after deposit");
    let accounted_underlying = lending
        .expected_assets_for_redeem(accounted_shares)
        .expect("expected assets for redeem should be computable");
    let accounted_underlying_ui = usdc_native_to_ui(accounted_underlying);
    let healthy_init_limit = accounted_underlying_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(healthy_init_leverage),
            FixedI80F48::ONE,
        );
    let tightened_maint_limit = accounted_underlying_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(tightened_maint_leverage),
            FixedI80F48::ONE,
        );
    let borrow_ui = midpoint(healthy_init_limit, tightened_maint_limit).to_num::<f64>();

    let borrow_destination = setup.test_f.usdc_mint.create_empty_token_account().await;
    deleveragee
        .try_bank_borrow_with_authority(
            borrow_destination.key,
            usdc_bank,
            borrow_ui,
            0,
            &deleveragee_authority,
        )
        .await?;

    reconfigure_same_asset_leverage(
        &setup.test_f,
        tightened_init_leverage,
        tightened_maint_leverage,
    )
    .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );
    let risk_admin_usdc_acc = setup
        .test_f
        .usdc_mint
        .create_token_account_and_mint_to(220.0)
        .await;

    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;
    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;
    let withdraw_ix = deleveragee
        .make_juplend_withdraw_ix(
            risk_admin_usdc_acc.key,
            &setup.bank_f,
            partial_withdraw_native,
            None,
        )
        .await;
    let repay_ix = deleveragee
        .make_repay_ix(risk_admin_usdc_acc.key, usdc_bank, partial_repay_ui, None)
        .await;
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    {
        let ctx = setup.test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[
                init_ix,
                ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
                start_ix,
                withdraw_ix,
                repay_ix,
                end_ix,
            ],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    Ok(())
}
