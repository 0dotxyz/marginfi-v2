use fixed::types::I80F48;
use fixtures::prelude::*;
use marginfi::state::bank::BankImpl;
use marginfi_type_crate::{
    constants::PREMIUM_ACTIVE,
    types::{milli_to_u32, FeeStateV2, PremiumEntry, MAX_PREMIUM_ENTRIES},
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use fixtures::marginfi_group::MarginfiGroupFixture;

fn entry(collateral_tag: u16, liability_tag: u16, percent: f64) -> PremiumEntry {
    PremiumEntry {
        collateral_tag,
        liability_tag,
        rate: milli_to_u32(I80F48::from_num(percent / 100.0)),
    }
}

async fn load_fee_state_v2(test_f: &TestFixture) -> FeeStateV2 {
    let key = MarginfiGroupFixture::fee_state_v2_key();
    let account = test_f
        .context
        .borrow_mut()
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();
    *bytemuck::from_bytes::<FeeStateV2>(&account.data[8..])
}

#[tokio::test]
async fn premium_config_happy_path() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;

    group_f.try_init_and_copy_fee_state_v2().await?;

    // Global fee admin (payer) sets the premium wallet
    let premium_wallet = Keypair::new().pubkey();
    group_f
        .try_edit_fee_state_v2_premium(Some(premium_wallet))
        .await?;

    let fee_state_v2 = load_fee_state_v2(&test_f).await;
    assert_eq!(fee_state_v2.premium_wallet, premium_wallet);
    // Copy must not have clobbered the premium fields but did copy v1 fields
    assert_eq!(fee_state_v2.global_fee_admin, test_f.payer());

    // emode admin (payer) sets the matrix; entries are stored sorted
    group_f
        .try_configure_group_premium(vec![entry(200, 100, 1.0), entry(100, 200, 0.5)])
        .await?;

    let group = group_f.load().await;
    assert_eq!(group.premium_settings.entry_count, 2);
    assert_eq!(
        group.premium_settings.entry_capacity,
        MAX_PREMIUM_ENTRIES as u16
    );
    assert_eq!(group.premium_entries[0].collateral_tag, 100);
    assert_eq!(group.premium_entries[1].collateral_tag, 200);

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

    // Empty matrix = matrix off (entry_count is the single source of truth)
    group_f.try_configure_group_premium(vec![]).await?;
    let group = group_f.load().await;
    assert_eq!(group.premium_settings.entry_count, 0);

    // The PERMISSIONLESS copy must never clobber the premium fields: re-running it after the
    // wallet was set has to leave the wallet intact (it assigns only the named v1 fields).
    group_f.try_init_and_copy_fee_state_v2().await?;
    let fee_state_v2 = load_fee_state_v2(&test_f).await;
    assert_eq!(fee_state_v2.premium_wallet, premium_wallet);

    Ok(())
}

#[tokio::test]
async fn premium_config_wrong_admin_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;
    group_f.try_init_and_copy_fee_state_v2().await?;

    let intruder = Keypair::new();

    let res = group_f
        .try_configure_group_premium_with_signer(vec![entry(1, 2, 1.0)], &intruder)
        .await;
    assert!(res.is_err());

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let res = group_f
        .try_configure_bank_premium_with_signer(usdc_bank_f, 100, true, &intruder)
        .await;
    assert!(res.is_err());

    let res = group_f
        .try_edit_fee_state_v2_premium_with_signer(Some(Pubkey::new_unique()), &intruder)
        .await;
    assert!(res.is_err());

    Ok(())
}

#[tokio::test]
async fn premium_config_matrix_validation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let group_f = &test_f.marginfi_group;
    group_f.try_init_and_copy_fee_state_v2().await?;

    // Zero tags rejected
    let res = group_f
        .try_configure_group_premium(vec![entry(0, 100, 1.0)])
        .await;
    assert!(res.is_err());
    let res = group_f
        .try_configure_group_premium(vec![entry(100, 0, 1.0)])
        .await;
    assert!(res.is_err());

    // Duplicate (collateral, liability) pair rejected
    let res = group_f
        .try_configure_group_premium(vec![entry(1, 2, 1.0), entry(1, 2, 2.0)])
        .await;
    assert!(res.is_err());

    // Same collateral against different liabilities is fine
    group_f
        .try_configure_group_premium(vec![entry(1, 2, 1.0), entry(1, 3, 2.0)])
        .await?;

    // 65 entries rejected; exactly 64 accepted
    let full: Vec<PremiumEntry> = (1..=MAX_PREMIUM_ENTRIES as u16)
        .map(|i| entry(i, 1000, 1.0))
        .collect();
    let mut overfull = full.clone();
    overfull.push(entry(1001, 1000, 1.0));
    let res = group_f.try_configure_group_premium(overfull).await;
    assert!(res.is_err());
    group_f.try_configure_group_premium(full).await?;
    let group = group_f.load().await;
    assert_eq!(
        group.premium_settings.entry_count,
        MAX_PREMIUM_ENTRIES as u16
    );

    Ok(())
}

