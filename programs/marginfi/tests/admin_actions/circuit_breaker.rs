use fixtures::bank::BankFixture;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::test::{PYTH_PYUSD_FEED, PYTH_SOL_FEED, PYTH_USDC_FEED};
use fixtures::{assert_custom_error, prelude::*};
use marginfi::prelude::*;
use marginfi_type_crate::types::{BankConfigOpt, BankOperationalState};
use solana_program_test::{BanksClientError, *};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;

/// Standard circuit-breaker config used across these tests: 5%/10%/25% deviation tiers with
/// 10m/1h/4h halt durations.
fn cb_config() -> BankConfigOpt {
    BankConfigOpt {
        circuit_breaker_enabled: Some(true),
        cb_deviation_bps_tiers: Some([500, 1000, 2500]),
        cb_tier_durations_seconds: Some([600, 3600, 14400]),
        cb_escalation_window_mult: Some(2),
        cb_ema_alpha_bps: Some(1000),
        ..Default::default()
    }
}

/// Warms `bank`'s price cache, enables the circuit breaker on it, then trips it into an active
/// halt by pulsing a large oracle spike — driving the halt entirely through real instructions.
/// Afterward `feed` is restored to `base_native` and every standard feed is refreshed to the
/// post-trip clock, so callers can exercise a halted but otherwise healthy bank.
async fn enable_cb_and_trip_halt(
    test_f: &TestFixture,
    bank: &BankFixture,
    feed: Pubkey,
    base_native: i64,
) -> anyhow::Result<()> {
    let warm_time: i64 = 100;
    let warm_slot: u64 = 1_000;
    test_f
        .set_pyth_oracle_price_native(feed, base_native, 0, warm_time)
        .await;
    test_f.set_clock(warm_slot, warm_time).await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(bank)
        .await?;

    bank.update_config(cb_config(), None).await?;

    // A single +100% spike trips a halt on the first breaching pulse.
    let trip_time = warm_time + 1;
    test_f.set_clock(warm_slot + 10, trip_time).await;
    test_f
        .set_pyth_oracle_price_native(feed, base_native.saturating_mul(2), 0, trip_time)
        .await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(bank)
        .await?;

    // Restore the feed so callers don't trip the inline price gate, and refresh the standard
    // feeds so post-trip borrows/withdraws see fresh oracles.
    test_f
        .set_pyth_oracle_price_native(feed, base_native, 0, trip_time)
        .await;
    for refreshed in [PYTH_SOL_FEED, PYTH_USDC_FEED, PYTH_PYUSD_FEED] {
        test_f.set_pyth_oracle_timestamp(refreshed, trip_time).await;
    }
    Ok(())
}

async fn enable_cb_and_trip_tier3_storm(
    test_f: &TestFixture,
    bank: &BankFixture,
    feed: Pubkey,
    base_native: i64,
    pre_break_state: Option<BankOperationalState>,
) -> anyhow::Result<()> {
    let mut clock_time: i64 = 100;
    let mut clock_slot: u64 = 1_000;
    test_f
        .set_pyth_oracle_price_native(feed, base_native, 0, clock_time)
        .await;
    test_f.set_clock(clock_slot, clock_time).await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(bank)
        .await?;

    bank.update_config(cb_config(), None).await?;
    if let Some(operational_state) = pre_break_state {
        bank.update_config(
            BankConfigOpt {
                operational_state: Some(operational_state),
                ..Default::default()
            },
            None,
        )
        .await?;
    }

    for _ in 0..3 {
        clock_time += 1;
        clock_slot += 10;
        // Warp before setting the clock: each pulse transaction is byte-identical to the last, so
        // without a fresh blockhash BanksClient signature-dedupes it into the cached result and the
        // breaker never escalates. `set_clock` still dictates the slot/time the breaker observes.
        test_f
            .context
            .borrow_mut()
            .warp_to_slot(clock_slot)
            .unwrap();
        test_f.set_clock(clock_slot, clock_time).await;
        test_f
            .set_pyth_oracle_price_native(feed, base_native.saturating_mul(2), 0, clock_time)
            .await;
        test_f
            .marginfi_group
            .try_pulse_bank_price_cache(bank)
            .await?;

        let bank_state = bank.load().await;
        clock_time = bank_state.cb_halt_ended_at;
    }

    Ok(())
}

