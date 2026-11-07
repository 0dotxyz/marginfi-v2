use anchor_lang::{InstructionData, ToAccountMetas};
use bytemuck::from_bytes_mut;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, assert_eq_noise, native, prelude::*};
use marginfi::state::bank::BankImpl;
use marginfi::{constants::LIQUIDATION_FLAT_FEE_DEFAULT, prelude::*};
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{Bank, BankConfigOpt, BankOperationalState, ACCOUNT_IN_RECEIVERSHIP},
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
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
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let payer = test_f.context.borrow().payer.pubkey().clone();

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // A pointless deposit to the liquidator so the liquidatee has collateral to borrow...
    let liq_usdc_account = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liq_usdc_account.key, usdc_bank, 99.0, None)
        .await?;

    // Set up an unhealthy liquidatee...
    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 100)
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
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
            .make_deposit_ix(liq_usdc_account.key, usdc_bank, 1.0, None)
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
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let payer = test_f.context.borrow().payer.pubkey().clone();

    // A pointless deposit to the liquidator so the liquidatee has collateral to borrow...
    let liq_token_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    liquidator
        .try_bank_deposit(liq_token_account.key, usdc_bank, 99.0, None)
        .await?;

    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 1)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            1.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            1.0,
            0,
            &liquidatee_authority,
        )
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
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let payer = test_f.context.borrow().payer.pubkey().clone();

    // A pointless deposit to the liquidator so the liquidatee has collateral to borrow...
    let liq_token_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    liquidator
        .try_bank_deposit(liq_token_account.key, usdc_bank, 99.0, None)
        .await?;

    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 1)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            1.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            1.0,
            0,
            &liquidatee_authority,
        )
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
        .make_deposit_ix(liq_token_account.key, usdc_bank, 1.0, None)
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
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;

    // Note: Sol is $10, USDC is $1
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // liquidator provides initial liquidity and keeps some for repayment
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // setup liquidatee (after bank has liquidity for them to borrow)
    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    // * Note: Deposited $20 in SOL, borrowed $10 in USDC
    // * Note: all asset/liab weights in testing are 1, e.g. $20 in SOL = $20 exactly in value
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
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
// Group rate limiting is now event-driven and read-only during user instructions.
#[tokio::test]
async fn liquidate_receiver_repay_without_oracles_should_succeed() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // Enable group rate limiting to ensure receivership repay still skips any oracle-dependent
    // flow accounting path.
    {
        let ctx = test_f.context.borrow_mut();
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ConfigureGroupRateLimits {
                marginfi_group: test_f.marginfi_group.key,
                admin: ctx.payer.pubkey(),
                instruction_sysvar: solana_sdk::sysvar::instructions::ID,
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
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
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
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
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
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            1.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
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

/// Deposits SOL against a USDC borrow and records the account for liquidation, leaving the
/// bank weights and oracle at the fixture defaults (SOL at $10).
async fn setup_liquidatee(
    sol_deposit: f64,
    usdc_borrow: f64,
) -> anyhow::Result<(
    TestFixture,
    MarginfiAccountFixture,
    TokenAccountFixture,
    Pubkey,
)> {
    setup_liquidatee_with_balances(
        &[(BankMint::Sol, sol_deposit)],
        &[(BankMint::Usdc, usdc_borrow)],
    )
    .await
}

/// As `setup_liquidatee`, but deposits each `(bank, amount)` in `deposits`, then borrows each in
/// `borrows`. The liquidator seeds 100 tokens into every borrowed bank and keeps 100 USDC to repay.
async fn setup_liquidatee_with_balances(
    deposits: &[(BankMint, f64)],
    borrows: &[(BankMint, f64)],
) -> anyhow::Result<(
    TestFixture,
    MarginfiAccountFixture,
    TokenAccountFixture,
    Pubkey,
)> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    for (mint, amount) in deposits {
        let bank = test_f.get_bank(mint);
        let user_token = bank
            .mint
            .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), *amount)
            .await;
        liquidatee
            .try_bank_deposit_with_authority(
                user_token.key,
                bank,
                *amount,
                None,
                &liquidatee_authority,
            )
            .await?;
    }
    for (mint, amount) in borrows {
        let bank = test_f.get_bank(mint);
        if *mint != BankMint::Usdc {
            let liquidity = bank.mint.create_token_account_and_mint_to(100).await;
            liquidator
                .try_bank_deposit(liquidity.key, bank, 100, None)
                .await?;
        }
        let user_token = bank
            .mint
            .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
            .await;
        liquidatee
            .try_bank_borrow_with_authority(user_token.key, bank, *amount, 0, &liquidatee_authority)
            .await?;
    }

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

    Ok((test_f, liquidatee, liquidator_usdc_acc, record_pk))
}

