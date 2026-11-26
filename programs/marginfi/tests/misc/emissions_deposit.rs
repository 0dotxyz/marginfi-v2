use fixed::types::I80F48;
use fixtures::{assert_custom_error, bank::BankFixture, native, prelude::*};
use marginfi::{
    assert_eq_with_tolerance, constants::MIN_EMISSIONS_SHARE_SUPPLY, prelude::MarginfiError,
    state::bank::BankImpl,
};
use marginfi_type_crate::constants::{EMISSIONS_STREAM_MAX_SECONDS, EMISSIONS_STREAM_MIN_SECONDS};
use marginfi_type_crate::types::{BankConfigOpt, BankOperationalState};
use solana_program_test::*;
use solana_sdk::pubkey::Pubkey;
use test_case::test_case;

/// Native USDC units a second the streamed tests pay: 50 USDC streams over `STREAM_SECONDS`.
const EMISSIONS_PER_SECOND: u64 = 500;
const STREAM_SECONDS: u64 = 100_000;
/// Seconds a visitor holds in the streamed tests, in which the stream pays 12.5 USDC.
const HOLD_SECONDS: i64 = 25_000;

/// How a test pays same-bank emissions: at once, or streamed over `STREAM_SECONDS`.
#[derive(Clone, Copy)]
enum Payout {
    AtOnce,
    Streamed,
}

async fn try_emissions_deposit(
    bank: &BankFixture,
    payout: Payout,
    amount: u64,
    funding_account: Pubkey,
) -> Result<(), BanksClientError> {
    match payout {
        Payout::AtOnce => bank.try_emissions_deposit(amount, funding_account).await,
        Payout::Streamed => {
            bank.try_emissions_deposit_streamed(amount, STREAM_SECONDS, funding_account)
                .await
        }
    }
}

/// Lets a streamed deposit pay out in full: pins the clock past `STREAM_SECONDS` and accrues.
async fn wait_for_payout(
    test_f: &TestFixture,
    bank: &BankFixture,
    payout: Payout,
) -> anyhow::Result<()> {
    if let Payout::Streamed = payout {
        let now = test_f.get_clock().await.unix_timestamp;
        test_f.pin_clock(now + STREAM_SECONDS as i64, &[]).await;
        test_f.marginfi_group.try_accrue_interest(bank).await?;
    }
    Ok(())
}

#[test_case(Payout::AtOnce; "at once")]
#[test_case(Payout::Streamed; "streamed")]
#[tokio::test]
async fn emissions_deposit_fails_when_bank_paused(payout: Payout) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

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
    let res = try_emissions_deposit(usdc_bank, payout, native!(50, "USDC"), funding.key).await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankPaused);

    Ok(())
}

#[test_case(Payout::AtOnce; "at once")]
#[test_case(Payout::Streamed; "streamed")]
#[tokio::test]
async fn emissions_deposit_fails_when_bank_reduce_only(payout: Payout) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

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
    let res = try_emissions_deposit(usdc_bank, payout, native!(50, "USDC"), funding.key).await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::BankReduceOnly);

    Ok(())
}

#[test_case(Payout::AtOnce; "at once")]
#[test_case(Payout::Streamed; "streamed")]
#[tokio::test]
async fn emissions_deposit_fails_with_nonzero_transfer_fee(payout: Payout) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::T22WithFee,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;

    let t22_bank = test_f.get_bank(&BankMint::T22WithFee);

    let depositor = test_f.create_marginfi_account().await;
    let depositor_t22 = t22_bank.mint.create_token_account_and_mint_to(100).await;
    depositor
        .try_bank_deposit(depositor_t22.key, t22_bank, 50.0, None)
        .await?;

    let funding = t22_bank.mint.create_token_account_and_mint_to(50).await;
    let res = try_emissions_deposit(
        t22_bank,
        payout,
        native!(50, t22_bank.mint.mint.decimals),
        funding.key,
    )
    .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidTransfer);

    Ok(())
}

