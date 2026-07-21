use fixtures::prelude::*;
use marginfi_type_crate::types::{FeeState, MarginfiGroup};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::pubkey::Pubkey;

async fn group_account_len(test_f: &TestFixture) -> usize {
    let banks_client = test_f.context.borrow().banks_client.clone();
    banks_client
        .get_account(test_f.marginfi_group.key)
        .await
        .unwrap()
        .unwrap()
        .data
        .len()
}

/// The mainnet migration rehearsal: a v1-sized group cannot be loaded by this program
/// version (bricked between upgrade and resize), the permissionless resize un-bricks it, and
/// state survives byte-for-byte.
#[tokio::test]
async fn group_resize_unbricks_v1_account() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;

    // New groups are born at the current (full) size with the reserved region zeroed
    let banks_client = test_f.context.borrow().banks_client.clone();
    let fresh = banks_client.get_account(group_f.key).await?.unwrap();
    assert_eq!(fresh.data.len(), 8 + MarginfiGroup::LEN);
    assert!(fresh.data[8 + MarginfiGroup::V1_LEN..]
        .iter()
        .all(|b| *b == 0));

    let group = group_f.load().await;
    let admin = group.admin;
    let curve_admin = group.delegate_curve_admin;
    let limit_admin = group.delegate_limit_admin;
    let flow_admin = group.delegate_flow_admin;
    let emissions_admin = group.delegate_emissions_admin;
    let metadata_admin = group.metadata_admin;
    let risk_admin = group.risk_admin;

    // Simulate the mainnet group as it exists BEFORE this deploy: v1-sized. Under this
    // program version any ix loading it fails — this is the (brief) window between the
    // program upgrade and the resize transaction.
    group_f.truncate_group_account_to_v1().await;
    let res = group_f
        .try_update_with_flow_admin(
            admin,
            Pubkey::new_unique(),
            curve_admin,
            limit_admin,
            flow_admin,
            emissions_admin,
            metadata_admin,
            risk_admin,
        )
        .await;
    assert!(res.is_err());

    // The permissionless resize un-bricks it; state is preserved and the group is operable
    group_f.try_resize_group_account().await?;
    assert_eq!(group_account_len(&test_f).await, 8 + MarginfiGroup::LEN);
    let group = group_f.load().await;
    assert_eq!(group.admin, admin);
    let new_emode_admin = Pubkey::new_unique();
    group_f
        .try_update_with_flow_admin(
            admin,
            new_emode_admin,
            curve_admin,
            limit_admin,
            flow_admin,
            emissions_admin,
            metadata_admin,
            risk_admin,
        )
        .await?;
    let group = group_f.load().await;
    assert_eq!(group.emode_admin, new_emode_admin);

    // Resizing an already-grown account is rejected. (Warp a slot first: this transaction is
    // byte-identical to the earlier successful resize, and with the same blockhash it would be
    // signature-deduped by BanksClient into that cached success.)
    test_f.context.borrow_mut().warp_to_slot(100).unwrap();
    let res = group_f.try_resize_group_account().await;
    assert!(res.is_err());

    Ok(())
}

#[tokio::test]
async fn group_resize_preserves_state_byte_for_byte() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;
    group_f.truncate_group_account_to_v1().await;

    let banks_client = test_f.context.borrow().banks_client.clone();
    let rent = banks_client.get_rent().await?;

    // Match the real mainnet state exactly: v1 size AND v1-level lamports, so the resize must
    // actually exercise the payer rent top-up (truncation alone would leave the original
    // full-size lamports and skip that branch).
    let mut before = banks_client.get_account(group_f.key).await?.unwrap();
    assert_eq!(before.data.len(), 8 + MarginfiGroup::V1_LEN);
    before.lamports = rent.minimum_balance(before.data.len());
    test_f
        .context
        .borrow_mut()
        .set_account(&group_f.key, &before.clone().into());

    group_f.try_resize_group_account().await?;

    let after = banks_client.get_account(group_f.key).await?.unwrap();
    // Grown to the current size; the v1 prefix is byte-identical; the growth is all zeros
    assert_eq!(after.data.len(), 8 + MarginfiGroup::LEN);
    assert_eq!(&after.data[..before.data.len()], &before.data[..]);
    assert!(after.data[before.data.len()..].iter().all(|b| *b == 0));
    // Still owned by the program; the payer topped it up to exactly the new rent minimum
    assert_eq!(after.owner, marginfi::ID);
    assert_eq!(after.lamports, rent.minimum_balance(after.data.len()));

    Ok(())
}

