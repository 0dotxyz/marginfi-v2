use super::premium::{advance_clock, premium_test_fixture};
use super::transfer_position::{
    asset_shares, indebted_source, receiver_with_usdc_deposit, usdc_depositor,
};
use anchor_lang::prelude::Clock;
use fixed::types::I80F48;
use fixtures::bank::BankFixture;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::utils::find_position_escrow_pda;
use fixtures::{assert_custom_error, native, prelude::*};
use marginfi::constants::POSITION_ESCROW_EXPIRY_SECONDS;
use marginfi::prelude::*;
use marginfi_type_crate::types::{PositionEscrow, ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair};

/// Asset shares worth `ui` whole USDC while the bank has no borrows.
fn usdc_shares(ui: u64) -> I80F48 {
    I80F48::from_num(native!(ui, "USDC"))
}

/// A new authority and an empty account it owns.
async fn empty_receiver(test_f: &TestFixture) -> (Keypair, MarginfiAccountFixture) {
    let receiver = Keypair::new();
    let account = account_of(test_f, &receiver).await;
    (receiver, account)
}

async fn account_of(test_f: &TestFixture, authority: &Keypair) -> MarginfiAccountFixture {
    MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        authority,
    )
    .await
}

/// Sends `ui` USDC from `source` into its escrow for `receiver`.
async fn escrow_usdc(
    test_f: &TestFixture,
    source: &MarginfiAccountFixture,
    receiver: &MarginfiAccountFixture,
    ui: f64,
) -> anyhow::Result<Pubkey> {
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let payer = test_f.payer_keypair();
    source
        .try_escrow_position(receiver, usdc_bank_f, ui, &payer)
        .await?;
    Ok(find_position_escrow_pda(&usdc_bank_f.key, &receiver.key, &source.key).0)
}

async fn total_asset_shares(bank: &BankFixture) -> I80F48 {
    bank.load().await.total_asset_shares.into()
}

async fn total_escrowed_asset_shares(bank: &BankFixture) -> I80F48 {
    bank.load().await.total_escrowed_asset_shares.into()
}

async fn is_closed(test_f: &TestFixture, key: Pubkey) -> bool {
    let ctx = test_f.context.borrow_mut();
    ctx.banks_client.get_account(key).await.unwrap().is_none()
}

async fn now(test_f: &TestFixture) -> i64 {
    let ctx = test_f.context.borrow_mut();
    let clock: Clock = ctx.banks_client.get_sysvar().await.unwrap();
    clock.unix_timestamp
}

/// Escrowed shares leave the source but stay in the bank's totals, where the bank also counts them
/// as escrowed, and the receiver accepts them into an account of its choice.
#[tokio::test]
async fn position_escrow_holds_shares_until_the_receiver_accepts() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (receiver_key, receiver) = empty_receiver(&test_f).await;
    let receiver_other = account_of(&test_f, &receiver_key).await;
    let bank_shares = total_asset_shares(usdc_bank_f).await;

    let escrow = escrow_usdc(&test_f, &source, &receiver, 100.0).await?;
    let escrow_account = test_f.load_and_deserialize::<PositionEscrow>(&escrow).await;
    assert_eq!(I80F48::from(escrow_account.asset_shares), usdc_shares(100));
    assert_eq!(asset_shares(&source, usdc_bank_f).await, usdc_shares(400));
    assert_eq!(total_asset_shares(usdc_bank_f).await, bank_shares);
    assert_eq!(
        total_escrowed_asset_shares(usdc_bank_f).await,
        usdc_shares(100)
    );

    receiver_other
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&receiver_key))
        .await?;
    assert_eq!(
        asset_shares(&receiver_other, usdc_bank_f).await,
        usdc_shares(100)
    );
    assert_eq!(total_asset_shares(usdc_bank_f).await, bank_shares);
    assert_eq!(total_escrowed_asset_shares(usdc_bank_f).await, I80F48::ZERO);
    assert!(is_closed(&test_f, escrow).await);
    Ok(())
}

/// An account cannot send into an escrow to itself.
#[tokio::test]
async fn position_escrow_rejects_identical_accounts() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;

    let res = source
        .try_escrow_position(&source, usdc_bank_f, 100.0, &test_f.payer_keypair())
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferIdenticalAccounts
    );
    Ok(())
}

/// Without the receiver's signature, an escrow lands nowhere the receiver holds no deposit.
#[tokio::test]
async fn position_escrow_claim_needs_the_receivers_signature() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (_receiver_key, receiver) = empty_receiver(&test_f).await;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 100.0).await?;

    let res = receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, None)
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferConsentRequired
    );

    let (stranger_key, stranger) = empty_receiver(&test_f).await;
    let res = stranger
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&stranger_key))
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferConsentRequired
    );
    Ok(())
}