#[cfg(feature = "transfer-hook")]
#[test_case(Payout::AtOnce; "at once")]
#[test_case(Payout::Streamed; "streamed")]
#[tokio::test]
async fn emissions_deposit_fails_with_transfer_hook(payout: Payout) -> anyhow::Result<()> {
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

    let depositor = test_f.create_marginfi_account().await;
    let depositor_t22 = t22_bank.mint.create_token_account_and_mint_to(100).await;
    depositor
        .try_bank_deposit(depositor_t22.key, t22_bank, 50.0, None)
        .await?;

    let funding = t22_bank.mint.create_token_account_and_mint_to(50).await;
    let res = try_emissions_deposit(t22_bank, payout, native!(50, "USDC"), funding.key).await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidTransfer);

    Ok(())
}

#[test_case(Payout::AtOnce; "at once")]
#[test_case(Payout::Streamed; "streamed")]
#[tokio::test]
async fn emissions_deposit_succeeds_with_inactive_t22_extensions(
    payout: Payout,
) -> anyhow::Result<()> {
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
    try_emissions_deposit(
        t22_bank,
        payout,
        native!(emissions_deposit, "USDC"),
        funding.key,
    )
    .await?;
    wait_for_payout(&test_f, t22_bank, payout).await?;

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
        .try_emissions_deposit_with_mint(
            native!(emissions_deposit, "USDC"),
            emissions_funding.key,
            emissions_mint_fixture.key,
        )
        .await;

    assert_custom_error!(res.unwrap_err(), MarginfiError::InvalidEmissionsMint);

    Ok(())
}

/// A donation moves `asset_share_value` by `amount / total_asset_shares`, so banks below
/// `MIN_EMISSIONS_SHARE_SUPPLY` do not accept one.
#[test_case(Payout::AtOnce; "at once")]
#[test_case(Payout::Streamed; "streamed")]
#[tokio::test]
async fn emissions_deposit_fails_below_minimum_share_supply(payout: Payout) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let depositor = test_f.create_marginfi_account().await;
    let depositor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    let below_floor = MIN_EMISSIONS_SHARE_SUPPLY.to_num::<u64>() - 1;
    depositor
        .try_bank_deposit(
            depositor_usdc.key,
            usdc_bank,
            below_floor as f64 / 10f64.powi(6),
            None,
        )
        .await?;

    let funding = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    let res = try_emissions_deposit(usdc_bank, payout, native!(10, "USDC"), funding.key).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::EmissionsUpdateError);

    // Rejected before the transfer, so nothing moved.
    let bank = usdc_bank.load().await;
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::ONE);
    assert_eq!(
        I80F48::from(bank.total_asset_shares),
        I80F48::from_num(below_floor)
    );

    Ok(())
}

/// The floor is inclusive: a bank sitting exactly on it accepts emissions normally.
#[test_case(Payout::AtOnce; "at once")]
#[test_case(Payout::Streamed; "streamed")]
#[tokio::test]
async fn emissions_deposit_succeeds_at_minimum_share_supply(payout: Payout) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let depositor = test_f.create_marginfi_account().await;
    let depositor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    let at_floor = MIN_EMISSIONS_SHARE_SUPPLY.to_num::<u64>();
    depositor
        .try_bank_deposit(
            depositor_usdc.key,
            usdc_bank,
            at_floor as f64 / 10f64.powi(6),
            None,
        )
        .await?;

    let bank = usdc_bank.load().await;
    assert_eq!(
        I80F48::from(bank.total_asset_shares),
        MIN_EMISSIONS_SHARE_SUPPLY
    );

    let funding = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    let donation = native!(10, "USDC");
    try_emissions_deposit(usdc_bank, payout, donation, funding.key).await?;
    wait_for_payout(&test_f, usdc_bank, payout).await?;

    // Share value rises by donation / supply, and the supply is unchanged.
    let bank = usdc_bank.load().await;
    assert_eq!(
        I80F48::from(bank.total_asset_shares),
        MIN_EMISSIONS_SHARE_SUPPLY
    );
    assert_eq!(
        I80F48::from(bank.asset_share_value),
        I80F48::ONE + I80F48::from_num(donation) / MIN_EMISSIONS_SHARE_SUPPLY
    );

    Ok(())
}

