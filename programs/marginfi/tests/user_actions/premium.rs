//! Variable-borrow premium: accrual, snapshots, repay settlement, crank, sweep.
//!
//! Most tests use a ZERO-interest USDC bank so all liability growth is premium and the math
//! can be asserted exactly (within u32 rate-encoding tolerance).

use anchor_lang::prelude::Clock;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::{assert_custom_error, assert_eq_noise, native, prelude::*};
use marginfi::state::bank::BankImpl;
use marginfi_type_crate::types::{
    make_points, milli_to_u32, u32_to_milli, Balance, BankConfig, BankConfigOpt,
    InterestRateConfig, MarginfiAccount, PremiumEntry,
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

const TAG_STABLE: u16 = 100;
const TAG_SOL: u16 = 200;
const YEAR: i64 = 365 * 24 * 60 * 60;

fn entry(collateral_tag: u16, liability_tag: u16, percent: f64) -> PremiumEntry {
    PremiumEntry {
        collateral_tag,
        liability_tag,
        rate: milli_to_u32(I80F48::from_num(percent / 100.0)),
    }
}

fn zero_interest_config() -> InterestRateConfig {
    InterestRateConfig {
        zero_util_rate: milli_to_u32(I80F48!(0)),
        hundred_util_rate: milli_to_u32(I80F48!(0)),
        points: make_points(&[]),
        ..*DEFAULT_TEST_BANK_INTEREST_RATE_CONFIG
    }
}

/// USDC (borrowed, tag stable, zero base interest) + SOL (collateral, tag sol), pair
/// (sol -> stable) = 1% APR, no cap.
async fn premium_test_fixture() -> TestFixture {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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

    // Genesis starts at unix_timestamp 0; move to a realistic epoch so balance timestamps are
    // nonzero (a `last_update == 0` reads as uninitialized to the premium engine, by design).
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await
        .unwrap();
    group_f
        .try_configure_bank_premium(test_f.get_bank(&BankMint::Usdc), TAG_STABLE, true)
        .await
        .unwrap();
    group_f
        .try_configure_bank_premium(test_f.get_bank(&BankMint::Sol), TAG_SOL, true)
        .await
        .unwrap();

    test_f
}

/// A lender supplying USDC liquidity + a borrower with SOL collateral borrowing USDC.
/// Returns (lender, borrower, borrower's USDC token account key).
async fn setup_borrower(
    test_f: &TestFixture,
    usdc_borrowed: f64,
) -> (MarginfiAccountFixture, MarginfiAccountFixture, Pubkey) {
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await
        .unwrap();

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 999, None)
        .await
        .unwrap();
    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc_bank_f, usdc_borrowed)
        .await
        .unwrap();

    (lender, borrower, borrower_usdc.key)
}

async fn advance_clock(test_f: &TestFixture, seconds: i64) {
    advance_clock_with_feeds(
        test_f,
        seconds,
        &[PYTH_USDC_FEED, PYTH_SOL_FEED, PYTH_SOL_EQUIVALENT_FEED],
    )
    .await;
}

async fn advance_clock_with_feeds(test_f: &TestFixture, seconds: i64, feeds: &[Pubkey]) {
    let new_timestamp = {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await.unwrap();
        clock.unix_timestamp += seconds;
        ctx.set_sysvar(&clock);
        clock.unix_timestamp
    };
    // Keep the mock oracles fresh so health checks (and the premium crank) don't hit staleness
    for feed in feeds {
        test_f.set_pyth_oracle_timestamp(*feed, new_timestamp).await;
    }
}

fn usdc_balance<'a>(account: &'a MarginfiAccount, usdc_bank: &Pubkey) -> &'a Balance {
    account
        .lending_account
        .balances
        .iter()
        .find(|b| b.is_active() && b.bank_pk == *usdc_bank)
        .unwrap()
}

fn snapshot_percent(balance: &Balance) -> f64 {
    u32_to_milli(balance.premium_rate_snapshot).to_num::<f64>() * 100.0
}

#[tokio::test]
async fn premium_snapshot_set_on_borrow() -> anyhow::Result<()> {
    // Story 1: 100% SOL collateral, borrow stable at (sol, stable) = 1% => snapshot 1%.
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;

    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    let rate = snapshot_percent(balance);
    assert!((rate - 1.0).abs() < 0.0001, "snapshot {} != 1%", rate);
    // Nothing materialized yet
    assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);

    Ok(())
}

#[tokio::test]
async fn premium_accrues_lazily_and_pulse_materializes() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;

    advance_clock(&test_f, YEAR).await;

    // Anyone can pulse: materializes 1000 USDC x 1% x 1yr = 10 USDC premium
    borrower.try_lending_account_pulse_health().await?;

    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    assert_eq_noise!(
        I80F48::from(balance.premium_outstanding),
        I80F48::from(native!(10, "USDC")),
        I80F48!(50) // encoding granularity
    );
    // Snapshot unchanged (collateral mix unchanged)
    assert!((snapshot_percent(balance) - 1.0).abs() < 0.0001);

    // Pulsing again immediately adds nothing (elapsed 0)
    borrower.try_lending_account_pulse_health().await?;
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    assert_eq_noise!(
        I80F48::from(balance.premium_outstanding),
        I80F48::from(native!(10, "USDC")),
        I80F48!(50)
    );

    // Realized-only: the bank counter is untouched until tokens arrive
    let usdc_bank = usdc_bank_f.load().await;
    assert_eq!(
        I80F48::from(usdc_bank.collected_premium_outstanding),
        I80F48::ZERO
    );

    Ok(())
}

#[tokio::test]
async fn premium_activation_never_charges_retroactively() -> anyhow::Result<()> {
    // Borrow while premium is fully unconfigured; enable it 180 days later; the pre-activation
    // window must cost nothing.
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
    advance_clock(&test_f, 1_700_000_000).await;
    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;

    // Dormant for 180 days with premium OFF
    advance_clock(&test_f, YEAR / 2).await;

    // Risk team turns premium on
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(test_f.get_bank(&BankMint::Sol), TAG_SOL, true)
        .await?;

    // First pulse after activation: snapshot 0 -> 1%, accrual clock bumped, NOTHING charged
    borrower.try_lending_account_pulse_health().await?;
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);
    assert!((snapshot_percent(balance) - 1.0).abs() < 0.0001);

    // From now on it accrues: 30 days at 1% on 1000 USDC ~= 0.8219 USDC
    advance_clock(&test_f, 30 * 24 * 60 * 60).await;
    borrower.try_lending_account_pulse_health().await?;
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    let expected = I80F48::from(native!(1_000, "USDC"))
        * I80F48!(0.01)
        * I80F48::from_num(30.0 * 24.0 * 60.0 * 60.0 / (365.0 * 24.0 * 60.0 * 60.0));
    assert_eq_noise!(
        I80F48::from(balance.premium_outstanding),
        expected,
        I80F48!(50)
    );

    Ok(())
}

