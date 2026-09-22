use super::premium::{advance_clock, premium_test_fixture, setup_borrower};
use anchor_lang::prelude::Clock;
use bytemuck::from_bytes_mut;
use fixed::types::I80F48;
use fixtures::bank::BankFixture;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, prelude::*, ui_to_native};
use marginfi::prelude::*;
use marginfi::state::bank::BankImpl;
use marginfi_type_crate::types::{Bank, BankConfigOpt, BankOperationalState, ACCOUNT_FROZEN};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::signature::Keypair;

async fn liability_shares(account: &MarginfiAccountFixture, bank: &BankFixture) -> I80F48 {
    account
        .load()
        .await
        .lending_account
        .get_balance(&bank.key)
        .unwrap()
        .liability_shares
        .into()
}

/// A lender funding the SOL bank, and a source account holding `usdc_deposit` USDC that borrowed
/// 30 SOL against it.
async fn indebted_source(test_f: &TestFixture, usdc_deposit: f64) -> MarginfiAccountFixture {
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let lender_f = test_f.create_marginfi_account().await;
    let lender_sol = test_f.sol_mint.create_token_account_and_mint_to(500).await;
    lender_f
        .try_bank_deposit(lender_sol.key, sol_bank_f, 200.0, None)
        .await
        .unwrap();
    let source_f = test_f.create_marginfi_account().await;
    let source_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    source_f
        .try_bank_deposit(source_usdc.key, usdc_bank_f, usdc_deposit, None)
        .await
        .unwrap();
    let source_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    source_f
        .try_bank_borrow(source_sol.key, sol_bank_f, 30.0)
        .await
        .unwrap();
    source_f
}

