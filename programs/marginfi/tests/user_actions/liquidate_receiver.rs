use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, assert_eq_noise, native, prelude::*};
use marginfi::state::bank::BankImpl;
use marginfi::{
    constants::LIQUIDATION_FLAT_FEE_DEFAULT, prelude::*,
    state::marginfi_account::MarginfiAccountImpl,
};
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{BankConfigOpt, ACCOUNT_IN_RECEIVERSHIP},
};
use solana_program_test::*;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, pubkey::Pubkey, signer::Signer,
    transaction::Transaction,
};

#[tokio::test]
async fn liquidate_start_fails_on_healthy_account() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let user = test_f.create_marginfi_account().await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let user_token_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    user.try_bank_deposit(user_token_account.key, usdc_bank, 100, None)
        .await?;
    let payer = test_f.context.borrow().payer.pubkey();

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), user.key.as_ref()],
        &marginfi::ID,
    );

    let init_ix = user.make_init_liquidation_record_ix(record_pk, payer).await;
    let start_ix = user.make_start_liquidation_ix(record_pk, payer).await;
    let end_ix = user
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    let ctx = test_f.context.borrow_mut();
    let init_tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.last_blockhash,
    );
    ctx.banks_client
        .process_transaction_with_preflight(init_tx)
        .await?;

    // Liquidation on a healthy account should fail
    let start_tx = Transaction::new_signed_with_payer(
        &[start_ix, end_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.last_blockhash,
    );
    let res = ctx
        .banks_client
        .process_transaction_with_preflight(start_tx)
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::HealthyAccount);
    Ok(())
}

// Note: You cannot have any instructions (except init, compute budget or kamino refreshes) before the start instruction.
// This means the liquidator must either pre-configure anything they need to complete the tx or finish it
// all between start and end!
#[tokio::test]
async fn liquidate_start_must_be_first() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // liquidator setup doesn't really matter here, but we demonstrate that you cannot do any ix
    // before start_liquidate, even something innocuous as here with deposit.
    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;
    let payer = test_f.context.borrow().payer.pubkey().clone();

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // A pointless deposit to the liquidator so the liquidatee has collateral to borrow...
    let liq_usdc_account = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liq_usdc_account.key, usdc_bank, 99.0, None)
        .await?;

    // Set up an unhealthy liquidatee...
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.001).into()),
                asset_weight_maint: Some(I80F48!(0.002).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );

    let init_ix = liquidatee
        .make_init_liquidation_record_ix(record_pk, payer)
        .await;
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    // The init can happen in its own tx...
    {
        let ctx = test_f.context.borrow_mut();
        let init_tx = Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&payer),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client.process_transaction(init_tx).await?;
    } //release borrow of ctx

    // Deposit ix is forbidden
    {
        // Sneaky Sneaky...
        let deposit_ix = liquidator
            .make_bank_deposit_ix(liq_usdc_account.key, usdc_bank, 1.0, None)
            .await;

        let ctx = test_f.context.borrow_mut();

        let tx = Transaction::new_signed_with_payer(
            &[deposit_ix, start_ix.clone(), end_ix.clone()],
            Some(&payer),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_err());
        assert_custom_error!(res.unwrap_err(), MarginfiError::StartNotFirst);
    } // drop borrow of ctx

    // Start twice is forbidden
    {
        let ctx = test_f.context.borrow_mut();

        let tx = Transaction::new_signed_with_payer(
            &[start_ix.clone(), start_ix.clone(), end_ix.clone()],
            Some(&payer),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_err());
        assert_custom_error!(res.unwrap_err(), MarginfiError::StartRepeats);
    } // drop borrow of ctx

    let kamino_usdc_bank = test_f.get_bank(&BankMint::KaminoUsdc);

    // Impostor Kamino refreshes are forbidden
    {
        let mut fake_kamino_refresh_ix = liquidator
            .make_kamino_refresh_reserve_ix(kamino_usdc_bank)
            .await;
        // Sneaky Sneaky...
        fake_kamino_refresh_ix.program_id = FAKE_KAMINO_PROGRAM_ID;

        let ctx = test_f.context.borrow_mut();

        let tx = Transaction::new_signed_with_payer(
            &[fake_kamino_refresh_ix, start_ix.clone(), end_ix.clone()],
            Some(&payer),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_err());
        assert_custom_error!(res.unwrap_err(), MarginfiError::ForbiddenIx);
    } // drop borrow of ctx

    // Compute budget ix and genuine Kamino refreshes ARE permitted before start
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);
    // Note: we allow any kamino refreshes, not necessarily the ones for the banks participating in liquidation
    let kamino_refresh_reserve_ix = liquidator
        .make_kamino_refresh_reserve_ix(kamino_usdc_bank)
        .await;
    let kamino_refresh_obligation_ix = liquidator
        .make_kamino_refresh_obligation_ix(kamino_usdc_bank)
        .await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.105, None, true)
        .await;
    let repay_ix = liquidatee
        .make_bank_repay_ix(liq_usdc_account.key, usdc_bank, 2.0, None)
        .await;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[
            compute_ix,
            kamino_refresh_reserve_ix,
            kamino_refresh_obligation_ix,
            start_ix,
            repay_ix,
            withdraw_ix,
            end_ix,
        ],
        Some(&payer),
        &[&ctx.payer],
        ctx.last_blockhash,
    );
    ctx.banks_client
        .process_transaction_with_preflight(tx)
        .await?;
    Ok(())
}

