mod bank_cache;
mod borrow;
mod close_account;
mod close_balance;
mod create_account;
mod create_account_pda;
mod create_account_pda_cpi;
mod deposit;
mod emissions;
mod flash_loan;
mod kamino;
mod limit_orders;
mod limit_orders_common;
mod limit_orders_multi;
mod liquidate;
mod liquidate_receiver;
mod liquidate_receiver_cpi;
mod order;
mod panic_mode_user_interactions;
mod repay;
mod transfer_account_pda;
mod withdraw;

use anchor_lang::prelude::Clock;
use fixed::types::I80F48;
use fixtures::{assert_eq_noise, native, prelude::*};
use marginfi::state::{bank::BankImpl, marginfi_account::BankAccountWrapper};
use pretty_assertions::assert_eq;
use solana_program_test::*;

#[tokio::test]
async fn automatic_interest_payments() -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    // Create lender user accounts and deposit SOL asset
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_bank_f, 1_000, None)
        .await?;

    // Create borrower user accounts and deposit USDC asset
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_usdc.key, usdc_bank_f, 1_000, None)
        .await?;

    // Borrow SOL from borrower mfi account
    borrower_mfi_account_f
        .try_bank_borrow(lender_token_account_sol.key, sol_bank_f, 99)
        .await?;

    // Let a year go by
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 year
        clock.unix_timestamp += 365 * 24 * 60 * 60;
        ctx.set_sysvar(&clock);
    }

    // Repay principal, leaving only the accrued interest
    borrower_mfi_account_f
        .try_bank_repay(lender_token_account_sol.key, sol_bank_f, 99, None)
        .await?;

    let sol_bank = sol_bank_f.load().await;
    let borrower_mfi_account = borrower_mfi_account_f.load().await;
    let lender_mfi_account = lender_mfi_account_f.load().await;

    // Due to balances sorting, SOL may be not at index 1 -> determine its actual index first
    let sol_index = borrower_mfi_account
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == sol_bank_f.key)
        .unwrap();

    // Verify that interest accrued matches on both sides
    assert_eq_noise!(
        sol_bank
            .get_liability_amount(
                borrower_mfi_account.lending_account.balances[sol_index]
                    .liability_shares
                    .into()
            )
            .unwrap(),
        I80F48::from(native!(11.761, "SOL", f64)),
        native!(0.0002, "SOL", f64)
    );

    assert_eq_noise!(
        sol_bank
            .get_asset_amount(
                lender_mfi_account.lending_account.balances[0]
                    .asset_shares
                    .into()
            )
            .unwrap(),
        I80F48::from(native!(1011.761, "SOL", f64)),
        native!(0.0002, "SOL", f64)
    );
    // TODO: check health is sane

    Ok(())
}

// Regression

#[tokio::test]
async fn marginfi_account_correct_balance_selection_after_closing_position() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_bank_f, 1_000, None)
        .await?;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(2_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 2_000, None)
        .await?;

    lender_mfi_account_f
        .try_bank_withdraw(lender_token_account_sol.key, sol_bank_f, 0, Some(true))
        .await
        .unwrap();

    let mut marginfi_account = lender_mfi_account_f.load().await;
    let mut usdc_bank = usdc_bank_f.load().await;

    let bank_account = BankAccountWrapper::find(
        &usdc_bank_f.key,
        &mut usdc_bank,
        &mut marginfi_account.lending_account,
    );

    assert!(bank_account.is_ok());

    let bank_account = bank_account.unwrap();

    assert_eq!(
        bank_account
            .bank
            .get_asset_amount(bank_account.balance.asset_shares.into())
            .unwrap()
            .to_num::<u64>(),
        native!(2_000, "USDC")
    );

    Ok(())
}

#[tokio::test]
async fn set_emissions_destination_account() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let account_f = test_f.create_marginfi_account().await;

    // Default is Pubkey::default
    let account = account_f.load().await;
    assert_eq!(
        account.emissions_destination_account,
        solana_sdk::pubkey::Pubkey::default()
    );

    // Set to the account authority
    let authority = account.authority;
    account_f.try_set_emissions_destination(authority).await?;

    let account = account_f.load().await;
    assert_eq!(account.emissions_destination_account, authority);

    // Update to a different key
    let other = solana_sdk::pubkey::Pubkey::new_unique();
    account_f.try_set_emissions_destination(other).await?;

    let account = account_f.load().await;
    assert_eq!(account.emissions_destination_account, other);

    Ok(())
}
