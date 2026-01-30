use anchor_spl::token_2022::spl_token_2022::extension::{
    transfer_fee::TransferFeeConfig, BaseStateWithExtensions,
};
use fixtures::{
    assert_custom_error,
    spl::TokenAccountFixture,
    test::{BankMint, TestFixture, TestSettings},
};
use marginfi::errors::MarginfiError;
use solana_program_test::tokio;
use solana_sdk::pubkey::Pubkey;
use test_case::test_case;

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::Sol)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::T22WithFee)]
#[tokio::test]
async fn marginfi_group_withdraw_fees_and_insurance_fund_as_admin_success(
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let bank_f = test_f.banks.get_mut(&bank_mint).unwrap();

    let insurance_vault_balance = 1_000;
    let fee_vault_balance = 750;

    // Mint `insurance_vault_balance` USDC to the insurance vault
    let bank = bank_f.load().await;
    bank_f
        .mint
        .mint_to(&bank.insurance_vault, insurance_vault_balance as f64)
        .await;

    // Create a receiving account and try to withdraw `insurance_vault_balance` USDC from the insurance vault
    let receiving_account = bank_f.mint.create_empty_token_account().await;
    bank_f
        .try_withdraw_insurance(&receiving_account, insurance_vault_balance)
        .await?;

    let transfer_fee = bank_f
        .mint
        .load_state()
        .await
        .get_extension::<TransferFeeConfig>()
        .map(|tf| {
            tf.calculate_epoch_fee(0, insurance_vault_balance)
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let expected_received_balance = insurance_vault_balance - transfer_fee;
    assert_eq!(receiving_account.balance().await, expected_received_balance); // Verifies that the receiving account balance is 1000 USDC

    // Mint `fee_vault_balance` USDC to the fee vault
    bank_f
        .mint
        .mint_to(&bank.fee_vault, fee_vault_balance as f64)
        .await;

    // Create a receiving account and try to withdraw `fee_vault_balance` USDC from the fee vault
    let receiving_account = bank_f.mint.create_empty_token_account().await;
    bank_f
        .try_withdraw_fees(&receiving_account, fee_vault_balance)
        .await?;

    let transfer_fee = bank_f
        .mint
        .load_state()
        .await
        .get_extension::<TransferFeeConfig>()
        .map(|tf| tf.calculate_epoch_fee(0, fee_vault_balance).unwrap_or(0))
        .unwrap_or(0);

    let expected_received_balance = fee_vault_balance - transfer_fee;
    assert_eq!(receiving_account.balance().await, expected_received_balance); // Verifies that the receiving account balance is 750 USDC

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::Sol)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::T22WithFee)]
#[tokio::test]
async fn marginfi_group_withdraw_fees_and_insurance_fund_as_non_admin_failure(
    bank_mint: BankMint,
) -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let bank_f = test_f.banks.get_mut(&bank_mint).unwrap();
    let bank = bank_f.load().await;

    let insurance_vault_balance = 1_000;
    let fee_vault_balance = 750;

    // Update the admin of the marginfi group
    test_f
        .marginfi_group
        .try_update(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        )
        .await?;

    // Mint `insurance_vault_balance` USDC to the insurance vault
    bank_f
        .mint
        .mint_to(&bank.insurance_vault, insurance_vault_balance as f64)
        .await;

    // Create a receiving account and try to withdraw `insurance_vault_balance` USDC from the insurance vault
    let receiving_account = bank_f.mint.create_empty_token_account().await;
    let res = bank_f
        .try_withdraw_insurance(&receiving_account, insurance_vault_balance)
        .await;

    // Unable to withdraw 1000 USDC from the insurance vault, because the signer is not the admin
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    // Mint `fee_vault_balance` USDC to the fee vault
    bank_f
        .mint
        .mint_to(&bank.fee_vault, fee_vault_balance as f64)
        .await;

    // Create a receiving account and try to withdraw `fee_vault_balance` USDC from the fee vault
    let receiving_account = bank_f.mint.create_empty_token_account().await;
    let res = bank_f
        .try_withdraw_fees(&receiving_account, fee_vault_balance)
        .await;

    // Unable to withdraw `fee_vault_balance` USDC from the fee vault, because the signer is not the admin
    assert_custom_error!(res.unwrap_err(), MarginfiError::Unauthorized);

    Ok(())
}

#[test_case(BankMint::Usdc)]
#[test_case(BankMint::Sol)]
#[test_case(BankMint::PyUSD)]
#[test_case(BankMint::T22WithFee)]
#[tokio::test]
async fn marginfi_group_withdraw_fees_permissonless(bank_mint: BankMint) -> anyhow::Result<()> {
    // Setup test executor with non-admin payer
    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let bank_f = test_f.banks.get_mut(&bank_mint).unwrap();

    let fee_vault_balance = 750;

    // Mint `insurance_vault_balance` USDC to the insurance vault
    let bank = bank_f.load().await;

    // Mint `fee_vault_balance` USDC to the fee vault
    bank_f
        .mint
        .mint_to(&bank.fee_vault, fee_vault_balance as f64)
        .await;

    // Create a receiving account and try to withdraw `fee_vault_balance` USDC from the fee vault
    let receiving_account = bank_f.mint.create_empty_token_account().await;

    // Withdrawal fails because the destination account is not set for the bank
    let res = bank_f
        .try_withdraw_fees_permissionless(&receiving_account, fee_vault_balance)
        .await;
    assert!(res.is_err());
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::InvalidFeesDestinationAccount
    );

    // Now derive canonical ATA for the receiving account and set it as the fees destination for the bank
    let destination_ata = TokenAccountFixture::new_from_ata(
        test_f.context,
        &bank_f.mint.key,
        &receiving_account.key,
        &receiving_account.token_program,
    )
    .await;
    bank_f
        .try_set_fees_destination_account(&destination_ata)
        .await?;

    // Withdrawal still fails because the destination account provided is not matching the one we set for the bank
    let res = bank_f
        .try_withdraw_fees_permissionless(&receiving_account, fee_vault_balance)
        .await;
    assert!(res.is_err());
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::InvalidFeesDestinationAccount
    );

    // Use proper destination account -> should succeed
    bank_f
        .try_withdraw_fees_permissionless(&destination_ata, fee_vault_balance)
        .await?;

    let transfer_fee = bank_f
        .mint
        .load_state()
        .await
        .get_extension::<TransferFeeConfig>()
        .map(|tf| tf.calculate_epoch_fee(0, fee_vault_balance).unwrap_or(0))
        .unwrap_or(0);

    let expected_received_balance = fee_vault_balance - transfer_fee;
    assert_eq!(destination_ata.balance().await, expected_received_balance); // Verifies that the receiving account balance is 750 USDC

    Ok(())
}
