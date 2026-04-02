use fixed::types::I80F48;
use fixtures::{assert_eq_noise, native, prelude::*};
use marginfi::state::bank::{BankImpl, BankVaultType};
use marginfi_type_crate::types::BankOperationalState;
use pretty_assertions::assert_eq;
use solana_program_test::*;

#[tokio::test]
async fn test_cross_bank_socialize_no_insurance() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let deposit_amount = 1_000.;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);

    let lp_mfi_account = test_f.create_marginfi_account().await;
    let lp_token_account = test_f
        .get_bank(&BankMint::Usdc)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account
        .try_bank_deposit(
            lp_token_account.key,
            test_f.get_bank(&BankMint::Usdc),
            deposit_amount,
            None,
        )
        .await?;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let destination = usdc_bank.mint.create_empty_token_account().await;
    let socialize_amount = native!(100., usdc_bank.mint.mint.decimals, f64);

    let pre_vault = usdc_bank
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let pre_bank = usdc_bank.load().await;
    let pre_lp_value = pre_bank.get_asset_amount(
        lp_mfi_account.load().await.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    test_f
        .marginfi_group
        .try_admin_cross_bank_socialize(usdc_bank, destination.key, socialize_amount, false)
        .await?;

    let post_vault = usdc_bank
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let post_bank = usdc_bank.load().await;
    let post_lp_value = post_bank.get_asset_amount(
        lp_mfi_account.load().await.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    // Vault decreased by exactly the socialized amount
    assert_eq!(pre_vault - post_vault, socialize_amount);
    // Destination received the tokens
    assert_eq!(destination.balance().await, socialize_amount);
    // LP deposit value decreased by the socialized amount (within rounding tolerance)
    assert_eq_noise!(
        pre_lp_value - post_lp_value,
        I80F48::from_num(socialize_amount),
        I80F48::ONE
    );
    // Bank is still operational
    assert_eq!(
        post_bank.config.operational_state,
        BankOperationalState::Operational
    );

    Ok(())
}

#[tokio::test]
async fn test_cross_bank_socialize_with_insurance() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let deposit_amount = 1_000.;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);

    let lp_mfi_account = test_f.create_marginfi_account().await;
    let lp_token_account = test_f
        .get_bank(&BankMint::Usdc)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account
        .try_bank_deposit(
            lp_token_account.key,
            test_f.get_bank(&BankMint::Usdc),
            deposit_amount,
            None,
        )
        .await?;

    // Fund insurance vault with 50 USDC
    let insurance_amount = 50.;
    {
        let bank_mut = test_f.get_bank_mut(&BankMint::Usdc);
        let insurance_vault = bank_mut.get_vault(BankVaultType::Insurance).0;
        bank_mut
            .mint
            .mint_to(&insurance_vault, insurance_amount)
            .await;
    }

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let destination = usdc_bank.mint.create_empty_token_account().await;
    let socialize_amount = native!(100., usdc_bank.mint.mint.decimals, f64);
    let insurance_native = native!(insurance_amount, usdc_bank.mint.mint.decimals, f64);

    let pre_vault = usdc_bank
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let pre_bank = usdc_bank.load().await;
    let pre_lp_value = pre_bank.get_asset_amount(
        lp_mfi_account.load().await.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    test_f
        .marginfi_group
        .try_admin_cross_bank_socialize(usdc_bank, destination.key, socialize_amount, true)
        .await?;

    let post_insurance = usdc_bank
        .get_vault_token_account(BankVaultType::Insurance)
        .await
        .balance()
        .await;
    let post_vault = usdc_bank
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let post_bank = usdc_bank.load().await;
    let post_lp_value = post_bank.get_asset_amount(
        lp_mfi_account.load().await.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    // Insurance fully drained
    assert_eq!(post_insurance, 0);
    // Liquidity vault only decreased by the non-insured portion
    let expected_socialized = socialize_amount - insurance_native;
    assert_eq!(pre_vault - post_vault, expected_socialized);
    // Destination received the full amount (insurance + socialized)
    assert_eq!(destination.balance().await, socialize_amount);
    // LP deposit value only decreased by the socialized portion (insurance absorbed the rest)
    assert_eq_noise!(
        pre_lp_value - post_lp_value,
        I80F48::from_num(expected_socialized),
        I80F48::ONE
    );

    Ok(())
}

#[tokio::test]
async fn test_cross_bank_socialize_capped_at_deposits() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let deposit_amount = 100.;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);

    let lp_mfi_account = test_f.create_marginfi_account().await;
    let lp_token_account = test_f
        .get_bank(&BankMint::Usdc)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account
        .try_bank_deposit(
            lp_token_account.key,
            test_f.get_bank(&BankMint::Usdc),
            deposit_amount,
            None,
        )
        .await?;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let destination = usdc_bank.mint.create_empty_token_account().await;
    let deposit_native = native!(deposit_amount, usdc_bank.mint.mint.decimals, f64);

    // Request 500 USDC but only 100 deposited — should be capped
    let socialize_amount = native!(500., usdc_bank.mint.mint.decimals, f64);

    test_f
        .marginfi_group
        .try_admin_cross_bank_socialize(usdc_bank, destination.key, socialize_amount, false)
        .await?;

    let post_vault = usdc_bank
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let post_bank = usdc_bank.load().await;
    let post_lp_value = post_bank.get_asset_amount(
        lp_mfi_account.load().await.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    // Vault fully drained (capped at deposit amount)
    assert_eq!(post_vault, 0);
    // Destination received the deposit amount, not the requested 500
    assert_eq!(destination.balance().await, deposit_native);
    // LP shares worth zero — bank killed
    assert_eq!(post_lp_value, I80F48::ZERO);
    assert_eq!(
        post_bank.config.operational_state,
        BankOperationalState::KilledByBankruptcy
    );

    Ok(())
}