/// Borrowing FROM a halted bank must fail with `BankCircuitBreakerHalted`.
#[tokio::test]
async fn cb_halt_blocks_borrow_on_halted_bank() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // LP funds the SOL bank.
    let lp = test_f.create_marginfi_account().await;
    let lp_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(lp_sol_acc.key, sol_bank, 10, None)
        .await?;

    // Borrower posts USDC collateral.
    let borrower = test_f.create_marginfi_account().await;
    let borrower_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_usdc_acc.key, usdc_bank, 1_000, None)
        .await?;
    let borrower_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    // Trip the SOL bank (the bank being borrowed from) into a real halt.
    enable_cb_and_trip_halt(&test_f, sol_bank, PYTH_SOL_FEED, 10_000_000_000).await?;

    let result = borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await;
    assert_custom_error!(result.unwrap_err(), MarginfiError::BankCircuitBreakerHalted);
    Ok(())
}

/// A risk-carrying withdraw (the account has an open liability) from a halted bank must fail
/// with `BankCircuitBreakerHalted`.
#[tokio::test]
async fn cb_halt_blocks_risk_withdraw_on_halted_bank() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // LP funds the SOL bank so the borrower can draw a liability.
    let lp = test_f.create_marginfi_account().await;
    let lp_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(lp_sol_acc.key, sol_bank, 10, None)
        .await?;

    // Borrower posts USDC collateral and opens a SOL debt, so the USDC withdraw carries risk.
    let borrower = test_f.create_marginfi_account().await;
    let borrower_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_usdc_acc.key, usdc_bank, 1_000, None)
        .await?;
    let borrower_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await?;

    enable_cb_and_trip_halt(&test_f, usdc_bank, PYTH_USDC_FEED, 1_000_000).await?;

    let result = borrower
        .try_bank_withdraw(borrower_usdc_acc.key, usdc_bank, 100, None)
        .await;
    assert_custom_error!(result.unwrap_err(), MarginfiError::BankCircuitBreakerHalted);
    Ok(())
}

/// A risk-free withdraw (the account carries no liability) from a halted bank must still
/// succeed — the halt only blocks risk-carrying actions.
#[tokio::test]
async fn cb_halt_allows_riskless_withdraw_on_halted_bank() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let depositor = test_f.create_marginfi_account().await;
    let token_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    depositor
        .try_bank_deposit(token_acc.key, usdc_bank, 500, None)
        .await?;

    enable_cb_and_trip_halt(&test_f, usdc_bank, PYTH_USDC_FEED, 1_000_000).await?;

    depositor
        .try_bank_withdraw(token_acc.key, usdc_bank, 100, None)
        .await?;
    Ok(())
}

/// Repay against a halted bank must succeed: a halt should not prevent a borrower from
/// reducing their debt.
#[tokio::test]
async fn cb_halt_allows_repay_on_halted_bank() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // LP funds the SOL bank.
    let lp = test_f.create_marginfi_account().await;
    let lp_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(lp_sol_acc.key, sol_bank, 10, None)
        .await?;

    // Borrower opens a SOL debt against USDC collateral.
    let borrower = test_f.create_marginfi_account().await;
    let borrower_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_usdc_acc.key, usdc_bank, 1_000, None)
        .await?;
    let borrower_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await?;

    enable_cb_and_trip_halt(&test_f, sol_bank, PYTH_SOL_FEED, 10_000_000_000).await?;

    // Repay must succeed under halt.
    borrower
        .try_bank_repay(borrower_sol_acc.key, sol_bank, 1, None)
        .await?;
    Ok(())
}

/// Deposit into a halted bank must succeed.
#[tokio::test]
async fn cb_halt_allows_deposit_on_halted_bank() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let depositor = test_f.create_marginfi_account().await;
    let token_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;

    enable_cb_and_trip_halt(&test_f, usdc_bank, PYTH_USDC_FEED, 1_000_000).await?;

    depositor
        .try_bank_deposit(token_acc.key, usdc_bank, 500, None)
        .await?;
    Ok(())
}

