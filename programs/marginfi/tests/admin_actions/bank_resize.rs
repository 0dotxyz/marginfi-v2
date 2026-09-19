use fixtures::prelude::*;
use marginfi_type_crate::types::Bank;
use pretty_assertions::assert_eq;
use solana_program_test::*;

/// Mainnet migration rehearsal: a v1-sized bank is bricked under this program version, the
/// permissionless resize un-bricks it, and state survives byte-for-byte.
#[tokio::test]
async fn bank_resize_unbricks_v1_account() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc = test_f.get_bank(&BankMint::Usdc);
    let sol = test_f.get_bank(&BankMint::Sol);
    let banks_client = test_f.context.borrow().banks_client.clone();
    let rent = banks_client.get_rent().await?;

    let fresh = banks_client.get_account(usdc.key).await?.unwrap();
    assert_eq!(fresh.data.len(), 8 + Bank::LEN);
    assert!(fresh.data[8 + Bank::V1_LEN..].iter().all(|b| *b == 0));

    // The mainnet bank before this deploy: v1 size and v1-level lamports. Any ix loading it
    // fails until resized.
    test_f
        .marginfi_group
        .truncate_bank_account_to_v1(usdc.key)
        .await;
    let mut before = banks_client.get_account(usdc.key).await?.unwrap();
    assert_eq!(before.data.len(), 8 + Bank::V1_LEN);
    before.lamports = rent.minimum_balance(before.data.len());
    test_f
        .context
        .borrow_mut()
        .set_account(&usdc.key, &before.clone().into());

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = usdc.mint.create_token_account_and_mint_to(1_000.0).await;
    let res = lender
        .try_bank_deposit(lender_usdc.key, usdc, 1_000.0, None)
        .await;
    assert!(res.is_err());

    test_f
        .marginfi_group
        .try_resize_bank_account(usdc.key)
        .await?;

    let after = banks_client.get_account(usdc.key).await?.unwrap();
    assert_eq!(after.data.len(), 8 + Bank::LEN);
    assert_eq!(&after.data[..before.data.len()], &before.data[..]);
    assert!(after.data[before.data.len()..].iter().all(|b| *b == 0));
    assert_eq!(after.owner, marginfi::ID);
    assert_eq!(after.lamports, rent.minimum_balance(after.data.len()));

    lender
        .try_bank_deposit(lender_usdc.key, usdc, 1_000.0, None)
        .await?;
    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = sol.mint.create_token_account_and_mint_to(100.0).await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol, 100.0, None)
        .await?;
    let borrower_usdc = usdc.mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc, 100.0)
        .await?;
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

    // Resizing an already-grown account is rejected. Warp first: on the same blockhash
    // BanksClient dedups this byte-identical transaction into the earlier cached success.
    test_f.context.borrow_mut().warp_to_slot(100).unwrap();
    let res = test_f
        .marginfi_group
        .try_resize_bank_account(usdc.key)
        .await;
    assert!(res.is_err());
    assert_eq!(
        banks_client
            .get_account(usdc.key)
            .await?
            .unwrap()
            .data
            .len(),
        8 + Bank::LEN
    );

    Ok(())
}

#[tokio::test]
async fn bank_resize_rejects_unsupported_accounts() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let res = test_f
        .marginfi_group
        .try_resize_bank_account(test_f.marginfi_group.key)
        .await;
    assert!(res.is_err());

    let res = test_f
        .marginfi_group
        .try_resize_bank_account(test_f.usdc_mint.key)
        .await;
    assert!(res.is_err());

    Ok(())
}