#[tokio::test]
async fn premium_partial_repay_settles_premium_first() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (_lender, borrower, borrower_usdc) = setup_borrower(&test_f, 1_000.0).await;

    advance_clock(&test_f, YEAR).await;
    borrower.try_lending_account_pulse_health().await?;

    // Premium outstanding ~= 10 USDC. Repay 4 USDC: ALL of it goes to premium, none to
    // principal (zero-interest bank: debt stays exactly 1000).
    borrower
        .try_bank_repay(borrower_usdc, usdc_bank_f, 4, None)
        .await?;

    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    let usdc_bank = usdc_bank_f.load().await;
    let debt = usdc_bank.get_liability_amount(balance.liability_shares.into())?;
    assert_eq_noise!(debt, I80F48::from(native!(1_000, "USDC")), I80F48!(1));
    assert_eq_noise!(
        I80F48::from(balance.premium_outstanding),
        I80F48::from(native!(6, "USDC")),
        I80F48!(50)
    );
    // The 4 USDC premium leg is realized on the bank, pending sweep
    assert_eq_noise!(
        I80F48::from(usdc_bank.collected_premium_outstanding),
        I80F48::from(native!(4, "USDC")),
        I80F48!(1)
    );

    // Repay 106 more: remaining ~6 premium settles first, ~100 reduces principal
    borrower
        .try_bank_repay(borrower_usdc, usdc_bank_f, 106, None)
        .await?;
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    let usdc_bank = usdc_bank_f.load().await;
    let debt = usdc_bank.get_liability_amount(balance.liability_shares.into())?;
    assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);
    assert_eq_noise!(debt, I80F48::from(native!(900, "USDC")), I80F48!(100));
    assert_eq_noise!(
        I80F48::from(usdc_bank.collected_premium_outstanding),
        I80F48::from(native!(10, "USDC")),
        I80F48!(100)
    );

    Ok(())
}

#[tokio::test]
async fn premium_repay_all_settles_and_sweep_pays_premium_wallet() -> anyhow::Result<()> {
    let mut test_f = premium_test_fixture().await;
    let (_lender, borrower, borrower_usdc_key) = setup_borrower(&test_f, 1_000.0).await;

    // Fund the borrower's wallet so they can cover debt + premium
    test_f.usdc_mint.mint_to(&borrower_usdc_key, 1_000f64).await;
    let test_f = test_f;
    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    advance_clock(&test_f, YEAR).await;

    let vault_before = usdc_bank_f
        .get_vault_token_account(marginfi::state::bank::BankVaultType::Liquidity)
        .await
        .balance()
        .await;

    // repay_all transfers ceil(1000 + 10) USDC; the premium leg lands on the bank counter
    borrower
        .try_bank_repay(borrower_usdc_key, usdc_bank_f, 0, Some(true))
        .await?;

    let account = borrower.load().await;
    assert!(account
        .lending_account
        .balances
        .iter()
        .all(|b| !b.is_active() || b.bank_pk != usdc_bank_f.key));

    let usdc_bank = usdc_bank_f.load().await;
    let collected: I80F48 = usdc_bank.collected_premium_outstanding.into();
    assert_eq_noise!(collected, I80F48::from(native!(10, "USDC")), I80F48!(50));

    let vault_after = usdc_bank_f
        .get_vault_token_account(marginfi::state::bank::BankVaultType::Liquidity)
        .await
        .balance()
        .await;
    let received = vault_after - vault_before;
    assert_eq_noise!(
        I80F48::from_num(received),
        I80F48::from(native!(1_010, "USDC")),
        I80F48!(100)
    );

    // Sweep to the premium wallet's canonical ATA (permissionless)
    let premium_wallet = Keypair::new().pubkey();
    group_f
        .try_edit_fee_state_premium(Some(premium_wallet))
        .await?;
    let premium_ata = TokenAccountFixture::new_from_ata(
        test_f.context.clone(),
        &test_f.usdc_mint.key,
        &premium_wallet,
        &test_f.usdc_mint.token_program,
    )
    .await;
    let ata_expected = get_associated_token_address_with_program_id(
        &premium_wallet,
        &test_f.usdc_mint.key,
        &test_f.usdc_mint.token_program,
    );
    assert_eq!(premium_ata.key, ata_expected);

    group_f
        .try_collect_premium_fees(usdc_bank_f, premium_ata.key)
        .await?;

    let swept = premium_ata.balance().await;
    assert_eq_noise!(
        I80F48::from_num(swept),
        I80F48::from(native!(10, "USDC")),
        I80F48!(100)
    );
    // Like collect_bank_fees, the sweep transfers whole native units; sub-unit dust remains
    let usdc_bank = usdc_bank_f.load().await;
    assert!(I80F48::from(usdc_bank.collected_premium_outstanding) < I80F48::ONE);

    Ok(())
}

#[tokio::test]
async fn premium_sweep_without_realized_premium_transfers_nothing() -> anyhow::Result<()> {
    // Premium claimed (materialized on the balance) but never repaid: the sweep must not touch
    // lender liquidity.
    let test_f = premium_test_fixture().await;
    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;

    advance_clock(&test_f, YEAR).await;
    borrower.try_lending_account_pulse_health().await?;

    let premium_wallet = Keypair::new().pubkey();
    group_f
        .try_edit_fee_state_premium(Some(premium_wallet))
        .await?;
    let premium_ata = TokenAccountFixture::new_from_ata(
        test_f.context.clone(),
        &test_f.usdc_mint.key,
        &premium_wallet,
        &test_f.usdc_mint.token_program,
    )
    .await;

    group_f
        .try_collect_premium_fees(usdc_bank_f, premium_ata.key)
        .await?;
    assert_eq!(premium_ata.balance().await, 0);

    Ok(())
}

#[tokio::test]
async fn premium_sweep_requires_canonical_ata_and_configured_wallet() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    // Wallet not configured yet -> PremiumWalletNotSet
    let random_ata = test_f.usdc_mint.create_empty_token_account().await;
    let res = group_f
        .try_collect_premium_fees(usdc_bank_f, random_ata.key)
        .await;
    assert!(res.is_err());

    // Configured wallet, but a non-canonical destination -> InvalidPremiumAta
    let premium_wallet = Keypair::new().pubkey();
    group_f
        .try_edit_fee_state_premium(Some(premium_wallet))
        .await?;
    let res = group_f
        .try_collect_premium_fees(usdc_bank_f, random_ata.key)
        .await;
    assert!(res.is_err());

    Ok(())
}

