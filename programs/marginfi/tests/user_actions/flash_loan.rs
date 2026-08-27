use anchor_lang::prelude::AccountMeta;
use anchor_lang::solana_program::{instruction::Instruction, pubkey::Pubkey};
use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::{assert_custom_error, bank::BankFixture, prelude::*};
use marginfi::prelude::*;
use marginfi_type_crate::constants::{
    INSURANCE_VAULT_AUTHORITY_SEED, INSURANCE_VAULT_SEED, LIQUIDITY_VAULT_SEED,
};
use pretty_assertions::assert_eq;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::*;
use solana_sdk::signature::Keypair;
use solana_sdk::{signer::Signer, transaction::Transaction};
use solana_system_interface::program as system_program;

// Flashloan tests
// 1. Flashloan success (1 action)
// 2. Flashloan success (3 actions)
// 3. Flashloan fails because of bad account health
// 4. Flashloan fails because of non whitelisted account
// 5. Flashloan fails because of missing `end_flashloan` ix
// 6. Flashloan fails because of invalid instructions sysvar
// 7. Flashloan fails because of invalid `end_flashloan` ix order
// 8. Flashloan fails because `end_flashloan` ix is for another account
// 9. Flashloan fails because account is already in a flashloan
// 10. Flashloan fails because account transfer during flashloan

#[tokio::test]
async fn flashloan_success_1op() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;
    // Borrow SOL
    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let repay_ix = borrower_mfi_account_f
        .make_repay_ix(
            borrower_token_account_f_sol.key,
            sol_bank,
            1_000,
            Some(true),
        )
        .await;

    let flash_loan_result = borrower_mfi_account_f
        .try_flashloan(vec![borrow_ix, repay_ix], vec![], vec![], None)
        .await;

    assert!(flash_loan_result.is_ok());

    Ok(())
}

#[tokio::test]
async fn flashloan_success_3op() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;

    // Create borrow and repay instructions
    let mut ixs = Vec::new();
    for _ in 0..3 {
        let borrow_ix = borrower_mfi_account_f
            .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
            .await;
        ixs.push(borrow_ix);

        let repay_ix = borrower_mfi_account_f
            .make_repay_ix(
                borrower_token_account_f_sol.key,
                sol_bank,
                1_000,
                Some(true),
            )
            .await;
        ixs.push(repay_ix);
    }

    ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(1_400_000));

    let flash_loan_result = borrower_mfi_account_f
        .try_flashloan(ixs, vec![], vec![], None)
        .await;

    assert!(flash_loan_result.is_ok());

    Ok(())
}

#[tokio::test]
async fn flashloan_fail_account_health() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;
    // Borrow SOL
    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let flash_loan_result = borrower_mfi_account_f
        .try_flashloan(vec![borrow_ix], vec![], vec![sol_bank.key], None)
        .await;

    assert_custom_error!(
        flash_loan_result.unwrap_err(),
        MarginfiError::RiskEngineInitRejected
    );

    Ok(())
}

#[tokio::test]
// Note: The flashloan flag is now deprecated
async fn flashloan_ok_missing_flag() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;
    // Borrow SOL

    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let repay_ix = borrower_mfi_account_f
        .make_repay_ix(
            borrower_token_account_f_sol.key,
            sol_bank,
            1_000,
            Some(true),
        )
        .await;

    let flash_loan_result = borrower_mfi_account_f
        .try_flashloan(vec![borrow_ix, repay_ix], vec![], vec![], None)
        .await;

    assert!(flash_loan_result.is_ok());

    Ok(())
}

#[tokio::test]
async fn flashloan_fail_missing_fe_ix() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;
    // Borrow SOL

    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let repay_ix = borrower_mfi_account_f
        .make_repay_ix(
            borrower_token_account_f_sol.key,
            sol_bank,
            1_000,
            Some(true),
        )
        .await;

    let mut ixs = vec![borrow_ix, repay_ix];

    let start_ix = borrower_mfi_account_f
        .make_lending_account_start_flashloan_ix(ixs.len() as u64)
        .await;

    ixs.insert(0, start_ix);

    let ctx = test_f.context.borrow_mut();

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&ctx.payer.pubkey().clone()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );

    let res = ctx.banks_client.process_transaction(tx).await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalFlashloan);

    Ok(())
}

#[tokio::test]
async fn flashloan_fail_missing_invalid_sysvar_ixs() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;
    // Borrow SOL

    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let repay_ix = borrower_mfi_account_f
        .make_repay_ix(
            borrower_token_account_f_sol.key,
            sol_bank,
            1_000,
            Some(true),
        )
        .await;

    let mut ixs = vec![borrow_ix, repay_ix];

    let start_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::LendingAccountStartFlashloan {
            marginfi_account: borrower_mfi_account_f.key,
            authority: test_f.context.borrow().payer.pubkey(),
            ixs_sysvar: Pubkey::default(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingAccountStartFlashloan {
            end_index: ixs.len() as u64 + 1,
        }
        .data(),
    };

    let end_ix = borrower_mfi_account_f
        .make_lending_account_end_flashloan_ix(vec![], vec![])
        .await;

    ixs.insert(0, start_ix);
    ixs.push(end_ix);

    let ctx = test_f.context.borrow_mut();

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&ctx.payer.pubkey().clone()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );

    let res = ctx.banks_client.process_transaction(tx).await;

    assert!(res.is_err());

    Ok(())
}

