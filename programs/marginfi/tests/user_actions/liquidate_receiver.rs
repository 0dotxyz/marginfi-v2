use anchor_lang::{InstructionData, ToAccountMetas};
use bytemuck::from_bytes_mut;
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
    types::{Bank, BankConfigOpt, ACCOUNT_IN_RECEIVERSHIP},
};
use solana_program_test::*;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, instruction::Instruction, pubkey::Pubkey,
    signer::Signer, transaction::Transaction,
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
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    ctx.banks_client
        .process_transaction_with_preflight(init_tx)
        .await?;

    // Liquidation on a healthy account should fail
    let start_tx = Transaction::new_signed_with_payer(
        &[start_ix, end_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.105, None)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liq_usdc_account.key, usdc_bank, 2.0, None)
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
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;

        let tx = Transaction::new_signed_with_payer(
            &[start_ix.clone()],
            Some(&payer),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    ctx.banks_client
        .process_transaction_with_preflight(init_tx)
        .await?;

    let tx = Transaction::new_signed_with_payer(
        &[start_ix, forbidden_deposit_ix, end_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.210, None)
        .await;
    // Repay $2
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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

// Repay during receivership should not require remaining accounts even when rate limits are enabled.
// This currently fails because repay fetches a rate-limit price before checking the receivership flag.
#[tokio::test]
async fn liquidate_receiver_repay_without_oracles_should_succeed() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // Enable group rate limiting so repay attempts to fetch a rate-limit price.
    {
        let ctx = test_f.context.borrow_mut();
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ConfigureGroupRateLimits {
                marginfi_group: test_f.marginfi_group.key,
                admin: ctx.payer.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::ConfigureGroupRateLimits {
                hourly_max_outflow_usd: Some(1_000_000),
                daily_max_outflow_usd: None,
            }
            .data(),
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    // Clear the cached oracle timestamp to force InvalidRateLimitPrice when no oracles are passed.
    let mut bank_ai = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(usdc_bank.key)
        .await
        .unwrap()
        .unwrap();
    let bank = from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);
    bank.cache.last_oracle_price = I80F48::ZERO.into();
    bank.cache.last_oracle_price_timestamp = 0;
    test_f
        .context
        .borrow_mut()
        .set_account(&usdc_bank.key, &bank_ai.into());

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.0001, None)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 0.001, None)
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
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;
    assert!(
        res.is_ok(),
        "repay during receivership without remaining accounts should succeed"
    );
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.3, None)
        .await;
    // $2
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
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
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.1, None)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
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
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.4, Some(true))
        .await;
    // The entire liability
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, Some(true))
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
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
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.09, None)
        .await;
    // Repay $2, (realizing a loss of $1.1)
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
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
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    ctx.banks_client
        .process_transaction_with_preflight(tx)
        .await?;
    Ok(())
}

// Unlocking a bank's liq_cache_locked flag mid-liquidation (via withdraw_all or repay_all on
// a different MarginfiAccount sharing the same bank) should fail, because end_liquidation
// requires the lock to be held for every bank the liquidatee still has positions in.

#[tokio::test]
async fn liquidate_receiver_other_account_withdraw_all_clears_bank_cache_lock(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Liquidator deposits USDC (provides liquidity) and a small SOL deposit
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;
    let liquidator_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    liquidator
        .try_bank_deposit(liquidator_sol_acc.key, sol_bank, 0.5, None)
        .await?;

    // Liquidatee: deposit SOL, borrow USDC
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    // Make liquidatee unhealthy
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;

    // Partial withdraw from liquidatee (does NOT clear liq_cache_locked)
    let liquidator_sol_dest = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_dest.key, sol_bank, 0.210, None)
        .await;

    // Partial repay on behalf of liquidatee (does NOT clear liq_cache_locked)
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
        .await;

    // Liquidator's withdraw_all on sol_bank unlocks the bank mid-liquidation
    let liquidator_sol_dest2 = test_f.sol_mint.create_empty_token_account().await;
    let liquidator_withdraw_all_ix = liquidator
        .make_bank_withdraw_ix(liquidator_sol_dest2.key, sol_bank, 0.5, Some(true))
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

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);
    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[
            compute_ix,
            start_ix,
            withdraw_ix,
            repay_ix,
            liquidator_withdraw_all_ix,
            end_ix,
        ],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;

    // Fails: unlocking bank mid-liquidation should fail
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InternalLogicError);
    Ok(())
}

