use anchor_lang::{InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_anchor_error, assert_custom_error, prelude::*};
use juplend_mocks::{
    liquidity_instructions::{
        CHANGE_STATUS_DISCRIMINATOR, PAUSE_TOKEN_DISCRIMINATOR, PAUSE_USER_DISCRIMINATOR,
        UNPAUSE_TOKEN_DISCRIMINATOR, UNPAUSE_USER_DISCRIMINATOR, UPDATE_GUARDIANS_DISCRIMINATOR,
        UPDATE_USER_CLASS_DISCRIMINATOR,
    },
    state::{
        LIQUIDITY_STATUS_OFFSET, RESERVE_INTERACTING_PROTOCOL_OFFSET, SUPPLY_POSITION_STATUS_OFFSET,
    },
};
use marginfi::{errors::MarginfiError, state::bank::BankImpl};
use marginfi_type_crate::{
    constants::JUPLEND_EMERGENCY,
    pdas::{
        derive_juplend_auth_list, derive_juplend_borrow_position, derive_juplend_liquidity,
        JUPLEND_LIQUIDITY_PROGRAM_ID,
    },
    types::health_cache::{ENGINE_OK, ORACLE_OK},
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::BanksClientError;
use solana_sdk::{
    account::AccountSharedData,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signer,
    transaction::Transaction,
};

async fn propagate(
    setup: &JuplendBankSetup,
    reserve: Pubkey,
    position: Pubkey,
    nonce: u32,
) -> Result<(), BanksClientError> {
    let ctx = setup.test_f.context.borrow_mut();
    let ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::PropagateJuplendEmergency {
            lending: setup.lending,
            liquidity: derive_juplend_liquidity().0,
            token_reserve: reserve,
            supply_position: position,
            bank: setup.bank_f.key,
        }
        .to_account_metas(None),
        data: marginfi::instruction::PropagateJuplendEmergency {}.data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(500_000 + nonce),
            ix,
        ],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    ctx.banks_client.process_transaction(tx).await
}

async fn set_bytes(setup: &JuplendBankSetup, key: Pubkey, offset: usize, data: &[u8]) {
    let mut account = setup.test_f.try_load(&key).await.unwrap().unwrap();
    account.data[offset..offset + data.len()].copy_from_slice(data);
    setup
        .test_f
        .context
        .borrow_mut()
        .set_account(&key, &AccountSharedData::from(account));
}

/// A Fluid Liquidity instruction's accounts and data.
type FluidIx = (Vec<AccountMeta>, Vec<u8>);

async fn send_fluid_instruction(
    setup: &JuplendBankSetup,
    (accounts, data): FluidIx,
) -> anyhow::Result<()> {
    let ctx = setup.test_f.context.borrow_mut();
    let ix = Instruction {
        program_id: JUPLEND_LIQUIDITY_PROGRAM_ID,
        accounts,
        data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await?,
    );
    ctx.banks_client.process_transaction(tx).await?;
    Ok(())
}

/// Fluid's global lockdown, on (`true`) or off.
fn fluid_change_status(payer: Pubkey, locked: bool) -> FluidIx {
    let mut data = CHANGE_STATUS_DISCRIMINATOR.to_vec();
    data.push(u8::from(locked));
    let accounts = vec![
        AccountMeta::new_readonly(payer, true),
        AccountMeta::new(derive_juplend_liquidity().0, false),
        AccountMeta::new_readonly(derive_juplend_auth_list().0, false),
    ];
    (accounts, data)
}

/// Fluid's `pause_token` / `unpause_token` (by discriminator) on the mint's token reserve.
fn fluid_token_pause(
    payer: Pubkey,
    discriminator: [u8; 8],
    mint: Pubkey,
    reserve: Pubkey,
) -> FluidIx {
    let mut data = discriminator.to_vec();
    data.extend_from_slice(mint.as_ref());
    let accounts = vec![
        AccountMeta::new_readonly(payer, true),
        AccountMeta::new_readonly(derive_juplend_auth_list().0, false),
        AccountMeta::new(reserve, false),
    ];
    (accounts, data)
}

