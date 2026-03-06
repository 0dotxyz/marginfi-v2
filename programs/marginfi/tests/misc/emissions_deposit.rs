use fixed::types::I80F48;
use fixtures::{assert_custom_error, native, prelude::*};
use marginfi::{assert_eq_with_tolerance, prelude::MarginfiError, state::bank::BankImpl};
use marginfi_type_crate::types::{BankConfigOpt, BankOperationalState};
use solana_program_test::*;

#[tokio::test]
async fn emissions_deposit_fails_when_bank_paused() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    usdc_bank
        .set_emissions(usdc_bank.mint.key, 0, I80F48::ZERO, 0)
        .await;

    let depositor = test_f.create_marginfi_account().await;
    let depositor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    depositor
        .try_bank_deposit(depositor_usdc.key, usdc_bank, 100.0, None)
        .await?;

    usdc_bank
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::Paused),
                ..Default::default()
            },
            None,
        )
        .await?;

    let funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    let res = usdc_bank
        .try_emissions_deposit(native!(50, "USDC"), funding.key)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankPaused);

    Ok(())
}

#[tokio::test]
async fn emissions_deposit_fails_when_bank_reduce_only() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    usdc_bank
        .set_emissions(usdc_bank.mint.key, 0, I80F48::ZERO, 0)
        .await;

    let depositor = test_f.create_marginfi_account().await;
    let depositor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    depositor
        .try_bank_deposit(depositor_usdc.key, usdc_bank, 100.0, None)
        .await?;

    usdc_bank
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    let res = usdc_bank
        .try_emissions_deposit(native!(50, "USDC"), funding.key)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankReduceOnly);

    Ok(())
}

#[tokio::test]
async fn emissions_deposit_fails_with_nonzero_transfer_fee() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::T22WithFee,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;

    let t22_bank = test_f.get_bank(&BankMint::T22WithFee);

    t22_bank
        .set_emissions(t22_bank.mint.key, 0, I80F48::ZERO, 0)
        .await;

    let depositor = test_f.create_marginfi_account().await;
    let depositor_t22 = t22_bank.mint.create_token_account_and_mint_to(100).await;
    depositor
        .try_bank_deposit(depositor_t22.key, t22_bank, 50.0, None)
        .await?;

    let funding = t22_bank.mint.create_token_account_and_mint_to(50).await;
    let res = t22_bank
        .try_emissions_deposit(native!(50, t22_bank.mint.mint.decimals), funding.key)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidTransfer);

    Ok(())
}

#[cfg(feature = "transfer-hook")]
#[tokio::test]
async fn emissions_deposit_fails_with_transfer_hook() -> anyhow::Result<()> {
    use fixtures::spl::SupportedExtension;

    let test_f = TestFixture::new_with_t22_extension(
        Some(TestSettings {
            banks: vec![TestBankSetting {
                mint: BankMint::UsdcT22,
                config: None,
            }],
            protocol_fees: false,
        }),
        &[SupportedExtension::TransferHook],
    )
    .await;

    let t22_bank = test_f.get_bank(&BankMint::UsdcT22);

    t22_bank
        .set_emissions(t22_bank.mint.key, 0, I80F48::ZERO, 0)
        .await;

    let depositor = test_f.create_marginfi_account().await;
    let depositor_t22 = t22_bank.mint.create_token_account_and_mint_to(100).await;
    depositor
        .try_bank_deposit(depositor_t22.key, t22_bank, 50.0, None)
        .await?;

    let funding = t22_bank.mint.create_token_account_and_mint_to(50).await;
    let res = t22_bank
        .try_emissions_deposit(native!(50, "USDC"), funding.key)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidTransfer);

    Ok(())
}

#[tokio::test]
async fn emissions_deposit_succeeds_with_inactive_t22_extensions() -> anyhow::Result<()> {
    use fixtures::spl::SupportedExtension;

    let test_f = TestFixture::new_with_t22_extension(
        Some(TestSettings {
            banks: vec![TestBankSetting {
                mint: BankMint::UsdcT22,
                config: None,
            }],
            protocol_fees: false,
        }),
        &[
            SupportedExtension::TransferFeeInactive,
            SupportedExtension::TransferHook,
        ],
    )
    .await;

    let t22_bank = test_f.get_bank(&BankMint::UsdcT22);

    t22_bank
        .set_emissions(t22_bank.mint.key, 0, I80F48::ZERO, 0)
        .await;

    let depositor = test_f.create_marginfi_account().await;
    let depositor_t22 = t22_bank.mint.create_token_account_and_mint_to(100).await;
    depositor
        .try_bank_deposit(depositor_t22.key, t22_bank, 100.0, None)
        .await?;

    let bank_before = t22_bank.load().await;
    let shares_before = I80F48::from(bank_before.total_asset_shares);
    let share_value_before = I80F48::from(bank_before.asset_share_value);
    let liquidity_vault_before =
        TokenAccountFixture::fetch(test_f.context.clone(), bank_before.liquidity_vault)
            .await
            .balance()
            .await;

    let emissions_deposit = 50;
    let funding = t22_bank.mint.create_token_account_and_mint_to(50).await;
    t22_bank
        .try_emissions_deposit(native!(emissions_deposit, "USDC"), funding.key)
        .await?;

    let bank_after = t22_bank.load().await;
    let shares_after = I80F48::from(bank_after.total_asset_shares);
    let share_value_after = I80F48::from(bank_after.asset_share_value);
    let liquidity_vault_after =
        TokenAccountFixture::fetch(test_f.context.clone(), bank_after.liquidity_vault)
            .await
            .balance()
            .await;

    let deposit_amount = 100;
    let asset_shares_value_multiplier = 1.0 + emissions_deposit as f64 / deposit_amount as f64;

    assert_eq!(shares_after, shares_before);

    // Should be equal, zero liabilities are present
    assert_eq!(
        share_value_before
            .checked_mul(I80F48::from_num(asset_shares_value_multiplier))
            .unwrap(),
        share_value_after
    );
    assert_eq!(
        liquidity_vault_after - liquidity_vault_before,
        native!(emissions_deposit, "USDC")
    );
    assert_eq!(I80F48::from(bank_after.emissions_remaining), I80F48::ZERO);

    Ok(())
}

