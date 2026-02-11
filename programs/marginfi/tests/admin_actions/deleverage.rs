use anchor_lang::error::ErrorCode;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_anchor_error, assert_custom_error, assert_eq_noise, native, prelude::*};
use marginfi::state::bank::BankImpl;
use marginfi::{prelude::*, state::marginfi_account::MarginfiAccountImpl};
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{BankConfigOpt, ACCOUNT_IN_RECEIVERSHIP},
};
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, transaction::Transaction};

#[tokio::test]
async fn deleverage_happy_path() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let risk_admin = test_f.payer().clone();
    assert_eq!(risk_admin, test_f.marginfi_group.load().await.admin);

    let lp = test_f.create_marginfi_account().await;
    let deleveragee = test_f.create_marginfi_account().await;

    // Note: Sol is $10, USDC is $1
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // LP provides initial liquidity
    let lp_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Setup deleveragee (after bank has liquidity for them to borrow)
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;

    // * Note: Deposited $30 in SOL, borrowed $20 in USDC
    // * Note: all asset/liab weights in testing are 1, e.g. $30 in SOL = $30 exactly in value
    deleveragee
        .try_bank_deposit(user_token_sol.key, sol_bank, 3.0, None)
        .await?;
    deleveragee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 20.0)
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );

    // Risk admin will withdraw some sol and will repay some usdc
    let risk_admin_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    let risk_admin_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    // Tweak the weights so that the health can improve as part of deleveraging.
    // Note: the (deleveragee) account IS still healthy but forced deleveraging is anyway allowed.
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.7).into()),
                asset_weight_maint: Some(I80F48!(0.8).into()), // ($30 of SOL now worth $24)
                ..Default::default()
            },
            None,
        )
        .await?;

    // Note: deleveraging also (like liquidation) uses liquidation record - to ensure we do not worsen the account health.
    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;

    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;

    // Seize 1.0 * 10 = $10.0 ($8 weighted)
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(risk_admin_sol_acc.key, sol_bank, 1.0, None, true)
        .await;

    // Repay $10
    let repay_ix = deleveragee
        .make_bank_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 10.0, None)
        .await;

    // Health should improve from $4 (24 - 20) to $6 (16 - 10)
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    // Record sol balances before
    let fee_pre = {
        let ctx = test_f.context.borrow_mut();
        let fee_bal = ctx
            .banks_client
            .get_balance(test_f.marginfi_group.fee_wallet)
            .await?;
        fee_bal
    };

    // Send the tx
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let risk_admin_sol_tokens = risk_admin_sol_acc.balance().await;
    assert_eq!(risk_admin_sol_tokens, native!(1.0, "SOL", f64));
    let risk_admin_usdc_tokens = risk_admin_usdc_acc.balance().await;
    assert_eq!(risk_admin_usdc_tokens, native!(190, "USDC"));

    let deleveragee_ma = deleveragee.load().await;
    let sol_bank_state = sol_bank.load().await;
    let usdc_bank_state = usdc_bank.load().await;
    let sol_index = deleveragee_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.bank_pk == sol_bank.key)
        .unwrap();
    let usdc_index = deleveragee_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.bank_pk == usdc_bank.key)
        .unwrap();
    let sol_amount = sol_bank_state.get_asset_amount(
        deleveragee_ma.lending_account.balances[sol_index]
            .asset_shares
            .into(),
    )?;
    let usdc_liab = usdc_bank_state.get_liability_amount(
        deleveragee_ma.lending_account.balances[usdc_index]
            .liability_shares
            .into(),
    )?;

    // 20, in native sol decimals
    assert_eq_noise!(sol_amount, I80F48!(2000000000));
    // 10, in native usdc decimals
    assert_eq_noise!(usdc_liab, I80F48!(10000000));
    // Receivership ends at the end of the tx, we never see the flag enabled
    assert_eq!(deleveragee_ma.get_flag(ACCOUNT_IN_RECEIVERSHIP), false);

    let fee_post = {
        let ctx = test_f.context.borrow_mut();
        let fee_bal = ctx
            .banks_client
            .get_balance(test_f.marginfi_group.fee_wallet)
            .await?;
        fee_bal
    };

    // Note: no fees charged for deleveraging!
    assert_eq!(fee_pre, fee_post);

    // Now make deleveragee unhealthy to ensure it can handle this too
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.4).into()), // ($20 of SOL now worth $8)
                ..Default::default()
            },
            None,
        )
        .await?;

    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;

    // Seize .210 * 10 = $2.10
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(risk_admin_sol_acc.key, sol_bank, 0.210, None, true)
        .await;

    // Repay $2
    let repay_ix = deleveragee
        .make_bank_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 2.0, None)
        .await;

    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    // Send the tx
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[
                /*init_ix - no need for this already*/ start_ix,
                withdraw_ix,
                repay_ix,
                end_ix,
            ],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let risk_admin_sol_tokens = risk_admin_sol_acc.balance().await;
    assert_eq!(risk_admin_sol_tokens, native!(1.21, "SOL", f64));
    let risk_admin_usdc_tokens = risk_admin_usdc_acc.balance().await;
    assert_eq!(risk_admin_usdc_tokens, native!(188, "USDC"));

    let deleveragee_ma = deleveragee.load().await;
    let sol_bank_state = sol_bank.load().await;
    let usdc_bank_state = usdc_bank.load().await;
    let sol_amount = sol_bank_state.get_asset_amount(
        deleveragee_ma.lending_account.balances[sol_index]
            .asset_shares
            .into(),
    )?;
    let usdc_liab = usdc_bank_state.get_liability_amount(
        deleveragee_ma.lending_account.balances[usdc_index]
            .liability_shares
            .into(),
    )?;
    // 20 - 2.10, in native sol decimals
    assert_eq_noise!(sol_amount, I80F48!(1790000000));
    // 10 - 2, in native usdc decimals
    assert_eq_noise!(usdc_liab, I80F48!(8000000));

    Ok(())
}