#[tokio::test]
async fn test_position_transfer_basic() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let source_account_pre = source_account_f.load().await;
    let source_balance_pre = source_account_pre
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    let source_amount_pre: I80F48 = usdc_bank_f
        .load()
        .await
        .get_asset_amount(source_balance_pre.asset_shares.into())?
        .into();

    let transfer_amount = 100.0;
    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, transfer_amount, &payer)
        .await;
    assert!(
        res.is_ok(),
        "Transfer should succeed, got error: {:?}",
        res.err()
    );

    let source_account_post = source_account_f.load().await;
    let source_balance_post = source_account_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    let source_amount_post: I80F48 = usdc_bank_f
        .load()
        .await
        .get_asset_amount(source_balance_post.asset_shares.into())?
        .into();

    let decimals = usdc_bank_f.mint.mint.decimals as u32;
    let expected_native = I80F48::from_num(transfer_amount) * I80F48::from_num(10u64.pow(decimals));
    assert_eq!(source_amount_pre - source_amount_post, expected_native);

    let dest_account_post = dest_account_f.load().await;
    let dest_balance = dest_account_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .expect("Destination should have created a balance");

    let dest_amount: I80F48 = usdc_bank_f
        .load()
        .await
        .get_asset_amount(dest_balance.asset_shares.into())?
        .into();

    assert_eq!(dest_amount, expected_native);

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_identical_accounts() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    account_f
        .try_bank_deposit(token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let payer = test_f.payer_keypair();
    let res = account_f
        .try_position_transfer_with_authority(&account_f, usdc_bank_f, 100.0, &payer)
        .await;

    assert!(res.is_err(), "Transfer to self should fail");
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferIdenticalAccounts
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_insufficient_balance() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let token_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    source_account_f
        .try_bank_deposit(token_account.key, usdc_bank_f, 10.0, None)
        .await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 50.0, &payer)
        .await;

    assert!(res.is_err(), "Transfer exceeding balance should fail");
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferInsufficientFunds
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_receive_disabled_flag() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let mut dest_account = dest_account_f.load().await;
    dest_account.account_flags |=
        marginfi_type_crate::types::ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED;
    dest_account_f.set_account(&dest_account).await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 100.0, &payer)
        .await;

    assert!(
        res.is_err(),
        "Transfer to account with RECEIVE_DISABLED should fail"
    );
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferReceiveDisabled
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_below_minimum() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    source_account_f
        .try_bank_deposit(token_account.key, usdc_bank_f, 5_000.0, None)
        .await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 0.001, &payer)
        .await;

    assert!(res.is_err(), "Transfer below minimum should fail");
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::InvalidPositionTransferAmount
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_zero_amount() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_account_f
        .try_bank_deposit(token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 0.0, &payer)
        .await;

    assert!(res.is_err(), "Transfer of zero amount should fail");
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::InvalidPositionTransferAmount
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_destination_existing_balance() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;

    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let source_sol_token_account = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    source_account_f
        .try_bank_deposit(source_sol_token_account.key, sol_bank_f, 50.0, None)
        .await?;

    let dest_sol_token_account = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    dest_account_f
        .try_bank_deposit(dest_sol_token_account.key, sol_bank_f, 30.0, None)
        .await?;

    let dest_pre = dest_account_f.load().await;
    let active_balances_pre = dest_pre.lending_account.get_active_balances_iter().count();

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, sol_bank_f, 20.0, &payer)
        .await;

    assert!(
        res.is_ok(),
        "Transfer to bank with existing balance should succeed"
    );

    let dest_post = dest_account_f.load().await;
    let active_balances_post = dest_post.lending_account.get_active_balances_iter().count();

    assert_eq!(
        active_balances_pre, active_balances_post,
        "Should not create new balance slot when one already exists"
    );

    let dest_balance_post = dest_post
        .lending_account
        .get_balance(&sol_bank_f.key)
        .expect("SOL balance should exist");

    let dest_amount_post: I80F48 = sol_bank_f
        .load()
        .await
        .get_asset_amount(dest_balance_post.asset_shares.into())?
        .into();

    let sol_decimals = sol_bank_f.mint.mint.decimals as u32;
    assert_eq!(
        dest_amount_post,
        I80F48::from_num(50.0) * I80F48::from_num(10u64.pow(sol_decimals))
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_share_preservation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let source_account_pre = source_account_f.load().await;
    let source_balance_pre = source_account_pre
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    let source_shares_pre: I80F48 = source_balance_pre.asset_shares.into();

    let payer = test_f.payer_keypair();
    source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 100.0, &payer)
        .await?;

    let source_account_post = source_account_f.load().await;
    let source_balance_post = source_account_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    let source_shares_post: I80F48 = source_balance_post.asset_shares.into();

    let dest_account_post = dest_account_f.load().await;
    let dest_balance = dest_account_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    let dest_shares: I80F48 = dest_balance.asset_shares.into();

    // The bank has no borrows, so one share is one native unit.
    let decimals = usdc_bank_f.mint.mint.decimals as u32;
    let transferred_shares = I80F48::from_num(100.0) * I80F48::from_num(10u64.pow(decimals));
    assert_eq!(dest_shares, transferred_shares);
    assert_eq!(source_shares_post, source_shares_pre - transferred_shares);

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_paused_bank() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let mut bank_ai = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(usdc_bank_f.key)
        .await
        .unwrap()
        .unwrap();
    let bank = from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);
    bank.config.operational_state = BankOperationalState::Paused;
    {
        let mut ctx = test_f.context.borrow_mut();
        ctx.set_account(&usdc_bank_f.key, &bank_ai.into());
    }

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 100.0, &payer)
        .await;

    assert!(res.is_err(), "Transfer from paused bank should fail");
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankPaused);

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_reduce_only_bank() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let mut bank_ai = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(usdc_bank_f.key)
        .await
        .unwrap()
        .unwrap();
    let bank = from_bytes_mut::<Bank>(&mut bank_ai.data.as_mut_slice()[8..]);
    bank.config.operational_state = BankOperationalState::ReduceOnly;
    {
        let mut ctx = test_f.context.borrow_mut();
        ctx.set_account(&usdc_bank_f.key, &bank_ai.into());
    }

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 100.0, &payer)
        .await;

    assert!(res.is_err(), "Transfer from reduce-only bank should fail");
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankReduceOnly);

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_protocol_paused() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    test_f.marginfi_group.try_panic_pause().await?;

    test_f.marginfi_group.try_propagate_fee_state().await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 100.0, &payer)
        .await;

    assert!(res.is_err(), "Transfer when protocol paused should fail");
    assert_custom_error!(res.unwrap_err(), MarginfiError::ProtocolPaused);

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_full_balance() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f.usdc_mint.create_token_account_and_mint_to(300).await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 200.0, None)
        .await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 200.0, &payer)
        .await;

    assert!(res.is_ok(), "Transfer of full balance should succeed");

    // The emptied source slot stays active with zero shares.
    let source_post = source_account_f.load().await;
    let source_shares_post: I80F48 = source_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    assert_eq!(source_shares_post, I80F48::ZERO);

    let dest_post = dest_account_f.load().await;
    let dest_balance = dest_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .expect("Destination should have balance");

    let dest_amount: I80F48 = usdc_bank_f
        .load()
        .await
        .get_asset_amount(dest_balance.asset_shares.into())?
        .into();

    let usdc_decimals = usdc_bank_f.mint.mint.decimals as u32;
    assert_eq!(
        dest_amount,
        I80F48::from_num(200.0) * I80F48::from_num(10u64.pow(usdc_decimals))
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_exactly_minimum() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 10.0, None)
        .await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 1.0, &payer)
        .await;

    assert!(
        res.is_ok(),
        "Transfer at exactly $1.00 minimum should succeed"
    );

    let dest_post = dest_account_f.load().await;
    let dest_balance = dest_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .expect("Destination should have balance");

    let dest_amount: I80F48 = usdc_bank_f
        .load()
        .await
        .get_asset_amount(dest_balance.asset_shares.into())?
        .into();

    let usdc_decimals = usdc_bank_f.mint.mint.decimals as u32;
    assert_eq!(
        dest_amount,
        I80F48::from_num(1.0) * I80F48::from_num(10u64.pow(usdc_decimals))
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_just_below_minimum() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 10.0, None)
        .await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 0.99, &payer)
        .await;

    assert!(res.is_err(), "Transfer just below minimum should fail");
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::InvalidPositionTransferAmount
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_fee_collection() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let group = test_f.marginfi_group.load().await;
    let fee_wallet = group.fee_state_cache.global_fee_wallet;

    let fee_wallet_balance_pre = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(fee_wallet)
        .await?
        .map(|acc| acc.lamports)
        .unwrap_or(0);

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 100.0, &payer)
        .await;

    assert!(res.is_ok(), "Transfer should succeed");

    let fee_wallet_balance_post = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(fee_wallet)
        .await?
        .map(|acc| acc.lamports)
        .unwrap_or(0);

    let fee_amount: u64 = 500_000;
    assert_eq!(fee_wallet_balance_post, fee_wallet_balance_pre + fee_amount);

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_source_becomes_unhealthy() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let source_usdc_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    source_account_f
        .try_bank_deposit(source_usdc_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let lender_account_f = test_f.create_marginfi_account().await;
    let lender_sol_token_account = test_f.sol_mint.create_token_account_and_mint_to(500).await;
    lender_account_f
        .try_bank_deposit(lender_sol_token_account.key, sol_bank_f, 200.0, None)
        .await?;

    let source_sol_token_account = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    source_account_f
        .try_bank_borrow(source_sol_token_account.key, sol_bank_f, 30.0)
        .await?;

    // 500 USDC backs a 30 SOL ($300) debt; moving 250 leaves the source under-collateralized.
    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 250.0, &payer)
        .await;

    assert!(
        res.is_err(),
        "Transfer should fail when source becomes unhealthy"
    );
    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_transaction_rollback_on_failure() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let source_usdc_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    source_account_f
        .try_bank_deposit(source_usdc_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let lender_account_f = test_f.create_marginfi_account().await;
    let lender_sol_token_account = test_f.sol_mint.create_token_account_and_mint_to(500).await;
    lender_account_f
        .try_bank_deposit(lender_sol_token_account.key, sol_bank_f, 200.0, None)
        .await?;

    let source_sol_token_account = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    source_account_f
        .try_bank_borrow(source_sol_token_account.key, sol_bank_f, 30.0)
        .await?;

    let source_pre = source_account_f.load().await;
    let source_usdc_pre = source_pre
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    let source_usdc_shares_pre: I80F48 = source_usdc_pre.asset_shares.into();

    let dest_pre = dest_account_f.load().await;
    let dest_has_usdc_pre = dest_pre
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .is_some();

    // 500 USDC backs a 30 SOL ($300) debt; moving 250 leaves the source under-collateralized.
    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 250.0, &payer)
        .await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    let source_post = source_account_f.load().await;
    let source_usdc_post = source_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    let source_usdc_shares_post: I80F48 = source_usdc_post.asset_shares.into();

    assert_eq!(
        source_usdc_shares_pre, source_usdc_shares_post,
        "Source USDC shares should remain unchanged after failed transfer (atomicity)"
    );

    let dest_post = dest_account_f.load().await;
    let dest_has_usdc_post = dest_post
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .is_some();

    assert_eq!(
        dest_has_usdc_pre, dest_has_usdc_post,
        "Destination should not have created/removed USDC balance after failed transfer"
    );

    Ok(())
}