/// Direct liquidation must become admin/risk-admin-only when the liquidatee has any active halted
/// bank in their portfolio, even if the liquidation itself settles through a different,
/// non-halted asset/liability pair.
#[tokio::test]
async fn cb_halt_on_other_balance_blocks_non_admin_direct_liquidation() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let pyusd_bank = test_f.get_bank(&BankMint::PyUSD);

    // LP funds the SOL bank.
    let lp = test_f.create_marginfi_account().await;
    let lp_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(200).await;
    lp.try_bank_deposit(lp_sol_acc.key, sol_bank, 100, None)
        .await?;

    // Liquidatee: healthy at first, with an extra PyUSD balance that will later be halted.
    let liquidatee = test_f.create_marginfi_account().await;
    let liquidatee_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(2_000)
        .await;
    liquidatee
        .try_bank_deposit(liquidatee_usdc_acc.key, usdc_bank, 1_000, None)
        .await?;
    let liquidatee_pyusd_acc = test_f
        .pyusd_mint
        .create_token_account_and_mint_to(500)
        .await;
    liquidatee
        .try_bank_deposit(liquidatee_pyusd_acc.key, pyusd_bank, 100, None)
        .await?;
    let liquidatee_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    liquidatee
        .try_bank_borrow(liquidatee_sol_acc.key, sol_bank, 50)
        .await?;

    // Trip a real halt on the unrelated PyUSD leg.
    enable_cb_and_trip_halt(&test_f, pyusd_bank, PYTH_PYUSD_FEED, 1_000_000).await?;

    // Make the account unhealthy through the SOL debt leg.
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 30_000_000_000, 0, 1_000)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, 1_000)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_PYUSD_FEED, 1_000)
        .await;
    test_f.set_clock(2_000, 1_000).await;

    // Non-admin liquidator with its own authority. Direct liquidation should now fail with the
    // CB admin-only error even though the settlement pair is USDC/SOL, not PyUSD.
    let liquidator_authority = Keypair::new();
    let liquidator = MarginfiAccountFixture::new_with_authority(
        test_f.context.clone(),
        &test_f.marginfi_group.key,
        &liquidator_authority,
    )
    .await;
    let liquidator_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to_with_owner(&liquidator_authority.pubkey(), 2_000)
        .await;
    liquidator
        .try_bank_deposit_with_authority(
            liquidator_usdc_acc.key,
            usdc_bank,
            1_000,
            None,
            &liquidator_authority,
        )
        .await?;

    let result = liquidator
        .try_liquidate_with_authority(&liquidatee, usdc_bank, 100, sol_bank, &liquidator_authority)
        .await;
    assert_custom_error!(result.unwrap_err(), MarginfiError::CircuitBreakerAdminOnly);
    Ok(())
}

/// `clear_circuit_breaker` by `risk_admin` must restore both blocked operations: a freshly-cleared
/// bank should accept borrow and withdraw on the same transaction sequence that just failed.
#[tokio::test]
async fn clear_circuit_breaker_restores_borrow_and_withdraw() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let risk_admin = test_f.payer().clone(); // group's risk_admin defaults to payer

    let lp = test_f.create_marginfi_account().await;
    let lp_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(lp_sol_acc.key, sol_bank, 10, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_usdc_acc.key, usdc_bank, 1_000, None)
        .await?;
    let borrower_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    enable_cb_and_trip_halt(&test_f, sol_bank, PYTH_SOL_FEED, 10_000_000_000).await?;

    // Pre-clear: borrow blocked.
    let pre_clear = borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await;
    assert_custom_error!(
        pre_clear.unwrap_err(),
        MarginfiError::BankCircuitBreakerHalted
    );

    // Risk admin clears the halt.
    let clear_ix = sol_bank
        .make_clear_circuit_breaker_ix(risk_admin, false)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[clear_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    // Halt fields zeroed.
    let cleared = sol_bank.load().await;
    assert_eq!(cleared.cb_tier, 0);
    assert_eq!(cleared.cb_halt_started_at, 0);
    assert_eq!(cleared.cb_halt_ended_at, 0);
    assert_eq!(cleared.cb_tier3_consecutive_trips, 0);

    // Borrow now succeeds.
    borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await?;

    // Withdraw now succeeds (LP withdraws SOL from the bank that was previously halted).
    lp.try_bank_withdraw(lp_sol_acc.key, sol_bank, 1, None)
        .await?;
    Ok(())
}

