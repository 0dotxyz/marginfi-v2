use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::{
    constants::EXP_10_I80F48,
    types::{BankConfig, BankConfigOpt, BankOperationalState},
};
use pretty_assertions::assert_eq;
use solana_program_test::*;

// Helper function to convert shares to UI amounts
fn shares_to_ui(shares: I80F48, decimals: u8) -> I80F48 {
    let scale = *EXP_10_I80F48
        .get(decimals as usize)
        .expect("mint decimals exceed lookup table bounds");
    shares
        .checked_div(scale)
        .expect("scale must be non-zero for valid mints")
}

#[tokio::test]
async fn marginfi_group_bank_paused_should_error() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    test_f
        .marginfi_group
        .try_lending_pool_configure_bank(
            usdc_bank_f,
            BankConfigOpt {
                operational_state: Some(BankOperationalState::Paused),
                ..BankConfigOpt::default()
            },
        )
        .await?;

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    let res = lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankPaused);

    Ok(())
}

#[tokio::test]
async fn marginfi_group_bank_reduce_only_withdraw_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let res = lender_mfi_account_f
        .try_bank_withdraw(lender_token_account_usdc.key, usdc_bank_f, 0, Some(true))
        .await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn marginfi_group_bank_reduce_only_deposit_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_1_mfi_account = test_f.create_marginfi_account().await;
    let lender_1_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lender_1_mfi_account
        .try_bank_deposit(lender_1_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    let lender_2_mfi_account = test_f.create_marginfi_account().await;
    let lender_2_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_2_mfi_account
        .try_bank_deposit(lender_2_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    let lender_2_token_account_sol = test_f.sol_mint.create_empty_token_account().await;
    lender_2_mfi_account
        .try_bank_borrow(lender_2_token_account_sol.key, sol_bank_f, 1)
        .await?;

    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let res = lender_2_mfi_account
        .try_bank_repay(lender_2_token_account_sol.key, sol_bank_f, 1, None)
        .await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn marginfi_group_bank_reduce_only_borrow_failure() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_mfi_account = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lender_mfi_account
        .try_bank_deposit(lender_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    let borrower_mfi_account = test_f.create_marginfi_account().await;
    let borrower_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    borrower_mfi_account
        .try_bank_deposit(borrower_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    sol_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let borrower_token_account_sol = test_f.sol_mint.create_empty_token_account().await;
    let res = borrower_mfi_account
        .try_bank_borrow(borrower_token_account_sol.key, sol_bank_f, 1)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankReduceOnly);

    Ok(())
}

#[tokio::test]
async fn marginfi_group_bank_reduce_only_deposit_failure() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;

    let res = lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankReduceOnly);

    Ok(())
}