#[tokio::test]
async fn premium_snapshot_reprices_when_collateral_improves() -> anyhow::Result<()> {
    // Story 3-flavored: adding stable collateral halves the weighted rate on the next
    // health-checked action.
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;
    let group_f = &test_f.marginfi_group;
    // (sol -> stable) = 1%; sol_eq is untagged => 0% leg
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(test_f.get_bank(&BankMint::Sol), TAG_SOL, true)
        .await?;

    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;
    let account = borrower.load().await;
    assert!((snapshot_percent(usdc_balance(&account, &usdc_bank_f.key)) - 1.0).abs() < 0.0001);

    // Deposit an equal USD value of untagged (0% pair) collateral, then pulse to recompute:
    // the weighted rate drops to ~0.5%
    let sol_eq_account = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(sol_eq_account.key, sol_eq_bank_f, 999, None)
        .await?;
    borrower.try_lending_account_pulse_health().await?;

    let account = borrower.load().await;
    let rate = snapshot_percent(usdc_balance(&account, &usdc_bank_f.key));
    assert!((rate - 0.5).abs() < 0.001, "snapshot {} != ~0.5%", rate);

    Ok(())
}

#[tokio::test]
async fn premium_included_in_health_liabilities() -> anyhow::Result<()> {
    // The pending premium projects into the health cache liability value even without a claim.
    let test_f = premium_test_fixture().await;
    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;

    borrower.try_lending_account_pulse_health().await?;
    let liabs_t0: I80F48 = borrower.load().await.health_cache.liability_value.into();

    advance_clock(&test_f, YEAR).await;
    borrower.try_lending_account_pulse_health().await?;
    let liabs_t1: I80F48 = borrower.load().await.health_cache.liability_value.into();

    // Zero-interest bank: the ~1% growth in liability value is purely premium
    let growth = (liabs_t1 - liabs_t0) / liabs_t0;
    assert!(
        (growth.to_num::<f64>() - 0.01).abs() < 0.001,
        "liability growth {} != ~1%",
        growth
    );

    Ok(())
}

#[tokio::test]
async fn premium_deactivated_bank_writes_off_receivable_instead_of_settling() -> anyhow::Result<()>
{
    // Covers both admin deactivation and the legacy-`emissions_outstanding` migration case:
    // a nonzero receivable on a premium-INACTIVE bank must never be settled as premium (which
    // would divert repay tokens to the premium wallet) — it is written off on the next touch.
    let test_f = premium_test_fixture().await;
    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (_lender, borrower, borrower_usdc) = setup_borrower(&test_f, 1_000.0).await;

    // Accrue a real receivable (~10 USDC), then deactivate premium on the bank
    advance_clock(&test_f, YEAR).await;
    borrower.try_lending_account_pulse_health().await?;
    let account = borrower.load().await;
    assert!(
        I80F48::from(usdc_balance(&account, &usdc_bank_f.key).premium_outstanding) > I80F48::ZERO
    );
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, false)
        .await?;

    // Repay: nothing may reach the bank's premium counter; principal reduces by the full
    // amount; the receivable is written off.
    borrower
        .try_bank_repay(borrower_usdc, usdc_bank_f, 100, None)
        .await?;

    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    let usdc_bank = usdc_bank_f.load().await;
    assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);
    assert_eq!(
        I80F48::from(usdc_bank.collected_premium_outstanding),
        I80F48::ZERO
    );
    let debt = usdc_bank.get_liability_amount(balance.liability_shares.into())?;
    assert_eq_noise!(debt, I80F48::from(native!(900, "USDC")), I80F48!(100));

    // The permissionless pulse also cleans dormant balances on inactive banks
    advance_clock(&test_f, YEAR).await;
    borrower.try_lending_account_pulse_health().await?;
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);
    assert_eq!(balance.premium_rate_snapshot, 0);

    Ok(())
}

#[tokio::test]
async fn premium_reactivation_never_charges_for_deactivated_window() -> anyhow::Result<()> {
    // The off->on hole: a borrower whose balance is NEVER touched while the flag is off used
    // to be retro-charged (and health-cliffed) across the deactivated window on re-activation.
    // `premium_activated_at` clamps accrual to start at the latest activation.
    let test_f = premium_test_fixture().await;
    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;

    // Deactivate immediately; the borrower's snapshot (1%) and old last_update remain, and the
    // account is deliberately NOT touched or pulsed during the off window.
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, false)
        .await?;
    advance_clock(&test_f, 2 * YEAR).await;

    // Re-activate, then accrue for 1 more year.
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    advance_clock(&test_f, YEAR).await;
    borrower.try_lending_account_pulse_health().await?;

    // Charged for exactly 1 active year (~10 USDC), NOT the 2 deactivated years (~30 USDC).
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    assert_eq_noise!(
        I80F48::from(balance.premium_outstanding),
        I80F48::from(native!(10, "USDC")),
        I80F48!(50)
    );

    Ok(())
}

const TAG_LST: u16 = 300;
const SOL_EMODE_TAG: u16 = 501;
const LST_EMODE_TAG: u16 = 502;