/// Anyone may sweep an escrow into the receiver's existing, untagged deposit in the bank.
#[tokio::test]
async fn position_escrow_sweeps_into_an_existing_deposit() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (_receiver_key, receiver) = receiver_with_usdc_deposit(&test_f).await;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 100.0).await?;

    receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, None)
        .await?;
    assert_eq!(asset_shares(&receiver, usdc_bank_f).await, usdc_shares(110));
    assert!(is_closed(&test_f, escrow).await);
    Ok(())
}

/// The receiver may decline at any time, which returns the shares to the source account.
#[tokio::test]
async fn position_escrow_receiver_declines_back_to_the_source() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (receiver_key, receiver) = empty_receiver(&test_f).await;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 100.0).await?;

    source
        .try_return_position_escrow(escrow, usdc_bank_f, Some(&receiver_key))
        .await?;
    assert_eq!(asset_shares(&source, usdc_bank_f).await, usdc_shares(500));
    assert!(is_closed(&test_f, escrow).await);
    Ok(())
}

/// Before expiry only the receiver may return an escrow; after it, anyone may return it to the
/// source account, and only there.
#[tokio::test]
async fn position_escrow_returns_to_the_source_after_expiry() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (_receiver_key, receiver) = empty_receiver(&test_f).await;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 100.0).await?;

    let res = source
        .try_return_position_escrow(escrow, usdc_bank_f, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::PositionEscrowNotExpired);

    advance_clock(&test_f, POSITION_ESCROW_EXPIRY_SECONDS).await;
    let (_stranger_key, stranger) = empty_receiver(&test_f).await;
    let res = stranger
        .try_return_position_escrow(escrow, usdc_bank_f, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    source
        .try_return_position_escrow(escrow, usdc_bank_f, None)
        .await?;
    assert_eq!(asset_shares(&source, usdc_bank_f).await, usdc_shares(500));
    assert!(is_closed(&test_f, escrow).await);
    Ok(())
}

/// After expiry the sender's authority may return the shares into any of its accounts.
#[tokio::test]
async fn position_escrow_sender_picks_the_return_account() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let source_other = test_f.create_marginfi_account().await;
    let (_receiver_key, receiver) = empty_receiver(&test_f).await;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 100.0).await?;
    advance_clock(&test_f, POSITION_ESCROW_EXPIRY_SECONDS).await;

    let payer = test_f.payer_keypair();
    source_other
        .try_return_position_escrow(escrow, usdc_bank_f, Some(&payer))
        .await?;
    assert_eq!(
        asset_shares(&source_other, usdc_bank_f).await,
        usdc_shares(100)
    );
    Ok(())
}

/// Each send adds to the escrow and restarts its expiry.
#[tokio::test]
async fn position_escrow_send_restarts_the_expiry() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (_receiver_key, receiver) = empty_receiver(&test_f).await;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 100.0).await?;

    advance_clock(&test_f, 1_000).await;
    let payer = test_f.payer_keypair();
    source
        .try_escrow_position(&receiver, usdc_bank_f, 50.0, &payer)
        .await?;

    let escrow_account = test_f.load_and_deserialize::<PositionEscrow>(&escrow).await;
    assert_eq!(I80F48::from(escrow_account.asset_shares), usdc_shares(150));
    assert_eq!(
        escrow_account.expires_at,
        now(&test_f).await + POSITION_ESCROW_EXPIRY_SECONDS
    );
    Ok(())
}

#[tokio::test]
async fn position_escrow_rejects_a_receiver_that_disabled_receiving() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let source = usdc_depositor(&test_f).await;
    let (_receiver_key, receiver) = empty_receiver(&test_f).await;
    let mut receiver_account = receiver.load().await;
    receiver_account.account_flags |= ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED;
    receiver.set_account(&receiver_account).await?;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let payer = test_f.payer_keypair();
    let res = source
        .try_escrow_position(&receiver, usdc_bank_f, 100.0, &payer)
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferReceiveDisabled
    );
    Ok(())
}

#[tokio::test]
async fn position_escrow_rejects_debt() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let source = indebted_source(&test_f, 500.0).await;
    let (_receiver_key, receiver) = empty_receiver(&test_f).await;
    let payer = test_f.payer_keypair();
    let res = source
        .try_escrow_position(&receiver, sol_bank_f, 10.0, &payer)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::NoAssetFound);
    Ok(())
}
