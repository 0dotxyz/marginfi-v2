use anchor_lang::{InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_anchor_error, assert_custom_error, prelude::*};
use marginfi::{errors::MarginfiError, state::bank::BankImpl};
use marginfi_type_crate::{
    constants::JUPLEND_EMERGENCY,
    pdas::{
        derive_juplend_auth_list, derive_juplend_borrow_position, derive_juplend_liquidity,
        JUPLEND_LIQUIDITY_PROGRAM_ID,
    },
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

async fn send_fluid_instruction(
    setup: &JuplendBankSetup,
    accounts: Vec<AccountMeta>,
    data: Vec<u8>,
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
    set_bytes(&setup, liquidity, 72, &[1]).await;
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
    set_bytes(&setup, reserve, 144, JUPLEND_LIQUIDITY_PROGRAM_ID.as_ref()).await;
    set_bytes(&setup, liquidity, 72, &[0]).await;
    propagate(&setup, reserve, position, 2).await?;
    assert!(setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY));
    assert_custom_error!(
        borrower
            .try_bank_borrow_with_nonce(destination.key, sol_bank, 1.0, 102)
            .await
            .unwrap_err(),
        MarginfiError::RiskEngineInitRejected
    );
    set_bytes(&setup, position, 123, &[1]).await;
    set_bytes(&setup, reserve, 144, Pubkey::default().as_ref()).await;
    propagate(&setup, reserve, position, 3).await?;
    assert!(setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY));
    assert_custom_error!(
        borrower
            .try_bank_borrow_with_nonce(destination.key, sol_bank, 1.0, 103)
            .await
            .unwrap_err(),
        MarginfiError::RiskEngineInitRejected
    );
    set_bytes(&setup, position, 123, &[0]).await;
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

/// Requires BPF_OUT_DIR with the deployed Fluid Liquidity and Earn binaries overriding the
/// repository fixtures. The older checked-in Liquidity fixture has no pause_token instruction.
#[tokio::test]
#[ignore = "requires deployed Fluid Liquidity and Earn binaries in BPF_OUT_DIR"]
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

    // Exercise the deployed Liquidity program's real global pause and recovery instructions too.
    for (step, locked) in [true, false].into_iter().enumerate() {
        let ctx = setup.test_f.context.borrow_mut();
        let mut data = vec![236, 145, 131, 228, 227, 17, 192, 255];
        data.push(u8::from(locked));
        let ix = Instruction {
            program_id: JUPLEND_LIQUIDITY_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new_readonly(ctx.payer.pubkey(), true),
                AccountMeta::new(derive_juplend_liquidity().0, false),
                AccountMeta::new_readonly(derive_juplend_auth_list().0, false),
            ],
            data,
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        ctx.banks_client.process_transaction(tx).await?;
        drop(ctx);
        propagate(&setup, reserve, position, 10 + step as u32).await?;
        assert_eq!(
            setup.bank_f.load().await.get_flag(JUPLEND_EMERGENCY),
            locked
        );
    }

    for (step, discriminator) in [
        [226, 150, 72, 211, 159, 51, 226, 39], // pause_token
        [108, 117, 62, 30, 200, 92, 255, 202], // unpause_token
    ]
    .into_iter()
    .enumerate()
    {
        let ctx = setup.test_f.context.borrow_mut();
        let mut data = discriminator.to_vec();
        data.extend_from_slice(mint.as_ref());
        let ix = Instruction {
            program_id: JUPLEND_LIQUIDITY_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new_readonly(ctx.payer.pubkey(), true),
                AccountMeta::new_readonly(derive_juplend_auth_list().0, false),
                AccountMeta::new(reserve, false),
            ],
            data,
        };
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        ctx.banks_client.process_transaction(tx).await?;
        drop(ctx);

        let account = setup.test_f.try_load(&reserve).await?.unwrap();
        let expected = if step == 0 {
            JUPLEND_LIQUIDITY_PROGRAM_ID
        } else {
            Pubkey::default()
        };
        assert_eq!(&account.data[144..176], expected.as_ref());
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
    let payer = setup.test_f.payer();
    let auth = derive_juplend_auth_list().0;
    let mut guardians = vec![43, 62, 250, 138, 141, 117, 132, 97, 1, 0, 0, 0];
    guardians.extend_from_slice(payer.as_ref());
    guardians.push(1);
    send_fluid_instruction(
        &setup,
        vec![
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new_readonly(derive_juplend_liquidity().0, false),
            AccountMeta::new(auth, false),
        ],
        guardians,
    )
    .await?;
    let mut class = vec![12, 206, 68, 135, 63, 212, 48, 119, 1, 0, 0, 0];
    class.extend_from_slice(setup.lending.as_ref());
    class.push(0);
    send_fluid_instruction(
        &setup,
        vec![
            AccountMeta::new_readonly(payer, true),
            AccountMeta::new(auth, false),
        ],
        class,
    )
    .await?;
    let borrow_position = derive_juplend_borrow_position(&mint, &setup.lending).0;
    for (step, (discriminator, status)) in [
        ([18, 63, 43, 94, 239, 53, 101, 14], 1),
        ([71, 115, 128, 252, 182, 126, 234, 62], 0),
    ]
    .into_iter()
    .enumerate()
    {
        let mut data = discriminator.to_vec();
        data.extend_from_slice(setup.lending.as_ref());
        data.extend_from_slice(mint.as_ref());
        data.extend_from_slice(mint.as_ref());
        data.extend_from_slice(&[1, status, 0]); // Some(supply_status), None for borrow
        send_fluid_instruction(
            &setup,
            vec![
                AccountMeta::new_readonly(payer, true),
                AccountMeta::new_readonly(auth, false),
                AccountMeta::new(position, false),
                AccountMeta::new(borrow_position, false),
            ],
            data,
        )
        .await?;
        let account = setup.test_f.try_load(&position).await?.unwrap();
        assert_eq!(account.data[123], status);
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