// Below the minimum value threshold the account may end healthy, but the premium cap still
// binds: the liquidator cannot take the whole $4 of collateral for the $2 of debt repaid.
#[tokio::test]
async fn liquidate_receiver_low_value_acc_may_end_healthy() -> anyhow::Result<()> {
    //  .4 * 10 = $4, which is less than the minimum of $5
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee(0.4, 2.0).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
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

    let payer = test_f.payer().clone();
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![usdc_bank.key],
        )
        .await;

    // Seizing all $4 for the $2 repaid exceeds the 5% premium
    // NOTE: In receivership liquidation, you MUST PASS the oracle for the withdrawn asset even for
    // a withdraw-all. The entire balance is still withdrawn!
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix_include_closing_bank(
                liquidator_sol_acc.key,
                sol_bank,
                0.4,
                Some(true),
            )
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, Some(true))
            .await,
        end_ix.clone(),
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationPremiumTooHigh);
    }

    // Seizing $2.10 for the $2 repaid clears the debt and leaves the account healthy
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.21, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, Some(true))
            .await,
        end_ix,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(
        liquidator_sol_acc.balance().await,
        native!(0.21, "SOL", f64)
    );
    let marginfi_account = liquidatee.load().await;
    let mut balances = marginfi_account.lending_account.get_active_balances_iter();
    assert_eq!(balances.next().map(|b| b.bank_pk), Some(sol_bank.key));
    assert!(balances.next().is_none());

    Ok(())
}

// The $5 threshold lets a low-value account end healthy, but bad debt still requires a closeout:
// taking the $4 of collateral means repaying all $10 of debt.
#[tokio::test]
async fn liquidate_receiver_low_value_bad_debt_acc_must_close_out() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee(2.0, 10.0).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    // SOL falls to $2: $4 of collateral against $10 of debt, below the $5 closeout threshold
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 2_000_000_000, 0, 0)
        .await;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.5).into()),
                asset_weight_maint: Some(I80F48!(0.8).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let payer = test_f.payer().clone();
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![sol_bank.key],
        )
        .await;

    // Seizing all $4 without repaying anything
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix_include_closing_bank(
                liquidator_sol_acc.key,
                sol_bank,
                2.0,
                Some(true),
            )
            .await,
        end_ix.clone(),
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationPremiumTooHigh);
    }

    // Repaying $3.81 funds the $4 seized at the 5% premium, but leaves $6.19 of debt
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix_include_closing_bank(
                liquidator_sol_acc.key,
                sol_bank,
                2.0,
                Some(true),
            )
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 3.81, None)
            .await,
        end_ix,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationCloseoutRequired);
    }

    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix_include_closing_bank(
                liquidator_sol_acc.key,
                sol_bank,
                2.0,
                Some(true),
            )
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 10.0, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key, sol_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(liquidator_sol_acc.balance().await, native!(2, "SOL"));
    assert!(liquidatee
        .load()
        .await
        .lending_account
        .get_active_balances_iter()
        .next()
        .is_none());

    Ok(())
}

#[tokio::test]
async fn liquidate_receiver_allows_negative_profit() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
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

// Calling withdraw_all on a non-receivership account that shares a bank with the liquidatee
// must NOT clear the bank's liq_cache_locked flag. The lock is only cleared when the account
// being operated on has ACCOUNT_IN_RECEIVERSHIP set, preventing cross-account interference.
#[tokio::test]
async fn liquidate_receiver_other_account_withdraw_all_does_not_clear_bank_cache_lock(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;

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
    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
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

    // Liquidator's withdraw_all on sol_bank — should NOT affect the bank's liq_cache lock
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

    // Succeeds: withdraw_all on a non-receivership account should NOT clear the
    // bank's liq_cache_locked flag, so the liquidation completes normally.
    assert!(res.is_ok());
    Ok(())
}

// Same as above but for repay_all: repaying on a non-receivership account that shares a bank
// with the liquidatee must NOT clear the bank's liq_cache_locked flag.
#[tokio::test]
async fn liquidate_receiver_other_account_repay_all_does_not_clear_bank_cache_lock(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Liquidator deposits USDC (collateral + liquidity)
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Liquidatee deposits SOL (provides sol_bank liquidity for liquidator to borrow)
    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;

    // Liquidator borrows a small amount of SOL (using USDC as collateral)
    let liquidator_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(1).await;
    liquidator
        .try_bank_borrow(liquidator_sol_acc.key, sol_bank, 0.01)
        .await?;

    // Liquidatee borrows USDC
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
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

    // Liquidator's repay_all on sol_bank — should NOT affect the bank's liq_cache lock
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

    // Succeeds: repay_all on a non-receivership account should NOT clear the
    // bank's liq_cache_locked flag, so the liquidation completes normally.
    assert!(res.is_ok());
    Ok(())
}

