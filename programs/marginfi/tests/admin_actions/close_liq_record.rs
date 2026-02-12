use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::constants::LIQUIDATION_RECORD_SEED;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

/// Risk admin can close a LiquidationRecord and reclaim rent.
#[tokio::test]
async fn close_liquidation_record_happy_path() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let risk_admin = test_f.payer();
    let user = test_f.create_marginfi_account().await;

    // Derive the LiquidationRecord PDA
    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), user.key.as_ref()],
        &marginfi::ID,
    );

    // Initialize the liquidation record
    let init_ix = user
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Verify the record exists
    {
        let ctx = test_f.context.borrow_mut();
        let record_account = ctx.banks_client.get_account(record_pk).await?;
        assert!(record_account.is_some(), "LiquidationRecord should exist");
    }

    // Verify the MarginfiAccount points to the record
    let ma = user.load().await;
    assert_eq!(ma.liquidation_record, record_pk);

    // Capture receiver balance and record lamports before close so we can verify rent reclaim.
    let (balance_pre, record_lamports) = {
        let ctx = test_f.context.borrow_mut();
        let bal = ctx.banks_client.get_balance(risk_admin).await?;
        let record_acc = ctx
            .banks_client
            .get_account(record_pk)
            .await?
            .expect("record must exist");
        (bal, record_acc.lamports)
    };

    // Close the record as risk admin
    let group = ma.group;
    let close_ix = user
        .make_close_liquidation_record_ix(record_pk, group, risk_admin, risk_admin)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Verify the record account is closed
    {
        let ctx = test_f.context.borrow_mut();
        let record_account = ctx.banks_client.get_account(record_pk).await?;
        assert!(
            record_account.is_none(),
            "LiquidationRecord should be closed"
        );
    }

    // Verify the MarginfiAccount pointer has been cleared
    let ma_after = user.load().await;
    assert_eq!(ma_after.liquidation_record, Pubkey::default());

    // Verify rent was reclaimed: receiver gained at least (record_lamports - tx_fee).
    let balance_post = {
        let ctx = test_f.context.borrow_mut();
        ctx.banks_client.get_balance(risk_admin).await?
    };
    // The tx fee is 5_000 lamports; use a generous 10_000 tolerance.
    let expected_min = balance_pre
        .saturating_add(record_lamports)
        .saturating_sub(10_000);
    assert!(
        balance_post >= expected_min,
        "Rent not reclaimed: post {} < expected_min {} (pre {} + rent {} - fee tolerance)",
        balance_post,
        expected_min,
        balance_pre,
        record_lamports,
    );

    Ok(())
}

/// Non-admin (random signer) cannot close a LiquidationRecord.
#[tokio::test]
async fn close_liquidation_record_non_admin_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let risk_admin = test_f.payer();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), user.key.as_ref()],
        &marginfi::ID,
    );

    // Initialize the liquidation record
    let init_ix = user
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let ma = user.load().await;
    let group = ma.group;

    // Create a non-admin signer
    let non_admin = Keypair::new();

    // Airdrop some SOL to the non-admin so they can pay for tx fees
    {
        let ctx = test_f.context.borrow_mut();
        let transfer_ix = solana_sdk::system_instruction::transfer(
            &risk_admin,
            &non_admin.pubkey(),
            1_000_000_000,
        );
        let tx = Transaction::new_signed_with_payer(
            &[transfer_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Build the close ix with the non-admin as risk_admin (should fail)
    let close_ix = {
        use anchor_lang::{InstructionData, ToAccountMetas};
        Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::RiskAdminCloseLiquidationRecord {
                group,
                marginfi_account: user.key,
                liquidation_record: record_pk,
                risk_admin: non_admin.pubkey(),
                receiver: non_admin.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::RiskAdminCloseLiquidationRecord {}.data(),
        }
    };

    let res = {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&non_admin.pubkey()),
            &[&non_admin],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_err());
    // The has_one constraint on the group will produce an Unauthorized error
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    // Verify the record still exists
    {
        let ctx = test_f.context.borrow_mut();
        let record_account = ctx.banks_client.get_account(record_pk).await?;
        assert!(
            record_account.is_some(),
            "LiquidationRecord should still exist"
        );
    }

    Ok(())
}

/// Cannot close a LiquidationRecord that is actively in use (liquidation_receiver != default).
#[tokio::test]
async fn close_liquidation_record_active_record_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let risk_admin = test_f.payer();
    let payer = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, _bump) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), user.key.as_ref()],
        &marginfi::ID,
    );

    // We want an “active” record (non-default liquidation_receiver). Starting a deleverage would set
    // it, but start/end must be in the same tx, so we can’t leave it active via instructions.
    // Instead, we flip liquidation_receiver directly in account data for this test.

    // Step 1: init the record normally.
    let init_ix = user
        .make_init_liquidation_record_ix(record_pk, risk_admin)
        .await;
    {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx =
            Transaction::new_signed_with_payer(&[init_ix], Some(&risk_admin), &[&payer], blockhash);
        let banks_client = test_f.context.borrow().banks_client.clone();
        banks_client.process_transaction_with_preflight(tx).await?;
    }

    // Step 2: manually set liquidation_receiver to non-default to simulate an active record.
    {
        let mut record_acc = test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .get_account(record_pk)
            .await?
            .expect("record must exist");

        // LiquidationRecord layout is [8-byte discriminator][key][marginfi_account][record_payer]
        // [liquidation_receiver], so liquidation_receiver starts at byte 104.
        let offset = 8 + 3 * 32;
        record_acc.data[offset..offset + 32].copy_from_slice(risk_admin.as_ref());

        let mut ctx = test_f.context.borrow_mut();
        ctx.set_account(&record_pk, &record_acc.into());
    }

    // Step 3: attempt to close — should fail with IllegalAction.
    let ma = user.load().await;
    let close_ix = user
        .make_close_liquidation_record_ix(record_pk, ma.group, risk_admin, risk_admin)
        .await;
    let res = {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&risk_admin),
            &[&payer],
            blockhash,
        );
        let banks_client = test_f.context.borrow().banks_client.clone();
        banks_client.process_transaction_with_preflight(tx).await
    };

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);

    // Record still exists.
    {
        let banks_client = test_f.context.borrow().banks_client.clone();
        assert!(
            banks_client.get_account(record_pk).await?.is_some(),
            "record should survive failed close"
        );
    }

    Ok(())
}