// End must be last within the tx
#[tokio::test]
async fn liquidate_end_missing_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;
    let payer = test_f.context.borrow().payer.pubkey().clone();

    // A pointless deposit to the liquidator so the liquidatee has collateral to borrow...
    let liq_token_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    liquidator
        .try_bank_deposit(liq_token_account.key, usdc_bank, 99.0, None)
        .await?;

    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(1).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 1.0, None)
        .await?;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 1.0)
        .await?;

    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.001).into()),
                asset_weight_maint: Some(I80F48!(0.002).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    let init_ix = liquidatee
        .make_init_liquidation_record_ix(record_pk, payer)
        .await;
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer, // Note: payer must sign to pay the sol fee
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    // Missing end ix fails
    {
        let ctx = test_f.context.borrow_mut();
        let init_tx = Transaction::new_signed_with_payer(
            &[init_ix.clone()],
            Some(&payer),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;

        let tx = Transaction::new_signed_with_payer(
            &[start_ix.clone()],
            Some(&payer),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_err());
        assert_custom_error!(res.unwrap_err(), MarginfiError::EndNotLast);
    } // release borrow of ctx

    // Having other ixes after end also fails, it must actually be last.
    {
        let ctx = test_f.context.borrow_mut();
        let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);

        let tx = Transaction::new_signed_with_payer(
            &[start_ix.clone(), end_ix.clone(), compute_ix],
            Some(&payer),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_err());
        assert_custom_error!(res.unwrap_err(), MarginfiError::EndNotLast);
    }
    Ok(())
}

#[tokio::test]
async fn liquidate_with_forbidden_ix_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;
    let payer = test_f.context.borrow().payer.pubkey().clone();

    // A pointless deposit to the liquidator so the liquidatee has collateral to borrow...
    let liq_token_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    liquidator
        .try_bank_deposit(liq_token_account.key, usdc_bank, 99.0, None)
        .await?;

    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(1).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 1.0, None)
        .await?;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 1.0)
        .await?;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.001).into()),
                asset_weight_maint: Some(I80F48!(0.002).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    let init_ix = liquidatee
        .make_init_liquidation_record_ix(record_pk, payer)
        .await;
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    // Sneaky sneaky...
    let forbidden_deposit_ix = liquidator
        .make_bank_deposit_ix(liq_token_account.key, usdc_bank, 1.0, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    let ctx = test_f.context.borrow_mut();
    let init_tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.last_blockhash,
    );
    ctx.banks_client
        .process_transaction_with_preflight(init_tx)
        .await?;

    let tx = Transaction::new_signed_with_payer(
        &[start_ix, forbidden_deposit_ix, end_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.last_blockhash,
    );
    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::ForbiddenIx);
    Ok(())
}

