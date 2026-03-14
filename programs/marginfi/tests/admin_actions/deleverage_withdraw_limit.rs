use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::assert_custom_error;
use fixtures::prelude::*;
use marginfi::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;
use solana_program_test::*;
use solana_sdk::instruction::Instruction;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::{clock::Clock, transaction::Transaction};

async fn fund_signer(test_f: &TestFixture, signer: &Keypair) -> anyhow::Result<()> {
    let ctx = test_f.context.borrow_mut();
    let recent_blockhash = ctx.banks_client.get_latest_blockhash().await?;
    let tx = solana_sdk::system_transaction::transfer(
        &ctx.payer,
        &signer.pubkey(),
        10_000_000,
        recent_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await?;
    Ok(())
}

async fn next_deleverage_withdraw_limit_update_params(test_f: &TestFixture) -> (u64, u64) {
    let g: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    (
        g.deleverage_withdraw_last_admin_update_slot
            .saturating_add(1),
        g.deleverage_withdraw_last_admin_update_seq
            .saturating_add(1),
    )
}

#[tokio::test]
async fn limit_admin_can_configure_and_flow_admin_can_update_deleverage_withdraw_limit(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let limit_admin = Keypair::new();
    let flow_admin = Keypair::new();

    fund_signer(&test_f, &limit_admin).await?;
    fund_signer(&test_f, &flow_admin).await?;
    test_f
        .marginfi_group
        .try_update_with_flow_admin(
            test_f.payer(),
            test_f.payer(),
            test_f.payer(),
            limit_admin.pubkey(),
            flow_admin.pubkey(),
            test_f.payer(),
            test_f.payer(),
            test_f.payer(),
        )
        .await?;

    {
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::ConfigureDeleverageWithdrawalLimit {
                marginfi_group: test_f.marginfi_group.key,
                admin: limit_admin.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::ConfigureDeleverageWithdrawalLimit { limit: 100 }.data(),
        };
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&limit_admin.pubkey()),
            &[&limit_admin],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let slot = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot
    };

    let (event_start_slot, update_seq) =
        next_deleverage_withdraw_limit_update_params(&test_f).await;
    {
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateDeleverageWithdrawals {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: flow_admin.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateDeleverageWithdrawals {
                outflow_usd: 40,
                update_seq,
                event_start_slot,
                event_end_slot: slot,
            }
            .data(),
        };
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&flow_admin.pubkey()),
            &[&flow_admin],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let g: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    assert_eq!(g.deleverage_withdraw_window_cache.daily_limit, 100);
    assert_eq!(g.deleverage_withdraw_window_cache.withdrawn_today, 40);
    assert_eq!(g.deleverage_withdraw_last_admin_update_seq, 1);

    Ok(())
}

#[tokio::test]
async fn update_deleverage_withdraw_limit_applies_and_enforces_daily_limit() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    test_f
        .marginfi_group
        .try_update_deleverage_withdrawal_limit(100)
        .await?;

    let slot = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot
    };

    {
        let (event_start_slot, update_seq) =
            next_deleverage_withdraw_limit_update_params(&test_f).await;
        test_f
            .marginfi_group
            .try_admin_update_deleverage_withdrawals(95, update_seq, event_start_slot, slot)
            .await?;
    }

    let g: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    assert_eq!(g.deleverage_withdraw_window_cache.withdrawn_today, 95);
    assert_eq!(g.deleverage_withdraw_last_admin_update_seq, 1);

    let slot2 = {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot = clock.slot.saturating_add(1);
        ctx.set_sysvar(&clock);
        clock.slot
    };

    // 95 + 10 > 100 should fail at admin settlement.
    {
        let (event_start_slot, update_seq) =
            next_deleverage_withdraw_limit_update_params(&test_f).await;
        let res = test_f
            .marginfi_group
            .try_admin_update_deleverage_withdrawals(10, update_seq, event_start_slot, slot2)
            .await;

        assert!(res.is_err());
        assert_custom_error!(
            res.unwrap_err(),
            MarginfiError::DailyWithdrawalLimitExceeded
        );
    }

    Ok(())
}

#[tokio::test]
async fn update_deleverage_withdraw_limit_guard_errors() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let slot = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot
    };

    // Empty update payload (outflow == 0).
    {
        let (event_start_slot, update_seq) =
            next_deleverage_withdraw_limit_update_params(&test_f).await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateDeleverageWithdrawals {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateDeleverageWithdrawals {
                outflow_usd: 0,
                update_seq,
                event_start_slot,
                event_end_slot: slot,
            }
            .data(),
        };
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(
            res.unwrap_err(),
            MarginfiError::DeleverageWithdrawalUpdateEmpty
        );
    }

    // Invalid slot range.
    {
        let (event_start_slot, update_seq) =
            next_deleverage_withdraw_limit_update_params(&test_f).await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateDeleverageWithdrawals {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateDeleverageWithdrawals {
                outflow_usd: 1,
                update_seq,
                event_start_slot: event_start_slot.saturating_add(1),
                event_end_slot: event_start_slot,
            }
            .data(),
        };
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(
            res.unwrap_err(),
            MarginfiError::DeleverageWithdrawalUpdateInvalidSlotRange
        );
    }

    // Future slot.
    {
        let (event_start_slot, update_seq) =
            next_deleverage_withdraw_limit_update_params(&test_f).await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateDeleverageWithdrawals {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateDeleverageWithdrawals {
                outflow_usd: 1,
                update_seq,
                event_start_slot,
                event_end_slot: slot.saturating_add(1),
            }
            .data(),
        };
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(
            res.unwrap_err(),
            MarginfiError::DeleverageWithdrawalUpdateFutureSlot
        );
    }

    // Out-of-order sequence.
    {
        let (event_start_slot, update_seq) =
            next_deleverage_withdraw_limit_update_params(&test_f).await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateDeleverageWithdrawals {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateDeleverageWithdrawals {
                outflow_usd: 1,
                update_seq: update_seq.saturating_add(1),
                event_start_slot,
                event_end_slot: slot,
            }
            .data(),
        };
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(
            res.unwrap_err(),
            MarginfiError::DeleverageWithdrawalUpdateOutOfOrderSeq
        );
    }

    // Unauthorized signer.
    {
        let (event_start_slot, update_seq) =
            next_deleverage_withdraw_limit_update_params(&test_f).await;
        let attacker = Keypair::new();
        {
            let ctx = test_f.context.borrow_mut();
            let recent_blockhash = ctx.banks_client.get_latest_blockhash().await?;
            let tx = solana_sdk::system_transaction::transfer(
                &ctx.payer,
                &attacker.pubkey(),
                10_000_000,
                recent_blockhash,
            );
            ctx.banks_client.process_transaction(tx).await?;
        }

        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateDeleverageWithdrawals {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: attacker.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateDeleverageWithdrawals {
                outflow_usd: 1,
                update_seq,
                event_start_slot,
                event_end_slot: slot,
            }
            .data(),
        };
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&attacker.pubkey()),
            &[&attacker],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);
    }

    Ok(())
}