/// A USDC bank holding a 100 USDC deposit, and the second the deposit was made.
async fn usdc_bank_with_a_holder() -> anyhow::Result<(TestFixture, i64)> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let holder = test_f.create_marginfi_account().await;
    let holder_usdc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    holder
        .try_bank_deposit(holder_usdc.key, usdc_bank, 100, None)
        .await?;
    let now = test_f.get_clock().await.unix_timestamp;
    Ok((test_f, now))
}

/// A visitor deposits 100 USDC beside the holder's 100 just before a 50 USDC emissions deposit
/// and withdraws in the same second. Paid at once, the visitor takes half of it; streamed, none.
#[test_case(Payout::AtOnce, native!(125, "USDC"); "at once")]
#[test_case(Payout::Streamed, native!(100, "USDC"); "streamed")]
#[tokio::test]
async fn a_deposit_withdrawn_in_the_second_of_an_emissions_deposit_takes_a_share_only_when_paid_at_once(
    payout: Payout,
    withdrawn: u64,
) -> anyhow::Result<()> {
    let (test_f, _) = usdc_bank_with_a_holder().await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let visitor = test_f.create_marginfi_account().await;
    let visitor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    visitor
        .try_bank_deposit(visitor_usdc.key, usdc_bank, 100, None)
        .await?;
    let funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    try_emissions_deposit(usdc_bank, payout, native!(50, "USDC"), funding.key).await?;
    visitor
        .try_bank_withdraw(visitor_usdc.key, usdc_bank, 0, Some(true))
        .await?;

    assert_eq!(visitor_usdc.balance().await, withdrawn);
    Ok(())
}

/// A visitor deposits 100 USDC beside the holder's 100 after a 50 USDC emissions deposit. Paid at
/// once it earns none (its shares round down a unit); streamed, half of 12.5 over `HOLD_SECONDS`.
#[test_case(Payout::AtOnce, native!(99.999999, "USDC", f64); "at once")]
#[test_case(Payout::Streamed, native!(106.25, "USDC", f64); "streamed")]
#[tokio::test]
async fn a_deposit_made_after_an_emissions_deposit_earns_from_it_only_when_streamed(
    payout: Payout,
    withdrawn: u64,
) -> anyhow::Result<()> {
    let (test_f, now) = usdc_bank_with_a_holder().await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    try_emissions_deposit(usdc_bank, payout, native!(50, "USDC"), funding.key).await?;
    let visitor = test_f.create_marginfi_account().await;
    let visitor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(100).await;
    visitor
        .try_bank_deposit(visitor_usdc.key, usdc_bank, 100, None)
        .await?;
    test_f
        .pin_clock(now + HOLD_SECONDS, &[PYTH_USDC_FEED, PYTH_SOL_FEED])
        .await;
    visitor
        .try_bank_withdraw(visitor_usdc.key, usdc_bank, 0, Some(true))
        .await?;

    assert_eq!(visitor_usdc.balance().await, withdrawn);
    Ok(())
}

/// Over its period, a streamed deposit of half the supply lifts every deposit by half.
#[tokio::test]
async fn a_streamed_emissions_deposit_pays_depositors_pro_rata_over_its_period(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let depositor_a = test_f.create_marginfi_account().await;
    let depositor_b = test_f.create_marginfi_account().await;
    let depositor_a_usdc = test_f.usdc_mint.create_token_account_and_mint_to(40).await;
    let depositor_b_usdc = test_f.usdc_mint.create_token_account_and_mint_to(60).await;
    depositor_a
        .try_bank_deposit(depositor_a_usdc.key, usdc_bank, 40, None)
        .await?;
    depositor_b
        .try_bank_deposit(depositor_b_usdc.key, usdc_bank, 60, None)
        .await?;

    let emissions_funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    try_emissions_deposit(
        usdc_bank,
        Payout::Streamed,
        native!(50, "USDC"),
        emissions_funding.key,
    )
    .await?;
    let bank = usdc_bank.load().await;
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::ONE);
    assert_eq!(
        I80F48::from(bank.emissions_stream_pending),
        I80F48::from_num(native!(50, "USDC"))
    );
    assert_eq!(bank.emissions_stream_seconds, STREAM_SECONDS);

    wait_for_payout(&test_f, usdc_bank, Payout::Streamed).await?;
    let bank = usdc_bank.load().await;
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::from_num(1.5));
    assert_eq!(I80F48::from(bank.emissions_stream_pending), I80F48::ZERO);
    assert_eq!(bank.emissions_stream_seconds, 0);
    for (depositor, expected) in [
        (&depositor_a, native!(60, "USDC")),
        (&depositor_b, native!(90, "USDC")),
    ] {
        let shares = depositor
            .load()
            .await
            .lending_account
            .balances
            .iter()
            .find(|b| b.is_active() && b.bank_pk == usdc_bank.key)
            .unwrap()
            .asset_shares;
        assert_eq!(
            bank.get_asset_amount(I80F48::from(shares))?,
            I80F48::from_num(expected)
        );
    }

    Ok(())
}

