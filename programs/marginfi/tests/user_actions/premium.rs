//! Variable-borrow premium: accrual, snapshots, repay settlement, crank, sweep.
//!
//! Most tests use a ZERO-interest USDC bank so all liability growth is premium and the math
//! can be asserted exactly (within u32 rate-encoding tolerance).

use anchor_lang::prelude::Clock;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use fixtures::{assert_eq_noise, native, prelude::*};
use marginfi::state::bank::BankImpl;
use marginfi_type_crate::types::{
    make_points, milli_to_u32, u32_to_milli, Balance, BankConfig, InterestRateConfig,
    MarginfiAccount, PremiumEntry,
};
use pretty_assertions::assert_eq;
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use fixtures::marginfi_account::MarginfiAccountFixture;

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
    group_f.try_init_and_copy_fee_state_v2().await.unwrap();
    group_f
        .try_configure_group_premium(vec![entry(TAG_SOL, TAG_STABLE, 1.0)])
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
    let borrower_sol = test_f.sol_mint.create_token_account_and_mint_to(1_000).await;
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
    let new_timestamp = {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await.unwrap();
        clock.unix_timestamp += seconds;
        ctx.set_sysvar(&clock);
        clock.unix_timestamp
    };
    // Keep the mock oracles fresh so health checks (and the premium crank) don't hit staleness
    for feed in [PYTH_USDC_FEED, PYTH_SOL_FEED, PYTH_SOL_EQUIVALENT_FEED] {
        test_f.set_pyth_oracle_timestamp(feed, new_timestamp).await;
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
    group_f.try_init_and_copy_fee_state_v2().await?;
    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let (_lender, borrower, _) = setup_borrower(&test_f, 1_000.0).await;

    // Dormant for 180 days with premium OFF
    advance_clock(&test_f, YEAR / 2).await;

    // Risk team turns premium on
    group_f
        .try_configure_group_premium(vec![entry(TAG_SOL, TAG_STABLE, 1.0)])
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
    test_f
        .usdc_mint
        .mint_to(&borrower_usdc_key, 1_000f64)
        .await;
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
        .try_edit_fee_state_v2_premium(Some(premium_wallet))
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
        .try_edit_fee_state_v2_premium(Some(premium_wallet))
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
        .try_edit_fee_state_v2_premium(Some(premium_wallet))
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
    group_f.try_init_and_copy_fee_state_v2().await?;
    // (sol -> stable) = 1%; sol_eq is untagged => 0% leg
    group_f
        .try_configure_group_premium(vec![entry(TAG_SOL, TAG_STABLE, 1.0)])
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
        I80F48::from(usdc_balance(&account, &usdc_bank_f.key).premium_outstanding)
            > I80F48::ZERO
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
    group_f.try_init_and_copy_fee_state_v2().await?;
    group_f
        .try_configure_group_premium(vec![
            entry(TAG_SOL, TAG_LST, 4.0),
            entry(TAG_STABLE, TAG_LST, 6.0),
        ])
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
    let mixed_lst = test_f.sol_equivalent_mint.create_empty_token_account().await;
    mixed
        .try_bank_borrow(mixed_lst.key, lst_bank_f, 20)
        .await?;

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
    let borrower_lst = test_f.sol_equivalent_mint.create_empty_token_account().await;
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