#[tokio::test]
async fn deleverage_tokenless_up_to_limit() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let risk_admin = test_f.payer().clone();
    assert_eq!(risk_admin, test_f.marginfi_group.load().await.risk_admin);

    let lp = test_f.create_marginfi_account().await;
    let deleveragee = test_f.create_marginfi_account().await;

    // Note: Sol is $10, USDC is $1
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let res = test_f
        .marginfi_group
        .try_update_deleverage_withdrawal_limit(0)
        .await; // try to set the zero limit -> fails
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::ZeroWithdrawalLimit);

    // LP provides initial liquidity
    let lp_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Setup deleveragee (after bank has liquidity for them to borrow)
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;

    // * Note: Deposited $30 in SOL, borrowed $20 in USDC
    // * Note: all asset/liab weights in testing are 1, e.g. $30 in SOL = $30 exactly in value
    deleveragee
        .try_bank_deposit(user_token_sol.key, sol_bank, 3.0, None)
        .await?;
    deleveragee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 20.0)
        .await?;

    test_f
        .marginfi_group
        .try_update_deleverage_withdrawal_limit(10)
        .await?; // cap the deleverages by $10
    test_f
        .marginfi_group
        .try_lending_pool_configure_bank(
            usdc_bank,
            BankConfigOpt {
                tokenless_repayments_allowed: Some(true), // enable repayments with no USDC on the account
                ..BankConfigOpt::default()
            },
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );

    // Risk admin will withdraw some sol and will "repay" some usdc (in fact there is no usdc on the ATA, so it will be tokenless)
    let risk_admin_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(10).await;
    let risk_admin_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    // Note: deleveraging also (like liquidation) uses liquidation record - to ensure we do not worsen the account health.
    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;

    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;

    // Seize 1.0 * 10 = $10.0
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(risk_admin_sol_acc.key, sol_bank, 1.0, None, true)
        .await;

    // Repay $10 (even though tokenless repayments are allowed, if withdraw_all is false, we will repay WITH tokens)
    let repay_ix = deleveragee
        .make_bank_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 10.0, None)
        .await;

    // Health should not change: from $4 (24 - 20) to $4 (14 - 10)
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    // Send the tx
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let risk_admin_sol_tokens = risk_admin_sol_acc.balance().await;
    assert_eq!(risk_admin_sol_tokens, native!(1.0, "SOL", f64));
    let risk_admin_usdc_tokens = risk_admin_usdc_acc.balance().await;
    assert_eq!(risk_admin_usdc_tokens, native!(0, "USDC"));

    // Now let's try to seize more (and thus "repay" more) and see it failing due to daily withdrawal limit reach
    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;

    // Seize 1.0 * 10 = $10.0
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(risk_admin_sol_acc.key, sol_bank, 1.0, None, true)
        .await;

    // "Repay" $10 with nothing
    let repay_ix = deleveragee
        .make_bank_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 10.0, None)
        .await;

    // Health should not have changed: from $4 (14 - 10) to $4 (4 - 0)
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix, withdraw_ix, repay_ix, end_ix],
        Some(&risk_admin),
        &[&ctx.payer],
        ctx.last_blockhash,
    );

    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;

    assert!(res.is_err());
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::DailyWithdrawalLimitExceeded
    );

    Ok(())
}