/// The period runs from `EMISSIONS_STREAM_MIN_SECONDS` to `EMISSIONS_STREAM_MAX_SECONDS`. Out of
/// range, the deposit fails before any token moves.
#[test_case(EMISSIONS_STREAM_MIN_SECONDS - 1, false)]
#[test_case(EMISSIONS_STREAM_MIN_SECONDS, true)]
#[test_case(EMISSIONS_STREAM_MAX_SECONDS, true)]
#[test_case(EMISSIONS_STREAM_MAX_SECONDS + 1, false)]
#[tokio::test]
async fn a_streamed_emissions_deposit_takes_a_period_from_the_minimum_to_the_maximum(
    seconds: u64,
    accepted: bool,
) -> anyhow::Result<()> {
    let (test_f, _) = usdc_bank_with_a_holder().await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let funding = test_f.usdc_mint.create_token_account_and_mint_to(10).await;
    let amount = native!(10, "USDC");
    let res = usdc_bank
        .try_emissions_deposit_streamed(amount, seconds, funding.key)
        .await;

    let bank = usdc_bank.load().await;
    if accepted {
        res?;
        assert_eq!(
            I80F48::from(bank.emissions_stream_pending),
            I80F48::from_num(amount)
        );
        assert_eq!(bank.emissions_stream_seconds, seconds);
    } else {
        assert_custom_error!(res.unwrap_err(), MarginfiError::EmissionsUpdateError);
        assert_eq!(funding.balance().await, amount);
        assert_eq!(I80F48::from(bank.emissions_stream_pending), I80F48::ZERO);
        assert_eq!(bank.emissions_stream_seconds, 0);
    }
    Ok(())
}

/// 30 USDC over 300,000 seconds has 20 left over 200,000 seconds after 100,000. Adding 20 USDC over
/// 600,000, half the stream, moves the seconds left halfway: 40 USDC over 400,000.
#[tokio::test]
async fn a_streamed_deposit_into_a_running_stream_moves_its_seconds_left_by_its_share(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // 2^27 native units of supply, so every payment below lands exactly in the share value.
    let supply = 1u64 << 27;
    let depositor = test_f.create_marginfi_account().await;
    let depositor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(200).await;
    depositor
        .try_bank_deposit(depositor_usdc.key, usdc_bank, supply as f64 / 1e6, None)
        .await?;
    let funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    usdc_bank
        .try_emissions_deposit_streamed(native!(30, "USDC"), 300_000, funding.key)
        .await?;

    let now = test_f.get_clock().await.unix_timestamp;
    test_f.pin_clock(now + 100_000, &[]).await;
    usdc_bank
        .try_emissions_deposit_streamed(native!(20, "USDC"), 600_000, funding.key)
        .await?;
    let paid = I80F48::from_num(native!(10, "USDC")) / I80F48::from_num(supply);
    let bank = usdc_bank.load().await;
    assert_eq!(
        I80F48::from(bank.emissions_stream_pending),
        I80F48::from_num(native!(40, "USDC"))
    );
    assert_eq!(bank.emissions_stream_seconds, 400_000);
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::ONE + paid);

    test_f.pin_clock(now + 500_000, &[]).await;
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    let bank = usdc_bank.load().await;
    assert_eq!(I80F48::from(bank.emissions_stream_pending), I80F48::ZERO);
    assert_eq!(bank.emissions_stream_seconds, 0);
    assert_eq!(
        I80F48::from(bank.asset_share_value),
        I80F48::ONE + paid + I80F48::from_num(native!(40, "USDC")) / I80F48::from_num(supply)
    );
    Ok(())
}