#[tokio::test]
async fn premium_composes_with_emode() -> anyhow::Result<()> {
    use marginfi_type_crate::types::EmodeEntry;

    // SOL: weak base collateral (0.5/0.6) boosted by emode to 0.9/0.94 for LST borrows.
    // USDC: 0.8/0.9, NO emode entry. LST (SolEquivalent): zero-interest premium-active borrow.
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(0.8).into(),
                    asset_weight_maint: I80F48!(0.9).into(),
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(0.5).into(),
                    asset_weight_maint: I80F48!(0.6).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let lst_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    // Emode: borrowing LST treats SOL-tagged collateral at 0.9/0.94 (base 0.5/0.6).
    group_f
        .try_lending_pool_configure_bank_emode(sol_bank_f, SOL_EMODE_TAG, &[])
        .await?;
    group_f
        .try_lending_pool_configure_bank_emode(
            lst_bank_f,
            LST_EMODE_TAG,
            &[EmodeEntry {
                collateral_bank_emode_tag: SOL_EMODE_TAG,
                flags: 0,
                pad0: [0; 5],
                asset_weight_init: I80F48!(0.9).into(),
                asset_weight_maint: I80F48!(0.94).into(),
            }],
        )
        .await?;

    // Premium: independent tag space; (SOL -> LST) = 4%, (stable -> LST) = 6%.
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_LST, 4.0))
        .await?;
    group_f
        .try_configure_group_premium(entry(TAG_STABLE, TAG_LST, 6.0))
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(lst_bank_f, TAG_LST, true)
        .await?;

    // LST liquidity
    let lender = test_f.create_marginfi_account().await;
    let lender_lst = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    lender
        .try_bank_deposit(lender_lst.key, lst_bank_f, 10_000, None)
        .await?;

    // ---- Part 1: the premium snapshot is UNWEIGHTED by emode ----
    // Equal USD of SOL ($1000, emode-boosted 0.9) and USDC ($1000, base 0.8). The unweighted
    // mean of the pair rates is exactly 5.0%; an (incorrect) risk-weighted mean would be
    // (0.9*4 + 0.8*6)/1.7 = ~4.94%.
    let mixed = test_f.create_marginfi_account().await;
    let mixed_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    mixed
        .try_bank_deposit(mixed_sol.key, sol_bank_f, 100, None)
        .await?;
    let mixed_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    mixed
        .try_bank_deposit(mixed_usdc.key, usdc_bank_f, 1_000, None)
        .await?;
    let mixed_lst = test_f
        .sol_equivalent_mint
        .create_empty_token_account()
        .await;
    mixed.try_bank_borrow(mixed_lst.key, lst_bank_f, 20).await?;

    let account = mixed.load().await;
    let rate = snapshot_percent(usdc_balance(&account, &lst_bank_f.key));
    assert!(
        (rate - 5.0).abs() < 0.001,
        "snapshot {} != 5.0% (unweighted mean; emode weights must not skew it)",
        rate
    );

    // ---- Part 2: emode enables the leverage, premium erodes it to liquidatable ----
    // $1000 SOL, borrow $700 LST: possible ONLY via the emode boost (base cap 0.5*1000=500).
    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 100, None)
        .await?;
    let borrower_lst = test_f
        .sol_equivalent_mint
        .create_empty_token_account()
        .await;
    borrower
        .try_bank_borrow(borrower_lst.key, lst_bank_f, 70)
        .await?;

    // Healthy at maintenance right after the borrow: 0.94*1000 = 940 vs 700.
    borrower.try_lending_account_pulse_health().await?;
    let hc = borrower.load().await.health_cache;
    let assets_maint: I80F48 = hc.asset_value_maint.into();
    let liabs_maint: I80F48 = hc.liability_value_maint.into();
    assert!(assets_maint > liabs_maint, "must start maint-healthy");

    // Zero-interest bank: all degradation is premium (4% APR on $700 = $28/yr against a $240
    // maintenance margin -> underwater within ~9 years of dormancy).
    advance_clock(&test_f, 9 * YEAR).await;
    borrower.try_lending_account_pulse_health().await?;
    let hc = borrower.load().await.health_cache;
    let assets_maint: I80F48 = hc.asset_value_maint.into();
    let liabs_maint: I80F48 = hc.liability_value_maint.into();
    assert!(
        liabs_maint > assets_maint,
        "premium must erode the emode-leveraged position to liquidatable: {} vs {}",
        liabs_maint,
        assets_maint
    );

    Ok(())
}

/// Direct liquidation must refresh premium snapshots on BOTH accounts: the liquidator's
/// newly-created liability weights against their post-liquidation collateral (instead of the
/// default 0%), and the liquidatee re-weights against what remains after seizure.
#[tokio::test]
async fn premium_direct_liquidation_refreshes_snapshots() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    // (sol -> stable) = 1%; sol_eq stays untagged => 0% leg
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    // USDC liquidity
    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    // Liquidatee: equal USD of tagged SOL + untagged SolEq -> weighted 0.5% on the USDC debt
    let liquidatee = test_f.create_marginfi_account().await;
    let liquidatee_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    liquidatee
        .try_bank_deposit(liquidatee_sol.key, sol_bank_f, 999, None)
        .await?;
    let liquidatee_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    liquidatee
        .try_bank_deposit(liquidatee_sol_eq.key, sol_eq_bank_f, 999, None)
        .await?;
    let liquidatee_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow(liquidatee_usdc.key, usdc_bank_f, 1_000)
        .await?;
    let account = liquidatee.load().await;
    assert!((snapshot_percent(usdc_balance(&account, &usdc_bank_f.key)) - 0.5).abs() < 0.001);

    // Liquidator: all-SOL collateral (their weighted rate on any stable debt is the full 1%)
    let liquidator = test_f.create_marginfi_account().await;
    let liquidator_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(8_000)
        .await;
    liquidator
        .try_bank_deposit(liquidator_sol.key, sol_bank_f, 8_000, None)
        .await?;

    // Synthetically crush the collateral weights so the liquidatee is liquidatable
    for mint in [BankMint::Sol, BankMint::SolEquivalent] {
        test_f
            .get_bank_mut(&mint)
            .update_config(
                BankConfigOpt {
                    asset_weight_init: Some(I80F48!(0.01).into()),
                    asset_weight_maint: Some(I80F48!(0.02).into()),
                    ..Default::default()
                },
                None,
            )
            .await?;
    }
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    // Seize 50 SOL ($500) against the USDC debt
    liquidator
        .try_liquidate(&liquidatee, sol_bank_f, 50, usdc_bank_f)
        .await?;

    // Liquidator: brand-new USDC liability must carry the weighted rate of their collateral
    // (all tagged SOL -> the full 1% pair rate), not the 0% a fresh balance starts with.
    let liquidator_account = liquidator.load().await;
    let liquidator_rate = snapshot_percent(usdc_balance(&liquidator_account, &usdc_bank_f.key));
    assert!(
        (liquidator_rate - 1.0).abs() < 0.001,
        "liquidator snapshot {} != 1.0%",
        liquidator_rate
    );

    // Liquidatee: tagged SOL dropped 999 -> 949 while untagged SolEq kept 999, so the weighted
    // rate re-weights from 0.5% to 949/(949+999) = ~0.4872%.
    let liquidatee_account = liquidatee.load().await;
    let liquidatee_rate = snapshot_percent(usdc_balance(&liquidatee_account, &usdc_bank_f.key));
    let expected = 949.0 / (949.0 + 999.0);
    assert!(
        (liquidatee_rate - expected).abs() < 0.001,
        "liquidatee snapshot {} != {}",
        liquidatee_rate,
        expected
    );

    Ok(())
}

