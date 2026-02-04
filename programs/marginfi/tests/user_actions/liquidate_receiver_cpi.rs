use anchor_lang::{
    prelude::*, solana_program::instruction::Instruction, solana_program::sysvar, InstructionData,
};
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::{errors::MarginfiError, state::marginfi_account::MarginfiAccountImpl};
use marginfi_type_crate::{
    constants::{
        INSURANCE_VAULT_AUTHORITY_SEED, INSURANCE_VAULT_SEED, LIQUIDATION_RECORD_SEED,
        LIQUIDITY_VAULT_SEED,
    },
    types::{BankConfigOpt, ACCOUNT_IN_RECEIVERSHIP},
};
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signer::Signer, transaction::Transaction};

fn create_start_liq_cpi_metas(
    accounts: &mocks::accounts::StartLiquidationViaCpi,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(accounts.marginfi_account, false),
        AccountMeta::new(accounts.liquidation_record, false),
        AccountMeta::new_readonly(accounts.liquidation_receiver, false),
        AccountMeta::new_readonly(accounts.instructions_sysvar, false),
        AccountMeta::new_readonly(accounts.marginfi_program, false),
    ]
}

#[tokio::test]
async fn liquidate_start_then_cpi_start_on_different_accounts_exploit() -> anyhow::Result<()> {
    // Standard fixture with non-admin payer for banks (matches other tests in this module)
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // Banks
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Liquidator (provides liquidity)
    let liquidator = test_f.create_marginfi_account().await;
    let liq_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    let liq_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(5).await;
    liquidator
        .try_bank_deposit(liq_usdc_acc.key, usdc_bank, 100.0, None)
        .await?;

    // Two separate liquidatees: A (top-level start) and B (CPI start)
    let liquidatee_a = test_f.create_marginfi_account().await;
    let liquidatee_b = test_f.create_marginfi_account().await;

    // Make both accounts unhealthy
    let user_sol_a = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_usdc_a = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee_a
        .try_bank_deposit(user_sol_a.key, sol_bank, 2.0, None)
        .await?;
    liquidatee_a
        .try_bank_borrow(user_usdc_a.key, usdc_bank, 10.0)
        .await?;

    let user_sol_b = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let user_usdc_b = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee_b
        .try_bank_deposit(user_sol_b.key, sol_bank, 2.0, None)
        .await?;
    liquidatee_b
        .try_bank_borrow(user_usdc_b.key, usdc_bank, 10.0)
        .await?;

    // Degrade SOL weights so both become liquidatable
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

    // Derive liquidation record PDAs for A and B
    let (record_a, _bump_a) = Pubkey::find_program_address(
        &[
            LIQUIDATION_RECORD_SEED.as_bytes(),
            liquidatee_a.key.as_ref(),
        ],
        &marginfi::ID,
    );
    let (record_b, _bump_b) = Pubkey::find_program_address(
        &[
            LIQUIDATION_RECORD_SEED.as_bytes(),
            liquidatee_b.key.as_ref(),
        ],
        &marginfi::ID,
    );

    // Init liquidation record for both accounts
    {
        let ctx = test_f.context.borrow_mut();
        for (who, rec) in [(&liquidatee_a, record_a), (&liquidatee_b, record_b)] {
            let init_ix = who
                .make_init_liquidation_record_ix(rec, ctx.payer.pubkey())
                .await;
            let tx = Transaction::new_signed_with_payer(
                &[init_ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer],
                ctx.last_blockhash,
            );
            ctx.banks_client
                .process_transaction_with_preflight(tx)
                .await?;
        }
    } // release borrow

    // The "normal" start for A
    let payer = test_f.payer();
    let start_ix_a = liquidatee_a
        .make_start_liquidation_ix(record_a, payer)
        .await;

    // The shifty start-via-CPI for B
    let start_via_cpi_accounts = mocks::accounts::StartLiquidationViaCpi {
        marginfi_account: liquidatee_b.key,
        liquidation_record: record_b,
        liquidation_receiver: test_f.payer(), // arbitrary receiver
        instructions_sysvar: sysvar::instructions::id(),
        marginfi_program: marginfi::ID,
    };
    let mut start_via_cpi_ix = Instruction {
        program_id: mocks::id(),
        accounts: create_start_liq_cpi_metas(&start_via_cpi_accounts),
        data: mocks::instruction::StartLiquidationViaCpi {}.data(),
    };
    let meta = &liquidatee_b
        .load_observation_account_metas(vec![], vec![])
        .await;
    start_via_cpi_ix.accounts.extend_from_slice(&meta);

    // withdraw/repay genuine for A
    let withdraw_ix = liquidatee_a
        .make_bank_withdraw_ix(liq_sol_acc.key, sol_bank, 0.09, None, true)
        .await;
    let repay_ix = liquidatee_a
        .make_bank_repay_ix(liq_usdc_acc.key, usdc_bank, 2.0, None)
        .await;

    // End for A
    let end_ix_a = liquidatee_a
        .make_end_liquidation_ix(
            record_a,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    // NOTE: No end for B! Sneaky sneaky!

    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[
                start_ix_a,
                start_via_cpi_ix,
                withdraw_ix,
                repay_ix,
                end_ix_a,
            ],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );

        // Should fail: CPI call doesn't appear as a top-level mrgn ix, so introspection of the
        // second start will fail. Otherwise, this account would be in receivership with no paired
        // end ix, leaving it in that state forever!
        let res = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(res.is_err());
        assert_custom_error!(res.unwrap_err(), MarginfiError::ForbiddenIx);
    }

    // Sanity: receivership flag should not remain set on either account after tx fails.
    let ma_a = liquidatee_a.load().await;
    let ma_b = liquidatee_b.load().await;
    assert_eq!(ma_a.get_flag(ACCOUNT_IN_RECEIVERSHIP), false);
    assert_eq!(ma_b.get_flag(ACCOUNT_IN_RECEIVERSHIP), false);

    Ok(())
}

