use super::premium::{advance_clock, premium_test_fixture};
use super::transfer_position::{
    asset_shares, indebted_source, usdc_depositor, user_with_usdc_deposit,
};
use anchor_lang::prelude::Clock;
use fixed::types::I80F48;
use fixtures::bank::BankFixture;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::utils::find_position_escrow_pda;
use fixtures::{assert_custom_error, native, prelude::*};
use marginfi::constants::POSITION_ESCROW_EXPIRY_SECONDS;
use marginfi::prelude::*;
use marginfi_type_crate::types::{
    PositionEscrow, ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED, SECONDS_PER_DAY,
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

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
    Ok(escrow_pda(
        usdc_bank_f,
        &receiver.load().await.authority,
        source,
    ))
}

/// The escrow `source` sends into for `receiver_authority` with an empty memo.
fn escrow_pda(
    bank: &BankFixture,
    receiver_authority: &Pubkey,
    source: &MarginfiAccountFixture,
) -> Pubkey {
    find_position_escrow_pda(&bank.key, receiver_authority, &source.key, &[0; 16]).0
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

/// An escrow can be sent to a wallet with no account yet. The named account and the memo are kept
/// for accounting, and the wallet accepts into an account it opens later.
#[tokio::test]
async fn position_escrow_reaches_a_wallet_with_no_account() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let wallet = Keypair::new();
    let named_account = Pubkey::new_unique();
    let memo = [7; 16];
    source
        .try_escrow_position_with_memo(
            wallet.pubkey(),
            named_account,
            usdc_bank_f,
            100.0,
            memo,
            &test_f.payer_keypair(),
        )
        .await?;
    let escrow = find_position_escrow_pda(&usdc_bank_f.key, &wallet.pubkey(), &source.key, &memo).0;
    let escrow_account = test_f.load_and_deserialize::<PositionEscrow>(&escrow).await;
    assert_eq!(escrow_account.receiver_authority, wallet.pubkey());
    assert_eq!(escrow_account.receiver_account, named_account);
    assert_eq!(escrow_account.memo, memo);

    let account = account_of(&test_f, &wallet).await;
    account
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&wallet))
        .await?;
    assert_eq!(asset_shares(&account, usdc_bank_f).await, usdc_shares(100));
    Ok(())
}

/// Each memo opens its own escrow, and a repeat memo adds to the escrow it opened.
#[tokio::test]
async fn position_escrow_opens_one_escrow_per_memo() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (receiver_key, receiver) = empty_receiver(&test_f).await;
    let payer = test_f.payer_keypair();
    for (memo, ui) in [([1; 16], 100.0), ([2; 16], 30.0), ([1; 16], 50.0)] {
        source
            .try_escrow_position_with_memo(
                receiver_key.pubkey(),
                receiver.key,
                usdc_bank_f,
                ui,
                memo,
                &payer,
            )
            .await?;
    }

    for (memo, ui) in [([1; 16], 150), ([2; 16], 30)] {
        let escrow =
            find_position_escrow_pda(&usdc_bank_f.key, &receiver_key.pubkey(), &source.key, &memo)
                .0;
        let escrow_account = test_f.load_and_deserialize::<PositionEscrow>(&escrow).await;
        assert_eq!(I80F48::from(escrow_account.asset_shares), usdc_shares(ui));
    }
    assert_eq!(source.load().await.active_position_escrows, 2);
    Ok(())
}

/// Without the receiver's signature, an escrow lands only in an untagged deposit that an account of
/// the receiver's authority holds, never in a stranger's.
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

    let (_depositor_key, depositor) = user_with_usdc_deposit(&test_f).await;
    let res = depositor
        .try_claim_position_escrow(escrow, usdc_bank_f, None)
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferConsentRequired
    );
    assert_eq!(asset_shares(&depositor, usdc_bank_f).await, usdc_shares(10));
    Ok(())
}

/// Anyone may sweep an escrow into an untagged deposit in the bank that any account of the
/// receiver's authority holds.
#[tokio::test]
async fn position_escrow_sweeps_into_an_existing_deposit() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (receiver_key, receiver) = user_with_usdc_deposit(&test_f).await;
    let named = account_of(&test_f, &receiver_key).await;
    let escrow = escrow_usdc(&test_f, &source, &named, 100.0).await?;

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

/// Disabling receiving does not stop a send, but no claim lands in that account, even with the
/// receiver's signature. The receiver may accept into another of its accounts.
#[tokio::test]
async fn position_escrow_receive_opt_out_blocks_claims_not_sends() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (receiver_key, receiver) = user_with_usdc_deposit(&test_f).await;
    let receiver_other = account_of(&test_f, &receiver_key).await;
    let mut receiver_account = receiver.load().await;
    receiver_account.account_flags |= ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED;
    receiver.set_account(&receiver_account).await?;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 100.0).await?;

    let res = receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, None)
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferReceiveDisabled
    );
    let res = receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&receiver_key))
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferReceiveDisabled
    );

    receiver_other
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&receiver_key))
        .await?;
    assert_eq!(
        asset_shares(&receiver_other, usdc_bank_f).await,
        usdc_shares(100)
    );
    Ok(())
}