/// Fluid's `pause_user` / `unpause_user` (by discriminator) on the Earn supply position, setting its
/// supply status to `status`. Needs `grant_fluid_pause_rights` first.
fn fluid_user_pause(
    setup: &JuplendBankSetup,
    payer: Pubkey,
    discriminator: [u8; 8],
    status: u8,
    mint: Pubkey,
    position: Pubkey,
) -> FluidIx {
    let mut data = discriminator.to_vec();
    data.extend_from_slice(setup.lending.as_ref());
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(mint.as_ref());
    data.extend_from_slice(&[1, status, 0]); // Some(supply_status), None for borrow
    let accounts = vec![
        AccountMeta::new_readonly(payer, true),
        AccountMeta::new_readonly(derive_juplend_auth_list().0, false),
        AccountMeta::new(position, false),
        AccountMeta::new(
            derive_juplend_borrow_position(&mint, &setup.lending).0,
            false,
        ),
    ];
    (accounts, data)
}

/// Makes the payer a Fluid guardian and gives the Earn position a user class, so the payer can pause
/// the position.
async fn grant_fluid_pause_rights(setup: &JuplendBankSetup) -> anyhow::Result<()> {
    let payer = setup.test_f.payer();
    let auth = derive_juplend_auth_list().0;
    let mut guardians = UPDATE_GUARDIANS_DISCRIMINATOR.to_vec();
    guardians.extend_from_slice(&1u32.to_le_bytes());
    guardians.extend_from_slice(payer.as_ref());
    guardians.push(1);
    let accounts = vec![
        AccountMeta::new_readonly(payer, true),
        AccountMeta::new_readonly(derive_juplend_liquidity().0, false),
        AccountMeta::new(auth, false),
    ];
    send_fluid_instruction(setup, (accounts, guardians)).await?;
    let mut class = UPDATE_USER_CLASS_DISCRIMINATOR.to_vec();
    class.extend_from_slice(&1u32.to_le_bytes());
    class.extend_from_slice(setup.lending.as_ref());
    class.push(0);
    let accounts = vec![
        AccountMeta::new_readonly(payer, true),
        AccountMeta::new(auth, false),
    ];
    send_fluid_instruction(setup, (accounts, class)).await
}

async fn pulse(
    setup: &JuplendBankSetup,
    user: &MarginfiAccountFixture,
    nonce: u32,
) -> anyhow::Result<()> {
    let mut ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::PulseHealth {
            marginfi_account: user.key,
            group: user.load().await.group,
        }
        .to_account_metas(None),
        data: marginfi::instruction::LendingAccountPulseHealth {}.data(),
    };
    ix.accounts
        .extend(user.load_observation_account_metas(vec![], vec![]).await);
    let ctx = setup.test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000 + nonce),
            ix,
        ],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await?,
    );
    ctx.banks_client.process_transaction(tx).await?;
    Ok(())
}