// A whitelisted external program can invoke marginfi::lending_account_close_balance via CPI.
// This should not clear bank liq_cache_locked when the target account is not in receivership.
#[tokio::test]
async fn liquidate_receiver_other_account_close_balance_via_cpi_does_not_clear_bank_cache_lock(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Liquidator collateral/liquidity.
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Liquidatee setup.
    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
        .await?;

    // Create an empty-but-active SOL balance on liquidator so close_balance is valid.
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    liquidator
        .try_bank_borrow(liquidator_sol_acc.key, sol_bank, 0.01)
        .await?;
    liquidator
        .try_bank_repay(liquidator_sol_acc.key, sol_bank, 0.01, Some(false))
        .await?;

    // Make liquidatee unhealthy.
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

    let payer = test_f.payer();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;

    // Partial liquidation operations on liquidatee.
    let liquidator_sol_dest = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_dest.key, sol_bank, 0.210, None)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
        .await;

    // Whitelisted kamino instruction that CPI-calls marginfi close_balance on liquidator's SOL bank.
    let liquidator_account = liquidator.load().await;
    let cpi_close_balance_ix = Instruction {
        program_id: kamino_mocks::kamino_lending::ID,
        accounts: vec![
            AccountMeta::new_readonly(liquidator_account.group, false),
            AccountMeta::new(liquidator.key, false),
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new(sol_bank.key, false),
            AccountMeta::new_readonly(marginfi::ID, false),
        ],
        data: kamino_mocks::CPI_CLOSE_BALANCE_IX_DATA.to_vec(),
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

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_000_000);
    let res = {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[
                compute_ix,
                start_ix,
                withdraw_ix,
                repay_ix,
                cpi_close_balance_ix,
                end_ix,
            ],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_ok());

    // Liquidation completed and no account should remain in receivership.
    let liquidatee_ma = liquidatee.load().await;
    let liquidator_ma = liquidator.load().await;
    assert!(!liquidatee_ma.get_flag(ACCOUNT_IN_RECEIVERSHIP));
    assert!(!liquidator_ma.get_flag(ACCOUNT_IN_RECEIVERSHIP));

    // CPI close_balance should have closed liquidator's SOL balance.
    assert!(liquidator_ma
        .lending_account
        .balances
        .iter()
        .all(|b| !(b.is_active() && b.bank_pk == sol_bank.key)));

    // Both involved banks should be unlocked after end_liquidation.
    let sol_bank_state = sol_bank.load().await;
    let usdc_bank_state = usdc_bank.load().await;
    assert!(!sol_bank_state.cache.is_liquidation_price_cache_locked());
    assert!(!usdc_bank_state.cache.is_liquidation_price_cache_locked());
    Ok(())
}

// Same lock-preservation invariant as above, with another distinct-authority external signer.
// That external user performs withdraw_all on a shared bank during A's receivership liquidation.
#[tokio::test]
async fn liquidate_receiver_external_user_signed_withdraw_all_does_not_clear_bank_cache_lock(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;

    // Independent user B with a different authority than payer.
    let user_b_authority = Keypair::new();
    let user_b = fixtures::marginfi_account::MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &user_b_authority,
    )
    .await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Liquidator provides USDC liquidity.
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Liquidatee setup.
    let user_token_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 10)
        .await;
    let user_token_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_deposit_with_authority(
            user_token_sol.key,
            sol_bank,
            2.0,
            None,
            &liquidatee_authority,
        )
        .await?;
    liquidatee
        .try_bank_borrow_with_authority(
            user_token_usdc.key,
            usdc_bank,
            10.0,
            0,
            &liquidatee_authority,
        )
        .await?;

    // User B creates a closeable SOL position on the same bank and will withdraw_all later.
    let user_b_sol_src = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&user_b_authority.pubkey(), 10)
        .await;
    user_b
        .try_bank_deposit_with_authority(user_b_sol_src.key, sol_bank, 0.5, None, &user_b_authority)
        .await?;

    // Make liquidatee unhealthy.
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

    let payer = test_f.payer();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;

    // Normal liquidation ops on A.
    let liquidator_sol_dest = test_f.sol_mint.create_empty_token_account().await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_dest.key, sol_bank, 0.210, None)
        .await;
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 2.0, None)
        .await;

    // User B's mid-liquidation action on the shared bank.
    let user_b_sol_dest = test_f
        .sol_mint
        .create_empty_token_account_with_owner(&user_b_authority.pubkey())
        .await;
    let user_b_withdraw_all_ix = user_b
        .make_withdraw_ix_with_authority(
            user_b_sol_dest.key,
            sol_bank,
            0.5,
            Some(true),
            user_b_authority.pubkey(),
        )
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
    let res = {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[
                compute_ix,
                start_ix,
                withdraw_ix,
                repay_ix,
                user_b_withdraw_all_ix,
                end_ix,
            ],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &user_b_authority],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_ok());

    // Liquidation completed and no involved account should remain in receivership.
    let liquidatee_ma = liquidatee.load().await;
    let liquidator_ma = liquidator.load().await;
    let user_b_ma = user_b.load().await;
    assert!(!liquidatee_ma.get_flag(ACCOUNT_IN_RECEIVERSHIP));
    assert!(!liquidator_ma.get_flag(ACCOUNT_IN_RECEIVERSHIP));
    assert!(!user_b_ma.get_flag(ACCOUNT_IN_RECEIVERSHIP));

    // External user's shared-bank balance should have been closed by withdraw_all.
    assert!(user_b_ma
        .lending_account
        .balances
        .iter()
        .all(|b| !(b.is_active() && b.bank_pk == sol_bank.key)));

    // Both involved banks should be unlocked after end_liquidation.
    let sol_bank_state = sol_bank.load().await;
    let usdc_bank_state = usdc_bank.load().await;
    assert!(!sol_bank_state.cache.is_liquidation_price_cache_locked());
    assert!(!usdc_bank_state.cache.is_liquidation_price_cache_locked());
    Ok(())
}

