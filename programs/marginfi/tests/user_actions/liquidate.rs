use anchor_spl::token_2022::spl_token_2022::extension::{
    transfer_fee::TransferFeeConfig, BaseStateWithExtensions,
};
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, assert_eq_noise, native, prelude::*};
use marginfi::{
    prelude::*,
    state::bank::{BankImpl, BankVaultType},
};
use marginfi_type_crate::types::{Bank, BankConfig, BankConfigOpt, EmodeEntry};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::clock::Clock;
use test_case::test_case;

#[test_case(100., 9.9, -1., 1., 2., BankMint::Usdc, BankMint::Sol)]
#[test_case(123., 122., -4., -4., 10., BankMint::SolEquivalent, BankMint::SolEqIsolated)]
#[test_case(1_000., 999., 0., 0., 10., BankMint::Usdc, BankMint::T22WithFee)]
#[test_case(2_000., 99., 400., -50., 1_500., BankMint::T22WithFee, BankMint::SolEquivalent)]
#[test_case(2_000., 1_999., 1000., 750., 2_000., BankMint::Usdc, BankMint::PyUSD)]
#[test_case(150., 16.9, 50., 0., 5., BankMint::Fixed, BankMint::Sol)]
#[test_case(50_000., 2., 75_000., 0., 1.5, BankMint::FixedLow, BankMint::Fixed)]
#[tokio::test]
async fn marginfi_account_liquidation_success(
    liquidatee_deposit_amount: f64,
    liquidatee_borrow_amount: f64,
    liquidator_deposit_amount: f64,
    liquidator_borrow_amount: f64,
    liquidate_amount: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // LP

    {
        let lp_deposit_amount = 2. * liquidatee_borrow_amount;
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
    }

    // Liquidatee

    let (
        liquidatee_mfi_account_f,
        liquidatee_borrow_amount_actual,
        liquidatee_collateral_index,
        liquidatee_debt_index,
    ) = {
        let liquidatee_mfi_account_f = test_f.create_marginfi_account().await;
        let liquidatee_wallet_balance = get_max_deposit_amount_pre_fee(liquidatee_deposit_amount);
        let liquidatee_collateral_token_account_f = test_f
            .get_bank_mut(&collateral_mint)
            .mint
            .create_token_account_and_mint_to(liquidatee_wallet_balance)
            .await;
        let liquidatee_debt_token_account_f = test_f
            .get_bank_mut(&debt_mint)
            .mint
            .create_empty_token_account()
            .await;
        let collateral_bank = test_f.get_bank(&collateral_mint);
        liquidatee_mfi_account_f
            .try_bank_deposit(
                liquidatee_collateral_token_account_f.key,
                collateral_bank,
                liquidatee_deposit_amount,
                None,
            )
            .await?;
        let debt_bank = test_f.get_bank(&debt_mint);
        liquidatee_mfi_account_f
            .try_bank_borrow(
                liquidatee_debt_token_account_f.key,
                debt_bank,
                liquidatee_borrow_amount,
            )
            .await?;

        let liquidatee_mfi_ma = liquidatee_mfi_account_f.load().await;
        // Due to balances sorting, collateral and debt may be not at indices 0 and 1, respectively -> determine them first
        let collateral_index = liquidatee_mfi_ma
            .lending_account
            .balances
            .iter()
            .position(|b| b.is_active() && b.bank_pk == collateral_bank.key)
            .unwrap();
        let debt_index = liquidatee_mfi_ma
            .lending_account
            .balances
            .iter()
            .position(|b| b.is_active() && b.bank_pk == debt_bank.key)
            .unwrap();

        let debt_bank = debt_bank.load().await;
        let borrow_amount_actual_native = debt_bank.get_liability_amount(
            liquidatee_mfi_ma.lending_account.balances[debt_index]
                .liability_shares
                .into(),
        )?;
        let borrow_amount_actual = borrow_amount_actual_native.to_num::<f64>()
            / 10_f64.powf(debt_bank.mint_decimals as f64);

        (
            liquidatee_mfi_account_f,
            borrow_amount_actual,
            collateral_index,
            debt_index,
        )
    };

    // Liquidator

    let liquidator_mfi_account_f = {
        let liquidator_mfi_account_f = test_f.create_marginfi_account().await;
        // This is just to be sure the liquidator is healthy even if its balances in debt and collateral banks are negative (liabs).
        let extra_collateral_amount = 10.0 * liquidatee_borrow_amount_actual;
        let liquidator_wallet_balance = get_max_deposit_amount_pre_fee(extra_collateral_amount);
        let liquidator_extra_collateral_token_account_f = test_f
            .get_bank_mut(&BankMint::SolSwbPull)
            .mint
            .create_token_account_and_mint_to(liquidator_wallet_balance)
            .await;

        liquidator_mfi_account_f
            .try_bank_deposit(
                liquidator_extra_collateral_token_account_f.key,
                test_f.get_bank(&BankMint::SolSwbPull),
                extra_collateral_amount,
                None,
            )
            .await?;

        // Now let's fill the balances in the actual collateral and debt banks.
        let liquidator_collateral_mint_f = &test_f.get_bank_mut(&collateral_mint).mint;

        if liquidator_deposit_amount > 0. {
            let liquidator_wallet_balance =
                get_max_deposit_amount_pre_fee(liquidator_deposit_amount);
            let liquidator_collateral_token_account_f = liquidator_collateral_mint_f
                .create_token_account_and_mint_to(liquidator_wallet_balance)
                .await;

            liquidator_mfi_account_f
                .try_bank_deposit(
                    liquidator_collateral_token_account_f.key,
                    test_f.get_bank(&collateral_mint),
                    liquidator_deposit_amount,
                    None,
                )
                .await?;
        } else if liquidator_deposit_amount < 0. {
            let liquidator_collateral_token_account_f = liquidator_collateral_mint_f
                .create_empty_token_account()
                .await;
            liquidator_mfi_account_f
                .try_bank_borrow(
                    liquidator_collateral_token_account_f.key,
                    test_f.get_bank(&collateral_mint),
                    -liquidator_deposit_amount,
                )
                .await?;
        };

        let liquidator_debt_mint_f = &test_f.get_bank_mut(&debt_mint).mint;

        if liquidator_borrow_amount > 0. {
            let liquidator_debt_token_account_f =
                liquidator_debt_mint_f.create_empty_token_account().await;
            liquidator_mfi_account_f
                .try_bank_borrow(
                    liquidator_debt_token_account_f.key,
                    test_f.get_bank(&debt_mint),
                    liquidator_borrow_amount,
                )
                .await?;
        } else if liquidator_borrow_amount < 0. {
            let liquidator_wallet_balance =
                get_max_deposit_amount_pre_fee(-liquidator_borrow_amount);
            let liquidator_debt_token_account_f = liquidator_debt_mint_f
                .create_token_account_and_mint_to(liquidator_wallet_balance)
                .await;
            liquidator_mfi_account_f
                .try_bank_deposit(
                    liquidator_debt_token_account_f.key,
                    test_f.get_bank(&debt_mint),
                    -liquidator_borrow_amount,
                    None,
                )
                .await?;
        };

        liquidator_mfi_account_f
    };

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    // Synthetically bring down the borrower account health by reducing the asset weights of the collateral bank
    test_f
        .get_bank_mut(&collateral_mint)
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.5).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let collateral_bank_f = test_f.get_bank(&collateral_mint);
    let debt_bank_f = test_f.get_bank(&debt_mint);

    liquidator_mfi_account_f
        .try_liquidate(
            &liquidatee_mfi_account_f,
            collateral_bank_f,
            liquidate_amount,
            debt_bank_f,
        )
        .await?;

    let liquidator_mfi_ma = liquidator_mfi_account_f.load().await;
    let liquidatee_mfi_ma = liquidatee_mfi_account_f.load().await;

    // Due to balances sorting (and the LP deposit in the third bank), collateral and debt may be not at indices 0 and 1 -> determine them first
    let liquidator_collateral_index = liquidator_mfi_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == collateral_bank_f.key)
        .unwrap();
    let liquidator_debt_index = liquidator_mfi_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == debt_bank_f.key)
        .unwrap();

    let collateral_bank = collateral_bank_f.load().await;
    let debt_bank = debt_bank_f.load().await;
    let liquidate_amount_native = I80F48::from(native!(
        liquidate_amount,
        collateral_bank_f.mint.mint.decimals,
        f64
    ));

    // Check liquidator collateral and debt balances
    let expected_collateral_mint_liquidator_balance = if liquidator_deposit_amount > 0.0 {
        I80F48::from(native!(
            liquidator_deposit_amount,
            collateral_bank_f.mint.mint.decimals,
            f64
        ))
        .checked_add(liquidate_amount_native)
        .unwrap()
    } else {
        liquidate_amount_native
            .checked_sub(I80F48::from(native!(
                -liquidator_deposit_amount,
                collateral_bank_f.mint.mint.decimals,
                f64
            )))
            .unwrap()
    };

    let collateral_mint_liquidator_balance = collateral_bank.get_asset_amount(
        liquidator_mfi_ma.lending_account.balances[liquidator_collateral_index]
            .asset_shares
            .into(),
    )?;
    assert_eq!(
        expected_collateral_mint_liquidator_balance,
        collateral_mint_liquidator_balance
    );

    let debt_paid_out = liquidate_amount * 0.975 * collateral_bank_f.get_price().await
        / debt_bank_f.get_price().await;
    let debt_paid_out_native =
        I80F48::from(native!(debt_paid_out, debt_bank_f.mint.mint.decimals, f64));

    let expected_debt_mint_liquidator_balance = if liquidator_borrow_amount > 0.0 {
        I80F48::from(native!(
            liquidator_borrow_amount,
            debt_bank_f.mint.mint.decimals,
            f64
        ))
        .checked_add(debt_paid_out_native)
        .unwrap()
    } else {
        debt_paid_out_native
            .checked_sub(I80F48::from(native!(
                -liquidator_borrow_amount,
                debt_bank_f.mint.mint.decimals,
                f64
            )))
            .unwrap()
    };

    let debt_mint_liquidator_balance = debt_bank.get_liability_amount(
        liquidator_mfi_ma.lending_account.balances[liquidator_debt_index]
            .liability_shares
            .into(),
    )?;

    assert_eq_noise!(
        expected_debt_mint_liquidator_balance,
        debt_mint_liquidator_balance,
        1.
    );

    // Check liquidatee collateral and debt balances
    let debt_covered = liquidate_amount * 0.95 * collateral_bank_f.get_price().await
        / debt_bank_f.get_price().await;

    let borrow_amount_native = I80F48::from(native!(
        liquidatee_borrow_amount_actual,
        debt_bank_f.mint.mint.decimals,
        f64
    ));
    let debt_covered_native =
        I80F48::from(native!(debt_covered, debt_bank_f.mint.mint.decimals, f64));

    let expected_debt_mint_liquidatee_balance = borrow_amount_native
        .checked_sub(debt_covered_native)
        .unwrap();
    let debt_mint_liquidatee_balance = debt_bank.get_liability_amount(
        liquidatee_mfi_ma.lending_account.balances[liquidatee_debt_index]
            .liability_shares
            .into(),
    )?;
    assert_eq_noise!(
        expected_debt_mint_liquidatee_balance,
        debt_mint_liquidatee_balance,
        1.
    );

    let expected_collateral_mint_liquidatee_balance = I80F48::from(native!(
        liquidatee_deposit_amount,
        collateral_bank_f.mint.mint.decimals,
        f64
    ))
    .checked_sub(liquidate_amount_native)
    .unwrap();
    let collateral_mint_liquidatee_balance = collateral_bank
        .get_asset_amount(
            liquidatee_mfi_ma.lending_account.balances[liquidatee_collateral_index]
                .asset_shares
                .into(),
        )
        .unwrap();
    assert_eq!(
        expected_collateral_mint_liquidatee_balance,
        collateral_mint_liquidatee_balance
    );

    let expected_debt_mint_liquidatee_balance = I80F48::from(native!(
        liquidatee_borrow_amount_actual,
        debt_bank_f.mint.mint.decimals,
        f64
    ))
    .checked_sub(debt_covered_native)
    .unwrap();

    let debt_mint_liquidatee_balance = debt_bank.get_liability_amount(
        liquidatee_mfi_ma.lending_account.balances[liquidatee_debt_index]
            .liability_shares
            .into(),
    )?;
    assert_eq_noise!(
        expected_debt_mint_liquidatee_balance,
        debt_mint_liquidatee_balance,
        1.
    );

    // Check the insurance fees
    let insurance_fund_fee = liquidate_amount * 0.025 * collateral_bank_f.get_price().await
        / debt_bank_f.get_price().await;
    let expected_insurance_fund_usdc_pre_fee =
        native!(insurance_fund_fee, debt_bank_f.mint.mint.decimals, f64);
    let if_transfer_fee = debt_bank_f
        .mint
        .load_state()
        .await
        .get_extension::<TransferFeeConfig>()
        .map(|tf| {
            tf.calculate_epoch_fee(0, expected_insurance_fund_usdc_pre_fee)
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let expected_insurance_fund_usdc =
        (expected_insurance_fund_usdc_pre_fee - if_transfer_fee) as i64;

    let insurance_fund_usdc = debt_bank_f
        .get_vault_token_account(BankVaultType::Insurance)
        .await
        .balance()
        .await as i64;
    assert_eq_noise!(expected_insurance_fund_usdc, insurance_fund_usdc, 1);

    Ok(())
}

#[test_case(100., 9.9, 1., BankMint::Usdc, BankMint::Sol)]
#[test_case(123., 122., 10., BankMint::SolEquivalent, BankMint::SolEqIsolated)]
#[test_case(1_000., 999., 10., BankMint::Usdc, BankMint::T22WithFee)]
#[test_case(2_000., 99., 1500., BankMint::T22WithFee, BankMint::SolEquivalent)]
#[test_case(2_000., 1_999., 2000., BankMint::Usdc, BankMint::PyUSD)]
#[test_case(50., 9.9, 0.5, BankMint::Fixed, BankMint::Sol)]
#[test_case(1_000_000., 49.5, 10_000., BankMint::FixedLow, BankMint::Fixed)]
#[tokio::test]
async fn marginfi_account_liquidation_with_delays_success(
    liquidatee_deposit_amount: f64,
    liquidatee_borrow_amount: f64,
    liquidate_amount: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // LP

    {
        let lp_deposit_amount = 2. * liquidatee_borrow_amount;
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
    }

    // Liquidatee

    let (
        liquidatee_mfi_account_f,
        liquidatee_borrow_amount_actual,
        liquidatee_collateral_index,
        liquidatee_debt_index,
    ) = {
        let liquidatee_mfi_account_f = test_f.create_marginfi_account().await;
        let liquidatee_wallet_balance = get_max_deposit_amount_pre_fee(liquidatee_deposit_amount);
        let liquidatee_collateral_token_account_f = test_f
            .get_bank_mut(&collateral_mint)
            .mint
            .create_token_account_and_mint_to(liquidatee_wallet_balance)
            .await;
        let liquidatee_debt_token_account_f = test_f
            .get_bank_mut(&debt_mint)
            .mint
            .create_empty_token_account()
            .await;
        let collateral_bank = test_f.get_bank(&collateral_mint);
        liquidatee_mfi_account_f
            .try_bank_deposit(
                liquidatee_collateral_token_account_f.key,
                collateral_bank,
                liquidatee_deposit_amount,
                None,
            )
            .await?;
        let debt_bank = test_f.get_bank(&debt_mint);
        liquidatee_mfi_account_f
            .try_bank_borrow(
                liquidatee_debt_token_account_f.key,
                debt_bank,
                liquidatee_borrow_amount,
            )
            .await?;

        let liquidatee_mfi_ma = liquidatee_mfi_account_f.load().await;
        // Due to balances sorting, collateral and debt may be not at indices 0 and 1, respectively -> determine them first
        let collateral_index = liquidatee_mfi_ma
            .lending_account
            .balances
            .iter()
            .position(|b| b.is_active() && b.bank_pk == collateral_bank.key)
            .unwrap();
        let debt_index = liquidatee_mfi_ma
            .lending_account
            .balances
            .iter()
            .position(|b| b.is_active() && b.bank_pk == debt_bank.key)
            .unwrap();

        let debt_bank = debt_bank.load().await;
        let borrow_amount_actual_native = debt_bank.get_liability_amount(
            liquidatee_mfi_ma.lending_account.balances[debt_index]
                .liability_shares
                .into(),
        )?;
        let borrow_amount_actual = borrow_amount_actual_native.to_num::<f64>()
            / 10_f64.powf(debt_bank.mint_decimals as f64);

        (
            liquidatee_mfi_account_f,
            borrow_amount_actual,
            collateral_index,
            debt_index,
        )
    };

    // Liquidator

    let liquidator_mfi_account_f = {
        let liquidator_mfi_account_f = test_f.create_marginfi_account().await;
        let liquidator_wallet_balance =
            get_max_deposit_amount_pre_fee(liquidatee_borrow_amount_actual);
        let liquidator_collateral_token_account_f = test_f
            .get_bank_mut(&debt_mint)
            .mint
            .create_token_account_and_mint_to(liquidator_wallet_balance)
            .await;

        liquidator_mfi_account_f
            .try_bank_deposit(
                liquidator_collateral_token_account_f.key,
                test_f.get_bank(&debt_mint),
                liquidatee_borrow_amount_actual,
                None,
            )
            .await?;

        liquidator_mfi_account_f
    };

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    // Synthetically bring down the borrower account health by reducing the asset weights of the collateral bank
    test_f
        .get_bank_mut(&collateral_mint)
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.5).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let collateral_bank_f = test_f.get_bank(&collateral_mint);
    let debt_bank_f = test_f.get_bank(&debt_mint);

    // This is just to test that the accounts' last_update field is properly updated upon modification
    let pre_liquidatee_last_update = liquidatee_mfi_account_f.load().await.last_update;
    let pre_liquidator_last_update = liquidator_mfi_account_f.load().await.last_update;
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 sec
        clock.unix_timestamp += 1;
        ctx.set_sysvar(&clock);
    }

    liquidator_mfi_account_f
        .try_liquidate(
            &liquidatee_mfi_account_f,
            collateral_bank_f,
            liquidate_amount,
            debt_bank_f,
        )
        .await?;

    let liquidator_mfi_ma = liquidator_mfi_account_f.load().await;
    let liquidatee_mfi_ma = liquidatee_mfi_account_f.load().await;

    assert_eq!(
        liquidator_mfi_ma.last_update,
        pre_liquidator_last_update + 1
    );
    assert_eq!(
        liquidatee_mfi_ma.last_update,
        pre_liquidatee_last_update + 1
    );
    // Due to balances sorting (and the LP deposit in the third bank), collateral and debt may be not at indices 0 and 1 -> determine them first
    let liquidator_collateral_index = liquidator_mfi_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == collateral_bank_f.key)
        .unwrap();
    let liquidator_debt_index = liquidator_mfi_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == debt_bank_f.key)
        .unwrap();

    let collateral_bank = collateral_bank_f.load().await;
    let debt_bank = debt_bank_f.load().await;
    let liquidate_amount_native = I80F48::from(native!(
        liquidate_amount,
        collateral_bank_f.mint.mint.decimals,
        f64
    ));

    // Check liquidator collateral and debt balances
    let expected_collateral_mint_liquidator_balance = liquidate_amount_native;

    let collateral_mint_liquidator_balance = collateral_bank.get_asset_amount(
        liquidator_mfi_ma.lending_account.balances[liquidator_collateral_index]
            .asset_shares
            .into(),
    )?;
    assert_eq_noise!(
        expected_collateral_mint_liquidator_balance,
        collateral_mint_liquidator_balance,
        1.
    );

    let debt_paid_out = liquidate_amount * 0.975 * collateral_bank_f.get_price().await
        / debt_bank_f.get_price().await;
    let debt_paid_out_native =
        I80F48::from(native!(debt_paid_out, debt_bank_f.mint.mint.decimals, f64));

    // We have to account for updated share value here due to 1 second of delay we introduced to check last_update
    let expected_debt_mint_liquidator_balance = I80F48::from(native!(
        liquidatee_borrow_amount_actual,
        debt_bank_f.mint.mint.decimals,
        f64
    ))
    .checked_mul(debt_bank.asset_share_value.into())
    .unwrap()
    .checked_sub(debt_paid_out_native)
    .unwrap();

    let debt_mint_liquidator_balance = debt_bank.get_asset_amount(
        liquidator_mfi_ma.lending_account.balances[liquidator_debt_index]
            .asset_shares
            .into(),
    )?;

    assert_eq_noise!(
        expected_debt_mint_liquidator_balance,
        debt_mint_liquidator_balance,
        1.
    );

    // Check liquidatee collateral and debt balances

    let debt_covered = liquidate_amount * 0.95 * collateral_bank_f.get_price().await
        / debt_bank_f.get_price().await;

    // We have to account for updated share value here due to 1 second of delay we introduced to check last_update
    let borrow_amount_native = I80F48::from(native!(
        liquidatee_borrow_amount_actual,
        debt_bank_f.mint.mint.decimals,
        f64
    ))
    .checked_mul(debt_bank.liability_share_value.into())
    .unwrap();
    let debt_covered_native =
        I80F48::from(native!(debt_covered, debt_bank_f.mint.mint.decimals, f64));

    let expected_debt_mint_liquidatee_balance = borrow_amount_native
        .checked_sub(debt_covered_native)
        .unwrap();
    let debt_mint_liquidatee_balance = debt_bank.get_liability_amount(
        liquidatee_mfi_ma.lending_account.balances[liquidatee_debt_index]
            .liability_shares
            .into(),
    )?;
    assert_eq_noise!(
        expected_debt_mint_liquidatee_balance,
        debt_mint_liquidatee_balance,
        1.
    );

    let expected_collateral_mint_liquidatee_balance = I80F48::from(native!(
        liquidatee_deposit_amount,
        collateral_bank_f.mint.mint.decimals,
        f64
    ))
    .checked_mul(collateral_bank.asset_share_value.into())
    .unwrap()
    .checked_sub(liquidate_amount_native)
    .unwrap();
    let collateral_mint_liquidatee_balance = collateral_bank
        .get_asset_amount(
            liquidatee_mfi_ma.lending_account.balances[liquidatee_collateral_index]
                .asset_shares
                .into(),
        )
        .unwrap();
    assert_eq_noise!(
        expected_collateral_mint_liquidatee_balance,
        collateral_mint_liquidatee_balance,
        1.
    );

    let expected_debt_mint_liquidatee_balance = I80F48::from(native!(
        liquidatee_borrow_amount_actual,
        debt_bank_f.mint.mint.decimals,
        f64
    ))
    .checked_mul(debt_bank.liability_share_value.into())
    .unwrap()
    .checked_sub(debt_covered_native)
    .unwrap();

    let debt_mint_liquidatee_balance = debt_bank.get_liability_amount(
        liquidatee_mfi_ma.lending_account.balances[liquidatee_debt_index]
            .liability_shares
            .into(),
    )?;
    assert_eq_noise!(
        expected_debt_mint_liquidatee_balance,
        debt_mint_liquidatee_balance,
        1.
    );

    // Check the insurance fees
    let insurance_fund_fee = liquidate_amount * 0.025 * collateral_bank_f.get_price().await
        / debt_bank_f.get_price().await;
    let expected_insurance_fund_usdc_pre_fee =
        native!(insurance_fund_fee, debt_bank_f.mint.mint.decimals, f64);
    let if_transfer_fee = debt_bank_f
        .mint
        .load_state()
        .await
        .get_extension::<TransferFeeConfig>()
        .map(|tf| {
            tf.calculate_epoch_fee(0, expected_insurance_fund_usdc_pre_fee)
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let expected_insurance_fund_usdc =
        (expected_insurance_fund_usdc_pre_fee - if_transfer_fee) as i64;

    let insurance_fund_usdc = debt_bank_f
        .get_vault_token_account(BankVaultType::Insurance)
        .await
        .balance()
        .await as i64;
    assert_eq_noise!(expected_insurance_fund_usdc, insurance_fund_usdc, 1);

    Ok(())
}

#[tokio::test]
async fn marginfi_account_liquidation_success_many_balances() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::many_banks_10())).await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let sol_eq1_bank_f = test_f.get_bank(&BankMint::SolEquivalent1);
    let sol_eq2_bank_f = test_f.get_bank(&BankMint::SolEquivalent2);
    let sol_eq3_bank_f = test_f.get_bank(&BankMint::SolEquivalent3);
    let sol_eq4_bank_f = test_f.get_bank(&BankMint::SolEquivalent4);
    let sol_eq5_bank_f = test_f.get_bank(&BankMint::SolEquivalent5);
    let sol_eq6_bank_f = test_f.get_bank(&BankMint::SolEquivalent6);
    let sol_eq7_bank_f = test_f.get_bank(&BankMint::SolEquivalent7);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(2_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 2_000, None)
        .await?;

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    let borrower_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;
    let borrower_token_account_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(5000)
        .await;

    // Borrower deposits 100 SOL worth of $1000
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    // Borrower borrows $999
    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc.key, usdc_bank_f, 999)
        .await?;

    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq_bank_f, 0, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq1_bank_f, 0, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq2_bank_f, 0, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq3_bank_f, 0, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq4_bank_f, 0, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq5_bank_f, 0, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq6_bank_f, 0, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq7_bank_f, 0, None)
        .await?;

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
        .try_liquidate(&borrower_mfi_account_f, sol_bank_f, 1, usdc_bank_f)
        .await?;

    // Checks
    let sol_bank: Bank = sol_bank_f.load().await;
    let usdc_bank: Bank = usdc_bank_f.load().await;

    let depositor_ma = lender_mfi_account_f.load().await;
    let borrower_ma = borrower_mfi_account_f.load().await;

    // Due to balances sorting, SOL and USDC may be not at indices 0 and 1, respectively -> determine them first
    let sol_index = depositor_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == sol_bank_f.key)
        .unwrap();
    let usdc_index = depositor_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == usdc_bank_f.key)
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
        usdc_bank
            .get_asset_amount(
                depositor_ma.lending_account.balances[usdc_index]
                    .asset_shares
                    .into()
            )
            .unwrap(),
        I80F48::from(native!(1990.25, "USDC", f64)),
        native!(0.00001, "USDC", f64)
    );

    let sol_index = borrower_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == sol_bank_f.key)
        .unwrap();
    let usdc_index = borrower_ma
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == usdc_bank_f.key)
        .unwrap();

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

    // Borrower should have 989.50 USDC
    assert_eq_noise!(
        usdc_bank
            .get_liability_amount(
                borrower_ma.lending_account.balances[usdc_index]
                    .liability_shares
                    .into()
            )
            .unwrap(),
        I80F48::from(native!(989.50, "USDC", f64)),
        native!(0.00001, "USDC", f64)
    );

    // Check insurance fund fee
    let insurance_fund_usdc = usdc_bank_f
        .get_vault_token_account(BankVaultType::Insurance)
        .await;

    assert_eq_noise!(
        insurance_fund_usdc.balance().await as i64,
        native!(0.25, "USDC", f64) as i64,
        1
    );

    Ok(())
}