#[tokio::test]
async fn flashloan_fail_invalid_end_fl_order() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;
    // Borrow SOL

    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let mut ixs = vec![borrow_ix];

    let start_ix = borrower_mfi_account_f
        .make_lending_account_start_flashloan_ix(0)
        .await;

    let end_ix = borrower_mfi_account_f
        .make_lending_account_end_flashloan_ix(vec![], vec![])
        .await;

    ixs.insert(0, start_ix);
    ixs.insert(0, end_ix);

    let ctx = test_f.context.borrow_mut();

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&ctx.payer.pubkey().clone()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );

    let res = ctx.banks_client.process_transaction(tx).await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalFlashloan);

    Ok(())
}

#[tokio::test]
async fn flashloan_fail_invalid_end_fl_different_m_account() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;
    // Borrow SOL

    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let mut ixs = vec![borrow_ix];

    let start_ix = borrower_mfi_account_f
        .make_lending_account_start_flashloan_ix(ixs.len() as u64 + 1)
        .await;

    let end_ix = lender_mfi_account_f
        .make_lending_account_end_flashloan_ix(vec![], vec![])
        .await;

    ixs.insert(0, start_ix);
    ixs.push(end_ix);

    let ctx = test_f.context.borrow_mut();

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&ctx.payer.pubkey().clone()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );

    let res = ctx.banks_client.process_transaction(tx).await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalFlashloan);

    Ok(())
}

#[tokio::test]
async fn flashloan_fail_already_in_flashloan() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;
    // Borrow SOL

    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let mut ixs = vec![borrow_ix];

    let start_ix = borrower_mfi_account_f
        .make_lending_account_start_flashloan_ix(ixs.len() as u64 + 2)
        .await;

    let end_ix = borrower_mfi_account_f
        .make_lending_account_end_flashloan_ix(vec![], vec![])
        .await;

    ixs.insert(0, start_ix.clone());
    ixs.insert(0, start_ix.clone());
    ixs.push(end_ix);

    let ctx = test_f.context.borrow_mut();

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&ctx.payer.pubkey().clone()),
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );

    let res = ctx.banks_client.process_transaction(tx).await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalFlashloan);

    Ok(())
}

#[tokio::test]
async fn flashloan_fail_account_transfer_during_flashloan() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;

    // Borrow SOL
    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let new_authority = Keypair::new();
    let new_account = Keypair::new();

    let account = borrower_mfi_account_f.load().await;

    let transfer_account_ix = Instruction {
        program_id: marginfi::ID,
        accounts: marginfi::accounts::TransferToNewAccount {
            old_marginfi_account: borrower_mfi_account_f.key,
            new_marginfi_account: new_account.pubkey(),
            group: account.group,
            authority: test_f.payer(),
            fee_payer: test_f.payer(),
            new_authority: new_authority.pubkey(),
            global_fee_wallet: test_f.marginfi_group.fee_wallet,
            fee_state: test_f.marginfi_group.fee_state,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: marginfi::instruction::TransferToNewAccount {}.data(),
    };

    let flash_loan_result = borrower_mfi_account_f
        .try_flashloan(
            vec![borrow_ix, transfer_account_ix],
            vec![],
            vec![sol_bank.key],
            Some(&new_account),
        )
        .await;

    assert_custom_error!(
        flash_loan_result.unwrap_err(),
        MarginfiError::AccountInFlashloan
    );

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
async fn flashloan_fail_bankruptcy_during_flashloan() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;

    // Borrow SOL
    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_sol.key, sol_bank, 1_000)
        .await;

    let bank_pk = sol_bank.key;
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

    // A sneaky trick attackers might try to pull is to simply bankrupt what they borrowed instead
    // of returning it...
    let payer = test_f.payer();
    let cpi_accounts = mocks::accounts::HandleBankruptcyViaCpi {
        group: test_f.marginfi_group.key,
        signer: payer,
        bank: bank_pk,
        marginfi_account: borrower_mfi_account_f.key,
        liquidity_vault,
        insurance_vault,
        insurance_vault_authority,
        token_program: anchor_spl::token::ID,
        marginfi_program: marginfi::ID,
    };

    let mut metas = create_handle_bankruptcy_cpi_metas(&cpi_accounts);
    let remaining = borrower_mfi_account_f
        .load_observation_account_metas(vec![], vec![])
        .await;

    metas.extend_from_slice(&remaining);

    let mut bankrupt_via_cpi_ix = Instruction {
        program_id: mocks::id(),
        accounts: metas,
        data: mocks::instruction::HandleBankruptcy {}.data(),
    };

    bankrupt_via_cpi_ix.accounts.extend_from_slice(&[]);

    let flash_loan_result = borrower_mfi_account_f
        .try_flashloan(
            vec![borrow_ix, bankrupt_via_cpi_ix],
            vec![],
            vec![sol_bank.key],
            None,
        )
        .await;

    assert_custom_error!(
        flash_loan_result.unwrap_err(),
        MarginfiError::AccountInFlashloan
    );

    Ok(())
}