#[tokio::test]
async fn emissions_same_bank_deposit_updates_asset_share_value() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    usdc_bank
        .set_emissions(usdc_bank.mint.key, 0, I80F48::ZERO, 0)
        .await;

    let emissions_funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;

    let depositor_a = test_f.create_marginfi_account().await;
    let depositor_b = test_f.create_marginfi_account().await;

    let depositor_a_usdc = test_f.usdc_mint.create_token_account_and_mint_to(40).await;
    let depositor_b_usdc = test_f.usdc_mint.create_token_account_and_mint_to(60).await;

    let depositor_a_amount = 40;
    depositor_a
        .try_bank_deposit(
            depositor_a_usdc.key,
            usdc_bank,
            depositor_a_amount as f64,
            None,
        )
        .await?;

    let depositor_b_amount = 60;
    depositor_b
        .try_bank_deposit(
            depositor_b_usdc.key,
            usdc_bank,
            depositor_b_amount as f64,
            None,
        )
        .await?;

    let bank_before = usdc_bank.load().await;
    let shares_before = I80F48::from(bank_before.total_asset_shares);
    let share_value_before = I80F48::from(bank_before.asset_share_value);

    let liquidity_vault_before =
        TokenAccountFixture::fetch(test_f.context.clone(), bank_before.liquidity_vault)
            .await
            .balance()
            .await;

    let emissions_deposit = 50;
    usdc_bank
        .try_emissions_deposit(native!(emissions_deposit, "USDC"), emissions_funding.key)
        .await?;

    let bank_after = usdc_bank.load().await;
    let shares_after = I80F48::from(bank_after.total_asset_shares);
    let share_value_after = I80F48::from(bank_after.asset_share_value);

    let liquidity_vault_after =
        TokenAccountFixture::fetch(test_f.context.clone(), bank_after.liquidity_vault)
            .await
            .balance()
            .await;

    let asset_shares_value_multiplier =
        1.0 + emissions_deposit as f64 / (depositor_a_amount + depositor_b_amount) as f64;

    assert_eq!(shares_after, shares_before);

    // Should be equal, zero liabilities are present
    assert_eq!(
        share_value_before
            .checked_mul(I80F48::from_num(asset_shares_value_multiplier))
            .unwrap(),
        share_value_after
    );
    assert_eq!(
        liquidity_vault_after - liquidity_vault_before,
        native!(emissions_deposit, "USDC")
    );
    assert_eq!(I80F48::from(bank_after.emissions_remaining), I80F48::ZERO);

    let depositor_a_state = depositor_a.load().await;
    let depositor_b_state = depositor_b.load().await;

    let depositor_a_shares = depositor_a_state
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == usdc_bank.key)
        .unwrap()
        .asset_shares;
    let depositor_b_shares = depositor_b_state
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == usdc_bank.key)
        .unwrap()
        .asset_shares;

    let depositor_a_assets = bank_after
        .get_asset_amount(I80F48::from(depositor_a_shares))?
        .to_num::<u64>();
    let depositor_b_assets = bank_after
        .get_asset_amount(I80F48::from(depositor_b_shares))?
        .to_num::<u64>();

    assert_eq_with_tolerance!(
        depositor_a_assets as i64,
        native!(60, "USDC") as i64,
        native!(1, "USDC") as i64
    );
    assert_eq_with_tolerance!(
        depositor_b_assets as i64,
        native!(90, "USDC") as i64,
        native!(1, "USDC") as i64
    );

    Ok(())
}

#[tokio::test]
async fn emissions_not_same_bank_deposit_updates_asset_share_value() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let emissions_mint_fixture = MintFixture::new(test_f.context.clone(), None, None).await;

    usdc_bank
        .set_emissions(emissions_mint_fixture.key, 0, I80F48::ZERO, 0)
        .await;

    let emissions_funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;

    let depositor_a = test_f.create_marginfi_account().await;
    let depositor_b = test_f.create_marginfi_account().await;

    let depositor_a_usdc = test_f.usdc_mint.create_token_account_and_mint_to(40).await;
    let depositor_b_usdc = test_f.usdc_mint.create_token_account_and_mint_to(60).await;

    let depositor_a_amount = 40;
    depositor_a
        .try_bank_deposit(
            depositor_a_usdc.key,
            usdc_bank,
            depositor_a_amount as f64,
            None,
        )
        .await?;

    let depositor_b_amount = 60;
    depositor_b
        .try_bank_deposit(
            depositor_b_usdc.key,
            usdc_bank,
            depositor_b_amount as f64,
            None,
        )
        .await?;

    let emissions_deposit = 50;
    let res = usdc_bank
        .try_emissions_deposit(native!(emissions_deposit, "USDC"), emissions_funding.key)
        .await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidEmissionsMint);

    Ok(())
}
