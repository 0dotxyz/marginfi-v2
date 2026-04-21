use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::prelude::*;
use fixtures::test::TestFixture;
use marginfi_type_crate::types::{MarginfiAccount, ACCOUNT_DISABLED};
use solana_program_test::tokio;
use solana_sdk::{clock::Clock, signature::Keypair, signer::Signer};

// Test transfer account authority.
// No transfer flag set -- no longer matters, tx should succeed.
// RUST_BACKTRACE=1 cargo test-bpf marginfi_account_authority_transfer_no_flag_set -- --exact
#[tokio::test]
async fn marginfi_account_transfer_happy_path() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let authority = Keypair::new();
    let marginfi_account = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &authority,
    )
    .await;
    let new_authority = Keypair::new();
    let new_account = Keypair::new();

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let isolated_bank = test_f.get_bank(&BankMint::SolEqIsolated);

    let lp = test_f.create_marginfi_account().await;
    let lp_tokens = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lp.try_bank_deposit(lp_tokens.key, isolated_bank, 1_000, None)
        .await?;

    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to_with_owner(&authority.pubkey(), 1_000)
        .await;
    let user_iso = test_f
        .sol_equivalent_mint
        .create_empty_token_account_with_owner(&authority.pubkey())
        .await;

    marginfi_account
        .try_bank_deposit_with_authority(user_usdc.key, usdc_bank, 1_000, None, &authority)
        .await?;
    marginfi_account
        .try_bank_borrow_with_authority(user_iso.key, isolated_bank, 10, 0, &authority)
        .await?;

    let last_update = marginfi_account.load().await.last_update;
    let account_before = marginfi_account.load().await;
    assert_eq!(account_before.indexer_flags.is_empty, 0);
    assert_eq!(account_before.indexer_flags.is_single_borrower, 1);
    assert_eq!(account_before.indexer_flags.has_isolated, 1);

    // This is just to test that the account's last_update field is properly updated upon modification
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 sec
        clock.unix_timestamp += 1;
        ctx.set_sysvar(&clock);
    }

    let res = marginfi_account
        .try_transfer_account(
            new_account.pubkey(),
            new_authority.pubkey(),
            Some(clone_keypair(&authority)),
            None,
            &new_account,
            test_f.marginfi_group.fee_wallet,
        )
        .await;
    assert!(res.is_ok());

    // Old account still has the old authority, but is now inactive
    let account_old = marginfi_account.load().await;
    assert_eq!(account_old.last_update, last_update + 1);
    assert_eq!(account_old.authority, authority.pubkey());
    assert_eq!(account_old.account_flags, ACCOUNT_DISABLED);
    assert_eq!(account_old.migrated_to, new_account.pubkey());
    assert_eq!(account_old.indexer_flags.is_empty, 1);
    assert_eq!(account_old.indexer_flags.is_single_borrower, 0);
    assert_eq!(account_old.indexer_flags.has_isolated, 0);

    // The new account has the new authority
    let account_new: MarginfiAccount = test_f.load_and_deserialize(&new_account.pubkey()).await;
    assert_eq!(account_new.authority, new_authority.pubkey());
    // Old account is recorded as the migration source
    assert_eq!(account_new.migrated_from, marginfi_account.key);
    assert_eq!(account_new.last_update, last_update + 1);
    assert_eq!(account_new.indexer_flags.is_empty, 0);
    assert_eq!(account_new.indexer_flags.is_single_borrower, 1);
    assert_eq!(account_new.indexer_flags.has_isolated, 1);

    // Attempting to transfer again should fail
    let new_account_again = Keypair::new();
    let res = marginfi_account
        .try_transfer_account(
            new_account_again.pubkey(),
            new_authority.pubkey(),
            Some(clone_keypair(&authority)),
            None,
            &new_account_again,
            test_f.marginfi_group.fee_wallet,
        )
        .await;
    assert!(res.is_err());

    Ok(())
}

#[tokio::test]
async fn marginfi_account_transfer_not_account_owner() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;
    let marginfi_account = test_f.create_marginfi_account().await;
    let new_authority = Keypair::new();
    let new_account = Keypair::new();
    let signer = Keypair::new();

    let tx = marginfi_account
        .get_tx_transfer_account(
            new_account.pubkey(),
            new_authority.pubkey(),
            Some(signer),
            None,
            &new_account,
            test_f.marginfi_group.fee_wallet,
        )
        .await;

    // Note: Sending this tx takes a very long time (longer than all the other tests combined)
    // because for some reason it takes longer for a signature verification fail to return than it
    // does for other errors. We simulate instead here for testing SPEEEEEED
    let ctx = test_f.context.borrow_mut();
    let res = ctx.banks_client.simulate_transaction(tx).await;
    let is_err = res.unwrap().result.unwrap().is_err();

    // Assert the response is an error due to fact that a non-owner of the
    // acount attempted to initialize this account transfer
    assert!(is_err);

    Ok(())
}