/// End-to-end production flow driven by a real Pyth price spike: a single ~9.9% spike on the SOL
/// feed trips the bank to tier 1 on the *first* breaching pulse (first-breach model — no
/// sustained-observation wait), blocking borrow; the halt expires into the escalation window
/// where borrow resumes at the (unspiked) reference price; one more spike inside that window
/// escalates to tier 2; the risk admin then clears the halt and borrow is fully restored.
///
/// SOL native decimals = 9 → native price = ui * 1e9. Reference seeds at $10. A spike to $10.99
/// gives ~990 bps deviation — above the 500 bps tier-1 threshold but below the 1000 bps tier-2
/// threshold — so it trips to exactly tier 1. Each pulse advances the slot by 10 (well past
/// `CB_MIN_PULSE_SLOT_GAP`) and bumps the Pyth `publish_time` by 1s so source-time dedup accepts
/// it.
#[tokio::test]
async fn pyth_spike_trips_tier1_then_escalates_to_tier2_then_admin_clears() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let risk_admin = test_f.payer().clone();

    // LP funds the SOL bank.
    let lp = test_f.create_marginfi_account().await;
    let lp_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(lp_sol_acc.key, sol_bank, 50, None)
        .await?;

    // Borrower posts USDC collateral large enough for several SOL borrows.
    let borrower = test_f.create_marginfi_account().await;
    let borrower_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    borrower
        .try_bank_deposit(borrower_usdc_acc.key, usdc_bank, 10_000, None)
        .await?;
    let borrower_sol_acc = test_f.sol_mint.create_empty_token_account().await;

    let base_native: i64 = 10_000_000_000; // $10.00 * 1e9
    let spike_native: i64 = 10_990_000_000; // $10.99 * 1e9 (~9.9% spike)

    // ---- Setup: warm the cache, then enable CB. Configure-bank's CB-enable path requires
    // `last_oracle_price > 0` and seeds `cb_reference_price` from it.
    let mut clock_time: i64 = 100;
    let mut clock_slot: u64 = 1_000;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, base_native, 0, clock_time)
        .await;
    test_f.set_clock(clock_slot, clock_time).await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;

    sol_bank
        .update_config(
            BankConfigOpt {
                circuit_breaker_enabled: Some(true),
                cb_deviation_bps_tiers: Some([500, 1000, 2500]),
                cb_tier_durations_seconds: Some([600, 3600, 14400]),
                cb_escalation_window_mult: Some(2),
                cb_ema_alpha_bps: Some(1000),
                ..Default::default()
            },
            None,
        )
        .await?;

    // ---- Stage 1: bank operational → borrow succeeds.
    clock_time += 1;
    clock_slot += 10;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, base_native, 0, clock_time)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, clock_time)
        .await;
    test_f.set_clock(clock_slot, clock_time).await;
    borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await?;

    // ---- Stage 2: a single pulse at $10.99 → first-breach tier-1 trip.
    clock_slot += 10;
    clock_time += 1;
    test_f.set_clock(clock_slot, clock_time).await;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, spike_native, 0, clock_time)
        .await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;
    let after_trip_1 = sol_bank.load().await;
    assert_eq!(
        after_trip_1.cb_tier, 1,
        "a single +9.9% pulse must trip tier 1 on first breach"
    );
    let halt_ended_1 = after_trip_1.cb_halt_ended_at;

    // Stage 2 cont'd: borrow blocked while halt is active.
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, spike_native, 0, clock_time)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, clock_time)
        .await;
    let blocked_t1 = borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await;
    assert_custom_error!(
        blocked_t1.unwrap_err(),
        MarginfiError::BankCircuitBreakerHalted
    );

    // ---- Stage 3: advance past halt_ended_at into the escalation window. Tier stays at 1 but
    // `is_cb_halted` is false, so borrow resumes.
    clock_time = halt_ended_1 + 10;
    clock_slot += 100;
    test_f.set_clock(clock_slot, clock_time).await;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, base_native, 0, clock_time)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, clock_time)
        .await;
    let escalation_borrow = borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await;
    assert!(
        escalation_borrow.is_ok(),
        "borrow must resume during escalation watch (halt expired, tier > 0)"
    );

    // ---- Stage 4: one more pulse at $10.99 inside the escalation window → escalates to tier 2
    // via `(cb_tier + 1).min(3)`.
    clock_slot += 10;
    clock_time += 1;
    test_f.set_clock(clock_slot, clock_time).await;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, spike_native, 0, clock_time)
        .await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;
    let after_trip_2 = sol_bank.load().await;
    assert_eq!(
        after_trip_2.cb_tier, 2,
        "re-breach inside the escalation window must escalate to tier 2"
    );

    // Stage 4 cont'd: borrow blocked at tier 2.
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, spike_native, 0, clock_time)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, clock_time)
        .await;
    let blocked_t2 = borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await;
    assert_custom_error!(
        blocked_t2.unwrap_err(),
        MarginfiError::BankCircuitBreakerHalted
    );

    // ---- Stage 5: risk admin clears the halt. Reseed the EMA so the post-clear oracle level is
    // accepted as the new reference (the price is still spiked).
    let clear_ix = sol_bank
        .make_clear_circuit_breaker_ix(risk_admin, true)
        .await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[clear_ix],
            Some(&risk_admin),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }
    let cleared = sol_bank.load().await;
    assert_eq!(cleared.cb_tier, 0);
    assert_eq!(cleared.cb_halt_ended_at, 0);

    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, spike_native, 0, clock_time)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, clock_time)
        .await;
    borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await?;
    Ok(())
}