#[tokio::test]
async fn juplend_emergency_propagation_blocks_borrow_and_preserves_maintenance(
) -> anyhow::Result<()> {
    let settings = TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Sol,
            ..Default::default()
        }],
        protocol_fees: false,
    };
    let setup = TestFixture::setup_juplend_bank(Some(settings)).await;
    let (borrower, collateral) = setup.create_user_with_liquidity(10_000.0).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &borrower, collateral.key, 1_000_000_000)
        .await?;
    let lender = setup.test_f.create_marginfi_account().await;
    let sol_bank = setup.test_f.get_bank(&BankMint::Sol);
    let lender_tokens = setup
        .test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    lender
        .try_bank_deposit(lender_tokens.key, sol_bank, 1_000.0, None)
        .await?;
    let destination = setup
        .test_f
        .sol_mint
        .create_token_account_and_mint_to(1.0)
        .await;

    let lending = setup.load_lending().await;
    let reserve = lending.token_reserves_liquidity;
    let position = lending.supply_position_on_liquidity;
    let liquidity = derive_juplend_liquidity().0;
    pulse(&setup, &borrower, 1).await?;
    let initial = borrower.load().await.health_cache;
    assert!(I80F48::from(initial.asset_value) > I80F48::ZERO);

    // Without propagation the bank deliberately continues using its previous state.
    set_bytes(&setup, liquidity, LIQUIDITY_STATUS_OFFSET, &[1]).await;
    pulse(&setup, &borrower, 2).await?;
    assert!(I80F48::from(borrower.load().await.health_cache.asset_value) > I80F48::ZERO);
    propagate(&setup, reserve, position, 1).await?;
    assert!(setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY));
    pulse(&setup, &borrower, 3).await?;
    let halted = borrower.load().await.health_cache;
    assert_eq!(I80F48::from(halted.asset_value), I80F48::ZERO);
    assert_eq!(halted.asset_value_maint, initial.asset_value_maint);
    assert_custom_error!(
        borrower
            .try_bank_borrow(destination.key, sol_bank, 1.0)
            .await
            .unwrap_err(),
        MarginfiError::RiskEngineInitRejected
    );

    // Unpausing global state must not clear a token lock or an Earn position pause.
    set_bytes(
        &setup,
        reserve,
        RESERVE_INTERACTING_PROTOCOL_OFFSET,
        JUPLEND_LIQUIDITY_PROGRAM_ID.as_ref(),
    )
    .await;
    set_bytes(&setup, liquidity, LIQUIDITY_STATUS_OFFSET, &[0]).await;
    propagate(&setup, reserve, position, 2).await?;
    assert!(setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY));
    assert_custom_error!(
        borrower
            .try_bank_borrow_with_nonce(destination.key, sol_bank, 1.0, 102)
            .await
            .unwrap_err(),
        MarginfiError::RiskEngineInitRejected
    );
    set_bytes(&setup, position, SUPPLY_POSITION_STATUS_OFFSET, &[1]).await;
    set_bytes(
        &setup,
        reserve,
        RESERVE_INTERACTING_PROTOCOL_OFFSET,
        Pubkey::default().as_ref(),
    )
    .await;
    propagate(&setup, reserve, position, 3).await?;
    assert!(setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY));
    assert_custom_error!(
        borrower
            .try_bank_borrow_with_nonce(destination.key, sol_bank, 1.0, 103)
            .await
            .unwrap_err(),
        MarginfiError::RiskEngineInitRejected
    );
    set_bytes(&setup, position, SUPPLY_POSITION_STATUS_OFFSET, &[0]).await;
    propagate(&setup, reserve, position, 4).await?;
    assert!(!setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY));
    pulse(&setup, &borrower, 4).await?;
    assert!(I80F48::from(borrower.load().await.health_cache.asset_value) > I80F48::ZERO);
    borrower
        .try_bank_borrow_with_nonce(destination.key, sol_bank, 1.0, 101)
        .await?;

    // Supplying a different account cannot clear an emergency for this bank.
    let err = propagate(&setup, liquidity, position, 5).await.unwrap_err();
    assert_custom_error!(err, MarginfiError::JuplendEmergencyValidationFailed);
    let err = propagate(&setup, reserve, liquidity, 6).await.unwrap_err();
    assert_custom_error!(err, MarginfiError::JuplendEmergencyValidationFailed);

    // An authenticated address with an unexpected owner cannot clear the flag.
    let mut account = setup.test_f.try_load(&liquidity).await?.unwrap();
    let original_owner = account.owner;
    account.owner = Pubkey::new_unique();
    setup
        .test_f
        .context
        .borrow_mut()
        .set_account(&liquidity, &AccountSharedData::from(account.clone()));
    propagate(&setup, reserve, position, 7).await?;
    assert!(setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY));
    account.owner = original_owner;
    setup
        .test_f
        .context
        .borrow_mut()
        .set_account(&liquidity, &AccountSharedData::from(account));
    propagate(&setup, reserve, position, 8).await?;
    assert!(!setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY));
    Ok(())
}

#[tokio::test]
async fn juplend_deployed_pause_token_propagates_and_recovers() -> anyhow::Result<()> {
    let setup = TestFixture::setup_juplend_bank(None).await;
    let (user, tokens) = setup.create_user_with_liquidity(2_000.0).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &user, tokens.key, 1_000_000_000)
        .await?;
    let lending = setup.load_lending().await;
    let mint = lending.mint;
    let reserve = lending.token_reserves_liquidity;
    let position = lending.supply_position_on_liquidity;
    let payer = setup.test_f.payer();

    // Exercise the deployed Liquidity program's real global pause and recovery instructions too.
    for (step, locked) in [true, false].into_iter().enumerate() {
        send_fluid_instruction(&setup, fluid_change_status(payer, locked)).await?;
        propagate(&setup, reserve, position, 10 + step as u32).await?;
        assert_eq!(
            setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY),
            locked
        );
    }

    for (step, discriminator) in [PAUSE_TOKEN_DISCRIMINATOR, UNPAUSE_TOKEN_DISCRIMINATOR]
        .into_iter()
        .enumerate()
    {
        send_fluid_instruction(
            &setup,
            fluid_token_pause(payer, discriminator, mint, reserve),
        )
        .await?;

        let account = setup.test_f.try_load(&reserve).await?.unwrap();
        let expected = if step == 0 {
            JUPLEND_LIQUIDITY_PROGRAM_ID
        } else {
            Pubkey::default()
        };
        assert_eq!(
            &account.data
                [RESERVE_INTERACTING_PROTOCOL_OFFSET..RESERVE_INTERACTING_PROTOCOL_OFFSET + 32],
            expected.as_ref()
        );
        propagate(&setup, reserve, position, 20 + step as u32).await?;
        assert_eq!(
            setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY),
            step == 0
        );
        if step == 0 {
            let err = setup
                .test_f
                .run_juplend_withdraw(&setup.bank_f, &user, tokens.key, 1_000_000, Some(false))
                .await
                .unwrap_err();
            assert_anchor_error!(err, 6043);
        }
    }

    // The Earn supply position can be paused independently of the token reserve.
    grant_fluid_pause_rights(&setup).await?;
    for (step, (discriminator, status)) in [
        (PAUSE_USER_DISCRIMINATOR, 1),
        (UNPAUSE_USER_DISCRIMINATOR, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let ix = fluid_user_pause(&setup, payer, discriminator, status, mint, position);
        send_fluid_instruction(&setup, ix).await?;
        let account = setup.test_f.try_load(&position).await?.unwrap();
        assert_eq!(account.data[SUPPLY_POSITION_STATUS_OFFSET], status);
        propagate(&setup, reserve, position, 30 + step as u32).await?;
        assert_eq!(
            setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY),
            status != 0
        );
        if status != 0 {
            let err = setup
                .test_f
                .run_juplend_withdraw(&setup.bank_f, &user, tokens.key, 1_000_000, Some(false))
                .await
                .unwrap_err();
            assert_anchor_error!(err, 6027);
        }
    }
    Ok(())
}