// During receivership, the liquidatee authority itself cannot act as liquidation receiver.
// A same-authority withdraw should fail with Unauthorized.
#[tokio::test]
async fn liquidate_receiver_same_authority_withdraw_fails_unauthorized() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidatee = test_f.create_marginfi_account().await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Provide USDC liquidity for the liquidatee borrow.
    let liquidator_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    liquidator
        .try_bank_deposit(liquidator_usdc_acc.key, usdc_bank, 100, None)
        .await?;

    // Liquidatee setup (authority == payer in this fixture).
    let user_token_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_token_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_deposit(user_token_sol.key, sol_bank, 2.0, None)
        .await?;
    liquidatee
        .try_bank_borrow(user_token_usdc.key, usdc_bank, 10.0)
        .await?;

    // Make liquidatee unhealthy so liquidation can start.
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

    let payer = test_f.payer();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(user_token_sol.key, sol_bank, 0.1, None)
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
        &[compute_ix, start_ix, withdraw_ix, end_ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    let res = ctx
        .banks_client
        .process_transaction_with_preflight(tx)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);
    Ok(())
}

// close_balance is not in the allowed instruction list for liquidation, so including it
// as a top-level instruction in a receivership transaction must be rejected with ForbiddenIx.
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

// Verify that withdraw_all/repay_all during liquidation properly clear the bank's liq_cache_locked
// flag so it does not remain permanently stale after the liquidation ends. Previously, closed
// balances were skipped by clear_liquidation_price_cache_locks in end_receivership, leaving
// liq_cache_locked set forever and freezing the bank's cache.
#[tokio::test]
async fn liquidate_receiver_closed_balances_do_not_leave_stale_cache_lock() -> anyhow::Result<()> {
    // Low value account, so it may end healthy. The debt nearly matches the collateral so
    // closing both balances stays within the premium cap.
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee(0.4, 3.85).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
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
    assert!(!sol_bank
        .load()
        .await
        .cache
        .is_liquidation_price_cache_locked());
    assert!(!usdc_bank
        .load()
        .await
        .cache
        .is_liquidation_price_cache_locked());

    let payer = test_f.payer().clone();
    let start_ix = liquidatee.make_start_liquidation_ix(record_pk, payer).await;
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    // withdraw_all closes the sol balance entirely
    let withdraw_ix = liquidatee
        .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.4, Some(true))
        .await;
    // repay_all closes the usdc liability entirely
    let repay_ix = liquidatee
        .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 3.85, Some(true))
        .await;
    // Exclude both banks from end_ix remaining accounts since both balances are closed.
    let end_ix = liquidatee
        .make_end_liquidation_ix(
            record_pk,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![usdc_bank.key, sol_bank.key],
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

/// Deposits $20 of SOL against a $19.50 USDC borrow, then weights SOL at a maintenance ratio of
/// 0.96, which puts the maintenance limit at 25x (against the 1.0 USDC liability weight) and lets
/// only a premium under 1/24 raise health, leaving the account $0.30 short of maintenance. Returns
/// the fixture, the parties, and the liquidation record.
async fn setup_high_leverage_liquidatee() -> anyhow::Result<(
    TestFixture,
    MarginfiAccountFixture,
    TokenAccountFixture,
    Pubkey,
)> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee(2.0, 19.5).await?;
    test_f
        .get_bank(&BankMint::Sol)
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.95).into()),
                asset_weight_maint: Some(I80F48!(0.96).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    Ok((test_f, liquidatee, liquidator_usdc_acc, record_pk))
}