#[tokio::test]
async fn liquidate_receiver_happy_path() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;

    // Note: Sol is $10, USDC is $1
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // liquidator provides initial liquidity and keeps some for repayment
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // setup liquidatee (after bank has liquidity for them to borrow)
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    // * Note: Deposited $20 in SOL, borrowed $10 in USDC
    // * Note: all asset/liab weights in testing are 1, e.g. $20 in SOL = $20 exactly in value
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    // make liquidatee unhealthy ($20 of SOL now worth $8)
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.4).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );

    {
        let ctx = test_f.context.borrow_mut();
        let init_ix = liquidatee
            .make_init_liquidation_record_ix(record_pk, ctx.payer.pubkey())
            .await;
        let init_tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    } // release borrow of test_f via ctx

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    // withdraw some sol to the liquidator and repay some usdc
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    // Seize .210 * 10 = $2.10
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.210, None, true)
        .await;
    // Repay $2
    let repay_ix = liquidatee
        .make_bank_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer, // Note: payer must sign to pay the sol fee
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    // record sol balances before liquidation
    let (payer_pre, fee_pre) = {
        let ctx = test_f.context.borrow_mut();
        let payer_bal = ctx.banks_client.get_balance(payer).await?;
        let fee_bal = ctx
            .banks_client
            .get_balance(test_f.marginfi_group.fee_wallet)
            .await?;
        (payer_bal, fee_bal)
    };

    // send the tx
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[start_ix, withdraw_ix, repay_ix, end_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    } // release borrow of test_f via ctx

    let liquidator_sol_tokens = liquidator_sol_acc.balance().await;
    assert_eq!(liquidator_sol_tokens, native!(0.210, "SOL", f64));
    let liquidator_usdc_tokens = liquidator_usdc_acc.balance().await;
    assert_eq!(liquidator_usdc_tokens, native!(98, "USDC"));

    let liquidatee_ma = liquidatee.load().await;
    let sol_bank_state = sol_bank.load().await;
    let usdc_bank_state = usdc_bank.load().await;
    let sol_index = liquidatee_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.bank_pk == sol_bank.key)
        .unwrap();
    let usdc_index = liquidatee_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.bank_pk == usdc_bank.key)
        .unwrap();
    let sol_amount = sol_bank_state.get_asset_amount(
        liquidatee_ma.lending_account.balances[sol_index]
            .asset_shares
            .into(),
    )?;
    let usdc_liab = usdc_bank_state.get_liability_amount(
        liquidatee_ma.lending_account.balances[usdc_index]
            .liability_shares
            .into(),
    )?;
    // 20 - 2.10, in native sol decimals
    assert_eq_noise!(sol_amount, I80F48!(1790000000));
    // 10 - 2, in native usdc decimals
    assert_eq_noise!(usdc_liab, I80F48!(8000000));
    // receivership ends at the end of the tx, we never see the flag enabled
    assert_eq!(liquidatee_ma.get_flag(ACCOUNT_IN_RECEIVERSHIP), false);

    let (payer_post, fee_post) = {
        let ctx = test_f.context.borrow_mut();
        let payer_bal = ctx.banks_client.get_balance(payer).await?;
        let fee_bal = ctx
            .banks_client
            .get_balance(test_f.marginfi_group.fee_wallet)
            .await?;
        (payer_bal, fee_bal)
    };
    // Note: 5000 lamps is the flat tx fee, this wallet also pays the tx fee in this test, in
    // practice this would not typically be the case.
    assert_eq!(
        payer_pre - payer_post,
        LIQUIDATION_FLAT_FEE_DEFAULT as u64 + 5000
    );
    assert_eq!(fee_post - fee_pre, LIQUIDATION_FLAT_FEE_DEFAULT as u64);
    Ok(())
}

// Here liquidator tries to seize more than the permitted premium, and should fail
#[tokio::test]
async fn liquidate_receiver_premium_too_high() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.4).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    {
        let ctx = test_f.context.borrow_mut();
        let init_ix = liquidatee
            .make_init_liquidation_record_ix(record_pk, ctx.payer.pubkey())
            .await;
        let init_tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    // .3 * 10 = $3
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.3, None, true)
        .await;
    // $2
    let repay_ix = liquidatee
        .make_bank_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix, withdraw_ix, repay_ix, end_ix],
        Some(&ctx.payer.pubkey()),
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
async fn liquidate_receiver_rejects_zero_weight_asset() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 1.0, None)
        .await?;
    liquidatee
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

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    {
        let ctx = test_f.context.borrow_mut();
        let init_ix = liquidatee
            .make_init_liquidation_record_ix(record_pk, ctx.payer.pubkey())
            .await;
        let init_tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.1, None, true)
        .await;
    let repay_ix = liquidatee
        .make_bank_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix, withdraw_ix, repay_ix, end_ix],
        Some(&ctx.payer.pubkey()),
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

// Here liquidator can zero-out the account because it falls below the minimum value threshold
#[tokio::test]
async fn liquidate_receiver_closes_out_low_value_acc() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    //  .4 * 10 = $4, which is less than the minimum of $5
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 0.4, None)
        .await?;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 2.0)
        .await?;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.4).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    {
        let ctx = test_f.context.borrow_mut();
        let init_ix = liquidatee
            .make_init_liquidation_record_ix(record_pk, ctx.payer.pubkey())
            .await;
        let init_tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    // NOTE: In receivership liquidation, you MUST PASS the oracle for the withdrawn asset even for
    // a withdraw-all. The entire balance is still withdrawn!
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.4, Some(true), true)
        .await;
    // The entire liability
    let repay_ix = liquidatee
        .make_bank_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, Some(true))
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![usdc_bank.key],
        )
        .await;

    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[start_ix, withdraw_ix, repay_ix, end_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_ok());
    } // release borrow of ctx

    // Account has been fully closed, all positions were seized and repaid.
    let marginfi_account = liquidatee.load().await;
    let active_balance_count = marginfi_account
        .lending_account
        .get_active_balances_iter()
        .count();
    // The lending position is closed.
    assert_eq!(0, active_balance_count);

    Ok(())
}

#[tokio::test]
async fn liquidate_receiver_allows_negative_profit() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.4).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );
    {
        let ctx = test_f.context.borrow_mut();
        let init_ix = liquidatee
            .make_init_liquidation_record_ix(record_pk, ctx.payer.pubkey())
            .await;
        let init_tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    // Seize 0.09 * 10 = $0.90
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.09, None, true)
        .await;
    // Repay $2, (realizing a loss of $1.1)
    let repay_ix = liquidatee
        .make_bank_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
        .await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    // The ix doesn't care that a loss was incurred.
    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[start_ix, withdraw_ix, repay_ix, end_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.last_blockhash,
    );
    ctx.banks_client
        .process_transaction_with_preflight(tx)
        .await?;
    Ok(())
}