#[tokio::test]
async fn marginfi_group_bank_reduce_only_worthless_for_new_loans() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(0.8).into(),
                    asset_weight_maint: I80F48!(0.9).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_mfi_account = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lender_mfi_account
        .try_bank_deposit(lender_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    let borrower_mfi_account = test_f.create_marginfi_account().await;
    let borrower_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    borrower_mfi_account
        .try_bank_deposit(borrower_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let borrower_token_account_sol = test_f.sol_mint.create_empty_token_account().await;
    let res = borrower_mfi_account
        .try_bank_borrow(borrower_token_account_sol.key, sol_bank_f, 1)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    borrower_mfi_account
        .try_lending_account_pulse_health()
        .await?;

    let borrower_account_data = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiAccount>(
            &borrower_mfi_account.key,
        )
        .await;

    // Check that health_cache shows the account is healthy
    assert!(
        borrower_account_data.health_cache.is_healthy(),
        "Account should be healthy (HEALTHY flag set) even though new borrows are rejected"
    );

    // Verify that assetValueMaint > liabilityValueMaint (no liabilities yet)
    let asset_value_maint = I80F48::from(borrower_account_data.health_cache.asset_value_maint);
    let liability_value_maint =
        I80F48::from(borrower_account_data.health_cache.liability_value_maint);

    assert!(
        asset_value_maint > liability_value_maint,
        "Asset value (maint) should be greater than liability value (maint). Assets: {}, Liabilities: {}",
        asset_value_maint,
        liability_value_maint
    );

    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::Operational),
                ..Default::default()
            },
            None,
        )
        .await?;

    let borrower_token_account_sol_2 = test_f.sol_mint.create_empty_token_account().await;
    let res = borrower_mfi_account
        .try_bank_borrow(borrower_token_account_sol_2.key, sol_bank_f, 1)
        .await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn marginfi_group_bank_reduce_only_maintains_worth_for_existing_loans() -> anyhow::Result<()>
{
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(0.9).into(),
                    asset_weight_maint: I80F48!(0.95).into(),
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(0.8).into(),
                    asset_weight_maint: I80F48!(0.9).into(),
                    liability_weight_init: I80F48!(1.1).into(),
                    liability_weight_maint: I80F48!(1.05).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_mfi_account = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lender_mfi_account
        .try_bank_deposit(lender_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    let borrower_mfi_account = test_f.create_marginfi_account().await;
    let borrower_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    borrower_mfi_account
        .try_bank_deposit(borrower_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    let borrower_token_account_sol = test_f.sol_mint.create_empty_token_account().await;
    borrower_mfi_account
        .try_bank_borrow(borrower_token_account_sol.key, sol_bank_f, 1)
        .await?;

    // Now set USDC bank to ReduceOnly
    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let borrower_token_account_sol_2 = test_f.sol_mint.create_empty_token_account().await;
    let res = borrower_mfi_account
        .try_bank_borrow(borrower_token_account_sol_2.key, sol_bank_f, 1)
        .await;

    // Should fail because USDC collateral is worth 0 for new borrows (Initial check)
    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    // The position is still healthy though - verify by checking the account still exists and has the loan
    let borrower_account_data = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiAccount>(
            &borrower_mfi_account.key,
        )
        .await;

    // Verify the USDC deposit and SOL borrow are still there
    let usdc_balance = borrower_account_data
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == usdc_bank_f.key)
        .unwrap();

    let sol_balance = borrower_account_data
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == sol_bank_f.key)
        .unwrap();

    // Verify the exact amounts remain unchanged (no interest accrual yet)
    let usdc_assets_ui = shares_to_ui(
        I80F48::from(usdc_balance.asset_shares),
        usdc_bank_f.mint.mint.decimals,
    );
    let sol_liabilities_ui = shares_to_ui(
        I80F48::from(sol_balance.liability_shares),
        sol_bank_f.mint.mint.decimals,
    );
    assert_eq!(
        usdc_assets_ui,
        I80F48::from_num(100_000),
        "USDC asset shares should equal the initial 100k deposit"
    );
    assert_eq!(
        sol_liabilities_ui,
        I80F48::from_num(1),
        "SOL liability shares should equal the 1 SOL borrow"
    );

    Ok(())
}

