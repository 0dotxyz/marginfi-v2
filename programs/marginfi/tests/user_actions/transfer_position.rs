use anchor_lang::prelude::Clock;
use bytemuck::from_bytes_mut;
use fixed::types::I80F48;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::*;
use marginfi::state::bank::BankImpl;
use marginfi_type_crate::types::{Bank, BankOperationalState};
use pretty_assertions::assert_eq;
use solana_program_test::*;

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
async fn test_position_transfer_send_disabled_flag() -> anyhow::Result<()> {
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

    let mut source_account = source_account_f.load().await;
    source_account.account_flags |=
        marginfi_type_crate::types::ACCOUNT_POSITION_TRANSFER_SEND_DISABLED;
    source_account_f.set_account(&source_account).await?;

    let payer = test_f.payer_keypair();
    let res = source_account_f
        .try_position_transfer_with_authority(&dest_account_f, usdc_bank_f, 100.0, &payer)
        .await;

    assert!(
        res.is_err(),
        "Transfer from account with SEND_DISABLED should fail"
    );
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferSendDisabled
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
    assert_custom_error!(res.unwrap_err(), MarginfiError::PositionTransferDisabled);

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
async fn test_position_transfer_destination_violates_risk_rules() -> anyhow::Result<()> {
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

#[tokio::test]
async fn test_position_transfer_with_accrued_interest() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let source_account_f = test_f.create_marginfi_account().await;
    let dest_account_f = test_f.create_marginfi_account().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let source_token_account = test_f
        .usdc_mint
        .create_token_account_and_mint_to(2_000)
        .await;
    source_account_f
        .try_bank_deposit(source_token_account.key, usdc_bank_f, 1_000.0, None)
        .await?;

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
        .try_accrue_interest(&usdc_bank_f)
        .await?;

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

    // The transfer burns the shares worth `transfer_amount` at the accrued share value and mints
    // shares for the amount those burned shares were worth, so the round trip is exact bank math.
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
