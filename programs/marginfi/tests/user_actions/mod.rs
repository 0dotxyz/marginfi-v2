mod bank_cache;
mod borrow;
mod close_account;
mod close_balance;
mod create_account;
mod create_account_pda;
mod create_account_pda_cpi;
mod deposit;
mod flash_loan;
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
use fixtures::{assert_custom_error, assert_eq_noise, native, prelude::*};
use marginfi::{
    assert_eq_with_tolerance,
    prelude::*,
    state::{bank::BankImpl, marginfi_account::BankAccountWrapper},
};
use marginfi_type_crate::constants::{
    EMISSIONS_FLAG_BORROW_ACTIVE, EMISSIONS_FLAG_LENDING_ACTIVE, MIN_EMISSIONS_START_TIME,
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::timing::SECONDS_PER_YEAR;

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
async fn emissions_test() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Setup emissions (Deposit for USDC, Borrow for SOL)

    let funding_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;

    usdc_bank
        .try_setup_emissions(
            EMISSIONS_FLAG_LENDING_ACTIVE,
            1_000_000,
            native!(50, "USDC"),
            usdc_bank.mint.key,
            funding_account.key,
            usdc_bank.get_token_program(),
        )
        .await?;

    // SOL Emissions are not in SOL Bank mint
    let sol_emissions_mint =
        MintFixture::new_token_22(test_f.context.clone(), None, Some(6), &[]).await;

    let funding_account = sol_emissions_mint
        .create_token_account_and_mint_to(200)
        .await;

    sol_bank
        .try_setup_emissions(
            EMISSIONS_FLAG_BORROW_ACTIVE,
            1_000_000,
            native!(100, 6),
            sol_emissions_mint.key,
            funding_account.key,
            sol_emissions_mint.token_program,
        )
        .await?;

    let sol_emissions_mint_2 =
        MintFixture::new_token_22(test_f.context.clone(), None, Some(6), &[]).await;

    let funding_account = sol_emissions_mint_2
        .create_token_account_and_mint_to(200)
        .await;

    let res = sol_bank
        .try_setup_emissions(
            EMISSIONS_FLAG_BORROW_ACTIVE,
            1_000_000,
            native!(50, 6),
            sol_emissions_mint_2.key,
            funding_account.key,
            sol_emissions_mint_2.token_program,
        )
        .await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::EmissionsAlreadySetup);

    // Fund SOL bank
    let sol_lender_account = test_f.create_marginfi_account().await;
    let sol_lender_token_account = test_f.sol_mint.create_token_account_and_mint_to(100).await;

    sol_lender_account
        .try_bank_deposit(sol_lender_token_account.key, sol_bank, 100, None)
        .await?;

    // Create account and setup positions
    test_f.set_time(MIN_EMISSIONS_START_TIME as i64);
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, MIN_EMISSIONS_START_TIME as i64)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_SOL_FEED, MIN_EMISSIONS_START_TIME as i64)
        .await;

    let mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f.usdc_mint.create_token_account_and_mint_to(50).await;

    mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank, 50, None)
        .await?;

    let sol_account = test_f.sol_mint.create_empty_token_account().await;

    mfi_account_f
        .try_bank_borrow(sol_account.key, sol_bank, 2)
        .await?;

    // Advance for half a year and claim half emissions
    test_f.advance_time((SECONDS_PER_YEAR / 2.0) as i64).await;

    let lender_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;

    mfi_account_f
        .try_withdraw_emissions(usdc_bank, &lender_token_account_usdc)
        .await?;

    let sol_emissions_ta = sol_emissions_mint.create_empty_token_account().await;

    mfi_account_f
        .try_withdraw_emissions(sol_bank, &sol_emissions_ta)
        .await?;

    assert_eq_with_tolerance!(
        lender_token_account_usdc.balance().await as i64,
        native!(25, "USDC") as i64,
        native!(1, "USDC") as i64
    );

    assert_eq_with_tolerance!(
        sol_emissions_ta.balance().await as i64,
        native!(1, 6) as i64,
        native!(0.1, 6, f64) as i64
    );

    // Advance for another half a year and claim the rest
    test_f.advance_time((SECONDS_PER_YEAR / 2.0) as i64).await;

    mfi_account_f
        .try_withdraw_emissions(usdc_bank, &lender_token_account_usdc)
        .await?;

    assert_eq_with_tolerance!(
        lender_token_account_usdc.balance().await as i64,
        native!(50, "USDC") as i64,
        native!(1, "USDC") as i64
    );

    mfi_account_f
        .try_withdraw_emissions(sol_bank, &sol_emissions_ta)
        .await?;

    assert_eq_with_tolerance!(
        sol_emissions_ta.balance().await as i64,
        native!(2, 6) as i64,
        native!(0.1, 6, f64) as i64
    );

    // Advance a year, and no more USDC emissions can be claimed (drained), SOL emissions can be claimed

    test_f.advance_time((SECONDS_PER_YEAR / 2.0) as i64).await;

    mfi_account_f
        .try_withdraw_emissions(usdc_bank, &lender_token_account_usdc)
        .await?;

    mfi_account_f
        .try_withdraw_emissions(sol_bank, &sol_emissions_ta)
        .await?;

    assert_eq_with_tolerance!(
        lender_token_account_usdc.balance().await as i64,
        native!(50, "USDC") as i64,
        native!(1, "USDC") as i64
    );

    assert_eq_with_tolerance!(
        sol_emissions_ta.balance().await as i64,
        native!(3, 6) as i64,
        native!(0.1, 6, f64) as i64
    );

    // SOL lendeing account can't claim emissions, bc SOL is borrow only emissions
    let sol_lender_emissions = sol_emissions_mint.create_empty_token_account().await;

    sol_lender_account
        .try_withdraw_emissions(sol_bank, &sol_lender_emissions)
        .await?;

    assert_eq!(sol_lender_emissions.balance().await as i64, 0);

    Ok(())
}