// The $0.50 of equity is under the 5% premium on all $19.50 of debt ($0.975), so the account must
// be closed out.
#[tokio::test]
async fn liquidate_receiver_high_leverage_must_close_out() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) =
        setup_high_leverage_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let payer = test_f.payer().clone();

    // Seizing $10.28 for $9.80 takes $0.48 of the $0.50 equity and leaves $9.72 against $9.70
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.028, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 9.8, None)
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationCloseoutRequired);
    }

    // Repaying all $19.50 of debt for $19.90 of SOL closes the account out. The user keeps $0.10.
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.99, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 19.5, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(
        liquidator_sol_acc.balance().await,
        native!(1.99, "SOL", f64)
    );
    let marginfi_account = liquidatee.load().await;
    let mut balances = marginfi_account.lending_account.get_active_balances_iter();
    assert_eq!(balances.next().map(|b| b.bank_pk), Some(sol_bank.key));
    assert!(balances.next().is_none());
    Ok(())
}

// Bad debt does not lift the closeout, so the liquidator absorbs the shortfall.
#[tokio::test]
async fn liquidate_receiver_high_leverage_bad_debt_must_close_out() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) =
        setup_high_leverage_liquidatee().await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    // SOL falls to $9.50: $19 of collateral against $19.50 of debt
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 9_500_000_000, 0, 0)
        .await;
    let payer = test_f.payer().clone();

    // Seizing all $19 of SOL for $18.10 repaid leaves $1.40 of debt and no collateral
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix_include_closing_bank(
                liquidator_sol_acc.key,
                sol_bank,
                2.0,
                Some(true),
            )
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 18.1, None)
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![sol_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationCloseoutRequired);
    }

    // Repaying all $19.50 for the $19 of SOL closes the account out
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix_include_closing_bank(
                liquidator_sol_acc.key,
                sol_bank,
                2.0,
                Some(true),
            )
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 19.5, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key, sol_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(liquidator_sol_acc.balance().await, native!(2, "SOL"));
    assert!(liquidatee
        .load()
        .await
        .lending_account
        .get_active_balances_iter()
        .next()
        .is_none());
    Ok(())
}

// At a 0.9 weight the 5% premium raises health, but the $0.50 of equity is still under the $0.975
// premium on all $19.50 of debt, so the account must be closed out.
#[tokio::test]
async fn liquidate_receiver_equity_below_premium_must_close_out() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee(2.0, 19.5).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.8).into()),
                asset_weight_maint: Some(I80F48!(0.9).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    let payer = test_f.payer().clone();

    // Seizing $11.55 for $11.00 repaid takes $0.55 of premium, $0.05 more than the account has
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.155, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 11.0, None)
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::WorseHealthPostLiquidation);
    }

    // $0.45 of premium leaves the account solvent, but only $0.05 of equity on $10.50 of debt
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 0.945, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 9.0, None)
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationCloseoutRequired);
    }

    // Repaying all $19.50 for $19.95 of SOL closes the account out. The user keeps $0.05.
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.995, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 19.5, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(
        liquidator_sol_acc.balance().await,
        native!(1.995, "SOL", f64)
    );
    let marginfi_account = liquidatee.load().await;
    let mut balances = marginfi_account.lending_account.get_active_balances_iter();
    assert_eq!(balances.next().map(|b| b.bank_pk), Some(sol_bank.key));
    assert!(balances.next().is_none());
    Ok(())
}