/// Enabling CB on a bank that was never pulsed must fail with `CircuitBreakerRequiresWarmCache`.
/// The seed-from-cache path needs a populated `last_oracle_price` to anchor the EMA reference.
#[tokio::test]
async fn cb_enable_fails_on_cold_cache() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Note: no pulse_bank_price_cache call. `last_oracle_price` is zero on a fresh bank.
    let result = sol_bank
        .update_config(
            BankConfigOpt {
                circuit_breaker_enabled: Some(true),
                cb_deviation_bps_tiers: Some([500, 1000, 2500]),
                cb_tier_durations_seconds: Some([600, 3600, 14400]),
                cb_escalation_window_mult: Some(2),
                cb_ema_alpha_bps: Some(1000),
                ..Default::default()
            },
            None,
        )
        .await;
    // `update_config` wraps the BanksClientError in anyhow::Error — downcast to access the
    // custom-error matcher.
    let err = result.unwrap_err().downcast::<BanksClientError>().unwrap();
    assert_custom_error!(err, MarginfiError::CircuitBreakerRequiresWarmCache);
    Ok(())
}

/// Enabling CB when the cached price is older than `CB_ENABLE_MAX_PRICE_AGE_SECONDS` must fail —
/// otherwise an attacker who pulses with a manipulated price minutes before admin's enable tx
/// could lock the EMA on a bad value. The freshness window forces admin to bundle (or near-bundle)
/// a pulse with the enable.
#[tokio::test]
async fn cb_enable_fails_on_stale_cache() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Warm the cache with a pulse, then advance the clock past the freshness window.
    let initial_clock_time: i64 = 100;
    let initial_clock_slot: u64 = 1_000;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 10_000_000_000, 0, initial_clock_time)
        .await;
    test_f
        .set_clock(initial_clock_slot, initial_clock_time)
        .await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;

    // Bump the clock 60s into the future (well past CB_ENABLE_MAX_PRICE_AGE_SECONDS = 30).
    test_f
        .set_clock(initial_clock_slot + 1_000, initial_clock_time + 60)
        .await;

    let result = sol_bank
        .update_config(
            BankConfigOpt {
                circuit_breaker_enabled: Some(true),
                cb_deviation_bps_tiers: Some([500, 1000, 2500]),
                cb_tier_durations_seconds: Some([600, 3600, 14400]),
                cb_escalation_window_mult: Some(2),
                cb_ema_alpha_bps: Some(1000),
                ..Default::default()
            },
            None,
        )
        .await;
    // `update_config` wraps the BanksClientError in anyhow::Error — downcast to access the
    // custom-error matcher.
    let err = result.unwrap_err().downcast::<BanksClientError>().unwrap();
    assert_custom_error!(err, MarginfiError::CircuitBreakerRequiresWarmCache);
    Ok(())
}