#[tokio::test]
async fn test_position_transfer_collateral_into_debt_holder() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let source_usdc_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    source_account_f
        .try_bank_deposit(source_usdc_token_account.key, usdc_bank_f, 500.0, None)
        .await?;

    let lender_usdc_account_f = test_f.create_marginfi_account().await;
    let lender_usdc_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    lender_usdc_account_f
        .try_bank_deposit(lender_usdc_token_account.key, usdc_bank_f, 5_000.0, None)
        .await?;

    let dest_sol_token_account = test_f
        .sol_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    dest_account_f
        .try_bank_deposit(dest_sol_token_account.key, sol_bank_f, 5_000.0, None)
        .await?;

    let dest_usdc_borrow_account = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    dest_account_f
        .try_bank_borrow(dest_usdc_borrow_account.key, usdc_bank_f, 10.0)
        .await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 100.0, &payer)
        .await;

    assert!(
        res.is_err(),
        "Transfer should fail when destination account has existing liabilities in the same bank (deposit-only constraint)"
    );
    assert_custom_error!(res.unwrap_err(), MarginfiError::OperationDepositOnly);

    Ok(())
}

/// A source holding 1,000 USDC and a destination holding SOL, after a day of USDC borrowing has
/// moved the USDC share value off 1.
async fn accrued_usdc_source(
    test_f: &TestFixture,
) -> anyhow::Result<(MarginfiAccountFixture, MarginfiAccountFixture)> {
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let source_account_f = test_f.create_marginfi_account().await;
    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(2_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 1_000.0, None)
        .await?;

    let dest_account_f = test_f.create_marginfi_account().await;
    let dest_sol_deposit = test_f.sol_mint.create_token_account_and_mint_to(2000).await;
    dest_account_f
        .try_bank_deposit(dest_sol_deposit.key, sol_bank_f, 500.0, None)
        .await?;

    let borrower_account = test_f.create_marginfi_account().await;
    let borrower_sol_deposit = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    borrower_account
        .try_bank_deposit(borrower_sol_deposit.key, sol_bank_f, 50.0, None)
        .await?;
    let borrower_usdc_borrow = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    borrower_account
        .try_bank_borrow(borrower_usdc_borrow.key, usdc_bank_f, 100.0)
        .await?;

    test_f.advance_time(86400).await;
    let now_ts = {
        let ctx = test_f.context.borrow_mut();
        let clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp
    };
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, now_ts)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_SOL_FEED, now_ts)
        .await;
    test_f
        .marginfi_group
        .try_accrue_interest(usdc_bank_f)
        .await?;
    Ok((source_account_f, dest_account_f))
}

