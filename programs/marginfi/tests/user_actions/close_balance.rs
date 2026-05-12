use fixtures::{
    assert_custom_error,
    test::{BankMint, TestFixture, TestSettings},
};
use marginfi::errors::MarginfiError;
use solana_program_test::tokio;
use solana_sdk::clock::Clock;

#[tokio::test]
async fn lending_account_close_balance() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_eq_bank = test_f.get_bank(&BankMint::SolEquivalent);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_eq_bank, 1_000, None)
        .await?;

    let lender_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_bank, 1_000, None)
        .await?;

    let res = lender_mfi_account_f.try_balance_close(sol_bank).await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_f_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let borrower_token_account_f_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let borrower_token_account_f_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_f_usdc.key, usdc_bank, 1_000, None)
        .await?;

    // Borrow SOL EQ
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol_eq.key, sol_eq_bank, 0.01)
        .await;

    assert!(res.is_ok());

    // Borrow SOL
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_bank, 0.01)
        .await;

    assert!(res.is_ok());

    // This issue is not that bad, because the user can still borrow other assets (isolated liab < empty threshold)
    let res = borrower_mfi_account_f.try_balance_close(sol_bank).await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalBalanceState);
    let last_update_before_repay = borrower_mfi_account_f.load().await.last_update;

    // Let a second go b
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 second
        clock.unix_timestamp += 1;
        ctx.set_sysvar(&clock);
    }

    // Repay isolated SOL EQ borrow successfully
    borrower_mfi_account_f
        .try_bank_repay(
            borrower_token_account_f_sol_eq.key,
            sol_eq_bank,
            0.01,
            Some(false),
        )
        .await?;
    let last_update_after_repay = borrower_mfi_account_f.load().await.last_update;
    assert_eq!(last_update_after_repay, last_update_before_repay + 1);

    // Let another second pass
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 second
        clock.unix_timestamp += 1;
        ctx.set_sysvar(&clock);
    }

    // Liability share in balance is smaller than 0.0001, so repay all should fail
    let res = borrower_mfi_account_f
        .try_bank_repay(
            borrower_token_account_f_sol_eq.key,
            sol_eq_bank,
            1,
            Some(true),
        )
        .await;
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::NoLiabilityFound);
    // No change to last_update if the ix was unsuccessful
    assert_eq!(
        last_update_after_repay,
        borrower_mfi_account_f.load().await.last_update
    );

    let res = borrower_mfi_account_f.try_balance_close(sol_eq_bank).await;
    assert!(res.is_ok());
    let account = borrower_mfi_account_f.load().await;
    // Balance closing also updates last_update
    assert_eq!(account.last_update, last_update_after_repay + 1);
    assert_eq!(account.indexer_flags.is_lending_only, 0);
    assert_eq!(account.indexer_flags.is_single_borrower, 1);

    Ok(())
}
