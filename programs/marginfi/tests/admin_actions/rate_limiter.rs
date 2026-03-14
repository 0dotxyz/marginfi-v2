use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::assert_custom_error;
use fixtures::prelude::*;
use marginfi::prelude::*;
use marginfi_type_crate::types::{Bank, MarginfiGroup};
use solana_program_test::*;
use solana_sdk::transaction::Transaction;
use solana_sdk::{
    clock::Clock, instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer,
};

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

async fn configure_group_hourly_limit_as(
    test_f: &TestFixture,
    admin: &Keypair,
    hourly_limit: u64,
) -> anyhow::Result<()> {
    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::ConfigureGroupRateLimits {
            marginfi_group: test_f.marginfi_group.key,
            admin: admin.pubkey(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureGroupRateLimits {
            hourly_max_outflow_usd: Some(hourly_limit),
            daily_max_outflow_usd: None,
        }
        .data(),
    };

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[admin],
        ctx.banks_client.get_latest_blockhash().await?,
    );
    ctx.banks_client
        .process_transaction_with_preflight(tx)
        .await?;
    Ok(())
}

async fn configure_bank_hourly_limit_as(
    test_f: &TestFixture,
    admin: &Keypair,
    bank: Pubkey,
    hourly_limit: u64,
) -> anyhow::Result<()> {
    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::ConfigureBankRateLimits {
            group: test_f.marginfi_group.key,
            admin: admin.pubkey(),
            bank,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureBankRateLimits {
            hourly_max_outflow: Some(hourly_limit),
            daily_max_outflow: None,
        }
        .data(),
    };

    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&admin.pubkey()),
        &[admin],
        ctx.banks_client.get_latest_blockhash().await?,
    );
    ctx.banks_client
        .process_transaction_with_preflight(tx)
        .await?;
    Ok(())
}

async fn configure_group_hourly_limit(
    test_f: &TestFixture,
    hourly_limit: u64,
) -> anyhow::Result<()> {
    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::ConfigureGroupRateLimits {
            marginfi_group: test_f.marginfi_group.key,
            admin: test_f.payer(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureGroupRateLimits {
            hourly_max_outflow_usd: Some(hourly_limit),
            daily_max_outflow_usd: None,
        }
        .data(),
    };

    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }
    Ok(())
}

async fn next_group_rate_limiter_update_params(test_f: &TestFixture) -> (u64, u64) {
    let g: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    (
        g.rate_limiter_last_admin_update_slot.saturating_add(1),
        g.rate_limiter_last_admin_update_seq.saturating_add(1),
    )
}

