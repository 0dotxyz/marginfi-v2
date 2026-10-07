//! The rate readings a bank records as it is priced: when one is taken, and what it holds.

use drift_mocks::{constants::SPOT_CUMULATIVE_INTEREST_PRECISION, state::MinimalSpotMarket};
use fixed::types::I80F48;
use fixtures::bank::BankFixture;
use fixtures::prelude::*;
use juplend_mocks::state::{Lending as JuplendLending, EXCHANGE_PRICES_PRECISION};
use marginfi::constants::MIN_EMISSIONS_SHARE_SUPPLY;
use marginfi_type_crate::constants::BANK_RATE_READING_SPACING_SECONDS;
use marginfi_type_crate::types::RateReading;
use solana_program_test::tokio;
use solana_sdk::account::AccountSharedData;

/// Pin the clock and republish the feeds the native banks here price from.
async fn pin_clock(test_f: &TestFixture, ts: i64) {
    test_f.pin_clock(ts, &[PYTH_USDC_FEED, PYTH_SOL_FEED]).await;
}

/// A native bank has no venue multiplier, so its reading is its share values alone.
async fn assert_newest_reading_is_the_share_values(bank_f: &BankFixture, ts: i64) {
    let bank = bank_f.load().await;
    assert_eq!(
        *bank.newest_rate_reading().unwrap(),
        RateReading::new(
            bank.asset_share_value.into(),
            bank.liability_share_value.into(),
            ts
        )
        .unwrap()
    );
}

#[tokio::test]
async fn readings_inside_the_spacing_are_not_recorded() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc = test_f.get_bank(&BankMint::Usdc);
    let group = &test_f.marginfi_group;

    pin_clock(&test_f, BASE_TS).await;
    group.try_pulse_bank_price_cache(usdc).await?;
    assert_eq!(usdc.load().await.recorded_rate_readings().count(), 1);

    let mut now = BASE_TS + BANK_RATE_READING_SPACING_SECONDS - 1;
    pin_clock(&test_f, now).await;
    group.try_pulse_bank_price_cache(usdc).await?;
    assert_eq!(usdc.load().await.recorded_rate_readings().count(), 1);

    now += 1;
    pin_clock(&test_f, now).await;
    group.try_pulse_bank_price_cache(usdc).await?;
    assert_eq!(usdc.load().await.recorded_rate_readings().count(), 2);
    assert_newest_reading_is_the_share_values(usdc, now).await;
    Ok(())
}

/// The pulse is not the only writer: any instruction that prices the bank takes a reading.
#[tokio::test]
async fn a_borrow_and_a_withdraw_take_readings() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc = test_f.get_bank(&BankMint::Usdc);
    let sol = test_f.get_bank(&BankMint::Sol);
    pin_clock(&test_f, BASE_TS).await;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc, 1_000.0, None)
        .await?;
    // A deposit prices nothing.
    assert_eq!(usdc.load().await.recorded_rate_readings().count(), 0);

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(100.0)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol, 100.0, None)
        .await?;
    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc, 100.0)
        .await?;
    assert_eq!(usdc.load().await.recorded_rate_readings().count(), 1);
    assert_newest_reading_is_the_share_values(usdc, BASE_TS).await;

    let now = BASE_TS + BANK_RATE_READING_SPACING_SECONDS;
    pin_clock(&test_f, now).await;
    lender
        .try_bank_withdraw(lender_usdc.key, usdc, 10.0, None)
        .await?;
    assert_eq!(usdc.load().await.recorded_rate_readings().count(), 2);
    assert_newest_reading_is_the_share_values(usdc, now).await;
    Ok(())
}

/// A pulse while the protocol is paused leaves the bank unaccrued, so it prices the bank without
/// taking a reading.
#[tokio::test]
async fn a_pulse_while_paused_takes_no_reading() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc = test_f.get_bank(&BankMint::Usdc);
    let group = &test_f.marginfi_group;

    pin_clock(&test_f, BASE_TS).await;
    group.try_pulse_bank_price_cache(usdc).await?;
    assert_eq!(usdc.load().await.recorded_rate_readings().count(), 1);

    group.try_panic_pause().await?;
    group.try_propagate_fee_state().await?;

    let now = BASE_TS + BANK_RATE_READING_SPACING_SECONDS;
    pin_clock(&test_f, now).await;
    group.try_pulse_bank_price_cache(usdc).await?;
    let bank = usdc.load().await;
    assert_eq!(bank.recorded_rate_readings().count(), 1);
    assert_eq!(bank.cache.last_oracle_price_timestamp, now);
    Ok(())
}

/// A share value driven past what a reading encodes leaves the ring as it was, and the instruction
/// pricing the bank still lands.
#[tokio::test]
async fn an_index_past_the_reading_range_is_skipped_without_blocking_pricing() -> anyhow::Result<()>
{
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc = test_f.get_bank(&BankMint::Usdc);
    let group = &test_f.marginfi_group;
    pin_clock(&test_f, BASE_TS).await;

    // The smallest share supply that accepts emissions, where a donation moves the share value most.
    let supply: u64 = MIN_EMISSIONS_SHARE_SUPPLY.to_num();
    let depositor = test_f.create_marginfi_account().await;
    let depositor_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(supply as f64 / 1_000_000.0)
        .await;
    depositor
        .try_bank_deposit(depositor_usdc.key, usdc, supply as f64 / 1_000_000.0, None)
        .await?;
    group.try_pulse_bank_price_cache(usdc).await?;
    assert_eq!(usdc.load().await.recorded_rate_readings().count(), 1);

    // 2^32 more per share, which lifts the share value to one past the largest encodable index.
    let donation = supply << 32;
    let funding = test_f
        .usdc_mint
        .create_token_account_and_mint_to(5_000_000_000.0)
        .await;
    usdc.try_emissions_deposit(donation, funding.key).await?;

    let now = BASE_TS + BANK_RATE_READING_SPACING_SECONDS;
    pin_clock(&test_f, now).await;
    group.try_pulse_bank_price_cache(usdc).await?;

    let bank = usdc.load().await;
    assert_eq!(
        I80F48::from(bank.asset_share_value),
        I80F48::from_num(u32::MAX) + I80F48::from_num(2)
    );
    assert_eq!(bank.recorded_rate_readings().count(), 1);
    assert_eq!(bank.newest_rate_reading().unwrap().timestamp, BASE_TS);
    assert_eq!(bank.cache.last_oracle_price_timestamp, now);
    Ok(())
}