#[tokio::test]
async fn test_position_transfer_with_accrued_interest() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (source_account_f, dest_account_f) = accrued_usdc_source(&test_f).await?;

    let bank_after_accrual = usdc_bank_f.load().await;
    let total_asset_shares_before: I80F48 = bank_after_accrual.total_asset_shares.into();

    let source_balance_before = source_account_f.load().await;
    let source_balance_info_before = source_balance_before
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    let source_shares_before: I80F48 = source_balance_info_before.asset_shares.into();

    let payer = test_f.payer_keypair();
    let transfer_amount = 100.0;
    source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, transfer_amount, &payer)
        .await?;

    // The expected shares follow the bank's own burn-then-mint round trip.
    let bank_after_transfer = usdc_bank_f.load().await;
    let total_asset_shares_after: I80F48 = bank_after_transfer.total_asset_shares.into();
    let decimals = usdc_bank_f.mint.mint.decimals as u32;
    let transfer_native = I80F48::from_num(transfer_amount) * I80F48::from_num(10u64.pow(decimals));
    let burned_shares = bank_after_transfer.get_asset_shares(transfer_native)?;
    let minted_shares = bank_after_transfer
        .get_asset_shares(bank_after_transfer.get_asset_amount(burned_shares)?)?;

    let source_balance_after = source_account_f.load().await;
    let source_shares_after: I80F48 = source_balance_after
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    let dest_balance_after = dest_account_f.load().await;
    let dest_shares: I80F48 = dest_balance_after
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .expect("Destination should have USDC balance")
        .asset_shares
        .into();

    assert_eq!(source_shares_after, source_shares_before - burned_shares);
    assert_eq!(dest_shares, minted_shares);
    assert_eq!(
        total_asset_shares_after,
        total_asset_shares_before - burned_shares + minted_shares
    );

    Ok(())
}