#[tokio::test]
async fn deleverage_cannot_worsen_health() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let risk_admin = test_f.payer().clone();
    assert_eq!(risk_admin, test_f.marginfi_group.load().await.risk_admin);

    let lp = test_f.create_marginfi_account().await;
    let deleveragee = test_f.create_marginfi_account().await;

    // Note: Sol is $10, USDC is $1
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // LP provides initial liquidity
    let lp_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Setup deleveragee (after bank has liquidity for them to borrow)
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;

    // * Note: Deposited $20 in SOL, borrowed $10 in USDC
    // * Note: all asset/liab weights in testing are 1, e.g. $20 in SOL = $20 exactly in value
    deleveragee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;
    deleveragee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    // Risk admin will (try to) withdraw some sol and will (try to) repay some usdc
    let risk_admin_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    let risk_admin_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );

    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;

    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;

    // Seize 1.2 * 10 = $12
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(risk_admin_sol_acc.key, sol_bank, 1.2, None, true)
        .await;

    // Repay $10
    let repay_ix = deleveragee
        .make_bank_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 10.0, None)
        .await;

    // Health decreases: $10 (20 - 10) -> $8 (8 - 0)
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
        Some(&risk_admin),
        &[&ctx.payer],
        ctx.last_blockhash,
    );

    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::WorseHealthPostLiquidation);

    Ok(())
}

#[tokio::test]
async fn deleverage_not_risk_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let risk_admin = test_f.payer().clone();
    assert_eq!(risk_admin, test_f.marginfi_group.load().await.admin);

    let group = test_f.marginfi_group.load().await;
    // Change risk_admin for the group so that the current one loses permission to deleverage
    let res = test_f
        .marginfi_group
        .try_update(
            group.admin,
            group.emode_admin,
            group.delegate_curve_admin,
            group.delegate_limit_admin,
            group.delegate_emissions_admin,
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        )
        .await;
    assert!(res.is_ok());

    let lp = test_f.create_marginfi_account().await;
    let deleveragee = test_f.create_marginfi_account().await;

    // Note: Sol is $10, USDC is $1
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // LP provides initial liquidity
    let lp_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Setup deleveragee (after bank has liquidity for them to borrow)
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;

    // * Note: Deposited $20 in SOL, borrowed $10 in USDC
    // * Note: all asset/liab weights in testing are 1, e.g. $20 in SOL = $20 exactly in value
    deleveragee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;
    deleveragee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    // Payer will (try to) withdraw some sol and will (try to) repay some usdc
    let payer_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    let payer_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );

    let payer = test_f.payer();
    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, payer)
        .await;

    let start_ix = deleveragee.make_start_deleverage_ix(record_pk, payer).await;

    // Seize 0.8 * 10 = $8
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(payer_sol_acc.key, sol_bank, 0.8, None, true)
        .await;

    // Repay $10
    let repay_ix = deleveragee
        .make_bank_repay_ix(payer_usdc_acc.key, usdc_bank, 10.0, None)
        .await;

    // Health decreases: $10 (20 - 10) -> $12 (12 - 0)
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, payer, vec![])
        .await;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
        Some(&payer),
        &[&ctx.payer],
        ctx.last_blockhash,
    );

    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;

    assert!(res.is_err());
    assert_anchor_error!(res.unwrap_err(), ErrorCode::ConstraintHasOne); // non-risk-admin trying to deleverage

    Ok(())
}

