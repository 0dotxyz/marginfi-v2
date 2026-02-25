use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::constants::LIQUIDATION_RECORD_SEED;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer,
    transaction::Transaction,
};

/// 60 days in seconds — matches the on-chain constant.
const SECS_60_DAYS: i64 = 60 * 24 * 3600;
/// 90 days in seconds — matches the on-chain constant.
const SECS_90_DAYS: i64 = 90 * 24 * 3600;
/// Base timestamp used for all time-dependent tests.
const BASE_TS: i64 = 1_000_000;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Set the clock, init a liquidation record, and return (record_pk, group).
async fn setup_record(test_f: &TestFixture, user: &MarginfiAccountFixture) -> (Pubkey, Pubkey) {
    let payer_pk = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let (record_pk, _) = Pubkey::find_program_address(
        &[LIQUIDATION_RECORD_SEED.as_bytes(), user.key.as_ref()],
        &marginfi::ID,
    );

    // Set a deterministic base time BEFORE init so last_activity_ts == BASE_TS.
    test_f.set_time(BASE_TS);

    let init_ix = user
        .make_init_liquidation_record_ix(record_pk, payer_pk)
        .await;
    {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&payer_pk),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await
            .unwrap();
    }

    let ma = user.load().await;
    assert_eq!(ma.liquidation_record, record_pk);
    (record_pk, ma.group)
}

// ---------------------------------------------------------------------------
// Risk-admin close tests
// ---------------------------------------------------------------------------

/// Risk admin can close a LiquidationRecord after 90 days and rent goes to record_payer.
#[tokio::test]
async fn admin_close_after_90d_succeeds() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let risk_admin = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, group) = setup_record(&test_f, &user).await;

    // Capture balance before close (risk_admin == record_payer in this fixture).
    let (balance_pre, record_lamports) = {
        let mut bc = test_f.context.borrow().banks_client.clone();
        let bal = bc.get_balance(risk_admin).await?;
        let acc = bc.get_account(record_pk).await?.expect("record exists");
        (bal, acc.lamports)
    };

    // Advance past 90 days.
    test_f.advance_time(SECS_90_DAYS + 1).await;

    let close_ix = user
        .make_close_liquidation_record_ix(record_pk, group, risk_admin, risk_admin)
        .await;
    {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&risk_admin),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Record is gone.
    {
        let mut bc = test_f.context.borrow().banks_client.clone();
        assert!(bc.get_account(record_pk).await?.is_none());
    }

    // Pointer cleared.
    let ma_after = user.load().await;
    assert_eq!(ma_after.liquidation_record, Pubkey::default());

    // Rent reclaimed (with tx-fee tolerance).
    let balance_post = {
        let mut bc = test_f.context.borrow().banks_client.clone();
        bc.get_balance(risk_admin).await?
    };
    let expected_min = balance_pre
        .saturating_add(record_lamports)
        .saturating_sub(10_000);
    assert!(balance_post >= expected_min);

    Ok(())
}

/// Risk admin CANNOT close a record before 90 days.
#[tokio::test]
async fn admin_close_before_90d_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let risk_admin = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, group) = setup_record(&test_f, &user).await;

    // Advance only 89 days — not enough.
    test_f.advance_time(SECS_90_DAYS - 1).await;

    let close_ix = user
        .make_close_liquidation_record_ix(record_pk, group, risk_admin, risk_admin)
        .await;
    let res = {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&risk_admin),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationRecordNotExpired);

    // Record still exists.
    {
        let mut bc = test_f.context.borrow().banks_client.clone();
        assert!(bc.get_account(record_pk).await?.is_some());
    }

    Ok(())
}