/// The Drift and JupLend fixtures boot at timestamp 0, which a reading treats as never written.
/// Their venue state is stale once it falls behind the clock, so the mocks are stamped to match.
const VENUE_READING_TS: i64 = 1;

async fn start_clock(test_f: &TestFixture) {
    let slot = test_f.get_clock().await.slot;
    test_f.set_clock(slot, VENUE_READING_TS).await;
}

/// Assert the newest reading on `bank_f` carries the venue's own exchange rate. A native bank's
/// multiplier is 1 and cannot distinguish the two; every integration can.
async fn assert_venue_reading_carries_multiplier(
    test_f: &TestFixture,
    bank_f: &BankFixture,
    multiplier: I80F48,
) {
    assert_ne!(
        multiplier,
        I80F48::ONE,
        "the venue should price its position away from 1, or this proves nothing"
    );
    let bank = bank_f.load().await;
    let reading = bank
        .newest_rate_reading()
        .expect("pricing the bank should have taken a reading");
    let expected = RateReading::new(
        I80F48::from(bank.asset_share_value) * multiplier,
        I80F48::from(bank.liability_share_value) * multiplier,
        test_f.get_clock().await.unix_timestamp,
    )
    .unwrap();
    assert_eq!(*reading, expected);
}

#[tokio::test]
async fn a_kamino_bank_reads_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;
    setup
        .test_f
        .marginfi_group
        .try_pulse_bank_price_cache(&setup.bank_f)
        .await?;

    // klend's collateral exchange rate: liquidity per collateral token.
    let (total_liq, total_col) = setup.load_reserve().await.scaled_supplies()?;
    assert_venue_reading_carries_multiplier(&setup.test_f, &setup.bank_f, total_liq / total_col)
        .await;
    Ok(())
}

#[tokio::test]
async fn a_drift_bank_reads_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_drift_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_drift_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;

    // The mock market boots with no accrued interest, so its multiplier is exactly 1. Advance it,
    // as the drift deposit/withdraw tests do, so the reading has something to carry.
    {
        let spot_market_key = setup.bank_f.load().await.integration_acc_1;
        let mut account = setup.test_f.try_load(&spot_market_key).await?.unwrap();
        let spot_market = bytemuck::from_bytes_mut::<MinimalSpotMarket>(
            &mut account.data[8..8 + std::mem::size_of::<MinimalSpotMarket>()],
        );
        spot_market.cumulative_deposit_interest =
            (SPOT_CUMULATIVE_INTEREST_PRECISION * 3 / 2).to_le_bytes();
        spot_market.last_interest_ts = VENUE_READING_TS as u64;
        setup
            .test_f
            .context
            .borrow_mut()
            .set_account(&spot_market_key, &AccountSharedData::from(account));
    }
    start_clock(&setup.test_f).await;
    setup
        .test_f
        .marginfi_group
        .try_pulse_bank_price_cache(&setup.bank_f)
        .await?;

    // Drift's scaled balances grow by the market's cumulative deposit interest.
    let cumulative =
        u128::from_le_bytes(setup.load_spot_market().await.cumulative_deposit_interest);
    let multiplier =
        I80F48::from_num(cumulative) / I80F48::from_num(SPOT_CUMULATIVE_INTEREST_PRECISION);
    assert_venue_reading_carries_multiplier(&setup.test_f, &setup.bank_f, multiplier).await;
    Ok(())
}

#[tokio::test]
async fn a_juplend_bank_reads_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_juplend_bank(None).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;

    // The mock lending state boots at parity, so advance its exchange price the way the juplend
    // withdraw tests do, leaving the multiplier something the reading must actually carry.
    {
        let mut account = setup.test_f.try_load(&setup.lending).await?.unwrap();
        let lending = bytemuck::from_bytes_mut::<JuplendLending>(
            &mut account.data[8..8 + std::mem::size_of::<JuplendLending>()],
        );
        lending.token_exchange_price = (EXCHANGE_PRICES_PRECISION * 3 / 2) as u64;
        lending.last_update_timestamp = VENUE_READING_TS as u64;
        setup
            .test_f
            .context
            .borrow_mut()
            .set_account(&setup.lending, &AccountSharedData::from(account));
    }
    start_clock(&setup.test_f).await;
    setup
        .test_f
        .marginfi_group
        .try_pulse_bank_price_cache(&setup.bank_f)
        .await?;

    // JupLend's fToken exchange price, which the liquidity layer advances as it earns.
    let multiplier = I80F48::from_num(setup.load_lending().await.token_exchange_price)
        / I80F48::from_num(EXCHANGE_PRICES_PRECISION);
    assert_venue_reading_carries_multiplier(&setup.test_f, &setup.bank_f, multiplier).await;
    Ok(())
}