#[tokio::test]
async fn deleverage_rejects_zero_weight_asset() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let lp = test_f.create_marginfi_account().await;
    let deleveragee = test_f.create_marginfi_account().await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let lp_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    deleveragee
        .try_bank_deposit(user_token_sol.key, sol_bank, 1.0, None)
        .await?;
    deleveragee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48::ZERO.into()),
                asset_weight_maint: Some(I80F48::ZERO.into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let risk_admin = test_f.payer().clone();
    let risk_admin_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let risk_admin_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );

    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;

    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;

    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(risk_admin_sol_acc.key, sol_bank, 0.1, None, true)
        .await;

    let repay_ix = deleveragee
        .make_bank_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 2.0, None)
        .await;

    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![])
        .await;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
        Some(&risk_admin),
        &[&ctx.payer],
        ctx.last_blockhash,
    );
    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationPremiumTooHigh);

    Ok(())
}

#[tokio::test]
async fn deleverage_can_close_out_balances() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let lp = test_f.create_marginfi_account().await;
    let deleveragee = test_f.create_marginfi_account().await;
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let lp_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(lp_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_pyusd = test_f.pyusd_mint.create_token_account_and_mint_to(10).await;
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;

    deleveragee
        .try_bank_deposit(user_token_pyusd.key, pyusd_bank, 10.0, None)
        .await?;
    deleveragee
        .try_bank_deposit(user_token_sol.key, sol_bank, 1.0, None)
        .await?;
    deleveragee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    // Tweak weights so that deleveraging can improve health
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.7).into()),
                asset_weight_maint: Some(I80F48!(0.8).into()), // ($10 of SOL now worth $8)
                ..Default::default()
            },
            None,
        )
        .await?;

    let risk_admin = test_f.payer().clone();
    let risk_admin_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let risk_admin_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), deleveragee.key.as_ref()],
        &marginfi::ID,
    );

    let init_ix = deleveragee
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;

    let start_ix = deleveragee
        .make_start_deleverage_ix(record_pk, risk_admin)
        .await;

    // NOTE: In deleveraging, you MUST PASS the oracle for the withdrawn asset even for
    // a withdraw-all. The entire balance is still withdrawn!
    let withdraw_ix = deleveragee
        .make_bank_withdraw_ix(risk_admin_sol_acc.key, sol_bank, 0.0, Some(true), true)
        .await;

    // The entire liability
    let repay_ix = deleveragee
        .make_bank_repay_ix(risk_admin_usdc_acc.key, usdc_bank, 0.0, Some(true))
        .await;

    // Health should improve from $8 (18 - 10) to $10 (10 - 0)
    let end_ix = deleveragee
        .make_end_deleverage_ix(record_pk, risk_admin, vec![usdc_bank.key, sol_bank.key])
        .await;

    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix, start_ix, withdraw_ix, repay_ix, end_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_ok());
    }

    // Participating balances were closed.
    let marginfi_account = deleveragee.load().await;
    let active_balance_count = marginfi_account
        .lending_account
        .get_active_balances_iter()
        .count();

    assert_eq!(1, active_balance_count);

    // The only active one is the deposit in PyUSD
    let active_position = marginfi_account.lending_account.balances.first().unwrap();
    assert_eq!(active_position.bank_pk, pyusd_bank.key);

    Ok(())
}