/// Collateral reaches an account of another authority on the sender's signature alone.
#[tokio::test]
async fn test_position_transfer_collateral_needs_no_receiver_signature() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source_f = test_f.create_marginfi_account().await;
    let source_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_f
        .try_bank_deposit(source_usdc.key, usdc_bank_f, 500.0, None)
        .await?;
    let receiver = Keypair::new();
    let dest_f = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &receiver,
    )
    .await;

    source_f
        .try_position_transfer(&dest_f, usdc_bank_f, 100.0)
        .await?;

    let decimals = usdc_bank_f.mint.mint.decimals as u32;
    let dest_shares: I80F48 = dest_f
        .load()
        .await
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    assert_eq!(
        dest_shares,
        I80F48::from_num(100.0) * I80F48::from_num(10u64.pow(decimals))
    );
    Ok(())
}

#[tokio::test]
async fn test_position_transfer_rejects_frozen_destination() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source_f = test_f.create_marginfi_account().await;
    let source_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_f
        .try_bank_deposit(source_usdc.key, usdc_bank_f, 500.0, None)
        .await?;
    let dest_f = test_f.create_marginfi_account().await;
    let mut dest_account = dest_f.load().await;
    dest_account.account_flags |= ACCOUNT_FROZEN;
    dest_f.set_account(&dest_account).await?;

    let res = source_f
        .try_position_transfer(&dest_f, usdc_bank_f, 100.0)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);
    Ok(())
}

/// Debt moves between two accounts of one authority without a second signature, burning and
/// minting liability shares at the bank's share value.
#[tokio::test]
async fn test_position_transfer_debt_between_own_accounts() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let source_f = indebted_source(&test_f, 500.0).await;
    let dest_f = test_f.create_marginfi_account().await;
    let dest_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    dest_f
        .try_bank_deposit(dest_usdc.key, usdc_bank_f, 500.0, None)
        .await?;

    let source_shares_before = liability_shares(&source_f, sol_bank_f).await;
    let total_shares_before: I80F48 = sol_bank_f.load().await.total_liability_shares.into();

    source_f
        .try_position_transfer(&dest_f, sol_bank_f, 10.0)
        .await?;

    let bank = sol_bank_f.load().await;
    let decimals = sol_bank_f.mint.mint.decimals as u32;
    let transfer_native = I80F48::from_num(10.0) * I80F48::from_num(10u64.pow(decimals));
    let burned_shares = bank.get_liability_shares(transfer_native)?;
    let minted_shares = bank.get_liability_shares(bank.get_liability_amount(burned_shares)?)?;
    assert_eq!(
        liability_shares(&source_f, sol_bank_f).await,
        source_shares_before - burned_shares
    );
    assert_eq!(liability_shares(&dest_f, sol_bank_f).await, minted_shares);
    assert_eq!(
        I80F48::from(bank.total_liability_shares),
        total_shares_before - burned_shares + minted_shares
    );
    Ok(())
}