#[tokio::test]
async fn emissions_test_2() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let funding_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;

    usdc_bank
        .try_setup_emissions(
            EMISSIONS_FLAG_LENDING_ACTIVE,
            1_000_000,
            native!(50, "USDC"),
            usdc_bank.mint.key,
            funding_account.key,
            usdc_bank.get_token_program(),
        )
        .await?;

    let usdc_bank_data = usdc_bank.load().await;

    assert_eq!(usdc_bank_data.flags, EMISSIONS_FLAG_LENDING_ACTIVE);

    assert_eq!(usdc_bank_data.emissions_rate, 1_000_000);

    assert_eq!(
        I80F48::from(usdc_bank_data.emissions_remaining),
        I80F48::from_num(native!(50, "USDC"))
    );

    usdc_bank
        .try_update_emissions(
            Some(EMISSIONS_FLAG_BORROW_ACTIVE),
            Some(500_000),
            Some((native!(25, "USDC"), funding_account.key)),
            usdc_bank.get_token_program(),
        )
        .await?;

    let usdc_bank_data = usdc_bank.load().await;

    assert_eq!(usdc_bank_data.flags, EMISSIONS_FLAG_BORROW_ACTIVE);

    assert_eq!(usdc_bank_data.emissions_rate, 500_000);

    assert_eq!(
        I80F48::from(usdc_bank_data.emissions_remaining),
        I80F48::from_num(native!(75, "USDC"))
    );

    Ok(())
}

#[tokio::test]
async fn emissions_setup_t22_with_fee() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let collateral_mint = BankMint::T22WithFee;
    let bank_f = test_f.get_bank(&collateral_mint);

    let funding_account = bank_f.mint.create_token_account_and_mint_to(100).await;

    let emissions_vault = get_emissions_token_account_address(bank_f.key, bank_f.mint.key).0;

    let pre_vault_balance = 0;

    bank_f
        .try_setup_emissions(
            EMISSIONS_FLAG_LENDING_ACTIVE,
            1_000_000,
            native!(50, bank_f.mint.mint.decimals),
            bank_f.mint.key,
            funding_account.key,
            bank_f.get_token_program(),
        )
        .await?;

    let post_vault_balance = TokenAccountFixture::fetch(test_f.context.clone(), emissions_vault)
        .await
        .balance()
        .await;

    let bank = bank_f.load().await;

    assert_eq!(bank.flags, EMISSIONS_FLAG_LENDING_ACTIVE);

    assert_eq!(bank.emissions_rate, 1_000_000);

    assert_eq!(
        I80F48::from(bank.emissions_remaining),
        I80F48::from_num(native!(50, bank_f.mint.mint.decimals))
    );

    let expected_vault_balance_delta = native!(50, bank_f.mint.mint.decimals) as u64;
    let actual_vault_balance_delta = post_vault_balance - pre_vault_balance;
    assert_eq!(expected_vault_balance_delta, actual_vault_balance_delta);

    let pre_vault_balance = TokenAccountFixture::fetch(test_f.context.clone(), emissions_vault)
        .await
        .balance()
        .await;

    bank_f
        .try_update_emissions(
            Some(EMISSIONS_FLAG_BORROW_ACTIVE),
            Some(500_000),
            Some((native!(25, bank_f.mint.mint.decimals), funding_account.key)),
            bank_f.get_token_program(),
        )
        .await?;

    let post_vault_balance = TokenAccountFixture::fetch(test_f.context.clone(), emissions_vault)
        .await
        .balance()
        .await;

    let bank_data = bank_f.load().await;

    assert_eq!(bank_data.flags, EMISSIONS_FLAG_BORROW_ACTIVE);

    assert_eq!(bank_data.emissions_rate, 500_000);

    assert_eq!(
        I80F48::from(bank_data.emissions_remaining),
        I80F48::from_num(native!(75, bank_f.mint.mint.decimals))
    );

    let expected_vault_balance_delta = native!(25, bank_f.mint.mint.decimals) as u64;
    let actual_vault_balance_delta = post_vault_balance - pre_vault_balance;
    assert_eq!(expected_vault_balance_delta, actual_vault_balance_delta);

    Ok(())
}