/// Refreshes the bank's Pyth feed timestamp, then sends Earn `update_rate` and a health pulse in one
/// transaction, so the Lending account is fresh when marginfi prices the bank.
async fn update_rate_and_pulse(
    setup: &JuplendBankSetup,
    user: &MarginfiAccountFixture,
    nonce: u32,
) -> anyhow::Result<()> {
    // Keep the bank's Pyth feed fresh, so the only freshness under test is the Lending account's.
    let feed = setup.bank_f.load().await.config.oracle_keys[0];
    let now = setup.test_f.get_clock().await.unix_timestamp;
    setup.test_f.set_pyth_oracle_timestamp(feed, now).await;
    let lending = setup.load_lending().await;
    let update_rate = Instruction {
        program_id: juplend_mocks::juplend_earn::ID,
        accounts: juplend_mocks::juplend_earn::client::accounts::UpdateRate {
            lending: setup.lending,
            mint: lending.mint,
            f_token_mint: lending.f_token_mint,
            supply_token_reserves_liquidity: lending.token_reserves_liquidity,
            rewards_rate_model: lending.rewards_rate_model,
        }
        .to_account_metas(None),
        data: juplend_mocks::juplend_earn::client::args::UpdateRate {}.data(),
    };
    let mut pulse_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::PulseHealth {
            marginfi_account: user.key,
            group: user.load().await.group,
        }
        .to_account_metas(None),
        data: marginfi::instruction::LendingAccountPulseHealth {}.data(),
    };
    pulse_ix
        .accounts
        .extend(user.load_observation_account_metas(vec![], vec![]).await);
    let ctx = setup.test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000 + nonce),
            update_rate,
            pulse_ix,
        ],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await?,
    );
    ctx.banks_client.process_transaction(tx).await?;
    Ok(())
}

/// Simulates a JupLend withdraw and returns the program that failed and its error code, read from
/// the transaction logs, or `None` if the withdraw would succeed.
async fn simulate_withdraw_failure(
    setup: &JuplendBankSetup,
    user: &MarginfiAccountFixture,
    destination: Pubkey,
    nonce: u32,
) -> Option<(String, String)> {
    let withdraw = user
        .make_juplend_withdraw_ix(destination, &setup.bank_f, 1_000_000, Some(false))
        .await;
    let ctx = setup.test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(2_000_000 + nonce),
            withdraw,
        ],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    let sim = ctx.banks_client.simulate_transaction(tx).await.unwrap();
    sim.result.unwrap().err()?;
    let logs = sim.simulation_details.map(|d| d.logs).unwrap_or_default();
    // The innermost "Program <id> failed: ..." line names the program that raised the error.
    let failed = logs
        .iter()
        .find(|l| l.starts_with("Program ") && l.contains(" failed: "))
        .cloned()
        .unwrap_or_default();
    let code = logs
        .iter()
        .find(|l| l.contains("Error Code:") || l.contains("Error Number:"))
        .cloned()
        .unwrap_or_default();
    Some((failed, code))
}

