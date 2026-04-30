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
async fn same_asset_emode_kamino_same_mint_position_is_healthy_then_turns_unhealthy_when_leverage_tightens(
) -> anyhow::Result<()> {
    let deposit_ui = 83.0;
    let deposit_native = native!(83, "USDC");
    let healthy_init_leverage = 6;
    let healthy_maint_leverage = 7;
    let tightened_init_leverage = 4;
    let tightened_maint_leverage = 5;

    let setup = TestFixture::setup_kamino_bank(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    let usdc_bank = add_same_asset_regular_kamino_bank(&setup).await?;
    configure_same_asset_pair(
        &setup.test_f,
        &usdc_bank,
        &setup.bank_f,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;

    let lp = setup.test_f.create_marginfi_account().await;
    let lp_usdc = usdc_bank.mint.create_token_account_and_mint_to(220.0).await;
    lp.try_bank_deposit(lp_usdc.key, &usdc_bank, 220.0, None)
        .await?;

    let reserve_before_deposit = setup.load_reserve().await;
    let (user, user_usdc) = setup.create_user_with_liquidity(deposit_ui).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_usdc.key, deposit_native)
        .await?;
    let expected_collateral_native =
        reserve_before_deposit.liquidity_to_collateral(deposit_native)?;
    let accounted_collateral_native = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino collateral should be active after deposit");
    assert_eq_with_tolerance!(
        accounted_collateral_native as i128,
        expected_collateral_native as i128,
        KAMINO_ROUNDING_TOLERANCE_NATIVE
    );

    // Deposit = 83 underlying USDC into Kamino.
    // Kamino stores the position in collateral-token units, but health prices those units through
    // the reserve exchange rate, so `accounted_underlying_ui` is the effective underlying USDC
    // collateral value seen by the risk engine.
    // On the nominal 83 USDC deposit:
    // - healthy init weight = 1 - 1/6 = 5 / 6 ~= 0.833333, so the healthy init limit is
    //   83 * 5 / 6 ~= 69.166667 USDC
    // - tightened maint weight = 1 - 1/5 = 4 / 5 = 0.8, so the tightened maint limit is
    //   83 * 4 / 5 = 66.4 USDC
    let reserve_after_deposit = setup.load_reserve().await;
    let accounted_underlying_ui = usdc_native_to_ui(
        reserve_after_deposit.collateral_to_liquidity(accounted_collateral_native)?,
    );
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

    refresh_same_asset_kamino_position(&setup, &user).await?;
    let borrow_destination = usdc_bank.mint.create_empty_token_account().await;
    user.try_bank_borrow(borrow_destination.key, &usdc_bank, borrow_ui)
        .await?;

    reconfigure_same_asset_leverage(
        &setup.test_f,
        tightened_init_leverage,
        tightened_maint_leverage,
    )
    .await?;
    refresh_same_asset_kamino_position(&setup, &user).await?;
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
async fn same_asset_emode_kamino_same_value_borrow_fails_once_the_liability_mint_changes(
) -> anyhow::Result<()> {
    let deposit_ui = 91.0;
    let deposit_native = native!(91, "USDC");
    let same_asset_init_leverage = 6;
    let same_asset_maint_leverage = 7;

    let setup = TestFixture::setup_kamino_bank(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Fixed,
                config: None,
            },
        ],
        protocol_fees: false,
    }))
    .await;
    let usdc_bank = add_same_asset_regular_kamino_bank(&setup).await?;
    configure_same_asset_pair(
        &setup.test_f,
        &usdc_bank,
        &setup.bank_f,
        0.5,
        0.5,
        same_asset_init_leverage,
        same_asset_maint_leverage,
    )
    .await?;
    let fixed_bank = setup.test_f.get_bank(&BankMint::Fixed);

    let lp_usdc_account = setup.test_f.create_marginfi_account().await;
    let lp_usdc = usdc_bank.mint.create_token_account_and_mint_to(240.0).await;
    lp_usdc_account
        .try_bank_deposit(lp_usdc.key, &usdc_bank, 240.0, None)
        .await?;

    let lp_fixed_account = setup.test_f.create_marginfi_account().await;
    let lp_fixed = setup
        .test_f
        .fixed_mint
        .create_token_account_and_mint_to(120.0)
        .await;
    lp_fixed_account
        .try_bank_deposit(lp_fixed.key, fixed_bank, 120.0, None)
        .await?;

    let reserve_before_deposit = setup.load_reserve().await;
    let (user, user_usdc) = setup.create_user_with_liquidity(deposit_ui).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_usdc.key, deposit_native)
        .await?;
    let expected_collateral_native =
        reserve_before_deposit.liquidity_to_collateral(deposit_native)?;
    let accounted_collateral_native = setup
        .load_user_accounted_collateral(&user)
        .await
        .expect("kamino collateral should be active after deposit");
    assert_eq_with_tolerance!(
        accounted_collateral_native as i128,
        expected_collateral_native as i128,
        KAMINO_ROUNDING_TOLERANCE_NATIVE
    );

    // Deposit = 91 underlying USDC into Kamino.
    // Convert the stored collateral-token balance back to effective underlying USDC before
    // computing the borrow limits.
    // On the nominal 91 USDC deposit:
    // - same-asset init weight = 1 - 1/6 = 5 / 6 ~= 0.833333, so the same-mint USDC limit is
    //   91 * 5 / 6 ~= 75.833333 USDC
    // - plain regular limit after the liability mint changes is only 91 * 0.5 = 45.5 USDC
    // Borrow is placed halfway between those limits.
    // FIXED is priced at $2, so the equal-valued FIXED borrow is about half the USDC UI amount.
    // Same-mint borrowing succeeds, while the same-value FIXED borrow fails once the same-asset
    // lift disappears.
    let reserve_after_deposit = setup.load_reserve().await;
    let accounted_underlying_ui = usdc_native_to_ui(
        reserve_after_deposit.collateral_to_liquidity(accounted_collateral_native)?,
    );
    let same_asset_limit = accounted_underlying_ui
        * compute_same_asset_emode_weight(
            FixedI80F48::from_num(same_asset_init_leverage),
            FixedI80F48::ONE,
        );
    let regular_limit = accounted_underlying_ui * FixedI80F48::from_num(0.5);
    let borrow_ui = midpoint(same_asset_limit, regular_limit).to_num::<f64>();
    let equivalent_fixed_borrow_ui =
        (FixedI80F48::from_num(borrow_ui) / FixedI80F48::from_num(2)).to_num::<f64>();

    refresh_same_asset_kamino_position(&setup, &user).await?;
    let user_usdc_borrow = usdc_bank.mint.create_empty_token_account().await;
    user.try_bank_borrow(user_usdc_borrow.key, &usdc_bank, borrow_ui)
        .await?;

    user.try_bank_repay(user_usdc_borrow.key, &usdc_bank, 0.0, Some(true))
        .await?;

    let user_fixed_borrow = setup.test_f.fixed_mint.create_empty_token_account().await;
    let res = user
        .try_bank_borrow(
            user_fixed_borrow.key,
            fixed_bank,
            equivalent_fixed_borrow_ui,
        )
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    Ok(())
}