#[tokio::test]
async fn marginfi_account_liquidation_failure_liquidatee_not_unhealthy() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    asset_weight_maint: I80F48!(1).into(),
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
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
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 200, None)
        .await?;

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    let borrower_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;

    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc.key, usdc_bank_f, 100)
        .await?;

    let res = lender_mfi_account_f
        .try_liquidate(&borrower_mfi_account_f, sol_bank_f, 1, usdc_bank_f)
        .await;

    assert!(res.is_err());

    assert_custom_error!(res.unwrap_err(), MarginfiError::HealthyAccount);

    Ok(())
}

#[tokio::test]
async fn marginfi_account_liquidation_failure_liquidation_too_severe() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 200, None)
        .await?;

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    let borrower_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 10, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc.key, usdc_bank_f, 61)
        .await?;

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

    let res = lender_mfi_account_f
        .try_liquidate(&borrower_mfi_account_f, sol_bank_f, 10, usdc_bank_f)
        .await;

    // Note: ExhaustedLiability is essentially unreachable now
    // assert_custom_error!(res.unwrap_err(), MarginfiError::ExhaustedLiability);
    assert_custom_error!(res.unwrap_err(), MarginfiError::OperationRepayOnly);

    let res = lender_mfi_account_f
        .try_liquidate(&borrower_mfi_account_f, sol_bank_f, 1, usdc_bank_f)
        .await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn marginfi_account_liquidation_failure_liquidator_no_collateral() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    liability_weight_init: I80F48!(1.2).into(),
                    liability_weight_maint: I80F48!(1.1).into(),
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: None,
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 200, None)
        .await?;

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    let borrower_token_account_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(100)
        .await;
    let borrower_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 10, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq_bank_f, 1, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc.key, usdc_bank_f, 60)
        .await?;

    sol_bank_f
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.3).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let res = lender_mfi_account_f
        .try_liquidate(&borrower_mfi_account_f, sol_eq_bank_f, 2, usdc_bank_f)
        .await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::OverliquidationAttempt);

    let res = lender_mfi_account_f
        .try_liquidate(&borrower_mfi_account_f, sol_eq_bank_f, 1, usdc_bank_f)
        .await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn marginfi_account_liquidation_failure_bank_not_liquidatable() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 200, None)
        .await?;

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    let borrower_token_account_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(100)
        .await;
    let borrower_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 10, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol_eq.key, sol_eq_bank_f, 1, None)
        .await?;
    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc.key, usdc_bank_f, 60)
        .await?;

    sol_bank_f
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.25).into()),
                asset_weight_maint: Some(I80F48!(0.4).into()),
                ..Default::default()
            },
            None,
        )
        .await?;

    let res = lender_mfi_account_f
        .try_liquidate(&borrower_mfi_account_f, sol_eq_bank_f, 1, sol_bank_f)
        .await;

    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::NoLiabilitiesInLiabilityBank
    );

    let res = lender_mfi_account_f
        .try_liquidate(&borrower_mfi_account_f, sol_bank_f, 1, usdc_bank_f)
        .await;

    assert!(res.is_ok());

    Ok(())
}