/// A frozen source cannot send, and the group admin cannot send for it.
#[tokio::test]
async fn position_escrow_frozen_source_cannot_send() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (owner, source) = user_with_usdc_deposit(&test_f).await;
    let (_receiver_key, receiver) = empty_receiver(&test_f).await;
    source.try_set_freeze(true).await?;

    let res = source
        .try_escrow_position(&receiver, usdc_bank_f, 5.0, &owner)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);
    let admin = test_f.payer_keypair();
    let res = source
        .try_escrow_position(&receiver, usdc_bank_f, 5.0, &admin)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);
    assert_eq!(asset_shares(&source, usdc_bank_f).await, usdc_shares(10));
    Ok(())
}

/// While the source is frozen, only the group admin may return an expired escrow, and only into
/// the source.
#[tokio::test]
async fn position_escrow_frozen_source_takes_returns_only_from_the_admin() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (owner, source) = user_with_usdc_deposit(&test_f).await;
    let owner_other = account_of(&test_f, &owner).await;
    let (receiver_key, receiver) = empty_receiver(&test_f).await;
    source
        .try_escrow_position(&receiver, usdc_bank_f, 5.0, &owner)
        .await?;
    let escrow = escrow_pda(usdc_bank_f, &receiver_key.pubkey(), &source);
    source.try_set_freeze(true).await?;
    advance_clock(&test_f, POSITION_ESCROW_EXPIRY_SECONDS).await;

    let res = source
        .try_return_position_escrow(escrow, usdc_bank_f, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);
    let res = source
        .try_return_position_escrow(escrow, usdc_bank_f, Some(&receiver_key))
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);
    let res = owner_other
        .try_return_position_escrow(escrow, usdc_bank_f, Some(&owner))
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    let admin = test_f.payer_keypair();
    source
        .try_return_position_escrow(escrow, usdc_bank_f, Some(&admin))
        .await?;
    assert_eq!(asset_shares(&source, usdc_bank_f).await, usdc_shares(10));
    assert!(is_closed(&test_f, escrow).await);
    Ok(())
}

/// A frozen source does not stop the receiver from accepting.
#[tokio::test]
async fn position_escrow_frozen_source_does_not_block_claims() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (owner, source) = user_with_usdc_deposit(&test_f).await;
    let (receiver_key, receiver) = empty_receiver(&test_f).await;
    source
        .try_escrow_position(&receiver, usdc_bank_f, 5.0, &owner)
        .await?;
    let escrow = escrow_pda(usdc_bank_f, &receiver_key.pubkey(), &source);
    source.try_set_freeze(true).await?;

    receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&receiver_key))
        .await?;
    assert_eq!(asset_shares(&receiver, usdc_bank_f).await, usdc_shares(5));
    Ok(())
}

/// A frozen receiver can still be sent to. While it is frozen, only the group admin may accept, and
/// only into the frozen account.
#[tokio::test]
async fn position_escrow_frozen_receiver_takes_claims_only_from_the_admin() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (receiver_key, receiver) = user_with_usdc_deposit(&test_f).await;
    let receiver_other = account_of(&test_f, &receiver_key).await;
    receiver.try_set_freeze(true).await?;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 5.0).await?;

    let res = receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&receiver_key))
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);
    let res = receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, None)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::AccountFrozen);
    let admin = test_f.payer_keypair();
    let res = receiver_other
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&admin))
        .await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::PositionTransferConsentRequired
    );

    receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&admin))
        .await?;
    assert_eq!(asset_shares(&receiver, usdc_bank_f).await, usdc_shares(15));
    assert!(is_closed(&test_f, escrow).await);
    Ok(())
}

/// An open escrow keeps the source from closing, by its owner or by `admin_close_account`, and from
/// migrating. Repeat sends into the same escrow count once.
#[tokio::test]
async fn position_escrow_keeps_the_source_open() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let source = usdc_depositor(&test_f).await;
    let (receiver_key, receiver) = empty_receiver(&test_f).await;
    let escrow = escrow_usdc(&test_f, &source, &receiver, 200.0).await?;
    escrow_usdc(&test_f, &source, &receiver, 300.0).await?;

    let res = source.try_close_account(0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);
    let new_account = Keypair::new();
    let res = source
        .try_transfer_account(
            new_account.pubkey(),
            test_f.payer(),
            None,
            None,
            &new_account,
            test_f.marginfi_group.fee_wallet,
        )
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);
    advance_clock(&test_f, 60 * SECONDS_PER_DAY + 1).await;
    let res = source
        .try_admin_close_account(test_f.marginfi_group.fee_wallet)
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::IllegalAction);

    receiver
        .try_claim_position_escrow(escrow, usdc_bank_f, Some(&receiver_key))
        .await?;
    source
        .try_admin_close_account(test_f.marginfi_group.fee_wallet)
        .await?;
    assert!(is_closed(&test_f, source.key).await);
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