#[tokio::test]
async fn same_asset_emode_position_can_be_liquidated_after_kamino_leverage_tightens(
) -> anyhow::Result<()> {
    let deposit_ui = 104.0;
    let deposit_native = native!(104, "USDC");
    let healthy_init_leverage = 6;
    let healthy_maint_leverage = 7;
    let tightened_init_leverage = 4;
    let tightened_maint_leverage = 5;
    let partial_withdraw_native = 50_000;
    let partial_repay_ui = 0.06;

    let setup = TestFixture::setup_kamino_bank(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    let usdc_bank = add_same_asset_regular_kamino_bank(&setup).await?;
    configure_same_asset_pair(
        &setup.test_f,
        &usdc_bank,
        &setup.bank_f,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;

    let lp = setup.test_f.create_marginfi_account().await;
    let lp_usdc = usdc_bank.mint.create_token_account_and_mint_to(260.0).await;
    lp.try_bank_deposit(lp_usdc.key, &usdc_bank, 170.0, None)
        .await?;

    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        setup.test_f.context.clone(),
        &setup.test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let liquidatee_usdc = setup
        .bank_f
        .mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), deposit_ui)
        .await;
    {
        let refresh_reserve_ix = liquidatee
            .make_kamino_refresh_reserve_ix(&setup.bank_f)
            .await;
        let refresh_obligation_ix = liquidatee
            .make_kamino_refresh_obligation_ix(&setup.bank_f)
            .await;
        let deposit_ix = liquidatee
            .make_kamino_deposit_ix_with_authority(
                liquidatee_usdc.key,
                &setup.bank_f,
                deposit_native,
                liquidatee_authority.pubkey(),
            )
            .await;
        let ctx = setup.test_f.context.borrow_mut();
        let deposit_tx = Transaction::new_signed_with_payer(
            &[refresh_reserve_ix, refresh_obligation_ix, deposit_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &liquidatee_authority],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(deposit_tx)
            .await?;
    }

    // Deposit = 104 underlying USDC through Kamino.
    // The liquidatee balance is stored in Kamino collateral-token units, so convert it back to
    // effective underlying USDC before computing the liability window.
    // On the nominal 104 USDC deposit:
    // - healthy init limit = 104 * 5 / 6 ~= 86.666667 USDC
    // - tightened maint limit = 104 * 4 / 5 = 83.2 USDC
    // Liquidation becomes valid only after the small 6/7 -> 4/5 tighten.
    let accounted_collateral_native = setup
        .load_user_accounted_collateral(&liquidatee)
        .await
        .expect("kamino collateral should be active after deposit");
    let reserve_after_deposit = setup.load_reserve().await;
    let accounted_underlying_ui = usdc_native_to_ui(
        reserve_after_deposit.collateral_to_liquidity(accounted_collateral_native)?,
    );
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

    refresh_same_asset_kamino_position(&setup, &liquidatee).await?;
    let borrow_destination = usdc_bank.mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow_with_authority(
            borrow_destination.key,
            &usdc_bank,
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
    let refresh_reserve_ix = liquidatee
        .make_kamino_refresh_reserve_ix(&setup.bank_f)
        .await;
    let refresh_obligation_ix = liquidatee
        .make_kamino_refresh_obligation_ix(&setup.bank_f)
        .await;
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_usdc_acc = usdc_bank.mint.create_token_account_and_mint_to(10.0).await;
    let withdraw_ix = liquidatee
        .make_kamino_withdraw_ix(
            liquidator_usdc_acc.key,
            &setup.bank_f,
            partial_withdraw_native,
            Some(false),
        )
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, &usdc_bank, partial_repay_ui, None)
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
                refresh_reserve_ix,
                refresh_obligation_ix,
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
async fn same_asset_emode_position_can_be_deleveraged_after_kamino_leverage_tightens(
) -> anyhow::Result<()> {
    let deposit_ui = 96.0;
    let deposit_native = native!(96, "USDC");
    let healthy_init_leverage = 6;
    let healthy_maint_leverage = 7;
    let tightened_init_leverage = 4;
    let tightened_maint_leverage = 5;
    let partial_withdraw_native = 50_000;
    let partial_repay_ui = 0.06;

    let setup = TestFixture::setup_kamino_bank(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;
    let usdc_bank = add_same_asset_regular_kamino_bank(&setup).await?;
    configure_same_asset_pair(
        &setup.test_f,
        &usdc_bank,
        &setup.bank_f,
        0.5,
        0.5,
        healthy_init_leverage,
        healthy_maint_leverage,
    )
    .await?;

    let risk_admin = setup.test_f.payer().clone();

    let lp = setup.test_f.create_marginfi_account().await;
    let lp_usdc_acc = usdc_bank.mint.create_token_account_and_mint_to(320.0).await;
    lp.try_bank_deposit(lp_usdc_acc.key, &usdc_bank, 180.0, None)
        .await?;

    let deleveragee_authority = Keypair::new();
    let deleveragee = MarginfiAccountFixture::new_with_authority(
        setup.test_f.context.clone(),
        &setup.test_f.marginfi_group.key,
        &deleveragee_authority,
    )
    .await;
    let deleveragee_usdc = setup
        .bank_f
        .mint
        .create_token_account_and_mint_to_with_owner(&deleveragee_authority.pubkey(), deposit_ui)
        .await;
    {
        let refresh_reserve_ix = deleveragee
            .make_kamino_refresh_reserve_ix(&setup.bank_f)
            .await;
        let refresh_obligation_ix = deleveragee
            .make_kamino_refresh_obligation_ix(&setup.bank_f)
            .await;
        let deposit_ix = deleveragee
            .make_kamino_deposit_ix_with_authority(
                deleveragee_usdc.key,
                &setup.bank_f,
                deposit_native,
                deleveragee_authority.pubkey(),
            )
            .await;
        let ctx = setup.test_f.context.borrow_mut();
        let deposit_tx = Transaction::new_signed_with_payer(
            &[refresh_reserve_ix, refresh_obligation_ix, deposit_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &deleveragee_authority],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(deposit_tx)
            .await?;
    }

    // Deposit = 96 underlying USDC through Kamino.
    // The deleveragee balance is stored in Kamino collateral-token units, so convert it back to
    // effective underlying USDC before computing the liability window.
    // On the nominal 96 USDC deposit:
    // - healthy init limit = 96 * 5 / 6 = 80 USDC
    // - tightened maint limit = 96 * 4 / 5 = 76.8 USDC
    let accounted_collateral_native = setup
        .load_user_accounted_collateral(&deleveragee)
        .await
        .expect("kamino collateral should be active after deposit");
    let reserve_after_deposit = setup.load_reserve().await;
    let accounted_underlying_ui = usdc_native_to_ui(
        reserve_after_deposit.collateral_to_liquidity(accounted_collateral_native)?,
    );
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

    refresh_same_asset_kamino_position(&setup, &deleveragee).await?;
    let borrow_destination = usdc_bank.mint.create_empty_token_account().await;
    deleveragee
        .try_bank_borrow_with_authority(
            borrow_destination.key,
            &usdc_bank,
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
        .bank_f
        .mint
        .create_token_account_and_mint_to(220.0)
        .await;

    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;
    let refresh_reserve_ix = deleveragee
        .make_kamino_refresh_reserve_ix(&setup.bank_f)
        .await;
    let refresh_obligation_ix = deleveragee
        .make_kamino_refresh_obligation_ix(&setup.bank_f)
        .await;
    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;
    let withdraw_ix = deleveragee
        .make_kamino_withdraw_ix(
            risk_admin_usdc_acc.key,
            &setup.bank_f,
            partial_withdraw_native,
            None,
        )
        .await;
    let repay_ix = deleveragee
        .make_repay_ix(risk_admin_usdc_acc.key, &usdc_bank, partial_repay_ui, None)
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
                refresh_reserve_ix,
                refresh_obligation_ix,
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
