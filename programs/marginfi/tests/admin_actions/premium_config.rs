use bytemuck::Zeroable;
use fixed::types::I80F48;
use fixtures::prelude::*;
use marginfi::state::bank::BankImpl;
use marginfi_type_crate::{
    constants::PREMIUM_ACTIVE,
    types::{milli_to_u32, FeeState, PremiumEntry, MAX_PREMIUM_ENTRIES},
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

fn entry(collateral_tag: u16, liability_tag: u16, percent: f64) -> PremiumEntry {
    PremiumEntry {
        collateral_tag,
        liability_tag,
        rate: milli_to_u32(I80F48::from_num(percent / 100.0)),
    }
}

async fn load_fee_state(test_f: &TestFixture) -> FeeState {
    let key = test_f.marginfi_group.fee_state;
    let account = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();
    *bytemuck::from_bytes::<FeeState>(&account.data[8..])
}

#[tokio::test]
async fn premium_config_happy_path() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;

    // Global fee admin (payer) sets the premium wallet
    let premium_wallet = Keypair::new().pubkey();
    group_f
        .try_edit_fee_state_premium(Some(premium_wallet))
        .await?;

    let fee_state = load_fee_state(&test_f).await;
    assert_eq!(fee_state.premium_wallet, premium_wallet);
    // Editing the premium wallet must not clobber the v1 fields
    assert_eq!(fee_state.global_fee_admin, test_f.payer());

    // emode admin (payer) sets pairs one at a time; entries are stored sorted regardless of
    // the order they were added in
    group_f
        .try_configure_group_premium(entry(200, 100, 1.0))
        .await?;
    group_f
        .try_configure_group_premium(entry(100, 200, 0.5))
        .await?;

    let group = group_f.load().await;
    assert_eq!(group.premium_settings.entry_count, 2);
    assert_eq!(
        group.premium_settings.entry_capacity,
        MAX_PREMIUM_ENTRIES as u16
    );
    assert_eq!(group.premium_entries[0].collateral_tag, 100);
    assert_eq!(group.premium_entries[1].collateral_tag, 200);

    // Re-configuring an existing pair updates its rate in place
    group_f
        .try_configure_group_premium(entry(100, 200, 2.0))
        .await?;
    let group = group_f.load().await;
    assert_eq!(group.premium_settings.entry_count, 2);
    assert_eq!(group.premium_entries[0].rate, entry(100, 200, 2.0).rate);

    // Rate 0 removes the pair and zeroes the vacated slot
    group_f
        .try_configure_group_premium(entry(100, 200, 0.0))
        .await?;
    let group = group_f.load().await;
    assert_eq!(group.premium_settings.entry_count, 1);
    assert_eq!(group.premium_entries[0].collateral_tag, 200);
    assert_eq!(group.premium_entries[1], PremiumEntry::zeroed());

    // Bank premium tag + PREMIUM_ACTIVE
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    group_f
        .try_configure_bank_premium(usdc_bank_f, 100, true)
        .await?;
    let usdc_bank = usdc_bank_f.load().await;
    assert_eq!(usdc_bank.premium_tag, 100);
    assert!(usdc_bank.get_flag(PREMIUM_ACTIVE));

    // Disabling clears the flag but keeps the tag
    group_f
        .try_configure_bank_premium(usdc_bank_f, 100, false)
        .await?;
    let usdc_bank = usdc_bank_f.load().await;
    assert!(!usdc_bank.get_flag(PREMIUM_ACTIVE));
    assert_eq!(usdc_bank.premium_tag, 100);

    // Removing the last pair turns the matrix off (entry_count is the single source of truth)
    group_f
        .try_configure_group_premium(entry(200, 100, 0.0))
        .await?;
    let group = group_f.load().await;
    assert_eq!(group.premium_settings.entry_count, 0);

    Ok(())
}

#[tokio::test]
async fn premium_config_wrong_admin_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;
    let intruder = Keypair::new();

    let res = group_f
        .try_configure_group_premium_with_signer(entry(1, 2, 1.0), &intruder)
        .await;
    assert!(res.is_err());

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let res = group_f
        .try_configure_bank_premium_with_signer(usdc_bank_f, 100, true, &intruder)
        .await;
    assert!(res.is_err());

    let res = group_f
        .try_edit_fee_state_premium_with_signer(Some(Pubkey::new_unique()), &intruder)
        .await;
    assert!(res.is_err());

    Ok(())
}

#[tokio::test]
async fn premium_config_matrix_validation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;
    // Zero tags rejected
    let res = group_f
        .try_configure_group_premium(entry(0, 100, 1.0))
        .await;
    assert!(res.is_err());
    let res = group_f
        .try_configure_group_premium(entry(100, 0, 1.0))
        .await;
    assert!(res.is_err());

    // Removing a pair that is not in the matrix fails loudly
    let res = group_f.try_configure_group_premium(entry(9, 9, 0.0)).await;
    assert!(res.is_err());

    // Fill to capacity one pair at a time; the 65th insert is rejected
    for i in 1..=MAX_PREMIUM_ENTRIES as u16 {
        group_f
            .try_configure_group_premium(entry(i, 1000, 1.0))
            .await?;
    }
    let group = group_f.load().await;
    assert_eq!(
        group.premium_settings.entry_count,
        MAX_PREMIUM_ENTRIES as u16
    );
    let res = group_f
        .try_configure_group_premium(entry(1001, 1000, 1.0))
        .await;
    assert!(res.is_err());

    // At capacity: updating an existing pair still works, and so does removing one
    group_f
        .try_configure_group_premium(entry(5, 1000, 3.0))
        .await?;
    let group = group_f.load().await;
    assert_eq!(group.premium_entries[4].rate, entry(5, 1000, 3.0).rate);
    group_f
        .try_configure_group_premium(entry(5, 1000, 0.0))
        .await?;
    let group = group_f.load().await;
    assert_eq!(
        group.premium_settings.entry_count,
        MAX_PREMIUM_ENTRIES as u16 - 1
    );

    Ok(())
}