#[tokio::test]
async fn fee_state_resize_unbricks_v1_account() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;
    let fee_state_key = group_f.fee_state;
    let banks_client = test_f.context.borrow().banks_client.clone();
    let rent = banks_client.get_rent().await?;

    // A fresh fee state is born at the current (full) size with the reserved region zeroed
    let fresh = banks_client.get_account(fee_state_key).await?.unwrap();
    assert_eq!(fresh.data.len(), 8 + FeeState::LEN);
    assert!(fresh.data[8 + FeeState::V1_LEN..].iter().all(|b| *b == 0));

    // Simulate the mainnet fee state: v1 size AND v1-level lamports (exercises the rent
    // top-up). Any ix loading it fails until resized.
    group_f.truncate_fee_state_to_v1().await;
    let mut before = banks_client.get_account(fee_state_key).await?.unwrap();
    assert_eq!(before.data.len(), 8 + FeeState::V1_LEN);
    before.lamports = rent.minimum_balance(before.data.len());
    test_f
        .context
        .borrow_mut()
        .set_account(&fee_state_key, &before.clone().into());

    let res = group_f.try_propagate_fee_state().await;
    assert!(res.is_err());

    // The permissionless resize un-bricks it
    group_f.try_resize_fee_state().await?;

    let after = banks_client.get_account(fee_state_key).await?.unwrap();
    assert_eq!(after.data.len(), 8 + FeeState::LEN);
    assert_eq!(&after.data[..before.data.len()], &before.data[..]);
    assert!(after.data[before.data.len()..].iter().all(|b| *b == 0));
    assert_eq!(after.owner, marginfi::ID);
    assert_eq!(after.lamports, rent.minimum_balance(after.data.len()));

    // Operable again, and a repeat resize is rejected. (Warp first: these transactions are
    // byte-identical to earlier ones, and with the same blockhash they would be
    // signature-deduped by BanksClient into the cached earlier results.)
    test_f.context.borrow_mut().warp_to_slot(50).unwrap();
    group_f.try_propagate_fee_state().await?;
    test_f.context.borrow_mut().warp_to_slot(100).unwrap();
    let res = group_f.try_resize_fee_state().await;
    assert!(res.is_err());

    Ok(())
}

#[tokio::test]
async fn group_resize_rejects_unsupported_accounts() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;

    // Owned by marginfi but the wrong discriminator (a bank)
    let bank_key = test_f.get_bank(&BankMint::Usdc).key;
    let res = group_f.try_resize_account_key(bank_key).await;
    assert!(res.is_err());

    // Not owned by marginfi at all (a mint)
    let res = group_f.try_resize_account_key(test_f.usdc_mint.key).await;
    assert!(res.is_err());

    // The group resize ix cannot touch the fee state (wrong discriminator)
    group_f.truncate_fee_state_to_v1().await;
    let res = group_f.try_resize_account_key(group_f.fee_state).await;
    assert!(res.is_err());

    // Attacker-crafted accounts: anyone can allocate+assign a ZERO-FILLED account to the
    // program (but can never write a discriminator into it, since only the owning program
    // writes data). Empty data must fail the length check without panicking; zeroed data
    // must fail the discriminator check.
    let empty_key = Pubkey::new_unique();
    let zeroed_key = Pubkey::new_unique();
    {
        let mut ctx = test_f.context.borrow_mut();
        for (key, data) in [(empty_key, vec![]), (zeroed_key, vec![0u8; 100])] {
            ctx.set_account(
                &key,
                &solana_sdk::account::Account {
                    lamports: 1_000_000,
                    data,
                    owner: marginfi::ID,
                    executable: false,
                    rent_epoch: 0,
                }
                .into(),
            );
        }
    }
    let res = group_f.try_resize_account_key(empty_key).await;
    assert!(res.is_err());
    let res = group_f.try_resize_account_key(zeroed_key).await;
    assert!(res.is_err());

    Ok(())
}
