use anchor_lang::prelude::Clock;
use fixed_macro::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::errors::MarginfiError;
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{BankConfigOpt, LiquidationRecord, MarginfiAccount},
};
use solana_program_test::*;
use solana_sdk::{
    account::Account, pubkey::Pubkey, signature::Keypair, signer::Signer, system_transaction,
    transaction::Transaction,
};

/// 60 days in seconds (must match the constant in the instruction)
const INACTIVITY_PERIOD_SECS: i64 = 60 * 24 * 60 * 60;

/// Directly write a timestamp into the liquidation record's most recent entry.
/// This simulates a past liquidation event without needing a full liquidation cycle.
async fn set_record_entry_timestamp(test_f: &TestFixture, record_pk: Pubkey, timestamp: i64) {
    let mut account = {
        let ctx = test_f.context.borrow_mut();
        ctx.banks_client
            .get_account(record_pk)
            .await
            .unwrap()
            .unwrap()
    };
    let record =
        bytemuck::from_bytes_mut::<LiquidationRecord>(&mut account.data.as_mut_slice()[8..]);
    record.entries[3].timestamp = timestamp;
    test_f
        .context
        .borrow_mut()
        .set_account(&record_pk, &account.into());
}

/// Helper: create an unhealthy liquidatee with a liquidation record initialized.
/// Returns (liquidatee, record_pk, payer_pubkey).
async fn setup_with_liquidation_record(
    test_f: &TestFixture,
) -> anyhow::Result<(MarginfiAccountFixture, Pubkey, Pubkey)> {
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // LP provides liquidity
    let lp = test_f.create_marginfi_account().await;
    let lp_token = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_token.key, usdc_bank, 200.0, None)
        .await?;

    // Liquidatee deposits SOL and borrows USDC
    let liquidatee_authority = Keypair::new();
    let liquidatee = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidatee_authority,
    )
    .await;

    let user_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to_with_owner(&liquidatee_authority.pubkey(), 100)
        .await;
    liquidatee
        .try_bank_deposit_with_authority(user_sol.key, sol_bank, 2.0, None, &liquidatee_authority)
        .await?;
    let user_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&liquidatee_authority.pubkey())
        .await;
    liquidatee
        .try_bank_borrow_with_authority(user_usdc.key, usdc_bank, 10.0, 0, &liquidatee_authority)
        .await?;

    // Make account unhealthy
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

    let payer = test_f.context.borrow().payer.pubkey();

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), liquidatee.key.as_ref()],
        &marginfi::ID,
    );

    // Init the liquidation record
    let init_ix = liquidatee
        .make_init_liquidation_record_ix(record_pk, payer)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&payer),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    Ok((liquidatee, record_pk, payer))
}

#[tokio::test]
async fn close_liquidation_record_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let (liquidatee, record_pk, payer) = setup_with_liquidation_record(&test_f).await?;

    // Verify record exists
    {
        let ctx = test_f.context.borrow_mut();
        let account: Account = ctx
            .banks_client
            .get_account(record_pk)
            .await?
            .expect("record should exist");
        assert!(account.lamports > 0);
    }

    // Close the record
    let close_ix = liquidatee
        .make_close_liquidation_record_ix(record_pk, payer)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&payer),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Verify record account is closed
    {
        let ctx = test_f.context.borrow_mut();
        let account = ctx.banks_client.get_account(record_pk).await?;
        assert!(account.is_none(), "record account should be closed");
    }

    // Verify marginfi account's liquidation_record field is reset
    let mfi_account: MarginfiAccount = liquidatee.load().await;
    assert_eq!(
        mfi_account.liquidation_record,
        Pubkey::default(),
        "liquidation_record field should be reset to default"
    );

    Ok(())
}

#[tokio::test]
async fn close_liquidation_record_fails_during_receivership() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let (liquidatee, record_pk, payer) = setup_with_liquidation_record(&test_f).await?;

    // Start a liquidation to put account into receivership
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

    // Start liquidation (puts account into receivership) then try to close in same tx
    let close_ix = liquidatee
        .make_close_liquidation_record_ix(record_pk, payer)
        .await;

    // Cannot close between start and end (account is in receivership)
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[start_ix.clone(), close_ix, end_ix.clone()],
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
    }

    Ok(())
}

#[tokio::test]
async fn close_liquidation_record_fails_wrong_payer() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let (liquidatee, record_pk, _payer) = setup_with_liquidation_record(&test_f).await?;

    // Try to close with a wrong record_payer
    let wrong_payer = Pubkey::new_unique();
    let close_ix = liquidatee
        .make_close_liquidation_record_ix(record_pk, wrong_payer)
        .await;

    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
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
    }

    Ok(())
}