/// Debt sent to another authority's account needs that authority's signature, while collateral
/// does not.
#[tokio::test]
async fn test_position_transfer_debt_requires_receiver_consent() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let source_f = indebted_source(&test_f, 1_000.0).await;
    let receiver = Keypair::new();
    let dest_f = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &receiver,
    )
    .await;
    source_f
        .try_position_transfer(&dest_f, usdc_bank_f, 300.0)
        .await?;

    let res = source_f
        .try_position_transfer(&dest_f, sol_bank_f, 10.0)
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferDebtConsentRequired
    );

    let payer = test_f.payer_keypair();
    source_f
        .try_position_transfer_with_authorities(&dest_f, sol_bank_f, 10.0, &payer, Some(&receiver))
        .await?;
    let bank = sol_bank_f.load().await;
    let decimals = sol_bank_f.mint.mint.decimals as u32;
    let burned_shares =
        bank.get_liability_shares(I80F48::from_num(10.0) * I80F48::from_num(10u64.pow(decimals)))?;
    let minted_shares = bank.get_liability_shares(bank.get_liability_amount(burned_shares)?)?;
    assert_eq!(liability_shares(&dest_f, sol_bank_f).await, minted_shares);
    Ok(())
}

#[tokio::test]
async fn test_position_transfer_debt_into_asset_holder() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let source_f = indebted_source(&test_f, 500.0).await;
    let dest_f = test_f.create_marginfi_account().await;
    let dest_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    dest_f
        .try_bank_deposit(dest_sol.key, sol_bank_f, 50.0, None)
        .await?;

    let res = source_f
        .try_position_transfer(&dest_f, sol_bank_f, 10.0)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OperationBorrowOnly);
    Ok(())
}

#[tokio::test]
async fn test_position_transfer_debt_makes_destination_unhealthy() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let source_f = indebted_source(&test_f, 500.0).await;
    let dest_f = test_f.create_marginfi_account().await;
    let dest_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    // 50 USDC cannot back 10 SOL ($100) of debt.
    dest_f
        .try_bank_deposit(dest_usdc.key, usdc_bank_f, 50.0, None)
        .await?;

    let res = source_f
        .try_position_transfer(&dest_f, sol_bank_f, 10.0)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);
    Ok(())
}

/// Moving the whole accrued position leaves the source only the dust below one native unit.
#[tokio::test]
async fn test_position_transfer_full_move_with_accrued_interest() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (source_account_f, dest_account_f) = accrued_usdc_source(&test_f).await?;

    let bank = usdc_bank_f.load().await;
    let source_shares_before: I80F48 = source_account_f
        .load()
        .await
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    let decimals = usdc_bank_f.mint.mint.decimals as u32;
    // One unit under the floor keeps float rounding in the UI conversion from overshooting.
    let target_native = bank.get_asset_amount(source_shares_before)?.to_num::<u64>() - 1;
    let ui_amount = target_native as f64 / 10u64.pow(decimals) as f64;
    let transfer_native = ui_to_native!(ui_amount, decimals);

    source_account_f
        .try_position_transfer(&dest_account_f, usdc_bank_f, ui_amount)
        .await?;

    let bank = usdc_bank_f.load().await;
    let burned_shares = bank.get_asset_shares(I80F48::from_num(transfer_native))?;
    let minted_shares = bank.get_asset_shares(bank.get_asset_amount(burned_shares)?)?;
    let source_shares_after: I80F48 = source_account_f
        .load()
        .await
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    let dest_shares: I80F48 = dest_account_f
        .load()
        .await
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    assert_eq!(source_shares_after, source_shares_before - burned_shares);
    assert_eq!(dest_shares, minted_shares);
    Ok(())
}