// Collateral of mixed weights: the $4 of equity is under the 5% premium on all $96 of debt
// ($4.80), whichever collateral the liquidator seizes.
#[tokio::test]
async fn liquidate_receiver_mixed_collateral_must_close_out() -> anyhow::Result<()> {
    // $90 of SOL at 1.0 and $10 of SOL-equivalent at 0.9, against $96 of USDC debt at 1.05:
    // maintenance health is $99 - $100.80 = -$1.80
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee_with_balances(
        &[(BankMint::Sol, 9.0), (BankMint::SolEquivalent, 1.0)],
        &[(BankMint::Usdc, 96.0)],
    )
    .await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank = test_f.get_bank(&BankMint::SolEquivalent);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    sol_eq_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.9).into()),
                asset_weight_maint: Some(I80F48!(0.9).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    usdc_bank
        .update_config(
            BankConfigOpt {
                liability_weight_init: Some(I80F48!(1.05).into()),
                liability_weight_maint: Some(I80F48!(1.05).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    let payer = test_f.payer().clone();

    // Seizing $83.99 of SOL for $80 repaid takes $3.99 of the $4 equity, raises health by a cent
    // and leaves $16.01 against $16
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 8.399, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 80.0, None)
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationCloseoutRequired);
    }

    // Repaying all $96 for the $90 of SOL and $9 of SOL-equivalent closes the account out. The
    // user keeps $1 of SOL-equivalent.
    let liquidator_sol_eq_acc = test_f
        .sol_equivalent_mint
        .create_empty_token_account()
        .await;
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_eq_acc.key, sol_eq_bank, 0.9, None)
            .await,
        liquidatee
            .make_bank_withdraw_ix_include_closing_bank(
                liquidator_sol_acc.key,
                sol_bank,
                9.0,
                Some(true),
            )
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 96.0, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key, sol_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(liquidator_sol_acc.balance().await, native!(9, "SOL"));
    assert_eq!(
        liquidator_sol_eq_acc.balance().await,
        native!(0.9, "SOL_EQ", f64)
    );
    let marginfi_account = liquidatee.load().await;
    let mut balances = marginfi_account.lending_account.get_active_balances_iter();
    assert_eq!(balances.next().map(|b| b.bank_pk), Some(sol_eq_bank.key));
    assert!(balances.next().is_none());
    Ok(())
}

// Repaying one debt in full does not close the account out: every liability must be repaid.
#[tokio::test]
async fn liquidate_receiver_closeout_repays_every_debt() -> anyhow::Result<()> {
    // $20 of SOL against $10 of USDC and $9.50 of PyUSD: the $0.50 of equity is under the 5%
    // premium on all $19.50 of debt ($0.975)
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee_with_balances(
        &[(BankMint::Sol, 2.0)],
        &[(BankMint::Usdc, 10.0), (BankMint::PyUSD, 9.5)],
    )
    .await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.8).into()),
                asset_weight_maint: Some(I80F48!(0.9).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    let payer = test_f.payer().clone();
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let liquidator_pyusd_acc = pyusd_bank.mint.create_token_account_and_mint_to(20).await;

    // Repaying all of the USDC for $10.40 of SOL leaves the $9.50 of PyUSD
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.04, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 10.0, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationCloseoutRequired);
    }

    // Repaying both debts for $19.95 of SOL closes the account out. The user keeps $0.05.
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.995, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 10.0, Some(true))
            .await,
        liquidatee
            .make_repay_ix(liquidator_pyusd_acc.key, pyusd_bank, 9.5, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key, pyusd_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(
        liquidator_sol_acc.balance().await,
        native!(1.995, "SOL", f64)
    );
    let marginfi_account = liquidatee.load().await;
    let mut balances = marginfi_account.lending_account.get_active_balances_iter();
    assert_eq!(balances.next().map(|b| b.bank_pk), Some(sol_bank.key));
    assert!(balances.next().is_none());
    Ok(())
}

/// $19 of SOL against $10 of USDC and $9.50 of PyUSD, in bad debt, with PyUSD put in `pyusd_state`.
async fn setup_bad_debt_liquidatee(
    pyusd_state: BankOperationalState,
) -> anyhow::Result<(
    TestFixture,
    MarginfiAccountFixture,
    TokenAccountFixture,
    Pubkey,
)> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee_with_balances(
        &[(BankMint::Sol, 2.0)],
        &[(BankMint::Usdc, 10.0), (BankMint::PyUSD, 9.5)],
    )
    .await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 9_500_000_000, 0, 0)
        .await;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.8).into()),
                asset_weight_maint: Some(I80F48!(0.9).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    {
        let mut ctx = test_f.context.borrow_mut();
        let mut bank_ai = ctx
            .banks_client
            .get_account(pyusd_bank.key)
            .await
            .unwrap()
            .unwrap();
        let bank = from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);
        bank.config.operational_state = pyusd_state;
        ctx.set_account(&pyusd_bank.key, &bank_ai.into());
    }
    Ok((test_f, liquidatee, liquidator_usdc_acc, record_pk))
}