#[tokio::test]
async fn marginfi_group_bank_reduce_only_can_borrow_with_other_collateral() -> anyhow::Result<()> {
    // Test that a user can borrow using non-ReduceOnly collateral even when holding ReduceOnly assets
    // This ensures ReduceOnly assets don't interfere with other valid collateral in the same account
    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(0.9).into(),
                    asset_weight_maint: I80F48!(0.95).into(),
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(0.8).into(),
                    asset_weight_maint: I80F48!(0.9).into(),
                    liability_weight_init: I80F48!(1.1).into(),
                    liability_weight_maint: I80F48!(1.05).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::PyUSD,
                config: None,
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc).clone();
    let sol_bank_f = test_f.get_bank(&BankMint::Sol).clone();
    let pyusd_bank_f = test_f.get_bank(&BankMint::PyUSD).clone();

    // Setup: lender deposits USDC, SOL, and PyUSD to make them available for borrowing
    let lender_mfi_account = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_mfi_account
        .try_bank_deposit(lender_token_account_usdc.key, &usdc_bank_f, 100_000, None)
        .await?;

    let lender_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lender_mfi_account
        .try_bank_deposit(lender_token_account_sol.key, &sol_bank_f, 100, None)
        .await?;

    let lender_token_account_pyusd = test_f
        .get_bank_mut(&BankMint::PyUSD)
        .mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_mfi_account
        .try_bank_deposit(lender_token_account_pyusd.key, &pyusd_bank_f, 100_000, None)
        .await?;

    // Borrower deposits both USDC and SOL as collateral (before any is set to ReduceOnly)
    let borrower_mfi_account = test_f.create_marginfi_account().await;

    let borrower_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(50_000)
        .await;
    borrower_mfi_account
        .try_bank_deposit(borrower_token_account_usdc.key, &usdc_bank_f, 50_000, None)
        .await?;

    let borrower_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    borrower_mfi_account
        .try_bank_deposit(borrower_token_account_sol.key, &sol_bank_f, 5, None)
        .await?;

    // Now set USDC bank to ReduceOnly
    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    // User now has both USDC (ReduceOnly) and SOL (Operational) in their account
    // They should be able to borrow PyUSD using their SOL deposit as collateral
    // The ReduceOnly USDC should not interfere with this borrow
    let borrower_token_account_pyusd = test_f
        .get_bank_mut(&BankMint::PyUSD)
        .mint
        .create_empty_token_account()
        .await;
    let res = borrower_mfi_account
        .try_bank_borrow(borrower_token_account_pyusd.key, &pyusd_bank_f, 1)
        .await;

    // Should succeed because SOL (operational) is valid collateral for borrowing PyUSD
    // Even though the account also holds USDC (ReduceOnly), it doesn't interfere
    assert!(
        res.is_ok(),
        "Borrowing PyUSD with SOL collateral should succeed even when holding ReduceOnly USDC"
    );

    // Verify the account state
    let borrower_account_data = test_f
        .load_and_deserialize::<marginfi_type_crate::types::MarginfiAccount>(
            &borrower_mfi_account.key,
        )
        .await;

    let usdc_balance = borrower_account_data
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == usdc_bank_f.key)
        .unwrap();

    let sol_balance = borrower_account_data
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == sol_bank_f.key)
        .unwrap();

    let pyusd_balance = borrower_account_data
        .lending_account
        .balances
        .iter()
        .find(|b| b.bank_pk == pyusd_bank_f.key)
        .unwrap();

    let usdc_assets_ui = shares_to_ui(
        I80F48::from(usdc_balance.asset_shares),
        usdc_bank_f.mint.mint.decimals,
    );
    let sol_assets_ui = shares_to_ui(
        I80F48::from(sol_balance.asset_shares),
        sol_bank_f.mint.mint.decimals,
    );
    let pyusd_liabilities_ui = shares_to_ui(
        I80F48::from(pyusd_balance.liability_shares),
        pyusd_bank_f.mint.mint.decimals,
    );

    // USDC (ReduceOnly) should have assets but no liabilities
    assert_eq!(
        usdc_assets_ui,
        I80F48::from_num(50_000),
        "USDC asset shares should equal the original 50k deposit"
    );
    assert_eq!(I80F48::from(usdc_balance.liability_shares), I80F48::ZERO);

    // SOL should have only assets (used as collateral, not borrowed)
    assert_eq!(
        sol_assets_ui,
        I80F48::from_num(5),
        "SOL asset shares should equal the original 5 SOL deposit"
    );
    assert_eq!(I80F48::from(sol_balance.liability_shares), I80F48::ZERO);

    // PyUSD should have only liabilities (borrowed)
    assert_eq!(I80F48::from(pyusd_balance.asset_shares), I80F48::ZERO);
    assert_eq!(
        pyusd_liabilities_ui,
        I80F48::from_num(1),
        "PyUSD liability shares should equal the 1 PyUSD borrow"
    );

    Ok(())
}
