use anchor_lang::prelude::Clock;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_eq_noise, native, prelude::*};
use marginfi::state::{
    bank::BankImpl, bank_cache::ComputedInterestRates, interest_rate::InterestRateConfigImpl,
};
use marginfi_type_crate::types::BankVaultType;
use marginfi_type_crate::types::{
    centi_to_u32, make_points, milli_to_u32, Bank, BankConfig, InterestRateConfig, MarginfiGroup,
    RatePoint,
};
use pretty_assertions::assert_eq;
use solana_program_test::*;

#[tokio::test]
async fn marginfi_group_accrue_interest_rates_success_1() -> anyhow::Result<()> {
    let interest_rate_config = InterestRateConfig {
        zero_util_rate: 0,
        points: make_points(&vec![RatePoint::new(
            centi_to_u32(I80F48!(0.9)),
            milli_to_u32(I80F48!(0.9)),
        )]),
        // Note: hundred_util_rate - doesn't matter, we are testing within a point
        ..*DEFAULT_TEST_BANK_INTEREST_RATE_CONFIG
    };
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config,
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
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

    let lender_mfi_account_f = test_f.create_marginfi_account().await;
    let lender_token_account_usdc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 100, None)
        .await?;

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 999, None)
        .await?;
    let borrower_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc.key, usdc_bank_f, 90)
        .await?;

    let time_delta = 365 * 24 * 60 * 60;
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        // Advance clock by 1 year
        clock.unix_timestamp += time_delta;
        ctx.set_sysvar(&clock);
    }

    test_f
        .marginfi_group
        .try_accrue_interest(usdc_bank_f)
        .await?;

    let borrower_mfi_account = borrower_mfi_account_f.load().await;

    // Due to balances sorting, USDC may be not at index 1 -> determine its actual index first
    let usdc_index = borrower_mfi_account
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == usdc_bank_f.key)
        .unwrap();

    let borrower_bank_account = borrower_mfi_account.lending_account.balances[usdc_index];
    let usdc_bank: Bank = usdc_bank_f.load().await;
    let liabilities =
        usdc_bank.get_liability_amount(borrower_bank_account.liability_shares.into())?;

    let lender_mfi_account = lender_mfi_account_f.load().await;
    let lender_bank_account = lender_mfi_account.lending_account.balances[0];
    let assets = usdc_bank.get_asset_amount(lender_bank_account.asset_shares.into())?;

    assert_eq_noise!(
        liabilities,
        I80F48::from(native!(171, "USDC")),
        I80F48!(100)
    );
    assert_eq_noise!(assets, I80F48::from(native!(181, "USDC")), I80F48!(100));
    assert_eq!(usdc_bank.cache.interest_accumulated_for, time_delta as u32,);
    assert_eq_noise!(
        I80F48::from(usdc_bank.cache.accumulated_since_last_update),
        I80F48::from(native!(81, "USDC")),
        I80F48!(100)
    );

    let total_liabilities =
        usdc_bank.get_liability_amount(usdc_bank.total_liability_shares.into())?;
    let total_assets = usdc_bank.get_asset_amount(usdc_bank.total_asset_shares.into())?;
    let ur = total_liabilities.checked_div(total_assets).unwrap();

    let ComputedInterestRates {
        base_rate_apr,
        lending_rate_apr,
        borrowing_rate_apr,
        ..
    } = interest_rate_config
        .create_interest_rate_calculator(&MarginfiGroup::default())
        .calc_interest_rate(ur)
        .unwrap();

    assert_eq!(usdc_bank.cache.base_rate, milli_to_u32(base_rate_apr));
    assert_eq!(
        usdc_bank.cache.borrowing_rate,
        milli_to_u32(borrowing_rate_apr)
    );
    assert_eq!(usdc_bank.cache.lending_rate, milli_to_u32(lending_rate_apr));

    Ok(())
}