// A closeout may leave debt in a bankrupt bank, while the collateral left covers it.
#[tokio::test]
async fn liquidate_receiver_closeout_leaves_bankrupt_bank_debt() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) =
        setup_bad_debt_liquidatee(BankOperationalState::KilledByBankruptcy).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);
    let payer = test_f.payer().clone();
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    // Leaving $5 of USDC fails. So does seizing $9.50095 of SOL, which leaves $9.49905 against the
    // $9.50 of PyUSD.
    for (withdraw_sol, repay_usdc, repay_all) in [(0.5, 5.0, None), (1.0001, 10.0, Some(true))] {
        let ixs = vec![
            liquidatee.make_start_liquidation_ix(record_pk, payer).await,
            liquidatee
                .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, withdraw_sol, None)
                .await,
            liquidatee
                .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, repay_usdc, repay_all)
                .await,
            liquidatee
                .make_end_liquidation_ix(
                    record_pk,
                    payer,
                    test_f.marginfi_group.fee_state,
                    test_f.marginfi_group.fee_wallet,
                    if repay_all.is_some() {
                        vec![usdc_bank.key]
                    } else {
                        vec![]
                    },
                )
                .await,
        ];
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationCloseoutRequired);
    }

    // Repaying the USDC for $9.50 of SOL leaves exactly $9.50 against the $9.50 of PyUSD
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.0, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 10.0, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(liquidator_sol_acc.balance().await, native!(1, "SOL"));
    let marginfi_account = liquidatee.load().await;
    let banks: Vec<Pubkey> = marginfi_account
        .lending_account
        .get_active_balances_iter()
        .map(|b| b.bank_pk)
        .collect();
    assert_eq!(banks.len(), 2);
    assert!(banks.contains(&sol_bank.key) && banks.contains(&pyusd_bank.key));
    Ok(())
}

// A closeout must repay debt in a paused bank too.
#[tokio::test]
async fn liquidate_receiver_closeout_repays_paused_debt() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) =
        setup_bad_debt_liquidatee(BankOperationalState::Paused).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);
    let payer = test_f.payer().clone();
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let liquidator_pyusd_acc = pyusd_bank.mint.create_token_account_and_mint_to(20).await;

    // Repaying only the USDC for $9.50 of SOL leaves the paused $9.50 of PyUSD
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.0, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 10.0, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationCloseoutRequired);
    }

    // Repaying both debts for $18.05 of SOL closes the account out
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.9, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 10.0, Some(true))
            .await,
        liquidatee
            .make_repay_ix(liquidator_pyusd_acc.key, pyusd_bank, 9.5, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key, pyusd_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(liquidator_sol_acc.balance().await, native!(1.9, "SOL", f64));
    let marginfi_account = liquidatee.load().await;
    assert_eq!(marginfi_account.account_flags, 0);
    let mut balances = marginfi_account.lending_account.get_active_balances_iter();
    assert_eq!(balances.next().map(|b| b.bank_pk), Some(sol_bank.key));
    assert!(balances.next().is_none());
    Ok(())
}

// A liquidation that isn't a closeout can't repay debt in a paused bank.
#[tokio::test]
async fn liquidate_receiver_paused_debt_repay_requires_closeout() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee_with_balances(
        &[(BankMint::Sol, 2.0)],
        &[(BankMint::Usdc, 10.0), (BankMint::PyUSD, 9.5)],
    )
    .await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.8).into()),
                asset_weight_maint: Some(I80F48!(0.9).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    pyusd_bank
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::Paused),
                ..Default::default()
            },
            None,
        )
        .await?;
    let payer = test_f.payer().clone();
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    let liquidator_pyusd_acc = pyusd_bank.mint.create_token_account_and_mint_to(20).await;

    // The closeout line is $20.475 of SOL, 5% over the $19.50 of debt. Repaying both debts fails at
    // $20.4752 of SOL and passes at $20.4748.
    for (sol_price, withdraw_sol, closeout) in
        [(10_237_600_000, 1.95, false), (10_237_400_000, 1.9, true)]
    {
        test_f
            .set_pyth_oracle_price_native(PYTH_SOL_FEED, sol_price, 0, 0)
            .await;
        let ixs = vec![
            liquidatee.make_start_liquidation_ix(record_pk, payer).await,
            liquidatee
                .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, withdraw_sol, None)
                .await,
            liquidatee
                .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 10.0, Some(true))
                .await,
            liquidatee
                .make_repay_ix(liquidator_pyusd_acc.key, pyusd_bank, 9.5, Some(true))
                .await,
            liquidatee
                .make_end_liquidation_ix(
                    record_pk,
                    payer,
                    test_f.marginfi_group.fee_state,
                    test_f.marginfi_group.fee_wallet,
                    vec![usdc_bank.key, pyusd_bank.key],
                )
                .await,
        ];
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        if closeout {
            res?;
        } else {
            assert_custom_error!(res.unwrap_err(), MarginfiError::BankPaused);
        }
    }

    assert_eq!(liquidator_sol_acc.balance().await, native!(1.9, "SOL", f64));
    Ok(())
}