/// During a halt, `accrue_interest` must advance `last_update` without compounding share values.
/// If interest kept accruing, lenders who can still deposit would silently benefit while borrowers
/// who can't borrow/withdraw kept paying — a free trade for whoever notices first. Asserts that
/// share values are byte-identical before and after a halt-spanning accrual call.
#[tokio::test]
async fn cb_halt_freezes_interest_accrual() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let usdc_bank = test_f.get_bank(&BankMint::Usdc);
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Open a borrow against SOL so there are non-zero shares on both sides.
    let lp = test_f.create_marginfi_account().await;
    let lp_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(lp_sol_acc.key, sol_bank, 10, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000)
        .await;
    borrower
        .try_bank_deposit(borrower_usdc_acc.key, usdc_bank, 1_000, None)
        .await?;
    let borrower_sol_acc = test_f.sol_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_sol_acc.key, sol_bank, 1)
        .await?;

    // Halt and advance time inside the halt window. Then trigger accrual via a deposit (which is
    // halt-safe and calls accrue_interest).
    enable_cb_and_trip_halt(&test_f, sol_bank, PYTH_SOL_FEED, 10_000_000_000).await?;

    // Snapshot after the setup pulses, which are allowed to accrue before the halt is established.
    let before = sol_bank.load().await;
    let asset_share_value_before = before.asset_share_value;
    let liability_share_value_before = before.liability_share_value;

    // Advance ~1 hour, staying well inside the tier-3 halt window. `set_clock` is used rather
    // than `advance_time` because the latter warps the slot, which lets the runtime recompute
    // the timestamp past `cb_halt_ended_at` and end the halt early.
    test_f.set_clock(1_500, 3_701).await;
    let extra_lp_sol = test_f.sol_mint.create_token_account_and_mint_to(10).await;
    lp.try_bank_deposit(extra_lp_sol.key, sol_bank, 1, None)
        .await?;

    let after = sol_bank.load().await;
    assert_eq!(
        after.asset_share_value, asset_share_value_before,
        "asset_share_value must not advance during a CB halt"
    );
    assert_eq!(
        after.liability_share_value, liability_share_value_before,
        "liability_share_value must not advance during a CB halt"
    );
    Ok(())
}

#[tokio::test]
async fn cb_tier3_storm_forces_paused_bank_to_circuit_broken_and_restores_paused(
) -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    enable_cb_and_trip_tier3_storm(
        &test_f,
        sol_bank,
        PYTH_SOL_FEED,
        10_000_000_000,
        Some(BankOperationalState::Paused),
    )
    .await?;

    let broken = sol_bank.load().await;
    assert_eq!(
        broken.config.operational_state,
        BankOperationalState::CircuitBroken
    );
    assert_eq!(
        broken.cb_pre_break_state,
        BankOperationalState::Paused as u8
    );

    let admin = test_f.payer().clone();
    let clear_ix = sol_bank.make_clear_circuit_breaker_ix(admin, false).await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[clear_ix],
            Some(&admin),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }

    let cleared = sol_bank.load().await;
    assert_eq!(
        cleared.config.operational_state,
        BankOperationalState::Paused
    );
    assert_eq!(cleared.cb_tier, 0);
    assert_eq!(cleared.cb_tier3_consecutive_trips, 0);
    Ok(())
}

