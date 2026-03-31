use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{
    assert_eq_noise, native,
    spl::SupportedExtension,
    test::{BankMint, TestBankSetting, TestFixture, TestSettings, DEFAULT_SOL_TEST_BANK_CONFIG},
};
use marginfi::state::bank::{BankImpl, BankVaultType};
use marginfi_type_crate::types::{Bank, BankConfig, BankConfigOpt};
use solana_program_test::tokio;
use test_case::test_case;

#[test_case(vec![])]
#[test_case(vec![SupportedExtension::TransferHook])]
#[test_case(vec![SupportedExtension::PermanentDelegate])]
#[test_case(vec![SupportedExtension::InterestBearing])]
#[test_case(vec![SupportedExtension::MintCloseAuthority])]
#[test_case(vec![SupportedExtension::PermanentDelegate, SupportedExtension::InterestBearing])]
#[test_case(vec![SupportedExtension::MintCloseAuthority, SupportedExtension::InterestBearing])]
#[test_case(vec![SupportedExtension::PermanentDelegate,SupportedExtension::MintCloseAuthority])]
#[test_case(vec![SupportedExtension::InterestBearing, SupportedExtension::MintCloseAuthority])]
#[test_case(vec![SupportedExtension::PermanentDelegate, SupportedExtension::InterestBearing, SupportedExtension::MintCloseAuthority])]
#[tokio::test]
async fn marginfi_account_liquidation_success_with_extension(
    extensions: Vec<SupportedExtension>,
) -> anyhow::Result<()> {
    let test_f = TestFixture::new_with_t22_extension(
        Some(TestSettings {
            banks: vec![
                TestBankSetting {
                    mint: BankMint::Usdc,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::UsdcT22,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::PyUSD,
                    ..TestBankSetting::default()
                },
                TestBankSetting {
                    mint: BankMint::Sol,
                    config: Some(BankConfig {
                        asset_weight_init: I80F48!(1).into(),
                        asset_weight_maint: I80F48!(1).into(),
                        ..*DEFAULT_SOL_TEST_BANK_CONFIG
                    }),
                },
            ],
            protocol_fees: false,
        }),
        &extensions,
    )
    .await;

    let usdc_t22_bank_f = test_f.get_bank(&BankMint::UsdcT22);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc_t22 = test_f
        .usdc_t22_mint
        .create_token_account_and_mint_to(2_500)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(
            lender_token_account_usdc_t22.key,
            usdc_t22_bank_f,
            2_000,
            None,
        )
        .await
        .unwrap();

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    let borrower_token_account_usdc_t22 = test_f.usdc_t22_mint.create_empty_token_account().await;

    // Borrower deposits 100 SOL worth of $1000
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc_t22.key, usdc_t22_bank_f, 900)
        .await
        .unwrap();
    assert_eq!(
        borrower_token_account_usdc_t22.balance().await,
        native!(900, "USDC")
    );

    // Synthetically bring down the borrower account health by reducing the asset weights of the SOL bank
    sol_bank_f
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.5).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    lender_mfi_account_f
        .try_liquidate(&borrower_mfi_account_f, sol_bank_f, 1, usdc_t22_bank_f)
        .await
        .unwrap();

    // Checks
    let sol_bank: Bank = sol_bank_f.load().await;
    let usdc_t22_bank: Bank = usdc_t22_bank_f.load().await;

    let depositor_ma = lender_mfi_account_f.load().await;
    let borrower_ma = borrower_mfi_account_f.load().await;

    // Due to balances sorting, SOL and USDC may be not at indices 0 and 1, respectively -> determine them first
    let sol_index = depositor_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == sol_bank_f.key)
        .unwrap();
    let usdc_t22_index = depositor_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == usdc_t22_bank_f.key)
        .unwrap();

    // Depositors should have 1 SOL
    assert_eq!(
        sol_bank
            .get_asset_amount(
                depositor_ma.lending_account.balances[sol_index]
                    .asset_shares
                    .into()
            )
            .unwrap(),
        I80F48::from(native!(1, "SOL"))
    );

    // Depositors should have 1990.25 USDC
    assert_eq_noise!(
        usdc_t22_bank
            .get_asset_amount(
                depositor_ma.lending_account.balances[usdc_t22_index]
                    .asset_shares
                    .into()
            )
            .unwrap(),
        I80F48::from(native!(1990.25, "USDC", f64)),
        native!(0.00001, "USDC", f64)
    );

    // Borrower should have 99 SOL
    assert_eq!(
        sol_bank
            .get_asset_amount(
                borrower_ma.lending_account.balances[sol_index]
                    .asset_shares
                    .into()
            )
            .unwrap(),
        I80F48::from(native!(99, "SOL"))
    );

    // Borrower should have 890.50 USDC
    assert_eq_noise!(
        usdc_t22_bank
            .get_liability_amount(
                borrower_ma.lending_account.balances[usdc_t22_index]
                    .liability_shares
                    .into()
            )
            .unwrap(),
        I80F48::from(native!(890.50, "USDC", f64)),
        native!(0.00001, "USDC", f64)
    );

    // Check insurance fund fee
    let insurance_fund_usdc = usdc_t22_bank_f
        .get_vault_token_account(BankVaultType::Insurance)
        .await;

    assert_eq_noise!(
        insurance_fund_usdc.balance().await as i64,
        native!(0.25, "USDC", f64) as i64,
        1
    );

    Ok(())
}