/// A visitor holding half the bank adds one native unit over the shortest period to pull a 50 USDC
/// stream forward. The stream keeps its seconds, so the visitor earns 250 units and loses the unit.
#[tokio::test]
async fn a_visitor_cannot_pull_a_stream_forward_with_a_tiny_short_deposit() -> anyhow::Result<()> {
    let (test_f, now) = usdc_bank_with_a_holder().await?;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    let funding = test_f.usdc_mint.create_token_account_and_mint_to(50).await;
    try_emissions_deposit(
        usdc_bank,
        Payout::Streamed,
        native!(50, "USDC"),
        funding.key,
    )
    .await?;
    let visitor = test_f.create_marginfi_account().await;
    let visitor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(101).await;
    visitor
        .try_bank_deposit(visitor_usdc.key, usdc_bank, 100, None)
        .await?;
    usdc_bank
        .try_emissions_deposit_streamed(1, EMISSIONS_STREAM_MIN_SECONDS, visitor_usdc.key)
        .await?;
    assert_eq!(
        usdc_bank.load().await.emissions_stream_seconds,
        STREAM_SECONDS
    );

    test_f
        .pin_clock(now + 1, &[PYTH_USDC_FEED, PYTH_SOL_FEED])
        .await;
    visitor
        .try_bank_withdraw(visitor_usdc.key, usdc_bank, 0, Some(true))
        .await?;

    assert_eq!(
        usdc_bank.load().await.emissions_stream_seconds,
        STREAM_SECONDS - 1
    );
    assert_eq!(
        visitor_usdc.balance().await,
        native!(101, "USDC") - 1 + EMISSIONS_PER_SECOND / 2
    );
    Ok(())
}

/// The stream holds while the share supply is under `MIN_EMISSIONS_SHARE_SUPPLY`: it pays nothing
/// and spends no seconds, and resumes once the supply is back on the floor.
#[tokio::test]
async fn the_emissions_stream_waits_while_the_supply_is_under_the_minimum() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let feeds = [PYTH_USDC_FEED, PYTH_SOL_FEED];

    let floor = MIN_EMISSIONS_SHARE_SUPPLY.to_num::<u64>();
    let depositor = test_f.create_marginfi_account().await;
    let depositor_usdc = test_f.usdc_mint.create_token_account_and_mint_to(10).await;
    depositor
        .try_bank_deposit(depositor_usdc.key, usdc_bank, floor as f64 / 1e6, None)
        .await?;
    let funding = test_f.usdc_mint.create_token_account_and_mint_to(10).await;
    let donation = native!(10, "USDC");
    let seconds = STREAM_SECONDS;
    usdc_bank
        .try_emissions_deposit_streamed(donation, seconds, funding.key)
        .await?;
    depositor
        .try_bank_withdraw(depositor_usdc.key, usdc_bank, 0.000_001, None)
        .await?;

    let now = test_f.get_clock().await.unix_timestamp;
    test_f.pin_clock(now + 1_000, &feeds).await;
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    let bank = usdc_bank.load().await;
    assert_eq!(I80F48::from(bank.asset_share_value), I80F48::ONE);
    assert_eq!(
        I80F48::from(bank.emissions_stream_pending),
        I80F48::from_num(donation)
    );
    assert_eq!(bank.emissions_stream_seconds, seconds);

    depositor
        .try_bank_deposit(depositor_usdc.key, usdc_bank, 0.000_001, None)
        .await?;
    test_f.pin_clock(now + 2_000, &feeds).await;
    test_f.marginfi_group.try_accrue_interest(usdc_bank).await?;
    let paid = 1_000 * (donation / seconds);
    let bank = usdc_bank.load().await;
    assert_eq!(
        I80F48::from(bank.asset_share_value),
        I80F48::ONE + I80F48::from_num(paid) / MIN_EMISSIONS_SHARE_SUPPLY
    );
    assert_eq!(
        I80F48::from(bank.emissions_stream_pending),
        I80F48::from_num(donation - paid)
    );
    assert_eq!(bank.emissions_stream_seconds, seconds - 1_000);
    Ok(())
}