/// Non-admin (random signer) cannot close a LiquidationRecord.
#[tokio::test]
async fn close_liquidation_record_non_admin_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let risk_admin = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, group) = setup_record(&test_f, &user).await;

    // Advance past 90 days so time gate is not the issue.
    test_f.advance_time(SECS_90_DAYS + 1).await;

    let non_admin = Keypair::new();

    // Fund the non-admin.
    {
        let blockhash = test_f.get_latest_blockhash().await;
        let transfer_ix = solana_sdk::system_instruction::transfer(
            &risk_admin,
            &non_admin.pubkey(),
            1_000_000_000,
        );
        let tx = Transaction::new_signed_with_payer(
            &[transfer_ix],
            Some(&risk_admin),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Build the close ix with the non-admin as risk_admin (should fail).
    let close_ix = {
        use anchor_lang::{InstructionData, ToAccountMetas};
        Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::RiskAdminCloseLiquidationRecord {
                group,
                marginfi_account: user.key,
                liquidation_record: record_pk,
                risk_admin: non_admin.pubkey(),
                record_payer: risk_admin,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::RiskAdminCloseLiquidationRecord {}.data(),
        }
    };

    let res = {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&non_admin.pubkey()),
            &[&non_admin],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    // Record still exists.
    {
        let mut bc = test_f.context.borrow().banks_client.clone();
        assert!(bc.get_account(record_pk).await?.is_some());
    }

    Ok(())
}

/// Cannot close a LiquidationRecord that is actively in use (liquidation_receiver != default).
#[tokio::test]
async fn close_liquidation_record_active_record_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let risk_admin = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, group) = setup_record(&test_f, &user).await;

    // Advance past 90 days so time gate is not the blocker for this test.
    test_f.advance_time(SECS_90_DAYS + 1).await;

    // Manually set liquidation_receiver to non-default to simulate an active record.
    {
        let mut record_acc = test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .get_account(record_pk)
            .await?
            .expect("record must exist");

        // Layout: [8-byte discriminator][key 32][marginfi_account 32][record_payer 32]
        //         [liquidation_receiver 32], so offset = 8 + 3*32 = 104.
        let offset = 8 + 3 * 32;
        record_acc.data[offset..offset + 32].copy_from_slice(risk_admin.as_ref());

        let mut ctx = test_f.context.borrow_mut();
        ctx.set_account(&record_pk, &record_acc.into());
    }

    let close_ix = user
        .make_close_liquidation_record_ix(record_pk, group, risk_admin, risk_admin)
        .await;
    let res = {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&risk_admin),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);

    // Record still exists.
    {
        let mut bc = test_f.context.borrow().banks_client.clone();
        assert!(bc.get_account(record_pk).await?.is_some());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Creator close tests
// ---------------------------------------------------------------------------

/// Creator can close after 60 days of inactivity.
#[tokio::test]
async fn creator_close_after_60d_succeeds() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let creator = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, group) = setup_record(&test_f, &user).await;

    let (balance_pre, record_lamports) = {
        let mut bc = test_f.context.borrow().banks_client.clone();
        let bal = bc.get_balance(creator).await?;
        let acc = bc.get_account(record_pk).await?.expect("record exists");
        (bal, acc.lamports)
    };

    // Advance past 60 days.
    test_f.advance_time(SECS_60_DAYS + 1).await;

    let close_ix = user
        .make_close_liquidation_record_by_creator_ix(record_pk, group, creator)
        .await;
    {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&creator),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Record is gone.
    {
        let mut bc = test_f.context.borrow().banks_client.clone();
        assert!(bc.get_account(record_pk).await?.is_none());
    }

    // Pointer cleared.
    let ma = user.load().await;
    assert_eq!(ma.liquidation_record, Pubkey::default());

    // Rent reclaimed.
    let balance_post = {
        let mut bc = test_f.context.borrow().banks_client.clone();
        bc.get_balance(creator).await?
    };
    let expected_min = balance_pre
        .saturating_add(record_lamports)
        .saturating_sub(10_000);
    assert!(balance_post >= expected_min);

    Ok(())
}

/// Creator CANNOT close before 60 days.
#[tokio::test]
async fn creator_close_before_60d_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let creator = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, group) = setup_record(&test_f, &user).await;

    // Only 59 days — not enough.
    test_f.advance_time(SECS_60_DAYS - 1).await;

    let close_ix = user
        .make_close_liquidation_record_by_creator_ix(record_pk, group, creator)
        .await;
    let res = {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&creator),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::LiquidationRecordNotExpired);

    // Record still exists.
    {
        let mut bc = test_f.context.borrow().banks_client.clone();
        assert!(bc.get_account(record_pk).await?.is_some());
    }

    Ok(())
}

/// Non-creator cannot use the creator-close instruction.
#[tokio::test]
async fn creator_close_wrong_signer_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let real_creator = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, group) = setup_record(&test_f, &user).await;

    test_f.advance_time(SECS_60_DAYS + 1).await;

    let non_creator = Keypair::new();

    // Fund.
    {
        let blockhash = test_f.get_latest_blockhash().await;
        let transfer_ix = solana_sdk::system_instruction::transfer(
            &real_creator,
            &non_creator.pubkey(),
            1_000_000_000,
        );
        let tx = Transaction::new_signed_with_payer(
            &[transfer_ix],
            Some(&real_creator),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Try close with non-creator as signer.
    let close_ix = {
        use anchor_lang::{InstructionData, ToAccountMetas};
        Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::CreatorCloseLiquidationRecord {
                marginfi_account: user.key,
                group,
                liquidation_record: record_pk,
                creator: non_creator.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::CloseLiquidationRecordByCreator {}.data(),
        }
    };

    let res = {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&non_creator.pubkey()),
            &[&non_creator],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}

/// Active record cannot be closed by creator either.
#[tokio::test]
async fn creator_close_active_record_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let creator = test_f.payer();
    let payer_kp = test_f.payer_keypair();
    let user = test_f.create_marginfi_account().await;

    let (record_pk, group) = setup_record(&test_f, &user).await;

    test_f.advance_time(SECS_60_DAYS + 1).await;

    // Set liquidation_receiver to non-default.
    {
        let mut record_acc = test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .get_account(record_pk)
            .await?
            .expect("record must exist");

        let offset = 8 + 3 * 32;
        record_acc.data[offset..offset + 32].copy_from_slice(creator.as_ref());

        let mut ctx = test_f.context.borrow_mut();
        ctx.set_account(&record_pk, &record_acc.into());
    }

    let close_ix = user
        .make_close_liquidation_record_by_creator_ix(record_pk, group, creator)
        .await;
    let res = {
        let blockhash = test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            &[close_ix],
            Some(&creator),
            &[&payer_kp],
            blockhash,
        );
        test_f
            .context
            .borrow()
            .banks_client
            .clone()
            .process_transaction_with_preflight(tx)
            .await
    };

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);

    Ok(())
}
