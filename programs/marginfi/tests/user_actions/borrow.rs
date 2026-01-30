use anchor_spl::token_2022::spl_token_2022::extension::{
    transfer_fee::TransferFeeConfig, BaseStateWithExtensions,
};
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, native, prelude::*, ui_to_native};
use marginfi::{
    assert_eq_with_tolerance,
    prelude::*,
    state::bank::{BankImpl, BankVaultType},
};
use marginfi_type_crate::types::{Bank, BankConfigOpt, EmodeEntry};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::clock::Clock;
use test_case::test_case;

#[test_case(100., 9., BankMint::Usdc, BankMint::Sol)]
#[test_case(123456.0, 12345.599999999, BankMint::Usdc, BankMint::Sol)]
#[test_case(1.0, 5.0, BankMint::Sol, BankMint::Usdc)]
#[test_case(240., 0.092, BankMint::PyUSD, BankMint::T22WithFee)]
#[test_case(36., 1.7, BankMint::T22WithFee, BankMint::Sol)]
#[test_case(200., 1.1, BankMint::Usdc, BankMint::SolSwbOrigFee)] // Sol @ ~ $153
#[test_case(150., 5., BankMint::Fixed, BankMint::Sol)]
#[test_case(300_000., 10., BankMint::FixedLow, BankMint::Fixed)]
#[tokio::test]
async fn marginfi_account_borrow_success(
    deposit_amount: f64,
    borrow_amount: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // LP

    let lp_deposit_amount = 2. * borrow_amount;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_collateral_token_account = test_f
        .get_bank(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_collateral_token_account.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await?;

    // User

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);
    let user_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let user_debt_token_account_f = test_f
        .get_bank(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    let collateral_bank = test_f.get_bank(&collateral_mint);
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            collateral_bank,
            deposit_amount,
            None,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let debt_bank_f = test_f.get_bank(&debt_mint);
    let bank_before = debt_bank_f.load().await;

    let pre_vault_balance = debt_bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let pre_user_debt_accounted = I80F48::ZERO;
    let pre_fee_group_fees: I80F48 = bank_before.collected_group_fees_outstanding.into();
    let pre_fee_program_fees: I80F48 = bank_before.collected_program_fees_outstanding.into();

    // This is just to test that the account's last_update field is properly updated upon modification
    let pre_last_update = user_mfi_account_f.load().await.last_update;
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 sec
        clock.unix_timestamp += 1;
        ctx.set_sysvar(&clock);
    }

    let res = user_mfi_account_f
        .try_bank_borrow(user_debt_token_account_f.key, debt_bank_f, borrow_amount)
        .await;
    assert!(res.is_ok());

    let post_vault_balance = debt_bank_f
        .get_vault_token_account(BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let marginfi_account = user_mfi_account_f.load().await;
    assert_eq!(marginfi_account.last_update, pre_last_update + 1);

    let balance = marginfi_account
        .lending_account
        .get_balance(&debt_bank_f.key)
        .unwrap();
    let post_user_debt_accounted = bank_before
        .get_asset_amount(balance.liability_shares.into())
        .unwrap();

    let borrow_amount_native = ui_to_native!(borrow_amount, debt_bank_f.mint.mint.decimals);
    let borrow_fee = debt_bank_f
        .mint
        .load_state()
        .await
        .get_extension::<TransferFeeConfig>()
        .map(|tf| {
            tf.calculate_inverse_epoch_fee(0, borrow_amount_native)
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let borrow_amount_pre_fee = borrow_amount_native + borrow_fee;
    let origination_fee_rate: I80F48 = bank_before
        .config
        .interest_rate_config
        .protocol_origination_fee
        .into();
    let program_fee_rate: I80F48 = test_f
        .marginfi_group
        .load()
        .await
        .fee_state_cache
        .program_fee_rate
        .into();
    let origination_fee: I80F48 = I80F48::from_num(borrow_amount_native)
        .checked_mul(origination_fee_rate)
        .unwrap();
    let program_origination_fee: I80F48 = origination_fee.checked_mul(program_fee_rate).unwrap();
    let group_origination_fee: I80F48 = origination_fee.saturating_sub(program_origination_fee);

    let active_balance_count = marginfi_account
        .lending_account
        .get_active_balances_iter()
        .count();
    assert_eq!(2, active_balance_count);

    let expected_liquidity_vault_delta = -(borrow_amount_pre_fee as i64);
    let actual_liquidity_vault_delta = post_vault_balance as i64 - pre_vault_balance as i64;
    let accounted_user_balance_delta = post_user_debt_accounted - pre_user_debt_accounted;

    // The liquidity vault paid out just the pre-origination fee amount (e.g. what the user borrowed
    // before accounting for the fee)
    assert_eq!(expected_liquidity_vault_delta, actual_liquidity_vault_delta);
    assert_eq_with_tolerance!(
        // Note: the user still gains debt which includes the origination fee
        I80F48::from(expected_liquidity_vault_delta) - origination_fee,
        -accounted_user_balance_delta,
        1
    );

    // The outstanding origination fee is recorded
    let bank_after = debt_bank_f.load().await;
    let post_fee_program_fees: I80F48 = bank_after.collected_program_fees_outstanding.into();
    assert_eq!(
        pre_fee_program_fees + program_origination_fee,
        post_fee_program_fees
    );

    let post_fee_group_fees: I80F48 = bank_after.collected_group_fees_outstanding.into();
    assert_eq!(
        pre_fee_group_fees + group_origination_fee,
        post_fee_group_fees
    );

    let health_cache = marginfi_account.health_cache;
    assert!(health_cache.is_engine_ok());
    assert!(health_cache.is_healthy());

    let asset_value: I80F48 = health_cache.asset_value.into();
    let asset_value: f64 = asset_value.to_num();
    let liab_value: I80F48 = health_cache.liability_value.into();
    let liab_value: f64 = liab_value.to_num();
    let collateral_price_roughly = get_mint_price(collateral_mint);
    let liablility_price_roughly: f64 = get_mint_price(debt_mint);
    // Apply a small discount to account for conf discounts, etc.
    let disc: f64 = 0.95;
    assert!(asset_value > deposit_amount * collateral_price_roughly * disc);
    assert!(liab_value > borrow_amount * liablility_price_roughly * disc);

    for (i, bal) in marginfi_account.lending_account.balances.iter().enumerate() {
        let shares: I80F48 = bal.asset_shares.into();
        if bal.is_active() {
            let price: f64 = f64::from_le_bytes(health_cache.prices[i]);
            if shares != I80F48::ZERO {
                assert!(price >= (collateral_price_roughly * disc));
            } else {
                assert!(price >= (liablility_price_roughly * disc));
            }
        }
    }

    Ok(())
}

#[test_case(100., 9., 10.000000001, BankMint::Usdc, BankMint::Sol)]
#[test_case(123_456., 12_345.6, 12_345.9, BankMint::Usdc, BankMint::Sol)]
#[test_case(1., 5., 11.98224, BankMint::Sol, BankMint::Usdc)]
#[test_case(240., 0.092, 500., BankMint::PyUSD, BankMint::T22WithFee)]
#[test_case(36., 1.7, 1.9, BankMint::T22WithFee, BankMint::Sol)]
#[test_case(1., 100., 155.9, BankMint::SolSwbPull, BankMint::Usdc)] // Sol @ ~ $155.1233
#[test_case(150., 29., 31., BankMint::Fixed, BankMint::Sol)] // Fixed = 2, SOL = 10
#[test_case(2_000., 0.02, 2.1, BankMint::FixedLow, BankMint::Fixed)] // Low = 0.0001, fixed = 2
#[tokio::test]
async fn marginfi_account_borrow_failure_not_enough_collateral(
    deposit_amount: f64,
    borrow_amount_ok: f64,
    borrow_amount_failed: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // LP

    let lp_deposit_amount = 2. * borrow_amount_failed;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_token_account_f_sol = test_f
        .get_bank(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_token_account_f_sol.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await?;

    // User

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);
    let borrower_debt_token_account_f = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    let borrower_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let collateral_bank = test_f.get_bank(&collateral_mint);
    borrower_mfi_account_f
        .try_bank_deposit(
            borrower_collateral_token_account_f.key,
            collateral_bank,
            deposit_amount,
            None,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let debt_bank_f = test_f.get_bank(&debt_mint);

    let res = borrower_mfi_account_f
        .try_bank_borrow(
            borrower_debt_token_account_f.key,
            debt_bank_f,
            borrow_amount_failed,
        )
        .await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::RiskEngineInitRejected);

    let res = borrower_mfi_account_f
        .try_bank_borrow(
            borrower_debt_token_account_f.key,
            debt_bank_f,
            borrow_amount_ok,
        )
        .await;
    assert!(res.is_ok());

    Ok(())
}

#[test_case(505., 500., 505.0000000001, BankMint::Usdc, BankMint::Sol)]
#[test_case(12_345.6, 12_345.5, 12_345.9, BankMint::Usdc, BankMint::Sol)]
#[test_case(0.91, 0.1, 0.98, BankMint::Sol, BankMint::Usdc)]
#[test_case(505., 0.092, 500., BankMint::PyUSD, BankMint::T22WithFee)]
#[test_case(1.8, 1.7, 1.9, BankMint::T22WithFee, BankMint::Sol)]
#[test_case(1.5, 1.4, 1.6, BankMint::SolSwbPull, BankMint::Usdc)]
#[test_case(150., 5., 151., BankMint::Fixed, BankMint::Sol)]
#[test_case(0.04, 0.02, 0.05, BankMint::FixedLow, BankMint::Fixed)]
#[tokio::test]
async fn marginfi_account_borrow_failure_borrow_limit(
    borrow_cap: f64,
    borrow_amount_ok: f64,
    borrow_amount_failed: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // LP

    let lp_deposit_amount = 2. * borrow_amount_failed;
    let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
    let lp_mfi_account_f = test_f.create_marginfi_account().await;
    let lp_collateral_token_account = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_token_account_and_mint_to(lp_wallet_balance)
        .await;
    lp_mfi_account_f
        .try_bank_deposit(
            lp_collateral_token_account.key,
            test_f.get_bank(&debt_mint),
            lp_deposit_amount,
            None,
        )
        .await
        .unwrap();

    // User

    let user_mfi_account_f = test_f.create_marginfi_account().await;
    let sufficient_collateral_amount = test_f
        .get_sufficient_collateral_for_outflow(borrow_amount_failed, &collateral_mint, &debt_mint)
        .await;
    let user_wallet_balance = get_max_deposit_amount_pre_fee(sufficient_collateral_amount);
    let user_collateral_token_account_f = test_f
        .get_bank_mut(&collateral_mint)
        .mint
        .create_token_account_and_mint_to(user_wallet_balance)
        .await;
    let user_debt_token_account_f = test_f
        .get_bank_mut(&debt_mint)
        .mint
        .create_empty_token_account()
        .await;
    user_mfi_account_f
        .try_bank_deposit(
            user_collateral_token_account_f.key,
            test_f.get_bank(&collateral_mint),
            sufficient_collateral_amount,
            None,
        )
        .await?;

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    let debt_mint_decimals = test_f.get_bank(&debt_mint).mint.mint.decimals;
    test_f
        .get_bank_mut(&debt_mint)
        .update_config(
            BankConfigOpt {
                borrow_limit: Some(native!(borrow_cap, debt_mint_decimals, f64)),
                ..Default::default()
            },
            None,
        )
        .await?;

    let debt_bank_f = test_f.get_bank(&debt_mint);

    let res = user_mfi_account_f
        .try_bank_borrow(
            user_debt_token_account_f.key,
            debt_bank_f,
            borrow_amount_failed,
        )
        .await;
    assert!(res.is_err());
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::BankLiabilityCapacityExceeded
    );

    let res = user_mfi_account_f
        .try_bank_borrow(user_debt_token_account_f.key, debt_bank_f, borrow_amount_ok)
        .await;
    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn isolated_borrows() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_eq_iso_bank = test_f.get_bank(&BankMint::SolEqIsolated);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_eq_iso_bank, 1_000, None)
        .await?;

    let lender_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_bank, 1_000, None)
        .await?;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_f_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let borrower_token_account_f_sol = test_f
        .sol_equivalent_mint
        .create_empty_token_account()
        .await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_f_usdc.key, usdc_bank, 1_000, None)
        .await?;

    // Borrow SOL EQ
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_eq_iso_bank, 10)
        .await;

    assert!(res.is_ok());

    // Repay isolated SOL EQ borrow and borrow SOL successfully,
    let borrower_sol_account = test_f.sol_mint.create_empty_token_account().await;
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_sol_account.key, sol_bank, 10)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IsolatedAccountIllegalState);

    borrower_mfi_account_f
        .try_bank_repay(
            borrower_token_account_f_sol.key,
            sol_eq_iso_bank,
            0,
            Some(true),
        )
        .await?;

    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_sol_account.key, sol_bank, 10)
        .await;

    assert!(res.is_ok());

    // Borrowing SOL EQ again fails
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_eq_iso_bank, 10)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::IsolatedAccountIllegalState);

    Ok(())
}