/// Order execution must re-weight surviving liabilities: the end-of-order health pass owns the
/// snapshot refresh that withdraw deferred while ACCOUNT_IN_ORDER_EXECUTION was set.
#[tokio::test]
async fn premium_order_execution_reweights_surviving_liability() -> anyhow::Result<()> {
    use marginfi_type_crate::types::{OrderTrigger, WrappedI80F48};
    use solana_sdk::{account::Account, transaction::Transaction};

    // USDC: surviving premium-active debt. PyUSD: order debt (closed by the keeper).
    // SOL: tagged order collateral. SolEq: untagged (0% leg) bystander collateral.
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
                    ..*DEFAULT_USDC_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::PyUSD,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
                    ..*DEFAULT_PYUSD_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock_with_feeds(
        &test_f,
        1_700_000_000,
        &[
            PYTH_USDC_FEED,
            PYTH_PYUSD_FEED,
            PYTH_SOL_FEED,
            PYTH_SOL_EQUIVALENT_FEED,
        ],
    )
    .await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let pyusd_bank_f = test_f.get_bank(&BankMint::PyUSD);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    // (sol -> stable) = 1%; sol_eq and pyusd stay untagged
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    // USDC + PyUSD liquidity
    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 10_000, None)
        .await?;
    let lender_pyusd = test_f
        .pyusd_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    lender
        .try_bank_deposit(lender_pyusd.key, pyusd_bank_f, 10_000, None)
        .await?;

    // User: $1000 tagged SOL + $1000 untagged SolEq; 100 USDC surviving debt at 0.5%, plus
    // 50 PyUSD debt the order will close.
    let user = test_f.create_marginfi_account().await;
    let user_sol = test_f.sol_mint.create_token_account_and_mint_to(101).await;
    user.try_bank_deposit(user_sol.key, sol_bank_f, 100, None)
        .await?;
    let user_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(101)
        .await;
    user.try_bank_deposit(user_sol_eq.key, sol_eq_bank_f, 100, None)
        .await?;
    let user_usdc = test_f.usdc_mint.create_empty_token_account().await;
    user.try_bank_borrow(user_usdc.key, usdc_bank_f, 100)
        .await?;
    let user_pyusd = test_f.pyusd_mint.create_empty_token_account().await;
    user.try_bank_borrow(user_pyusd.key, pyusd_bank_f, 50)
        .await?;

    let account = user.load().await;
    assert!((snapshot_percent(usdc_balance(&account, &usdc_bank_f.key)) - 0.5).abs() < 0.001);

    // A year of accrual at the 0.5% snapshot before the keeper executes
    advance_clock_with_feeds(
        &test_f,
        YEAR,
        &[
            PYTH_USDC_FEED,
            PYTH_PYUSD_FEED,
            PYTH_SOL_FEED,
            PYTH_SOL_EQUIVALENT_FEED,
        ],
    )
    .await;

    // Take-profit order on (SOL asset, PyUSD debt), instantly eligible: order net is
    // $1000 - $50 = $950 >= the $100 threshold.
    let order_pda = user
        .try_place_order(
            vec![sol_bank_f.key, pyusd_bank_f.key],
            OrderTrigger::TakeProfit {
                threshold: WrappedI80F48::from(I80F48!(100)),
                max_slippage: 0,
            },
        )
        .await?;

    // Keeper wallet + token accounts
    let keeper = Keypair::new();
    {
        let mut ctx = test_f.context.borrow_mut();
        let rent = ctx.banks_client.get_rent().await?;
        let account = Account {
            lamports: rent.minimum_balance(0) + 1_000_000_000,
            data: vec![],
            owner: solana_system_interface::program::ID,
            executable: false,
            rent_epoch: 0,
        };
        ctx.set_account(&keeper.pubkey(), &account.into());
    }
    let keeper_pyusd = test_f
        .pyusd_mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 500.0)
        .await
        .key;
    let keeper_sol = test_f
        .sol_mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;

    // Execute: repay all 50 PyUSD, withdraw the matching 5 SOL ($50)
    let (start_ix, execute_record) = user.make_start_execute_ix(order_pda, keeper.pubkey()).await;
    let repay_ix = user
        .make_repay_ix_with_authority(keeper_pyusd, pyusd_bank_f, 0.0, Some(true), keeper.pubkey())
        .await;
    let withdraw_ix = user
        .make_withdraw_ix_with_authority(keeper_sol, sol_bank_f, 5.0, None, keeper.pubkey())
        .await;
    let end_ix = user
        .make_end_execute_ix(
            order_pda,
            execute_record,
            keeper.pubkey(),
            keeper.pubkey(),
            vec![pyusd_bank_f.key],
        )
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[start_ix, repay_ix, withdraw_ix, end_ix],
            Some(&keeper.pubkey()),
            &[&keeper],
            ctx.banks_client.get_latest_blockhash().await?,
        );
        ctx.banks_client.process_transaction(tx).await?;
    }

    // Surviving USDC debt re-weights: tagged SOL dropped 100 -> 95 against 100 untagged SolEq,
    // so 950/(950+1000) = ~0.4872% (pre-fix it kept the stale 0.5%).
    let account = user.load().await;
    let rate = snapshot_percent(usdc_balance(&account, &usdc_bank_f.key));
    let expected = 950.0 / (950.0 + 1000.0);
    assert!(
        (rate - expected).abs() < 0.001,
        "surviving snapshot {} != {}",
        rate,
        expected
    );
    // end_execute claimed the elapsed year at the OLD 0.5% rate before re-weighting:
    // 100 USDC x 0.5% x 1yr = 0.5 USDC, not the lower post-order rate.
    assert_eq_noise!(
        I80F48::from(usdc_balance(&account, &usdc_bank_f.key).premium_outstanding),
        I80F48::from(native!(0.5, "USDC", f64)),
        I80F48!(50)
    );

    Ok(())
}