#[tokio::test]
async fn marginfi_group_accrue_interest_rates_success_2() -> anyhow::Result<()> {
    let protocol_fixed_fee_apr: I80F48 = I80F48!(0.01);
    let insurance_fee_fixed_apr: I80F48 = I80F48!(0.01);
    let interest_rate_config = InterestRateConfig {
        protocol_fixed_fee_apr: protocol_fixed_fee_apr.into(),
        insurance_fee_fixed_apr: insurance_fee_fixed_apr.into(),
        zero_util_rate: 0,
        points: make_points(&vec![RatePoint::new(
            centi_to_u32(I80F48!(0.9)),
            milli_to_u32(I80F48!(1)),
        )]),
        // Note: hundred_util_rate - doesn't matter, we are testing within a point
        ..*DEFAULT_TEST_BANK_INTEREST_RATE_CONFIG
    };
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    deposit_limit: native!(1_000_000_000, "USDC"),
                    interest_rate_config,
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    deposit_limit: native!(200_000_000, "SOL"),
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
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(
            lender_token_account_usdc.key,
            usdc_bank_f,
            100_000_000,
            None,
        )
        .await?;

    let borrower_mfi_account_f = test_f.create_marginfi_account().await;
    let borrower_token_account_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(10_000_000)
        .await;
    borrower_mfi_account_f
        .try_bank_deposit(borrower_token_account_sol.key, sol_bank_f, 10_000_000, None)
        .await?;
    let borrower_token_account_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower_mfi_account_f
        .try_bank_borrow(borrower_token_account_usdc.key, usdc_bank_f, 90_000_000)
        .await?;

    // Advance clock by 1 minute
    let time_delta = 60;
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await?;
        clock.unix_timestamp += time_delta;
        ctx.set_sysvar(&clock);
    }

    test_f
        .marginfi_group
        .try_accrue_interest(usdc_bank_f)
        .await?;

    // The program fee ata needs to exist, but doesn't need any assets.
    {
        let ctx = test_f.context.clone();
        let ata = TokenAccountFixture::new_from_ata(
            ctx,
            &test_f.usdc_mint.key,
            &test_f.marginfi_group.fee_wallet,
            &test_f.usdc_mint.token_program,
        )
        .await;
        let ata_expected = get_associated_token_address_with_program_id(
            &test_f.marginfi_group.fee_wallet,
            &test_f.usdc_mint.key,
            &test_f.usdc_mint.token_program,
        );
        assert_eq!(ata.key, ata_expected);
    }

    test_f.marginfi_group.try_collect_fees(usdc_bank_f).await?;

    let borrower_mfi_account = borrower_mfi_account_f.load().await;
    // Due to balances sorting, USDC may be not at index 0 -> determine its actual index first
    let usdc_index = borrower_mfi_account
        .lending_account
        .balances
        .iter()
        .position(|b| b.is_active() && b.bank_pk == usdc_bank_f.key)
        .unwrap();

    let borrower_bank_account = borrower_mfi_account.lending_account.balances[usdc_index];
    let usdc_bank = usdc_bank_f.load().await;
    let liabilities =
        usdc_bank.get_liability_amount(borrower_bank_account.liability_shares.into())?;

    let lender_mfi_account = lender_mfi_account_f.load().await;
    let lender_bank_account = lender_mfi_account.lending_account.balances[0];
    let assets = usdc_bank.get_asset_amount(lender_bank_account.asset_shares.into())?;

    assert_eq_noise!(liabilities, I80F48!(90000174657530), I80F48!(10));
    assert_eq_noise!(assets, I80F48!(100000171232876), I80F48!(10));

    let protocol_fees = usdc_bank_f
        .get_vault_token_account(BankVaultType::Fee)
        .await;
    let insurance_fees = usdc_bank_f
        .get_vault_token_account(BankVaultType::Insurance)
        .await;

    assert_eq!(protocol_fees.balance().await, 1712328);
    assert_eq!(insurance_fees.balance().await, 1712328);
    assert_eq!(usdc_bank.cache.interest_accumulated_for, time_delta as u32,);
    assert_eq_noise!(
        I80F48::from(usdc_bank.cache.accumulated_since_last_update),
        I80F48!(171232876),
        I80F48!(1)
    );

    let total_liabilities =
        usdc_bank.get_liability_amount(usdc_bank.total_liability_shares.into())?;
    let total_assets = usdc_bank.get_asset_amount(usdc_bank.total_asset_shares.into())?;
    let ur = total_liabilities.checked_div(total_assets).unwrap();

    let ComputedInterestRates {
        base_rate_apr,
        lending_rate_apr,
        borrowing_rate_apr,
        ..
    } = interest_rate_config
        .create_interest_rate_calculator(&MarginfiGroup::default())
        .calc_interest_rate(ur)
        .unwrap();

    assert_eq!(usdc_bank.cache.base_rate, milli_to_u32(base_rate_apr));
    assert_eq!(
        usdc_bank.cache.borrowing_rate,
        milli_to_u32(borrowing_rate_apr)
    );
    assert_eq!(usdc_bank.cache.lending_rate, milli_to_u32(lending_rate_apr));

    Ok(())
}