/// ENGINE_OK and ORACLE_OK: the pulse priced every balance without error.
const PRICED_OK: u32 = ENGINE_OK | ORACLE_OK;

/// Under each of Fluid's real pause instructions (global lockdown, token pause, supply-position
/// pause), Earn `update_rate` keeps refreshing the Lending account, so the JupLend bank stays
/// priceable and keeps its Maintenance value, while Fluid itself rejects the withdrawal.
#[tokio::test]
async fn juplend_fluid_pauses_keep_pricing_and_block_withdrawals() -> anyhow::Result<()> {
    let setup = TestFixture::setup_juplend_bank(None).await;
    let (user, tokens) = setup.create_user_with_liquidity(2_000.0).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &user, tokens.key, 1_000_000_000)
        .await?;
    let lending = setup.load_lending().await;
    let mint = lending.mint;
    let reserve = lending.token_reserves_liquidity;
    let position = lending.supply_position_on_liquidity;
    let payer = setup.test_f.payer();

    setup.test_f.advance_time(5).await;
    update_rate_and_pulse(&setup, &user, 1).await?;
    let baseline = user.load().await.health_cache;
    assert_eq!(
        baseline.flags & PRICED_OK,
        PRICED_OK,
        "baseline: pricing failed"
    );
    let baseline_maint = I80F48::from(baseline.asset_value_maint);
    assert!(baseline_maint > I80F48::ZERO);
    assert!(simulate_withdraw_failure(&setup, &user, tokens.key, 1)
        .await
        .is_none());

    grant_fluid_pause_rights(&setup).await?;
    // (name, pause, resume, Fluid Liquidity error that rejects the withdraw)
    let cases = [
        (
            "global lockdown",
            fluid_change_status(payer, true),
            fluid_change_status(payer, false),
            "ProtocolLockdown",
        ),
        (
            "token pause",
            fluid_token_pause(payer, PAUSE_TOKEN_DISCRIMINATOR, mint, reserve),
            fluid_token_pause(payer, UNPAUSE_TOKEN_DISCRIMINATOR, mint, reserve),
            "TokenLockDown",
        ),
        (
            "supply position pause",
            fluid_user_pause(&setup, payer, PAUSE_USER_DISCRIMINATOR, 1, mint, position),
            fluid_user_pause(&setup, payer, UNPAUSE_USER_DISCRIMINATOR, 0, mint, position),
            "UserPaused",
        ),
    ];

    for (i, (name, pause, resume, error)) in cases.into_iter().enumerate() {
        let nonce = 10 * (i as u32 + 1);
        send_fluid_instruction(&setup, pause).await?;
        propagate(&setup, reserve, position, nonce).await?;
        assert!(
            setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY),
            "{name}"
        );

        // Lending is stale before the refresh, so pricing depends on update_rate succeeding.
        setup.test_f.advance_time(5).await;
        assert!(setup
            .load_lending()
            .await
            .is_stale(setup.test_f.get_clock().await.unix_timestamp));
        update_rate_and_pulse(&setup, &user, nonce + 1).await?;
        let cache = user.load().await.health_cache;
        assert_eq!(cache.flags & PRICED_OK, PRICED_OK, "{name}: pricing failed");
        assert_eq!(I80F48::from(cache.asset_value), I80F48::ZERO, "{name}");
        let maint = I80F48::from(cache.asset_value_maint);
        // The exchange price only accrues, so Maintenance may rise slightly but never fall.
        assert!(
            maint >= baseline_maint
                && maint - baseline_maint <= baseline_maint / I80F48::from_num(10_000),
            "{name}: maintenance value moved from {baseline_maint} to {maint}"
        );

        let (failed, code) = simulate_withdraw_failure(&setup, &user, tokens.key, nonce + 2)
            .await
            .unwrap_or_else(|| panic!("{name}: withdraw succeeded"));
        assert!(
            failed.contains(&JUPLEND_LIQUIDITY_PROGRAM_ID.to_string()),
            "{name}: withdraw rejected by an unexpected program: {failed}"
        );
        assert!(
            code.contains(&format!("Error Code: {error}.")),
            "{name}: unexpected error: {code}"
        );

        send_fluid_instruction(&setup, resume).await?;
        propagate(&setup, reserve, position, nonce + 3).await?;
        assert!(
            !setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY),
            "{name}"
        );
        // Fluid itself has recovered, not just our flag.
        assert!(
            simulate_withdraw_failure(&setup, &user, tokens.key, nonce + 4)
                .await
                .is_none(),
            "{name}: withdraw still fails after resume"
        );
    }
    Ok(())
}