#[tokio::test]
async fn test_position_transfer_rejects_unauthorized_source_signer() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source_f = test_f.create_marginfi_account().await;
    let source_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    source_f
        .try_bank_deposit(source_usdc.key, usdc_bank_f, 500.0, None)
        .await?;
    let dest_f = test_f.create_marginfi_account().await;

    let res = source_f
        .try_position_transfer_with_authority(&dest_f, usdc_bank_f, 100.0, &Keypair::new())
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);
    Ok(())
}

#[tokio::test]
async fn test_position_transfer_debt_rejects_wrong_consenter() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let source_f = indebted_source(&test_f, 1_000.0).await;
    let dest_f = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &Keypair::new(),
    )
    .await;
    source_f
        .try_position_transfer(&dest_f, usdc_bank_f, 300.0)
        .await?;

    let payer = test_f.payer_keypair();
    let res = source_f
        .try_position_transfer_with_authorities(
            &dest_f,
            sol_bank_f,
            10.0,
            &payer,
            Some(&Keypair::new()),
        )
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferDebtConsentRequired
    );
    Ok(())
}

/// A frozen source blocks its authority but not the group admin, who remediates through it.
#[tokio::test]
async fn test_position_transfer_frozen_source_admin_only() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let funder_f = test_f.create_marginfi_account().await;
    let funder_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    funder_f
        .try_bank_deposit(funder_usdc.key, usdc_bank_f, 500.0, None)
        .await?;
    let owner = Keypair::new();
    let source_f = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &owner,
    )
    .await;
    funder_f
        .try_position_transfer(&source_f, usdc_bank_f, 300.0)
        .await?;
    let dest_f = test_f.create_marginfi_account().await;
    source_f.try_set_freeze(true).await?;

    let res = source_f
        .try_position_transfer_with_authority(&dest_f, usdc_bank_f, 100.0, &owner)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);

    let admin = test_f.payer_keypair();
    source_f
        .try_position_transfer_with_authority(&dest_f, usdc_bank_f, 100.0, &admin)
        .await?;
    let decimals = usdc_bank_f.mint.mint.decimals as u32;
    let dest_shares: I80F48 = dest_f
        .load()
        .await
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    assert_eq!(
        dest_shares,
        I80F48::from_num(100.0) * I80F48::from_num(10u64.pow(decimals))
    );
    Ok(())
}

#[tokio::test]
async fn test_position_transfer_respects_configured_minimum() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    // $500 minimum
    test_f
        .marginfi_group
        .try_edit_position_transfer_fees(None, Some(50_000))
        .await?;
    let source_f = test_f.create_marginfi_account().await;
    let source_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(2_000)
        .await;
    source_f
        .try_bank_deposit(source_usdc.key, usdc_bank_f, 1_000.0, None)
        .await?;
    let dest_f = test_f.create_marginfi_account().await;

    let res = source_f
        .try_position_transfer(&dest_f, usdc_bank_f, 400.0)
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::InvalidPositionTransferAmount
    );

    source_f
        .try_position_transfer(&dest_f, usdc_bank_f, 600.0)
        .await?;
    let decimals = usdc_bank_f.mint.mint.decimals as u32;
    let dest_shares: I80F48 = dest_f
        .load()
        .await
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    assert_eq!(
        dest_shares,
        I80F48::from_num(600.0) * I80F48::from_num(10u64.pow(decimals))
    );
    Ok(())
}

/// Moving a whole debt hands its accrued premium receivable to the destination instead of
/// writing it off with the emptied source liability.
#[tokio::test]
async fn test_position_transfer_debt_carries_premium() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;
    let dest_f = test_f.create_marginfi_account().await;
    let dest_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    dest_f
        .try_bank_deposit(dest_sol.key, sol_bank_f, 999.0, None)
        .await?;

    advance_clock(&test_f, 30 * 24 * 60 * 60).await;
    borrower.try_lending_account_pulse_health().await?;
    let receivable: I80F48 = borrower
        .load()
        .await
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .premium_outstanding
        .into();
    assert_ne!(receivable, I80F48::ZERO);

    // USDC accrues no base interest in this fixture, so the whole debt is exactly 1,000 USDC.
    borrower
        .try_position_transfer(&dest_f, usdc_bank_f, 1_000.0)
        .await?;

    let source_balance = borrower.load().await;
    let source_balance = source_balance
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    assert_eq!(I80F48::from(source_balance.liability_shares), I80F48::ZERO);
    assert_eq!(
        I80F48::from(source_balance.premium_outstanding),
        I80F48::ZERO
    );
    let dest_account = dest_f.load().await;
    let dest_balance = dest_account
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap();
    assert_eq!(I80F48::from(dest_balance.premium_outstanding), receivable);
    Ok(())
}