// A closeout may leave debt priced at zero.
#[tokio::test]
async fn liquidate_receiver_closeout_leaves_zero_priced_debt() -> anyhow::Result<()> {
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee_with_balances(
        &[(BankMint::Sol, 2.0)],
        &[(BankMint::Usdc, 10.0), (BankMint::Fixed, 4.75)],
    )
    .await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let fixed_bank = test_f.get_bank(&BankMint::Fixed);
    // The fixed-price debt is repriced to zero and SOL falls to $5.20: the $0.40 of equity left on
    // $10.40 of SOL against $10 of USDC is under the 5% premium on that debt ($0.50)
    test_f
        .marginfi_group
        .try_lending_pool_set_oracle_price_with_signer(
            &test_f.payer_keypair(),
            fixed_bank,
            I80F48::ZERO.into(),
        )
        .await?;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 5_200_000_000, 0, 0)
        .await;
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.8).into()),
                asset_weight_maint: Some(I80F48!(0.9).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    let payer = test_f.payer().clone();
    let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    // Repaying the USDC for $9.88 of SOL leaves $0.52 of SOL and the zero-priced debt
    let ixs = vec![
        liquidatee.make_start_liquidation_ix(record_pk, payer).await,
        liquidatee
            .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, 1.9, None)
            .await,
        liquidatee
            .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, 10.0, Some(true))
            .await,
        liquidatee
            .make_end_liquidation_ix(
                record_pk,
                payer,
                test_f.marginfi_group.fee_state,
                test_f.marginfi_group.fee_wallet,
                vec![usdc_bank.key],
            )
            .await,
    ];
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    assert_eq!(liquidator_sol_acc.balance().await, native!(1.9, "SOL", f64));
    let marginfi_account = liquidatee.load().await;
    let banks: Vec<Pubkey> = marginfi_account
        .lending_account
        .get_active_balances_iter()
        .map(|b| b.bank_pk)
        .collect();
    assert_eq!(banks.len(), 2);
    assert!(banks.contains(&sol_bank.key) && banks.contains(&fixed_bank.key));
    Ok(())
}

// Equity above the premium on all of the debt survives any run of partial liquidations. Each one at
// the full 5% premium lowers the equity and the premium on the remaining debt by the same amount.
#[tokio::test]
async fn liquidate_receiver_repeated_partials_keep_equity() -> anyhow::Result<()> {
    // $20 of SOL against $18.75 of USDC debt: $1.25 of equity, $0.3125 above the 5% premium on it
    let (test_f, liquidatee, liquidator_usdc_acc, record_pk) = setup_liquidatee(2.0, 18.75).await?;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.5).into()),
                asset_weight_maint: Some(I80F48!(0.5).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    let payer = test_f.payer().clone();

    // Three partials of $5.25 of SOL for $5 repaid leave $4.25 against $3.75. The last one, under
    // the $5 threshold, repays the rest for $3.9375 of SOL.
    for (withdraw_sol, repay_usdc, repay_all, exclude_banks) in [
        (0.525, 5.0, None, vec![]),
        (0.525, 5.0, None, vec![]),
        (0.525, 5.0, None, vec![]),
        (0.39375, 3.75, Some(true), vec![usdc_bank.key]),
    ] {
        let liquidator_sol_acc = test_f.sol_mint.create_empty_token_account().await;
        let ixs = vec![
            liquidatee.make_start_liquidation_ix(record_pk, payer).await,
            liquidatee
                .make_bank_withdraw_ix(liquidator_sol_acc.key, sol_bank, withdraw_sol, None)
                .await,
            liquidatee
                .make_repay_ix(liquidator_usdc_acc.key, usdc_bank, repay_usdc, repay_all)
                .await,
            liquidatee
                .make_end_liquidation_ix(
                    record_pk,
                    payer,
                    test_f.marginfi_group.fee_state,
                    test_f.marginfi_group.fee_wallet,
                    exclude_banks,
                )
                .await,
        ];
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // The user keeps exactly the $0.3125 gap
    let liquidatee_ma = liquidatee.load().await;
    let sol_bank_state = sol_bank.load().await;
    let mut balances = liquidatee_ma.lending_account.get_active_balances_iter();
    let sol_balance = balances.next().unwrap();
    assert_eq!(sol_balance.bank_pk, sol_bank.key);
    assert!(balances.next().is_none());
    assert_eq!(
        sol_bank_state.get_asset_amount(sol_balance.asset_shares.into())?,
        I80F48!(31250000)
    );
    Ok(())
}