/// A liquidation that seizes the UNTAGGED collateral leg must (a) claim the elapsed window at
/// the OLD lower rate — never retroactively at the new one — and (b) re-weight the survivor
/// mix UPWARD, since the remaining collateral is now more premium-heavy. This is the
/// undercharge direction a stale snapshot would get wrong.
#[tokio::test]
async fn premium_liquidation_claims_at_old_rate_and_seizing_untagged_raises_rate(
) -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    // (sol -> stable) = 1%; sol_eq stays untagged => 0% leg
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    // Liquidatee: equal USD of tagged SOL + untagged SolEq -> 0.5% on the 1000 USDC debt
    let liquidatee = test_f.create_marginfi_account().await;
    let liquidatee_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    liquidatee
        .try_bank_deposit(liquidatee_sol.key, sol_bank_f, 999, None)
        .await?;
    let liquidatee_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    liquidatee
        .try_bank_deposit(liquidatee_sol_eq.key, sol_eq_bank_f, 999, None)
        .await?;
    let liquidatee_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow(liquidatee_usdc.key, usdc_bank_f, 1_000)
        .await?;

    let liquidator = test_f.create_marginfi_account().await;
    let liquidator_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(8_000)
        .await;
    liquidator
        .try_bank_deposit(liquidator_sol.key, sol_bank_f, 8_000, None)
        .await?;

    // A year of accrual at the 0.5% snapshot before anything changes
    advance_clock(&test_f, YEAR).await;

    // Synthetically crush the collateral weights so the liquidatee is liquidatable
    for mint in [BankMint::Sol, BankMint::SolEquivalent] {
        test_f
            .get_bank_mut(&mint)
            .update_config(
                BankConfigOpt {
                    asset_weight_init: Some(I80F48!(0.01).into()),
                    asset_weight_maint: Some(I80F48!(0.02).into()),
                    ..Default::default()
                },
                None,
            )
            .await?;
    }
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    // Seize 50 UNTAGGED SolEq ($500) against the USDC debt
    liquidator
        .try_liquidate(&liquidatee, sol_eq_bank_f, 50, usdc_bank_f)
        .await?;

    let account = liquidatee.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    // The elapsed year was charged at the OLD 0.5% rate: 1000 USDC x 0.5% x 1yr = 5 USDC —
    // NOT at the higher post-liquidation rate.
    assert_eq_noise!(
        I80F48::from(balance.premium_outstanding),
        I80F48::from(native!(5, "USDC")),
        I80F48!(50)
    );
    // Untagged SolEq shrank 999 -> 949 while tagged SOL kept 999: the weighted rate RISES
    // from 0.5% to 999/(999+949) = ~0.5128%.
    let rate = snapshot_percent(balance);
    let expected = 999.0 / (999.0 + 949.0);
    assert!(
        (rate - expected).abs() < 0.001,
        "liquidatee snapshot {} != {}",
        rate,
        expected
    );

    Ok(())
}

/// Seizure also changes the LIQUIDATOR's collateral mix, so their pre-existing debt must
/// re-weight: an all-untagged liquidator gains tagged collateral and their 0% snapshot turns
/// nonzero — while the pre-liquidation window stays free (claimed at the old 0% rate).
#[tokio::test]
async fn premium_liquidation_reweights_liquidator_existing_debt() -> anyhow::Result<()> {
    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    // (sol -> stable) = 1%; sol_eq stays untagged => 0% leg
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    // Liquidatee: all-tagged SOL collateral -> the full 1% on the 1000 USDC debt
    let liquidatee = test_f.create_marginfi_account().await;
    let liquidatee_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    liquidatee
        .try_bank_deposit(liquidatee_sol.key, sol_bank_f, 999, None)
        .await?;
    let liquidatee_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow(liquidatee_usdc.key, usdc_bank_f, 1_000)
        .await?;

    // Liquidator: all-UNTAGGED SolEq collateral, so their existing 100 USDC debt is at 0%
    let liquidator = test_f.create_marginfi_account().await;
    let liquidator_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(500)
        .await;
    liquidator
        .try_bank_deposit(liquidator_sol_eq.key, sol_eq_bank_f, 500, None)
        .await?;
    let liquidator_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidator
        .try_bank_borrow(liquidator_usdc.key, usdc_bank_f, 100)
        .await?;
    let account = liquidator.load().await;
    assert_eq!(
        usdc_balance(&account, &usdc_bank_f.key).premium_rate_snapshot,
        0
    );

    // A year passes at the liquidator's 0% snapshot
    advance_clock(&test_f, YEAR).await;

    // Crush ONLY the liquidatee's collateral (SOL); the liquidator's SolEq keeps weight 1
    test_f
        .get_bank_mut(&BankMint::Sol)
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.01).into()),
                asset_weight_maint: Some(I80F48!(0.02).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    // Seize 50 tagged SOL ($500) against the USDC debt
    liquidator
        .try_liquidate(&liquidatee, sol_bank_f, 50, usdc_bank_f)
        .await?;

    // Liquidator: $5000 untagged SolEq + $500 seized tagged SOL -> 500/5500 of the 1% pair
    let account = liquidator.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    let rate = snapshot_percent(balance);
    let expected = 500.0 / 5500.0;
    assert!(
        (rate - expected).abs() < 0.001,
        "liquidator snapshot {} != {}",
        rate,
        expected
    );
    // The pre-liquidation year was at rate 0: nothing may be charged retroactively
    assert_eq!(I80F48::from(balance.premium_outstanding), I80F48::ZERO);

    // Liquidatee stays all-tagged: snapshot holds at 1%, and the elapsed year was claimed at
    // that rate (1000 USDC x 1% x 1yr = 10 USDC)
    let account = liquidatee.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    let rate = snapshot_percent(balance);
    assert!((rate - 1.0).abs() < 0.001, "liquidatee snapshot {}", rate);
    assert_eq_noise!(
        I80F48::from(balance.premium_outstanding),
        I80F48::from(native!(10, "USDC")),
        I80F48!(50)
    );

    Ok(())
}

/// A pulse during an oracle outage must NOT rewrite snapshots. Under `Initial` requirements the
/// health loop soft-skips a failed collateral oracle (values the leg at 0 and records
/// `internal_err`); writing rates from that pass would exclude the tagged leg — letting a
/// borrower zero their own premium by pulsing while the risky collateral's oracle is stale.
#[tokio::test]
async fn premium_pulse_with_stale_collateral_oracle_never_rewrites_snapshots() -> anyhow::Result<()>
{
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    // (sol -> stable) = 1%; sol_eq stays untagged => 0% leg
    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    // Borrower: equal USD of tagged SOL + untagged SolEq -> 0.5% on the 1000 USDC debt
    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 999, None)
        .await?;
    let borrower_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol_eq.key, sol_eq_bank_f, 999, None)
        .await?;
    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc_bank_f, 1_000)
        .await?;

    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    assert!((snapshot_percent(balance) - 0.5).abs() < 0.001);
    let last_update_at_borrow = balance.last_update;

    // An hour passes and the tagged SOL oracle goes stale (only the other feeds refresh)
    advance_clock_with_feeds(&test_f, 3_600, &[PYTH_USDC_FEED, PYTH_SOL_EQUIVALENT_FEED]).await;

    // The pulse itself succeeds (asset-side oracle failures soft-skip under Initial), but the
    // snapshot pass must be a full no-op: rate NOT re-weighted to 0%, accrual clock untouched.
    borrower.try_lending_account_pulse_health().await?;
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    let rate = snapshot_percent(balance);
    assert!(
        (rate - 0.5).abs() < 0.001,
        "stale-oracle pulse rewrote snapshot to {}%",
        rate
    );
    assert_eq!(balance.last_update, last_update_at_borrow);

    // Once the oracle is fresh again the pulse works normally: same mix -> same 0.5%, and the
    // FULL elapsed window (both hours) bills at that rate in one claim.
    advance_clock(&test_f, 3_600).await;
    borrower.try_lending_account_pulse_health().await?;
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    assert!((snapshot_percent(balance) - 0.5).abs() < 0.001);
    assert!(balance.last_update > last_update_at_borrow);
    let expected = I80F48::from(native!(1_000, "USDC"))
        * I80F48!(0.005)
        * I80F48::from_num(7_200.0 / (365.0 * 24.0 * 60.0 * 60.0));
    assert_eq_noise!(
        I80F48::from(balance.premium_outstanding),
        expected,
        I80F48!(50)
    );

    Ok(())
}