/// Anyone can call close, but rent always goes to the original record_payer.
#[tokio::test]
async fn close_liquidation_record_permissionless_caller() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let (liquidatee, record_pk, payer) = setup_with_liquidation_record(&test_f).await?;

    // Create a third-party signer (not the record_payer)
    let third_party = Keypair::new();
    {
        let ctx = test_f.context.borrow_mut();
        let blockhash = ctx.banks_client.get_latest_blockhash().await.unwrap();
        let tx = system_transaction::transfer(
            &ctx.payer,
            &third_party.pubkey(),
            1_000_000_000,
            blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;
    }

    // Record the record_payer's balance before close
    let payer_balance_before = {
        let ctx = test_f.context.borrow_mut();
        ctx.banks_client.get_balance(payer).await?
    };

    // Third party calls close — should succeed
    let close_ix = liquidatee
        .make_close_liquidation_record_ix(record_pk, payer)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&third_party.pubkey()),
            &[&third_party],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Verify record is closed
    {
        let ctx = test_f.context.borrow_mut();
        let account = ctx.banks_client.get_account(record_pk).await?;
        assert!(account.is_none(), "record account should be closed");
    }

    // Verify rent went to record_payer (not the third-party caller)
    let payer_balance_after = {
        let ctx = test_f.context.borrow_mut();
        ctx.banks_client.get_balance(payer).await?
    };
    assert!(
        payer_balance_after > payer_balance_before,
        "record_payer should have received rent back"
    );

    Ok(())
}

/// Passing a liquidation_record that belongs to a different marginfi account should fail.
#[tokio::test]
async fn close_liquidation_record_fails_wrong_record() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let payer = test_f.context.borrow().payer.pubkey();

    // LP provides liquidity
    let lp = test_f.create_marginfi_account().await;
    let lp_token = test_f.usdc_mint.create_token_account_and_mint_to(400).await;
    lp.try_bank_deposit(lp_token.key, usdc_bank, 400.0, None)
        .await?;

    // Create two liquidatees with their own authorities
    let auth_a = Keypair::new();
    let liquidatee_a = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &auth_a,
    )
    .await;
    let auth_b = Keypair::new();
    let liquidatee_b = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &auth_b,
    )
    .await;

    // Both deposit SOL and borrow USDC
    for (acct, auth) in [(&liquidatee_a, &auth_a), (&liquidatee_b, &auth_b)] {
        let sol_tok = test_f
            .sol_mint
            .create_token_account_and_mint_to_with_owner(&auth.pubkey(), 100)
            .await;
        acct.try_bank_deposit_with_authority(sol_tok.key, sol_bank, 2.0, None, auth)
            .await?;
        let usdc_tok = test_f
            .usdc_mint
            .create_empty_token_account_with_owner(&auth.pubkey())
            .await;
        acct.try_bank_borrow_with_authority(usdc_tok.key, usdc_bank, 10.0, 0, auth)
            .await?;
    }

    // Make both accounts unhealthy
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

    // Init liquidation records for both
    let (record_a, _) = Pubkey::find_program_address(
        &[
            LIQUIDATION_RECORD_SEED.as_bytes(),
            liquidatee_a.key.as_ref(),
        ],
        &marginfi::ID,
    );
    let (record_b, _) = Pubkey::find_program_address(
        &[
            LIQUIDATION_RECORD_SEED.as_bytes(),
            liquidatee_b.key.as_ref(),
        ],
        &marginfi::ID,
    );

    let init_a = liquidatee_a
        .make_init_liquidation_record_ix(record_a, payer)
        .await;
    let init_b = liquidatee_b
        .make_init_liquidation_record_ix(record_b, payer)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_a, init_b],
            Some(&payer),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Try to close record_b using liquidatee_a's account (has_one mismatch)
    let close_ix = liquidatee_a
        .make_close_liquidation_record_ix(record_b, payer)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_err());
        assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidLiquidationRecord);
    }

    // Verify both records still exist
    {
        let ctx = test_f.context.borrow_mut();
        assert!(ctx.banks_client.get_account(record_a).await?.is_some());
        assert!(ctx.banks_client.get_account(record_b).await?.is_some());
    }

    Ok(())
}

/// Closing a record that was recently liquidated (< 60 days) should fail.
#[tokio::test]
async fn close_liquidation_record_fails_before_inactivity_period() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let (liquidatee, record_pk, payer) = setup_with_liquidation_record(&test_f).await?;

    let now = 1_700_000_000i64;
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp = now;
        ctx.set_sysvar(&clock);
    }

    // Simulate a past liquidation by writing a recent timestamp into the record
    set_record_entry_timestamp(&test_f, record_pk, now - 100).await;

    // Try to close — should fail (only 100 seconds of inactivity, need 60 days)
    let close_ix = liquidatee
        .make_close_liquidation_record_ix(record_pk, payer)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&payer),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_err());
        assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);
    }

    Ok(())
}

/// Closing a record after 60 days of inactivity should succeed.
#[tokio::test]
async fn close_liquidation_record_after_inactivity_period() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let (liquidatee, record_pk, payer) = setup_with_liquidation_record(&test_f).await?;

    let liquidation_time = 1_700_000_000i64;

    // Simulate a past liquidation by writing a timestamp into the record
    set_record_entry_timestamp(&test_f, record_pk, liquidation_time).await;

    // Set clock to 60 days + 1 second after the liquidation
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp = liquidation_time + INACTIVITY_PERIOD_SECS + 1;
        ctx.set_sysvar(&clock);
    }

    // Now close should succeed
    let close_ix = liquidatee
        .make_close_liquidation_record_ix(record_pk, payer)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&payer),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Verify record account is closed
    {
        let ctx = test_f.context.borrow_mut();
        let account = ctx.banks_client.get_account(record_pk).await?;
        assert!(account.is_none(), "record account should be closed");
    }

    Ok(())
}