#[tokio::test]
async fn emode_borrows() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank = test_f.get_bank(&BankMint::SolEquivalent);
    let loaded_sol_eq_bank: Bank = test_f.load_and_deserialize(&sol_eq_bank.key).await;
    assert_eq!(
        loaded_sol_eq_bank.config.asset_weight_init,
        I80F48!(1.0).into()
    );
    assert_eq!(
        loaded_sol_eq_bank.config.asset_weight_maint,
        I80F48!(1.0).into()
    );

    // Bring asset weights down to 0.4 for SOL EQ bank
    let config_bank_opt = BankConfigOpt {
        asset_weight_init: Some(I80F48!(0.4).into()),
        asset_weight_maint: Some(I80F48!(0.4).into()),
        ..BankConfigOpt::default()
    };
    let res = sol_eq_bank
        .update_config(config_bank_opt.clone(), None)
        .await;
    assert!(res.is_ok());

    let loaded_sol_eq_bank: Bank = test_f.load_and_deserialize(&sol_eq_bank.key).await;
    assert_eq!(
        loaded_sol_eq_bank.config.asset_weight_init,
        I80F48!(0.4).into()
    );
    assert_eq!(
        loaded_sol_eq_bank.config.asset_weight_maint,
        I80F48!(0.4).into()
    );

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // Fund SOL lender
    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_sol = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let res = lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_eq_bank, 1_000, None)
        .await;
    assert!(res.is_ok());

    let lender_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let res = lender_mfi_account_f
        .try_bank_deposit(lender_token_account_sol.key, sol_bank, 1_000, None)
        .await;
    assert!(res.is_ok());

    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let res = lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank, 1_000, None)
        .await;
    assert!(res.is_ok());

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // Fund SOL borrower
    let borrower_mfi_account_f = test_f.create_marginfi_account().await;

    let borrower_token_account_f_sol = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    let res = borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_f_sol.key, sol_eq_bank, 1_000, None)
        .await;
    assert!(res.is_ok());

    // Borrow SOL against SOL EQ
    let borrower_token_account_f_sol = test_f.sol_mint.create_empty_token_account().await;

    // Check that we cannot borrow > ~400 SOL
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_bank, 399)
        .await;
    assert!(res.is_ok());
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_bank, 2)
        .await;
    assert!(res.is_err());

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // Configure emode for the banks so that SOL bank has a favourable rate for SOL_EQ bank
    let sol_eq_emode_tag = 1u16;
    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&sol_eq_bank, sol_eq_emode_tag, &[])
        .await;
    assert!(res.is_ok());

    let sol_emode_tag = 2u16;
    let emode_entries = vec![EmodeEntry {
        collateral_bank_emode_tag: sol_eq_emode_tag,
        flags: 0,
        pad0: [0, 0, 0, 0, 0],
        asset_weight_init: I80F48!(0.9).into(), // up from 0.4, gives 10x leverage
        asset_weight_maint: I80F48!(0.9).into(), // up from 0.4, gives 10x leverage
    }];

    let res = test_f
        .marginfi_group
        .try_lending_pool_configure_bank_emode(&sol_bank, sol_emode_tag, &emode_entries)
        .await;
    assert!(res.is_ok());

    // Verify that we CAN now borrow up to ~900 SOL (i.e. +500 to existing 399 liability)
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_bank, 500)
        .await;
    assert!(res.is_ok());
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_bank, 2)
        .await;
    assert!(res.is_err());

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // Now check that we cannot borrow any other asset since it would turn off the emode and put us in a bad health
    let borrower_token_account_f_usdc = test_f.usdc_mint.create_empty_token_account().await;
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_usdc.key, usdc_bank, 2)
        .await;
    assert!(res.is_err());

    // Repay part of the debt so that we can borrow some USDC (even though this would turn off the emode)
    let res = borrower_mfi_account_f
        .try_bank_repay(borrower_token_account_f_sol.key, sol_bank, 550, None)
        .await;
    assert!(res.is_ok());

    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_usdc.key, usdc_bank, 500)
        .await;
    assert!(res.is_ok());

    // Check that we cannot re-borrow ~the same amount (- borrowed USDC) of SOL anymore.
    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_bank, 400)
        .await;
    assert!(res.is_err());

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // Now fully repay the USDC debt and verify that the SOL borrow limits are again favourable (emode is ON)
    let res = borrower_mfi_account_f
        .try_bank_repay(borrower_token_account_f_usdc.key, usdc_bank, 0, Some(true))
        .await;
    assert!(res.is_ok());

    let res = borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_f_sol.key, sol_bank, 550)
        .await;
    assert!(res.is_ok());

    Ok(())
}