/// Debt origination during an oracle outage must revert, not open at 0%: with the snapshot
/// pass gated off (incomplete scratch), a fresh premium-active borrow would otherwise keep its
/// initial 0% snapshot for the whole outage.
#[tokio::test]
async fn premium_borrow_rejected_when_collateral_oracle_stale() -> anyhow::Result<()> {
    use marginfi::errors::MarginfiError;

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    // Borrower: tagged SOL + plenty of untagged SolEq (health passes even with SOL skipped)
    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 999, None)
        .await?;
    let borrower_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol_eq.key, sol_eq_bank_f, 999, None)
        .await?;

    // The tagged SOL oracle goes stale; the health check would soft-skip it and still pass on
    // the $9,990 of untagged SolEq — but the premium rate is then unpriceable.
    advance_clock_with_feeds(&test_f, 3_600, &[PYTH_USDC_FEED, PYTH_SOL_EQUIVALENT_FEED]).await;

    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    let res = borrower
        .try_bank_borrow(borrower_usdc.key, usdc_bank_f, 100)
        .await;
    assert!(res.is_err(), "borrow must revert during the oracle outage");
    assert_custom_error!(res.unwrap_err(), MarginfiError::PremiumSnapshotUnavailable);

    // Once the oracle is fresh the same borrow succeeds and snapshots at the weighted 0.5%
    advance_clock(&test_f, 60).await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc_bank_f, 100)
        .await?;
    let account = borrower.load().await;
    let rate = snapshot_percent(usdc_balance(&account, &usdc_bank_f.key));
    assert!((rate - 0.5).abs() < 0.001, "snapshot {} != 0.5%", rate);

    Ok(())
}

/// Clearing a liability without closing the balance (exact-amount repay, liquidation flips)
/// must clear the rate snapshot too: a re-borrow on the still-active balance would otherwise
/// accrue premium across the entire debt-free gap at the stale rate.
#[tokio::test]
async fn premium_reopened_liability_pays_nothing_for_debt_free_gap() -> anyhow::Result<()> {
    let test_f = premium_test_fixture().await;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let (_lender, borrower, borrower_usdc) = setup_borrower(&test_f, 1_000.0).await;

    let account = borrower.load().await;
    assert!((snapshot_percent(usdc_balance(&account, &usdc_bank_f.key)) - 1.0).abs() < 0.0001);

    // Exact-amount repay (NOT repay_all): clears the debt but keeps the balance slot active
    borrower
        .try_bank_repay(borrower_usdc, usdc_bank_f, 1_000, Some(false))
        .await?;
    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    assert!(balance.is_empty(marginfi_type_crate::types::BalanceSide::Liabilities));
    assert_eq!(
        balance.premium_rate_snapshot, 0,
        "cleared liability kept a stale snapshot"
    );

    // A debt-free year passes, then the borrower reopens on the same balance slot
    advance_clock(&test_f, YEAR).await;
    borrower
        .try_bank_borrow(borrower_usdc, usdc_bank_f, 500)
        .await?;

    let account = borrower.load().await;
    let balance = usdc_balance(&account, &usdc_bank_f.key);
    // Nothing may accrue for the gap (pre-fix: 500 x 1% x 1yr = 5 USDC charged retroactively)
    assert_eq!(
        I80F48::from(balance.premium_outstanding),
        I80F48::ZERO,
        "debt-free gap was charged retroactively"
    );
    // The new debt reprices normally going forward
    assert!((snapshot_percent(balance) - 1.0).abs() < 0.0001);

    Ok(())
}

/// Withdrawing collateral during an oracle outage must revert, not silently keep a stale rate:
/// the Initial-health pass soft-skips the failed collateral, so a borrower could otherwise
/// remove a leg (or just freeze a favorable snapshot) while premium-active debt survives.
#[tokio::test]
async fn premium_withdraw_rejected_when_collateral_oracle_stale() -> anyhow::Result<()> {
    use marginfi::errors::MarginfiError;

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 999, None)
        .await?;
    let borrower_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol_eq.key, sol_eq_bank_f, 999, None)
        .await?;
    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc_bank_f, 1_000)
        .await?;

    // Tagged SOL oracle goes stale; the withdraw would soft-skip it and still pass on the
    // untagged SolEq — but the premium rate is then unpriceable.
    advance_clock_with_feeds(&test_f, 3_600, &[PYTH_USDC_FEED, PYTH_SOL_EQUIVALENT_FEED]).await;

    let res = borrower
        .try_bank_withdraw(borrower_sol_eq.key, sol_eq_bank_f, 10, None)
        .await;
    assert!(
        res.is_err(),
        "withdraw must revert during the oracle outage"
    );
    assert_custom_error!(res.unwrap_err(), MarginfiError::PremiumSnapshotUnavailable);

    // Fresh oracles: the same withdraw succeeds and reprices the surviving debt normally.
    advance_clock(&test_f, 60).await;
    borrower
        .try_bank_withdraw(borrower_sol_eq.key, sol_eq_bank_f, 10, None)
        .await?;
    let account = borrower.load().await;
    let rate = snapshot_percent(usdc_balance(&account, &usdc_bank_f.key));
    assert!((rate - 0.5).abs() < 0.01, "snapshot {} not ~0.5%", rate);

    Ok(())
}