// ??? this test is occasionally too slow to complete, shave off some tests cases?
#[test_case(100., 9.9, 1., BankMint::Usdc, BankMint::Sol)]
#[test_case(123., 122., 1.23, BankMint::SolEquivalent, BankMint::SolEqIsolated)]
#[test_case(1_000., 1900., 10., BankMint::Usdc, BankMint::T22WithFee)]
#[test_case(2_000., 99., 20., BankMint::T22WithFee, BankMint::SolEquivalent)]
#[test_case(2_000., 1_999., 20., BankMint::Usdc, BankMint::PyUSD)]
#[test_case(50., 9.9, 0.5, BankMint::Fixed, BankMint::Sol)]
#[test_case(1_000_000., 49.5, 10_000., BankMint::FixedLow, BankMint::Fixed)]
#[tokio::test]
async fn marginfi_account_liquidation_emode(
    deposit_amount: f64,
    borrow_amount: f64,
    liquidate_amount: f64,
    collateral_mint: BankMint,
    debt_mint: BankMint,
) -> anyhow::Result<()> {
    // -------------------------------------------------------------------------
    // Setup
    // -------------------------------------------------------------------------

    let mut test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    // Configure group to allow higher max emode leverage (100x instead of default 20x)
    let admin = test_f.payer();
    test_f
        .marginfi_group
        .try_update_with_emode_leverage(
            admin,
            admin,
            admin,
            admin,
            admin,
            admin,
            admin,
            Some(I80F48!(99).into()), // init must be < maint
            Some(I80F48!(100).into()),
        )
        .await
        .unwrap();

    let emode_breaker_bank_mint = if collateral_mint == BankMint::Usdc {
        BankMint::SolEquivalent
    } else {
        BankMint::Usdc
    };

    // LP

    {
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
        let lp_deposit_amount = 10.0; // any small amount is enough
        let lp_wallet_balance = get_max_deposit_amount_pre_fee(lp_deposit_amount);
        let lp_collateral_token_account = test_f
            .get_bank(&emode_breaker_bank_mint)
            .mint
            .create_token_account_and_mint_to(lp_wallet_balance)
            .await;
        lp_mfi_account_f
            .try_bank_deposit(
                lp_collateral_token_account.key,
                test_f.get_bank(&emode_breaker_bank_mint),
                lp_deposit_amount,
                None,
            )
            .await?;
    }

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // Liquidatee

    let (liquidatee_mfi_account_f, borrow_amount_actual) = {
        let liquidatee_mfi_account_f = test_f.create_marginfi_account().await;
        let liquidatee_wallet_balance = get_max_deposit_amount_pre_fee(deposit_amount);
        let liquidatee_collateral_token_account_f = test_f
            .get_bank_mut(&collateral_mint)
            .mint
            .create_token_account_and_mint_to(liquidatee_wallet_balance)
            .await;
        let liquidatee_debt_token_account_f = test_f
            .get_bank_mut(&debt_mint)
            .mint
            .create_empty_token_account()
            .await;
        liquidatee_mfi_account_f
            .try_bank_deposit(
                liquidatee_collateral_token_account_f.key,
                test_f.get_bank(&collateral_mint),
                deposit_amount,
                None,
            )
            .await?;
        let debt_bank_f = test_f.get_bank(&debt_mint);
        liquidatee_mfi_account_f
            .try_bank_borrow(
                liquidatee_debt_token_account_f.key,
                debt_bank_f,
                borrow_amount,
            )
            .await?;

        let liquidatee_mfi_ma = liquidatee_mfi_account_f.load().await;

        // Due to balances sorting, debt may be not at index 1 -> determine its actual index first
        let debt_index = liquidatee_mfi_ma
            .lending_account
            .balances
            .iter()
            .position(|b| b.is_active() && b.bank_pk == debt_bank_f.key)
            .unwrap();

        let debt_bank = debt_bank_f.load().await;
        let borrow_amount_actual_native = debt_bank.get_liability_amount(
            liquidatee_mfi_ma.lending_account.balances[debt_index]
                .liability_shares
                .into(),
        )?;
        let borrow_amount_actual = borrow_amount_actual_native.to_num::<f64>()
            / 10_f64.powf(debt_bank.mint_decimals as f64);

        (liquidatee_mfi_account_f, borrow_amount_actual)
    };

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // Liquidator

    let liquidator_mfi_account_f = {
        let liquidator_mfi_account_f = test_f.create_marginfi_account().await;
        let liquidator_wallet_balance = get_max_deposit_amount_pre_fee(borrow_amount_actual);
        let liquidator_collateral_token_account_f = test_f
            .get_bank_mut(&debt_mint)
            .mint
            .create_token_account_and_mint_to(liquidator_wallet_balance)
            .await;
        liquidator_mfi_account_f
            .try_bank_deposit(
                liquidator_collateral_token_account_f.key,
                test_f.get_bank(&debt_mint),
                borrow_amount_actual * 0.85,
                None,
            )
            .await?;

        liquidator_mfi_account_f
    };

    // -------------------------------------------------------------------------
    // Test
    // -------------------------------------------------------------------------

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // First decrease the default weights to make liquidatee unhealthy
    {
        let collateral_bank = test_f.get_bank(&collateral_mint);

        let config_bank_opt = BankConfigOpt {
            asset_weight_init: Some(I80F48!(0.1).into()),
            asset_weight_maint: Some(I80F48!(0.1).into()),
            ..BankConfigOpt::default()
        };
        let res = collateral_bank
            .update_config(config_bank_opt.clone(), None)
            .await;
        assert!(res.is_ok());
    }

    // Emode is OFF -> liquidation will succeed
    {
        let collateral_bank_f = test_f.get_bank(&collateral_mint);
        let debt_bank_f = test_f.get_bank(&debt_mint);
        let res = liquidator_mfi_account_f
            .try_liquidate(
                &liquidatee_mfi_account_f,
                collateral_bank_f,
                liquidate_amount,
                debt_bank_f,
            )
            .await;
        assert!(res.is_ok());
    }

    // Now increase the weights back but only in emode
    let collateral_bank_emode_tag = 1u16;
    {
        let collateral_bank = test_f.get_bank(&collateral_mint);
        let debt_bank = test_f.get_bank(&debt_mint);

        let res = test_f
            .marginfi_group
            .try_lending_pool_configure_bank_emode(&collateral_bank, collateral_bank_emode_tag, &[])
            .await;
        assert!(res.is_ok());

        let debt_bank_emode_tag = 2u16;
        let emode_entries = vec![EmodeEntry {
            collateral_bank_emode_tag,
            flags: 0,
            pad0: [0, 0, 0, 0, 0],
            asset_weight_init: I80F48!(0.989).into(), // up from 0.1, gives ~90.9x leverage
            asset_weight_maint: I80F48!(0.989).into(), // up from 0.1, gives ~90.9x leverage
        }];

        let res = test_f
            .marginfi_group
            .try_lending_pool_configure_bank_emode(&debt_bank, debt_bank_emode_tag, &emode_entries)
            .await;
        assert!(res.is_ok());
    }

    // The account is healthy when emode is ON -> liquidation will fail
    {
        let collateral_bank_f = test_f.get_bank(&collateral_mint);
        let debt_bank_f = test_f.get_bank(&debt_mint);
        let res = liquidator_mfi_account_f
            .try_liquidate(
                &liquidatee_mfi_account_f,
                collateral_bank_f,
                liquidate_amount * 0.1, // just to differ from the previous liquidation transaction
                debt_bank_f,
            )
            .await;
        assert!(res.is_err());
    }

    // Decrease the emode weights drawing the liquidatee account liquidatable again
    {
        let debt_bank = test_f.get_bank(&debt_mint);

        let debt_bank_emode_tag = 2u16;
        let emode_entries = vec![EmodeEntry {
            collateral_bank_emode_tag,
            flags: 0,
            pad0: [0, 0, 0, 0, 0],
            asset_weight_init: I80F48!(0.6).into(), // down from 1.0
            asset_weight_maint: I80F48!(0.6).into(), // down from 1.0
        }];

        let res = test_f
            .marginfi_group
            .try_lending_pool_configure_bank_emode(&debt_bank, debt_bank_emode_tag, &emode_entries)
            .await;
        assert!(res.is_ok());
    }

    // The account is again unhealthy (emode is still ON) -> liquidation will suceed
    {
        let collateral_bank_f = test_f.get_bank(&collateral_mint);
        let debt_bank_f = test_f.get_bank(&debt_mint);
        let res = liquidator_mfi_account_f
            .try_liquidate(
                &liquidatee_mfi_account_f,
                collateral_bank_f,
                liquidate_amount * 2.0, // just to differ from the previous liquidation transaction
                debt_bank_f,
            )
            .await;
        assert!(res.is_ok());
    }

    // Now we check the scenario where the liquidator tries to liquidate a valid amount of liquidatee's debt
    // but fails due to rendering its own account in a bad health as a result of liquidation. This happens because
    // liquidatee's collateral is still in emode and is weighted as 0.6 whereas liquidator's collateral is NOT
    // in emode (see below) and is weighted by default - as 0.4. So even though the liquidator would actually gain
    // some money, its health would become worse.

    // Refresh blockhash to prevent expiry during long test
    test_f.refresh_blockhash().await;

    // This borrowing of another asset disables emode for the liquidator
    let liquidator_debt_token_account_f = {
        let liquidator_debt_token_account_f = test_f
            .get_bank_mut(&emode_breaker_bank_mint)
            .mint
            .create_empty_token_account()
            .await;
        let res = liquidator_mfi_account_f
            .try_bank_borrow(
                liquidator_debt_token_account_f.key,
                test_f.get_bank(&emode_breaker_bank_mint),
                0.1, // any trivial amount will do
            )
            .await;
        assert!(res.is_ok());
        liquidator_debt_token_account_f
    };

    {
        let collateral_bank_f = test_f.get_bank(&collateral_mint);
        let debt_bank_f = test_f.get_bank(&debt_mint);
        let res = liquidator_mfi_account_f
            .try_liquidate(
                &liquidatee_mfi_account_f,
                collateral_bank_f,
                liquidate_amount * 97.0, // try to liquidate as much as possible
                debt_bank_f,
            )
            .await;
        assert!(res.is_err());
    }

    // Finally we enable emode for liquidator as well. Now that both participants are in emode,
    // liquidation goes through and increases the health of both.
    {
        let res = liquidator_mfi_account_f
            .try_bank_repay(
                liquidator_debt_token_account_f.key,
                test_f.get_bank(&emode_breaker_bank_mint),
                0.0,
                Some(true),
            )
            .await;
        assert!(res.is_ok());
    }

    {
        let collateral_bank_f = test_f.get_bank(&collateral_mint);
        let debt_bank_f = test_f.get_bank(&debt_mint);
        let res = liquidator_mfi_account_f
            .try_liquidate(
                &liquidatee_mfi_account_f,
                collateral_bank_f,
                liquidate_amount * 97.0, // liquidate as much as possible
                debt_bank_f,
            )
            .await;
        assert!(res.is_ok());
    }

    Ok(())
}