#[tokio::test]
async fn test_cross_bank_socialize_capped_at_available_liquidity() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let deposit_amount = 1_000.;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);

    let lp_mfi_account = test_f.create_marginfi_account().await;
    let lp_token_account = test_f
        .get_bank(&BankMint::Usdc)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account
        .try_bank_deposit(
            lp_token_account.key,
            test_f.get_bank(&BankMint::Usdc),
            deposit_amount,
            None,
        )
        .await?;

    let borrower_mfi_account = test_f.create_marginfi_account().await;
    let borrower_sol_account = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let borrower_usdc_account = test_f.usdc_mint.create_empty_token_account().await;
    borrower_mfi_account
        .try_bank_deposit(
            borrower_sol_account.key,
            test_f.get_bank(&BankMint::Sol),
            1_000.,
            None,
        )
        .await?;
    borrower_mfi_account
        .try_bank_borrow(
            borrower_usdc_account.key,
            test_f.get_bank(&BankMint::Usdc),
            950.,
        )
        .await?;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let destination = usdc_bank.mint.create_empty_token_account().await;
    let socialize_amount = native!(100., usdc_bank.mint.mint.decimals, f64);

    let pre_vault = usdc_bank
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let pre_bank = usdc_bank.load().await;
    let pre_lp_value = pre_bank.get_asset_amount(
        lp_mfi_account.load().await.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    test_f
        .marginfi_group
        .try_admin_cross_bank_socialize(usdc_bank, destination.key, socialize_amount, false)
        .await?;

    let post_vault = usdc_bank
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let post_bank = usdc_bank.load().await;
    let post_lp_value = post_bank.get_asset_amount(
        lp_mfi_account.load().await.lending_account.balances[0]
            .asset_shares
            .into(),
    )?;

    assert_eq!(post_vault, 0);
    assert_eq!(destination.balance().await, pre_vault);
    assert_eq_noise!(
        pre_lp_value - post_lp_value,
        I80F48::from_num(pre_vault),
        I80F48::ONE
    );
    assert_eq!(
        post_bank.config.operational_state,
        BankOperationalState::Operational
    );

    Ok(())
}

#[tokio::test]
async fn test_cross_bank_socialize_zero_amount_fails() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let destination = usdc_bank.mint.create_empty_token_account().await;

    let res = test_f
        .marginfi_group
        .try_admin_cross_bank_socialize(usdc_bank, destination.key, 0, false)
        .await;

    assert!(res.is_err());

    Ok(())
}