fn create_handle_bankruptcy_cpi_metas(
    accounts: &mocks::accounts::HandleBankruptcyViaCpi,
) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(accounts.group, false),
        AccountMeta::new_readonly(accounts.signer, true),
        AccountMeta::new(accounts.bank, false),
        AccountMeta::new(accounts.marginfi_account, false),
        AccountMeta::new(accounts.liquidity_vault, false),
        AccountMeta::new(accounts.insurance_vault, false),
        AccountMeta::new_readonly(accounts.insurance_vault_authority, false),
        AccountMeta::new_readonly(accounts.token_program, false),
        AccountMeta::new_readonly(accounts.marginfi_program, false),
    ]
}

#[tokio::test]
async fn handle_bankruptcy_via_cpi_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // The victim in this exploit is any depositor who will absorb the socialized loss
    let victim = test_f.create_marginfi_account().await;
    let victim_usdc_acc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    victim
        .try_bank_deposit(victim_usdc_acc.key, usdc_bank, 100.0, None)
        .await?;

    let attacker_acc = test_f.create_marginfi_account().await;
    let attacker_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let attacker_usdc = test_f.usdc_mint.create_empty_token_account().await;
    let deposit_amt = 2.0;
    attacker_acc
        .try_bank_deposit(attacker_sol.key, sol_bank, deposit_amt, None)
        .await?;
    attacker_acc
        .try_bank_borrow(attacker_usdc.key, usdc_bank, 4.0)
        .await?;

    // Make the account unhealthy (not quite bankrupt!) by degrading SOL weights. Normally an
    // attacker must render themselves unhealthy by making some silly investment.
    sol_bank
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.01).into()),
                asset_weight_maint: Some(I80F48!(0.02).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    // Init liquidation record
    let (rec, _bump) = Pubkey::find_program_address(
        &[
            LIQUIDATION_RECORD_SEED.as_bytes(),
            attacker_acc.key.as_ref(),
        ],
        &marginfi::ID,
    );
    {
        let ctx = test_f.context.borrow_mut();
        let init_ix = attacker_acc
            .make_init_liquidation_record_ix(rec, ctx.payer.pubkey())
            .await;
        let tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    } // release borrow

    let payer = test_f.payer();
    let start_ix = attacker_acc.make_start_liquidation_ix(rec, payer).await;

    // Withdraw THE ENTIRE AMOUNT
    // * Note: Attacker has to liquidate their own account for this exploit to work, otherwise it's
    // still useful (attacker claims the whole deposit amount) but less effective (they don't get to
    // keep the entire borrow, the user does), of course the damage to the protocol is equal in both
    // events.
    let withdraw_ix = attacker_acc
        .make_bank_withdraw_ix(attacker_sol.key, sol_bank, deposit_amt, Some(true), true)
        .await;
    // Note: no repay required! We're going to clear that debt throught bankruptcy instead!
    let end_ix = attacker_acc
        .make_end_liquidation_ix(
            rec,
            payer,
            test_f.marginfi_group.fee_state,
            test_f.marginfi_group.fee_wallet,
            vec![],
        )
        .await;

    let bank_pk = usdc_bank.key;
    let (liquidity_vault, _) = Pubkey::find_program_address(
        &[LIQUIDITY_VAULT_SEED.as_bytes(), bank_pk.as_ref()],
        &marginfi::ID,
    );
    let (insurance_vault, _) = Pubkey::find_program_address(
        &[INSURANCE_VAULT_SEED.as_bytes(), bank_pk.as_ref()],
        &marginfi::ID,
    );
    let (insurance_vault_authority, _) = Pubkey::find_program_address(
        &[INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(), bank_pk.as_ref()],
        &marginfi::ID,
    );

    // Note: bankruptcy is restricted at the top level, but we can call it by CPI, because
    // bankruptcy itself doesn't ban CPI calls.
    let payer = test_f.payer();
    let cpi_accounts = mocks::accounts::HandleBankruptcyViaCpi {
        group: test_f.marginfi_group.key,
        signer: payer,
        bank: bank_pk,
        marginfi_account: attacker_acc.key,
        liquidity_vault,
        insurance_vault,
        insurance_vault_authority,
        token_program: anchor_spl::token::ID,
        marginfi_program: marginfi::ID,
    };

    let mut metas = create_handle_bankruptcy_cpi_metas(&cpi_accounts);
    let remaining = attacker_acc
        .load_observation_account_metas(vec![], vec![])
        .await;

    metas.extend_from_slice(&remaining);

    let mut bankrupt_via_cpi_ix = Instruction {
        program_id: mocks::id(),
        accounts: metas,
        data: mocks::instruction::HandleBankruptcy {}.data(),
    };

    bankrupt_via_cpi_ix.accounts.extend_from_slice(&[]);

    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[start_ix, withdraw_ix, bankrupt_via_cpi_ix, end_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            ctx.last_blockhash,
        );
        let result = ctx
            .banks_client
            .process_transaction_with_preflight(tx)
            .await;
        assert!(result.is_err());
    }

    Ok(())
}