/// A same-bank move leaves the bank's totals unchanged, so it clears a bank sitting exactly at
/// its deposit and borrow caps.
#[tokio::test]
async fn test_position_transfer_within_capped_banks() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let source_f = indebted_source(&test_f, 500.0).await;
    let dest_f = test_f.create_marginfi_account().await;
    let dest_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    dest_f
        .try_bank_deposit(dest_usdc.key, usdc_bank_f, 500.0, None)
        .await?;

    let usdc_bank = usdc_bank_f.load().await;
    let usdc_deposits: u64 = usdc_bank
        .get_asset_amount(usdc_bank.total_asset_shares.into())?
        .to_num();
    usdc_bank_f
        .update_config(
            BankConfigOpt {
                deposit_limit: Some(usdc_deposits),
                ..Default::default()
            },
            None,
        )
        .await?;
    let sol_bank = sol_bank_f.load().await;
    let sol_borrows: u64 = sol_bank
        .get_liability_amount(sol_bank.total_liability_shares.into())?
        .to_num();
    sol_bank_f
        .update_config(
            BankConfigOpt {
                borrow_limit: Some(sol_borrows),
                ..Default::default()
            },
            None,
        )
        .await?;

    source_f
        .try_position_transfer(&dest_f, usdc_bank_f, 100.0)
        .await?;
    source_f
        .try_position_transfer(&dest_f, sol_bank_f, 10.0)
        .await?;

    let usdc_decimals = usdc_bank_f.mint.mint.decimals as u32;
    let dest_usdc_shares: I80F48 = dest_f
        .load()
        .await
        .lending_account
        .get_balance(&usdc_bank_f.key)
        .unwrap()
        .asset_shares
        .into();
    assert_eq!(
        dest_usdc_shares,
        I80F48::from_num(600.0) * I80F48::from_num(10u64.pow(usdc_decimals))
    );
    let sol_bank = sol_bank_f.load().await;
    let sol_decimals = sol_bank_f.mint.mint.decimals as u32;
    let burned_shares = sol_bank
        .get_liability_shares(I80F48::from_num(10.0) * I80F48::from_num(10u64.pow(sol_decimals)))?;
    let minted_shares =
        sol_bank.get_liability_shares(sol_bank.get_liability_amount(burned_shares)?)?;
    assert_eq!(liability_shares(&dest_f, sol_bank_f).await, minted_shares);
    Ok(())
}

/// Asking for more debt than the source owes moves the whole debt.
#[tokio::test]
async fn test_position_transfer_debt_over_ask_moves_all() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let source_f = indebted_source(&test_f, 500.0).await;
    let dest_f = test_f.create_marginfi_account().await;
    let dest_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    dest_f
        .try_bank_deposit(dest_usdc.key, usdc_bank_f, 500.0, None)
        .await?;
    let source_shares_before = liability_shares(&source_f, sol_bank_f).await;

    source_f
        .try_position_transfer(&dest_f, sol_bank_f, 100.0)
        .await?;

    let bank = sol_bank_f.load().await;
    let burned_shares =
        bank.get_liability_shares(bank.get_liability_amount(source_shares_before)?)?;
    let minted_shares = bank.get_liability_shares(bank.get_liability_amount(burned_shares)?)?;
    assert_eq!(
        liability_shares(&source_f, sol_bank_f).await,
        source_shares_before - burned_shares
    );
    assert_eq!(liability_shares(&dest_f, sol_bank_f).await, minted_shares);
    Ok(())
}