/// The end-of-flashloan health pass owns the premium refresh for debt originated inside the
/// flashloan (borrow defers its own gate while ACCOUNT_IN_FLASHLOAN is set). An unpriceable
/// pass there must revert, not leave the new liability at its 0% snapshot.
#[tokio::test]
async fn premium_flashloan_end_rejected_when_collateral_oracle_stale() -> anyhow::Result<()> {
    use marginfi::errors::MarginfiError;

    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 999, None)
        .await?;
    let borrower_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol_eq.key, sol_eq_bank_f, 999, None)
        .await?;
    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;

    // A single borrow of premium-active USDC, wrapped in a flashloan: the borrow defers health
    // (and its premium gate); the end-flashloan pass must catch the unpriceable refresh.
    let borrow_ix = borrower
        .make_bank_borrow_ix(borrower_usdc.key, usdc_bank_f, 100)
        .await;

    advance_clock_with_feeds(&test_f, 3_600, &[PYTH_USDC_FEED, PYTH_SOL_EQUIVALENT_FEED]).await;

    let res = borrower
        .try_flashloan(vec![borrow_ix.clone()], vec![], vec![usdc_bank_f.key], None)
        .await;
    assert!(res.is_err(), "flashloan end must revert during the outage");
    assert_custom_error!(res.unwrap_err(), MarginfiError::PremiumSnapshotUnavailable);

    // Fresh oracles: the same flashloan closes and the new debt is priced at the weighted rate.
    advance_clock(&test_f, 60).await;
    let borrow_ix = borrower
        .make_bank_borrow_ix(borrower_usdc.key, usdc_bank_f, 100)
        .await;
    borrower
        .try_flashloan(vec![borrow_ix], vec![], vec![usdc_bank_f.key], None)
        .await?;
    let account = borrower.load().await;
    let rate = snapshot_percent(usdc_balance(&account, &usdc_bank_f.key));
    assert!((rate - 0.5).abs() < 0.001, "snapshot {} != 0.5%", rate);

    Ok(())
}

/// A liquidator that assumes debt during a collateral-oracle outage must revert: its
/// Initial-health refresh soft-skips the failed oracle, and leaving the newly-assumed liability
/// at a mispriced (possibly 0%) snapshot is the manipulation the gate blocks. The liquidator
/// keeps a fresh PyUSD leg so Initial health still passes with the staled SolEq skipped.
#[tokio::test]
async fn premium_liquidation_rejected_when_liquidator_collateral_oracle_stale() -> anyhow::Result<()>
{
    use marginfi::errors::MarginfiError;

    let mut test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
            TestBankSetting {
                mint: BankMint::PyUSD,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    interest_rate_config: zero_interest_config(),
                    ..*DEFAULT_PYUSD_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock_with_feeds(
        &test_f,
        1_700_000_000,
        &[
            PYTH_USDC_FEED,
            PYTH_SOL_FEED,
            PYTH_SOL_EQUIVALENT_FEED,
            PYTH_PYUSD_FEED,
        ],
    )
    .await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);
    let pyusd_bank_f = test_f.get_bank(&BankMint::PyUSD);

    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    // Liquidatee: all-tagged SOL collateral, borrows USDC (will be crushed to liquidatable)
    let liquidatee = test_f.create_marginfi_account().await;
    let liquidatee_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    liquidatee
        .try_bank_deposit(liquidatee_sol.key, sol_bank_f, 999, None)
        .await?;
    let liquidatee_usdc = test_f.usdc_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow(liquidatee_usdc.key, usdc_bank_f, 1_000)
        .await?;

    // Liquidator: fresh PyUSD (keeps Initial health positive) + untagged SolEq (staled below)
    let liquidator = test_f.create_marginfi_account().await;
    let liquidator_pyusd = test_f
        .pyusd_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    liquidator
        .try_bank_deposit(liquidator_pyusd.key, pyusd_bank_f, 9_999, None)
        .await?;
    let liquidator_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    liquidator
        .try_bank_deposit(liquidator_sol_eq.key, sol_eq_bank_f, 999, None)
        .await?;

    // Crush the liquidatee's SOL weight so they're liquidatable
    test_f
        .get_bank_mut(&BankMint::Sol)
        .update_config(
            BankConfigOpt {
                asset_weight_init: Some(I80F48!(0.01).into()),
                asset_weight_maint: Some(I80F48!(0.02).into()),
                ..Default::default()
            },
            None,
        )
        .await?;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    // Only the liquidator's untagged SolEq oracle goes stale. Its Initial-health check
    // soft-skips it (PyUSD keeps health positive), but the assumed USDC debt cannot be
    // premium-priced against an incomplete collateral pass -> revert.
    advance_clock_with_feeds(
        &test_f,
        3_600,
        &[PYTH_USDC_FEED, PYTH_SOL_FEED, PYTH_PYUSD_FEED],
    )
    .await;

    let res = liquidator
        .try_liquidate(&liquidatee, sol_bank_f, 50, usdc_bank_f)
        .await;
    assert!(res.is_err(), "liquidation must revert during the outage");
    assert_custom_error!(res.unwrap_err(), MarginfiError::PremiumSnapshotUnavailable);

    Ok(())
}

/// Complement to the reject tests: with the SAME incomplete pass (stale collateral oracle),
/// a withdraw must SUCCEED when the account's only debt is on a premium-INACTIVE bank — the
/// `refresh_unavailable` gate filters on `premium_active`, so inactive-only debt never blocks.
#[tokio::test]
async fn premium_withdraw_allowed_when_stale_oracle_but_debt_is_inactive() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: Some(BankConfig {
                    interest_rate_config: zero_interest_config(),
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
            TestBankSetting {
                mint: BankMint::SolEquivalent,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_EQUIVALENT_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;
    advance_clock(&test_f, 1_700_000_000).await;

    let group_f = &test_f.marginfi_group;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let sol_eq_bank_f = test_f.get_bank(&BankMint::SolEquivalent);

    group_f
        .try_configure_group_premium(entry(TAG_SOL, TAG_STABLE, 1.0))
        .await?;
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, true)
        .await?;
    group_f
        .try_configure_bank_premium(sol_bank_f, TAG_SOL, true)
        .await?;

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 999, None)
        .await?;
    let borrower_sol_eq = test_f
        .sol_equivalent_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_sol_eq.key, sol_eq_bank_f, 999, None)
        .await?;
    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, usdc_bank_f, 1_000)
        .await?;

    // Deactivate USDC premium: the borrower's only liability is now on an inactive bank, so the
    // health pass records it with `premium_active: false`.
    group_f
        .try_configure_bank_premium(usdc_bank_f, TAG_STABLE, false)
        .await?;

    // Same outage as the reject test: the tagged SOL oracle goes stale (incomplete pass).
    advance_clock_with_feeds(&test_f, 3_600, &[PYTH_USDC_FEED, PYTH_SOL_EQUIVALENT_FEED]).await;

    // With no premium-ACTIVE liability, the gate does not trip: the withdraw succeeds.
    borrower
        .try_bank_withdraw(borrower_sol_eq.key, sol_eq_bank_f, 10, None)
        .await?;

    Ok(())
}