/// `update_cache_price` is the single CB observation entry point. A live-oracle ix (here: a
/// risk-free withdraw) on a bank whose price exceeds the tier-1 threshold must trip the halt on
/// that first breach, even though no one ran the explicit pulse crank. Without this wiring the
/// CB only fired during pulses, so an unobserved oracle attack could land freely.
#[tokio::test]
async fn cb_observes_price_through_operational_path() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let sol_bank = test_f.get_bank(&BankMint::Sol);
    let usdc_bank = test_f.get_bank(&BankMint::Usdc);

    // Warm the cache and enable CB on SOL with a tight tier 1 threshold.
    let mut clock_time: i64 = 100;
    let mut clock_slot: u64 = 1_000;
    let base_native: i64 = 10_000_000_000; // $10
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, base_native, 0, clock_time)
        .await;
    test_f.set_clock(clock_slot, clock_time).await;
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(sol_bank)
        .await?;
    sol_bank
        .update_config(
            BankConfigOpt {
                circuit_breaker_enabled: Some(true),
                cb_deviation_bps_tiers: Some([500, 1000, 2500]),
                cb_tier_durations_seconds: Some([600, 3600, 14400]),
                cb_escalation_window_mult: Some(2),
                cb_ema_alpha_bps: Some(1000),
                ..Default::default()
            },
            None,
        )
        .await?;

    // Set up a borrower so withdraw paths fetch a live price.
    let lp = test_f.create_marginfi_account().await;
    let lp_sol_acc = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lp.try_bank_deposit(lp_sol_acc.key, sol_bank, 10, None)
        .await?;
    let borrower = test_f.create_marginfi_account().await;
    let borrower_usdc_acc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(10_000)
        .await;
    borrower
        .try_bank_deposit(borrower_usdc_acc.key, usdc_bank, 10_000, None)
        .await?;

    // Push the live oracle to a +9.9% spike (>= tier 1 = 500 bps). Then run a withdraw with a
    // fresh slot/timestamp and a strictly-advancing publish_time so the CB dedup accepts it.
    clock_slot += 10;
    clock_time += 1;
    test_f.set_clock(clock_slot, clock_time).await;
    test_f
        .set_pyth_oracle_price_native(PYTH_SOL_FEED, 10_990_000_000, 0, clock_time)
        .await;
    test_f
        .set_pyth_oracle_timestamp(PYTH_USDC_FEED, clock_time)
        .await;
    let pre = sol_bank.load().await;
    assert_eq!(pre.cb_tier, 0);

    lp.try_bank_withdraw(lp_sol_acc.key, sol_bank, 1, None)
        .await?;

    let post = sol_bank.load().await;
    assert_eq!(
        post.cb_tier, 1,
        "withdraw must feed the CB observation pipeline and trip on first breach"
    );
    Ok(())
}

/// admin and risk_admin must both be able to clear an active halt. The earlier behavior accepted
/// only `risk_admin`, which broke when `risk_admin == Pubkey::default()` (the default at group
/// init).
#[tokio::test]
async fn clear_circuit_breaker_accepts_either_authority() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    let sol_bank = test_f.get_bank(&BankMint::Sol);

    // Halt, then clear with admin (not risk_admin). In this fixture both are payer, so we
    // exercise the admin branch by passing payer to both calls — the semantic is that the path
    // accepts an authority that equals admin OR risk_admin, not strictly risk_admin.
    enable_cb_and_trip_halt(&test_f, sol_bank, PYTH_SOL_FEED, 10_000_000_000).await?;
    let admin = test_f.payer().clone();
    let clear_ix = sol_bank.make_clear_circuit_breaker_ix(admin, false).await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[clear_ix],
            Some(&admin),
            &[&ctx.payer],
            ctx.banks_client.get_latest_blockhash().await.unwrap(),
        );
        ctx.banks_client
            .process_transaction_with_preflight(tx)
            .await?;
    }
    let cleared = sol_bank.load().await;
    assert_eq!(cleared.cb_tier, 0);
    Ok(())
}