#[tokio::test]
async fn liquidate_receiver_other_account_repay_all_clears_bank_cache_lock(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Liquidator deposits USDC (collateral + liquidity)
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Liquidatee deposits SOL (provides sol_bank liquidity for liquidator to borrow)
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;

    // Liquidator borrows a small amount of SOL (using USDC as collateral)
    let liquidator_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(1).await;
    liquidator
        .try_bank_borrow(liquidator_sol_acc.key, sol_bank, 0.01)
        .await?;

    // Liquidatee borrows USDC
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    // Make liquidatee unhealthy
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;

    // Partial withdraw from liquidatee
    let liquidator_sol_dest = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_dest.key, sol_bank, 0.210, None)
        .await;

    // Partial repay on behalf of liquidatee
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
        .await;

    // Liquidator's repay_all on sol_bank unlocks the bank mid-liquidation
    let liquidator_repay_all_ix = liquidator
        .make_repay_ix(liquidator_sol_acc.key, sol_bank, 0.01, Some(true))
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

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);
    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[
            compute_ix,
            start_ix,
            withdraw_ix,
            repay_ix,
            liquidator_repay_all_ix,
            end_ix,
        ],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;

    // Fails: unlocking bank mid-liquidation should fail
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InternalLogicError);
    Ok(())
}

// close_balance is not in the allowed instruction list for liquidation, so including it
// in a receivership transaction must be rejected with ForbiddenIx. 
// otherwise there might be edge case of unlocking bank mid 
#[tokio::test]
async fn liquidate_receiver_close_balance_forbidden() -> anyhow::Result<()> {
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;

    // Build a close_balance ix targeting the liquidator's own account on usdc_bank
    let liquidator_account = liquidator.load().await;
    let close_balance_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::LendingAccountCloseBalance {
            group: liquidator_account.group,
            marginfi_account: liquidator.key,
            authority: payer,
            bank: usdc_bank.key,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountCloseBalance.data(),
    };

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
        &[start_ix, close_balance_ix, end_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::ForbiddenIx);
    Ok(())
}

// Verify that withdraw_all/repay_all during liquidation properly clear the bank's
// liq_cache_locked flag so it does not remain permanently stale after the liquidation ends.
// Previously, closed balances were skipped by clear_liquidation_price_cache_locks in
// end_receivership, leaving liq_cache_locked set forever and freezing the bank's cache.
#[tokio::test]
async fn liquidate_receiver_closed_balances_do_not_leave_stale_cache_lock() -> anyhow::Result<()> {
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
    // Low value account so full close-out is allowed
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

    // Confirm banks are unlocked before liquidation
    assert!(!sol_bank.load().await.cache.is_liquidation_price_cache_locked());
    assert!(!usdc_bank.load().await.cache.is_liquidation_price_cache_locked());

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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(init_tx)
            .await?;
    }

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    // withdraw_all closes the sol balance entirely
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.4, Some(true))
        .await;
    // repay_all closes the usdc liability entirely
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, Some(true))
        .await;
    // Exclude usdc_bank from end_ix remaining accounts since its balance is closed
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
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Both banks must have their liq_cache_locked flag cleared after liquidation.
    // Before the fix, banks whose balances were closed by withdraw_all/repay_all would
    // remain locked (stale), because end_receivership only iterated active balances.
    let sol_bank_state = sol_bank.load().await;
    let usdc_bank_state = usdc_bank.load().await;
    assert!(
        !sol_bank_state.cache.is_liquidation_price_cache_locked(),
        "sol_bank liq_cache_locked must be cleared after liquidation"
    );
    assert!(
        !usdc_bank_state.cache.is_liquidation_price_cache_locked(),
        "usdc_bank liq_cache_locked must be cleared after liquidation"
    );

    Ok(())
}