/// Seeds `sol_bank`'s circuit-breaker reference from `PYTH_SOL_FEED` at $10, then moves the live
/// price 10% past that reference (tier-0 is 5%). Nothing pulses the bank after the move, so it
/// never halts and the breach is visible only to the live price gate.
async fn seed_cb_reference_then_breach(
    test_f: &TestFixture,
    sol_bank: &BankFixture,
) -> anyhow::Result<()> {
    const WARM_SLOT: u64 = 1_000;
    const WARM_TIME: i64 = 100;

    test_f.set_clock(WARM_SLOT, WARM_TIME).await;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 10_000_000_000, 0, WARM_TIME)
        .await;
    test_f
        .set_pyth_oracle_price_native(PYTH_USDC_FEED, 1_000_000, 0, WARM_TIME)
        .await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;
    sol_bank.update_config(standard_cb_config(), None).await?;

    test_f.set_clock(WARM_SLOT + 10, WARM_TIME + 1).await;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 11_000_000_000, 0, WARM_TIME + 1)
        .await;
    test_f
        .set_pyth_oracle_price_native(PYTH_USDC_FEED, 1_000_000, 0, WARM_TIME + 1)
        .await;

    Ok(())
}

#[tokio::test]
// A bank that joins the account after the borrow is invisible to every inline gate.
async fn flashloan_fail_cb_gate_on_bank_added_after_borrow() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    seed_cb_reference_then_breach(&test_f, sol_bank).await?;

    // Fund USDC lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_f_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_f_usdc.key, usdc_bank, 1_000, None)
        .await?;

    // 105 USDC against 10 SOL passes initial health only at the breached $11, not at the $10
    // reference. On the normal ordering the borrow's inline gate sees the SOL bank and rejects.
    let control_mfi_account_f = test_f.create_marginfi_account().await;
    let control_token_account_f_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let control_token_account_f_usdc = test_f.usdc_mint.create_empty_token_account().await;
    control_mfi_account_f
        .try_bank_deposit(control_token_account_f_sol.key, sol_bank, 10, None)
        .await?;

    let control_result = control_mfi_account_f
        .try_bank_borrow(control_token_account_f_usdc.key, usdc_bank, 105)
        .await;

    assert_custom_error!(
        control_result.unwrap_err(),
        MarginfiError::CircuitBreakerPriceJump
    );

    // Same observation and same amounts, but the SOL balance is opened after the borrow
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_f_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let borrower_token_account_f_usdc = test_f.usdc_mint.create_empty_token_account().await;

    let borrow_ix = borrower_mfi_account_f
        .make_bank_borrow_ix(borrower_token_account_f_usdc.key, usdc_bank, 105)
        .await;
    let deposit_ix = borrower_mfi_account_f
        .make_deposit_ix(borrower_token_account_f_sol.key, sol_bank, 10, None)
        .await;

    let flash_loan_result = borrower_mfi_account_f
        .try_flashloan(
            vec![borrow_ix, deposit_ix],
            vec![],
            vec![usdc_bank.key, sol_bank.key],
            None,
        )
        .await;

    assert_custom_error!(
        flash_loan_result.unwrap_err(),
        MarginfiError::CircuitBreakerPriceJump
    );

    Ok(())
}

#[tokio::test]
// An envelope that ends with no liabilities carries no price risk and stays ungated.
async fn flashloan_ok_no_liabilities_during_cb_breach() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    seed_cb_reference_then_breach(&test_f, sol_bank).await?;

    let depositor_mfi_account_f = test_f.create_marginfi_account().await;
    let depositor_token_account_f_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    depositor_mfi_account_f
        .try_bank_deposit(depositor_token_account_f_sol.key, sol_bank, 9, None)
        .await?;

    let deposit_ix = depositor_mfi_account_f
        .make_deposit_ix(depositor_token_account_f_sol.key, sol_bank, 1, None)
        .await;
    let withdraw_ix = depositor_mfi_account_f
        .make_bank_withdraw_ix(depositor_token_account_f_sol.key, sol_bank, 1, None)
        .await;

    let flash_loan_result = depositor_mfi_account_f
        .try_flashloan(
            vec![deposit_ix, withdraw_ix],
            vec![],
            vec![sol_bank.key],
            None,
        )
        .await;

    assert!(flash_loan_result.is_ok());

    Ok(())
}