#[tokio::test]
async fn limit_admin_can_configure_and_flow_admin_can_update_rate_limits() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let bank = test_f.get_bank(&BankMint::Usdc);
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

    configure_bank_hourly_limit_as(&test_f, &limit_admin, bank.key, 55).await?;
    configure_group_hourly_limit_as(&test_f, &limit_admin, 100).await?;

    let bank_after: Bank = test_f.load_and_deserialize(&bank.key).await;
    assert_eq!(bank_after.rate_limiter.hourly.max_outflow, 55);

    let slot = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot
    };

    let (event_start_slot, update_seq) = next_group_rate_limiter_update_params(&test_f).await;
    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::UpdateGroupRateLimiter {
            marginfi_group: test_f.marginfi_group.key,
            delegate_flow_admin: flow_admin.pubkey(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::UpdateGroupRateLimiter {
            outflow_usd: Some(42),
            inflow_usd: None,
            update_seq,
            event_start_slot,
            event_end_slot: slot,
        }
        .data(),
    };

    {
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

    let group_after: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    assert_eq!(group_after.rate_limiter.hourly.cur_window_outflow, 42);
    assert_eq!(group_after.rate_limiter_last_admin_update_seq, 1);

    Ok(())
}

#[tokio::test]
async fn update_group_rate_limiter_applies_inflow_before_outflow() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    configure_group_hourly_limit(&test_f, 100).await?;

    let slot = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot
    };

    {
        let (event_start_slot, update_seq) = next_group_rate_limiter_update_params(&test_f).await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(95),
                inflow_usd: None,
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
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let slot2 = {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot = clock.slot.saturating_add(1);
        ctx.set_sysvar(&clock);
        clock.slot
    };

    // If outflow were applied first, this would fail against the 100 USD hourly cap.
    // With inflow-first ordering it succeeds: 95 - 10 + 15 = 100.
    {
        let g: MarginfiGroup = test_f
            .load_and_deserialize(&test_f.marginfi_group.key)
            .await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(15),
                inflow_usd: Some(10),
                update_seq: g.rate_limiter_last_admin_update_seq.saturating_add(1),
                event_start_slot: g.rate_limiter_last_admin_update_slot.saturating_add(1),
                event_end_slot: slot2,
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
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let g: MarginfiGroup = test_f
        .load_and_deserialize(&test_f.marginfi_group.key)
        .await;
    assert_eq!(g.rate_limiter.hourly.cur_window_outflow, 100);
    assert_eq!(g.rate_limiter_last_admin_update_seq, 2);

    Ok(())
}

#[tokio::test]
async fn update_group_rate_limiter_guard_errors() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    // Empty update payload.
    {
        let (event_start_slot, update_seq) = next_group_rate_limiter_update_params(&test_f).await;
        let slot = {
            let ctx = test_f.context.borrow_mut();
            let clock: Clock = ctx.banks_client.get_sysvar().await?;
            clock.slot
        };
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: None,
                inflow_usd: None,
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
        assert_custom_error!(res.unwrap_err(), MarginfiError::GroupRateLimiterUpdateEmpty);
    }

    // Invalid slot range.
    {
        let (next_start_slot, update_seq) = next_group_rate_limiter_update_params(&test_f).await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(1),
                inflow_usd: None,
                update_seq,
                event_start_slot: next_start_slot.saturating_add(1),
                event_end_slot: next_start_slot,
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
            MarginfiError::GroupRateLimiterUpdateInvalidSlotRange
        );
    }

    // Future slot.
    {
        let (event_start_slot, update_seq) = next_group_rate_limiter_update_params(&test_f).await;
        let slot = {
            let ctx = test_f.context.borrow_mut();
            let clock: Clock = ctx.banks_client.get_sysvar().await?;
            clock.slot
        };
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(1),
                inflow_usd: None,
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
            MarginfiError::GroupRateLimiterUpdateFutureSlot
        );
    }

    // Out-of-order sequence (initial seq must be 1).
    {
        let (event_start_slot, next_seq) = next_group_rate_limiter_update_params(&test_f).await;
        let slot = {
            let ctx = test_f.context.borrow_mut();
            let clock: Clock = ctx.banks_client.get_sysvar().await?;
            clock.slot
        };
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(1),
                inflow_usd: None,
                update_seq: next_seq.saturating_add(1),
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
            MarginfiError::GroupRateLimiterUpdateOutOfOrderSeq
        );
    }

    Ok(())
}

#[tokio::test]
async fn update_group_rate_limiter_out_of_order_slot_and_unauthorized() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    // Force clock forward so we can deterministically construct a "lower than last slot" update.
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot = clock.slot.saturating_add(10);
        ctx.set_sysvar(&clock);
    }

    let slot = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.slot
    };

    // First valid update sets last slot + seq.
    {
        let (event_start_slot, update_seq) = next_group_rate_limiter_update_params(&test_f).await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(1),
                inflow_usd: None,
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
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Slot progression must not move backward.
    {
        let g: MarginfiGroup = test_f
            .load_and_deserialize(&test_f.marginfi_group.key)
            .await;
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: test_f.payer(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(1),
                inflow_usd: None,
                update_seq: g.rate_limiter_last_admin_update_seq.saturating_add(1),
                event_start_slot: g.rate_limiter_last_admin_update_slot,
                event_end_slot: g.rate_limiter_last_admin_update_slot,
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
            MarginfiError::GroupRateLimiterUpdateOutOfOrderSlot
        );
    }

    // Only the configured delegate flow admin may call this instruction.
    {
        let next_slot = {
            let ctx = test_f.context.borrow_mut();
            let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
            clock.slot = clock.slot.saturating_add(1);
            ctx.set_sysvar(&clock);
            clock.slot
        };
        let g: MarginfiGroup = test_f
            .load_and_deserialize(&test_f.marginfi_group.key)
            .await;
        let unauthorized = Keypair::new();
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateGroupRateLimiter {
                marginfi_group: test_f.marginfi_group.key,
                delegate_flow_admin: unauthorized.pubkey(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::UpdateGroupRateLimiter {
                outflow_usd: Some(1),
                inflow_usd: None,
                update_seq: g.rate_limiter_last_admin_update_seq.saturating_add(1),
                event_start_slot: g.rate_limiter_last_admin_update_slot.saturating_add(1),
                event_end_slot: next_slot,
            }
            .data(),
        };
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer, &unauthorized],
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
