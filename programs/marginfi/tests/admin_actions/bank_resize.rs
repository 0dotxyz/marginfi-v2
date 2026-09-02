use fixtures::prelude::*;
use marginfi_type_crate::constants::{BANK_ACCOUNT_LEN, BANK_RESERVED_BYTES};
use marginfi_type_crate::types::Bank;
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::pubkey::Pubkey;

async fn account_len(test_f: &TestFixture, key: Pubkey) -> usize {
    let banks_client = test_f.context.borrow().banks_client.clone();
    banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap()
        .data
        .len()
}

/// The migration rehearsal. Unlike the group, a bank is NOT bricked before its resize: this
/// release keeps `Bank` at its v1 size deliberately, so the protocol keeps working throughout
/// and only a later release claims the reserve.
#[tokio::test]
async fn bank_resize_grows_to_the_reserved_length() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc = test_f.get_bank(&BankMint::Usdc);

    // New banks are already born at the resized length; the struct now fills it.
    assert_eq!(account_len(&test_f, usdc.key).await, BANK_ACCOUNT_LEN);
    assert_eq!(BANK_ACCOUNT_LEN, 8 + Bank::V1_LEN + BANK_RESERVED_BYTES);

    let before = usdc.load().await;

    // Simulate a mainnet bank as it exists before the migration has reached it.
    test_f
        .marginfi_group
        .truncate_bank_account_to_v1(usdc.key)
        .await;
    assert_eq!(account_len(&test_f, usdc.key).await, 8 + Bank::V1_LEN);

    test_f
        .marginfi_group
        .try_resize_bank_account(usdc.key)
        .await?;

    assert_eq!(account_len(&test_f, usdc.key).await, BANK_ACCOUNT_LEN);

    // The struct is a byte-identical prefix, and the reserve is zeroed so a later layout that
    // claims it reads the same value a freshly created bank would.
    let after = usdc.load().await;
    assert_eq!(after.asset_share_value, before.asset_share_value);
    assert_eq!(after.liability_share_value, before.liability_share_value);
    assert_eq!(after.mint, before.mint);
    assert_eq!(after.group, before.group);

    let banks_client = test_f.context.borrow().banks_client.clone();
    let data = banks_client.get_account(usdc.key).await?.unwrap().data;
    assert!(data[8 + Bank::V1_LEN..].iter().all(|b| *b == 0));

    Ok(())
}

/// The point of resizing ahead of the struct: an oversized account must stay fully operable, or
/// the protocol is down between this release and the one that claims the reserve.
#[tokio::test]
async fn a_resized_bank_still_lends_and_borrows() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc = test_f.get_bank(&BankMint::Usdc);
    let sol = test_f.get_bank(&BankMint::Sol);

    for bank in [usdc.key, sol.key] {
        test_f
            .marginfi_group
            .truncate_bank_account_to_v1(bank)
            .await;
        test_f.marginfi_group.try_resize_bank_account(bank).await?;
    }

    let lender = test_f.create_marginfi_account().await;
    let lender_sol = sol.mint.create_token_account_and_mint_to(100.0).await;
    lender
        .try_bank_deposit(lender_sol.key, sol, 100.0, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_usdc = usdc.mint.create_token_account_and_mint_to(1_000.0).await;
    borrower
        .try_bank_deposit(borrower_usdc.key, usdc, 1_000.0, None)
        .await?;
    let borrower_sol = sol.mint.create_empty_token_account().await;
    borrower.try_bank_borrow(borrower_sol.key, sol, 1.0).await?;

    let account = borrower.load().await;
    assert_eq!(
        account
            .lending_account
            .balances
            .iter()
            .filter(|b| b.is_active())
            .count(),
        2
    );
    Ok(())
}

/// Grow-only: a bank already at the target is rejected. Raising `BANK_ACCOUNT_LEN` in a later
/// release deliberately makes this callable again, so a reserve top-up needs no new instruction.
#[tokio::test]
async fn resizing_a_bank_already_at_the_target_is_rejected() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc = test_f.get_bank(&BankMint::Usdc);

    test_f
        .marginfi_group
        .truncate_bank_account_to_v1(usdc.key)
        .await;
    test_f
        .marginfi_group
        .try_resize_bank_account(usdc.key)
        .await?;

    // Warp first: this transaction is byte-identical to the one above, and on the same blockhash
    // BanksClient would signature-dedup it into that cached success.
    test_f.context.borrow_mut().warp_to_slot(100).unwrap();
    let res = test_f
        .marginfi_group
        .try_resize_bank_account(usdc.key)
        .await;
    assert!(res.is_err());
    assert_eq!(account_len(&test_f, usdc.key).await, BANK_ACCOUNT_LEN);

    Ok(())
}

#[tokio::test]
async fn resizing_a_non_bank_account_is_rejected() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // Right owner, wrong discriminator.
    let res = test_f
        .marginfi_group
        .try_resize_bank_account(test_f.marginfi_group.key)
        .await;
    assert!(res.is_err());

    Ok(())
}
