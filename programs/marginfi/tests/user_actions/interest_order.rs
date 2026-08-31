use anchor_lang::prelude::Clock;
use drift_mocks::{constants::SPOT_CUMULATIVE_INTEREST_PRECISION, state::MinimalSpotMarket};
use fixed::types::I80F48;
use fixed_macro::types::I80F48 as fp;
use fixtures::bank::BankFixture;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::test::{
    PYTH_PUSH_SOL_FULLV_FEED, PYTH_PUSH_SOL_PARTV_FEED, PYTH_PYUSD_FEED, PYTH_SOL_EQUIVALENT_FEED,
    PYTH_SOL_FEED, PYTH_USDC_FEED,
};
use fixtures::{assert_custom_error, prelude::*};
use juplend_mocks::state::{Lending as JuplendLending, EXCHANGE_PRICES_PRECISION};
use marginfi::prelude::MarginfiError;
use marginfi_type_crate::constants::{
    INTEREST_ANCHOR_MAX_AGE_WINDOWS, INTEREST_DEFAULT_PATIENCE_SECONDS,
    INTEREST_MAX_PATIENCE_SECONDS,
};
use marginfi_type_crate::types::{
    centi_to_u32, milli_to_u32, InterestTriggerConfig, OrderTrigger, PremiumEntry,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::{tokio, BanksClientError};
use solana_sdk::{
    account::{Account, AccountSharedData},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer as _,
    transaction::Transaction,
};

/// A plausible wall-clock start. `program-test` boots at timestamp 0, which reads as "never
/// armed", so every test pins a real time before touching an anchor.
const BASE_TS: i64 = 1_700_000_000;
const ASSET_DEPOSIT: f64 = 1_000.0; // USDC, $1,000 at the $1 test oracle
const LIABILITY_BORROW: f64 = 10.0; // SOL, $100 at the $10 test oracle
/// The SOL oracle price in the test ecosystem, used to size the USDC the keeper pulls back out to
/// cover the SOL it repaid.
const SOL_PRICE: f64 = 10.0;

/// Premium matrix tags for the two legs of the pair.
const TAG_COLLATERAL: u16 = 100;
const TAG_LIABILITY: u16 = 200;

// A near-idle borrow leg (1 SOL against a 1,000 SOL float) so its baseline rate is negligible and
// whatever a driver does to utilization is the only thing the window measures.
const SPIKE_LENDER_SOL: f64 = 1_000.0;
const SPIKE_BORROW_SOL: f64 = 1.0;
const SPIKE_BORROW: f64 = 800.0;
const SPIKE_COLLATERAL: f64 = 20_000.0;
/// The measurement window every fixture here configures. Long enough that a 30-day step is one
/// window, and so comfortably inside the two-window expiry bound.
const TEST_WINDOW_SECONDS: u32 = 30 * 24 * 3_600;
const TEST_WINDOW: i64 = TEST_WINDOW_SECONDS as i64;
/// Above anything the near-idle baseline produces, below what a held ~90% utilization does.
const SPIKE_MARGIN_APR: f64 = 0.02;

/// A near-idle borrow leg at the default size, with a margin small enough that base rates alone
/// miss it and a premium alone clears it.
fn premium_params() -> Params {
    Params {
        interest: Some(InterestTriggerConfig {
            window_seconds: Some(TEST_WINDOW_SECONDS),
            patience_seconds: Some(INTEREST_MAX_PATIENCE_SECONDS),
            min_negative_apr: Some(milli_to_u32(I80F48::from_num(0.01))),
        }),
        lender_sol: SPIKE_LENDER_SOL,
        ..Default::default()
    }
}

/// The fixture the spike pair shares: identical on both sides so only duration differs.
fn spike_params() -> Params {
    Params {
        interest: Some(InterestTriggerConfig {
            window_seconds: Some(TEST_WINDOW_SECONDS),
            patience_seconds: Some(INTEREST_MAX_PATIENCE_SECONDS),
            min_negative_apr: Some(milli_to_u32(I80F48::from_num(SPIKE_MARGIN_APR))),
        }),
        lender_sol: SPIKE_LENDER_SOL,
        borrow_sol: SPIKE_BORROW_SOL,
        ..Default::default()
    }
}

fn slippage(percent: f64) -> u32 {
    centi_to_u32(I80F48::from_num(percent / 100.0))
}

fn interest_config(window: u32, patience: u32) -> InterestTriggerConfig {
    InterestTriggerConfig {
        window_seconds: Some(window),
        patience_seconds: Some(patience),
        min_negative_apr: None,
    }
}

/// Pin the clock and republish every Pyth feed a fixture here can touch, so a padded account's
/// unrelated balances stay priceable across the long steps these tests take.
async fn pin_clock(test_f: &TestFixture, ts: i64) {
    {
        let ctx = test_f.context.borrow_mut();
        let mut clock: Clock = ctx.banks_client.get_sysvar().await.unwrap();
        clock.unix_timestamp = ts;
        ctx.set_sysvar(&clock);
    }
    for feed in [
        PYTH_SOL_FEED,
        PYTH_USDC_FEED,
        PYTH_SOL_EQUIVALENT_FEED,
        PYTH_PYUSD_FEED,
        PYTH_PUSH_SOL_FULLV_FEED,
        PYTH_PUSH_SOL_PARTV_FEED,
    ] {
        test_f.set_pyth_oracle_timestamp(feed, ts).await;
    }
}

async fn fund_keeper(test_f: &TestFixture, keeper: &Keypair) -> anyhow::Result<()> {
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
    Ok(())
}

struct InterestFixture {
    test_f: TestFixture,
    account_f: MarginfiAccountFixture,
    order: Pubkey,
    keeper: Keypair,
    keeper_sol: Pubkey,
    keeper_usdc: Pubkey,
    now: i64,
    /// Sizes the unwind withdrawal, so a fixture with a smaller borrow is unwound to scale.
    borrow_sol: f64,
}

struct Params {
    interest: Option<InterestTriggerConfig>,
    /// Far below the pair's value unless a test wants the price condition live.
    stop_loss: I80F48,
    /// SOL the lender supplies. A large float against a small borrow leaves the bank near zero
    /// utilization, so its rate is whatever a test drives it to and nothing else.
    lender_sol: f64,
    borrow_sol: f64,
    max_slippage_pct: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            interest: Some(interest_config(
                TEST_WINDOW_SECONDS,
                INTEREST_DEFAULT_PATIENCE_SECONDS,
            )),
            stop_loss: fp!(1),
            lender_sol: 100.0,
            borrow_sol: LIABILITY_BORROW,
            max_slippage_pct: 5.0,
        }
    }
}

/// An account whose SOL borrow drives that bank's utilization, so a rate can be pushed up and let
/// back down within one test.
struct RateDriver {
    account: MarginfiAccountFixture,
    sol_account: Pubkey,
}

impl RateDriver {
    /// Repay the whole borrow, returning the bank to its baseline utilization.
    async fn release(&self, fx: &InterestFixture) -> anyhow::Result<()> {
        self.account
            .try_bank_repay(
                self.sol_account,
                fx.test_f.get_bank(&BankMint::Sol),
                0,
                Some(true),
            )
            .await?;
        Ok(())
    }
}

/// A USDC lend against a SOL borrow. Only the SOL bank carries utilization, so the pair bleeds
/// carry by construction unless a test says otherwise.
async fn setup(p: Params) -> anyhow::Result<InterestFixture> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    pin_clock(&test_f, BASE_TS).await;

    let usdc = test_f.get_bank(&BankMint::Usdc);
    let sol = test_f.get_bank(&BankMint::Sol);

    let lender = test_f.create_marginfi_account().await;
    let lender_sol = sol
        .mint
        .create_token_account_and_mint_to(p.lender_sol)
        .await;
    lender
        .try_bank_deposit(lender_sol.key, sol, p.lender_sol, None)
        .await?;

    let account_f = test_f.create_marginfi_account().await;
    let borrower_usdc = usdc
        .mint
        .create_token_account_and_mint_to(ASSET_DEPOSIT)
        .await;
    account_f
        .try_bank_deposit(borrower_usdc.key, usdc, ASSET_DEPOSIT, None)
        .await?;
    let borrower_sol = sol.mint.create_empty_token_account().await;
    account_f
        .try_bank_borrow(borrower_sol.key, sol, p.borrow_sol)
        .await?;

    let order = account_f
        .try_place_order_with_interest(
            vec![usdc.key, sol.key],
            OrderTrigger::StopLoss {
                threshold: p.stop_loss.into(),
                max_slippage: slippage(p.max_slippage_pct),
            },
            p.interest,
        )
        .await?;

    let keeper = Keypair::new();
    fund_keeper(&test_f, &keeper).await?;
    let keeper_sol = sol
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 100_000.0)
        .await
        .key;
    let keeper_usdc = usdc
        .mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;

    Ok(InterestFixture {
        test_f,
        account_f,
        order,
        keeper,
        keeper_sol,
        keeper_usdc,
        now: BASE_TS,
        borrow_sol: p.borrow_sol,
    })
}

/// Open a SOL borrow against fresh USDC collateral, pushing the SOL bank's utilization (and so its
/// borrow rate) up until the driver is released.
async fn drive_sol_rate(
    fx: &InterestFixture,
    borrow_sol: f64,
    collateral_usdc: f64,
) -> anyhow::Result<RateDriver> {
    let usdc = fx.test_f.get_bank(&BankMint::Usdc);
    let sol = fx.test_f.get_bank(&BankMint::Sol);
    let account = fx.test_f.create_marginfi_account().await;
    let collateral = usdc
        .mint
        .create_token_account_and_mint_to(collateral_usdc)
        .await;
    account
        .try_bank_deposit(collateral.key, usdc, collateral_usdc, None)
        .await?;
    // Seeded with a spare principal's worth so the driver can still repay in full after interest
    // has accrued on top of what it borrowed.
    let sol_account = sol
        .mint
        .create_token_account_and_mint_to(borrow_sol)
        .await
        .key;
    account
        .try_bank_borrow(sol_account, sol, borrow_sol)
        .await?;
    Ok(RateDriver {
        account,
        sol_account,
    })
}

impl InterestFixture {
    async fn advance(&mut self, seconds: i64) {
        self.now += seconds;
        pin_clock(&self.test_f, self.now).await;
    }

    async fn arm(&self) -> std::result::Result<(), BanksClientError> {
        self.account_f.try_arm_order_interest(self.order).await
    }

    /// Close out the borrow leg's accrual at the current rate, so the next span accrues at
    /// whatever rate a driver sets rather than back-filling the whole elapsed period at it.
    async fn settle_borrow_rate(&self) -> anyhow::Result<()> {
        let sol = self.test_f.get_bank(&BankMint::Sol);
        self.test_f.marginfi_group.try_accrue_interest(sol).await?;
        Ok(())
    }

    async fn bank_last_update(&self, mint: &BankMint) -> i64 {
        self.test_f.get_bank(mint).load().await.last_update
    }

    /// Repay the SOL liability from the keeper's own tokens, pull `scale` times the covering USDC
    /// back out, and close. `scale` is against the ORIGINAL borrow, not the accrued liability.
    async fn unwind(&self, scale: f64) -> std::result::Result<(), BanksClientError> {
        self.unwind_with_budget(scale, 0).await
    }

    /// `compute_units` of 0 leaves the default budget; a fuller account needs more than 200k.
    async fn unwind_with_budget(
        &self,
        scale: f64,
        compute_units: u32,
    ) -> std::result::Result<(), BanksClientError> {
        // Both order banks go in writable: the trigger accrues them before reading their indices.
        let metas = self
            .account_f
            .load_observation_account_metas_with_flags(vec![], vec![], true, false)
            .await;
        self.unwind_inner(scale, metas, compute_units).await
    }

    async fn unwind_inner(
        &self,
        scale: f64,
        metas: Vec<AccountMeta>,
        compute_units: u32,
    ) -> std::result::Result<(), BanksClientError> {
        let usdc = self.test_f.get_bank(&BankMint::Usdc);
        let sol = self.test_f.get_bank(&BankMint::Sol);

        let (start_ix, execute_record) = self
            .account_f
            .make_start_execute_ix_with_metas(self.order, self.keeper.pubkey(), Some(metas))
            .await;
        let repay_ix = self
            .account_f
            .make_repay_ix_with_authority(
                self.keeper_sol,
                sol,
                0.0,
                Some(true),
                self.keeper.pubkey(),
            )
            .await;
        let withdraw_ix = self
            .account_f
            .make_withdraw_ix_with_authority(
                self.keeper_usdc,
                usdc,
                self.borrow_sol * SOL_PRICE * scale,
                None,
                self.keeper.pubkey(),
            )
            .await;
        let end_ix = self
            .account_f
            .make_end_execute_ix(
                self.order,
                execute_record,
                self.keeper.pubkey(),
                self.keeper.pubkey(),
                vec![sol.key],
            )
            .await;

        let mut ixs = Vec::with_capacity(5);
        if compute_units > 0 {
            ixs.push(ComputeBudgetInstruction::set_compute_unit_limit(
                compute_units,
            ));
        }
        ixs.extend([start_ix, repay_ix, withdraw_ix, end_ix]);
        self.process(&ixs).await
    }

    /// The same sandwich with the observation banks read-only, which the carry condition cannot
    /// read from.
    async fn unwind_with_readonly_banks(&self) -> std::result::Result<(), BanksClientError> {
        let metas = self
            .account_f
            .load_observation_account_metas(vec![], vec![])
            .await;
        let (start_ix, _) = self
            .account_f
            .make_start_execute_ix_with_metas(self.order, self.keeper.pubkey(), Some(metas))
            .await;
        self.process(&[start_ix]).await
    }

    /// A full unwind with read-only banks, for the case where a price trigger has to carry it.
    async fn unwind_readonly_full(&self, scale: f64) -> std::result::Result<(), BanksClientError> {
        let metas = self
            .account_f
            .load_observation_account_metas(vec![], vec![])
            .await;
        self.unwind_inner(scale, metas, 0).await
    }

    async fn process(&self, ixs: &[Instruction]) -> std::result::Result<(), BanksClientError> {
        let blockhash = self.test_f.get_latest_blockhash().await;
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&self.keeper.pubkey()),
            &[&self.keeper],
            blockhash,
        );
        self.test_f
            .context
            .borrow_mut()
            .banks_client
            .process_transaction(tx)
            .await
    }
}

/// The headline path: a pair whose borrow leg out-costs its idle lend leg is unwound once the
/// measurement window has run.
#[tokio::test]
async fn interest_order_fires_once_the_pair_has_bled_for_a_window() -> anyhow::Result<()> {
    // The default stop-loss sits far below the pair's value, so only carry can arm this.
    let mut fx = setup(Params::default()).await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;
    fx.unwind(1.0).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "order should be consumed by the execution"
    );
    let account = fx.account_f.load().await;
    let sol = fx.test_f.get_bank(&BankMint::Sol);
    assert!(
        !account
            .lending_account
            .balances
            .iter()
            .any(|b| b.is_active() && b.bank_pk == sol.key),
        "the borrow leg should be closed"
    );
    let usdc = fx.test_f.get_bank(&BankMint::Usdc);
    assert!(
        account
            .lending_account
            .balances
            .iter()
            .any(|b| b.is_active() && b.bank_pk == usdc.key),
        "the lend leg should survive"
    );
    Ok(())
}

/// An order whose carry condition was never anchored has no measurement to act on.
#[tokio::test]
async fn interest_order_is_inert_until_armed() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW).await;
    let res = fx.unwind(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestNotArmed);
    Ok(())
}

/// A span shorter than the configured window is not a rate measurement, so a spike inside it
/// cannot arm an exit.
#[tokio::test]
async fn interest_order_cannot_execute_before_its_window_elapses() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW - 1).await;
    let res = fx.unwind(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestWindowTooShort);
    Ok(())
}

/// Re-anchoring is admitted only once the standing anchor is itself a full window old, which is
/// what stops a keeper shortening the span it is measured over.
#[tokio::test]
async fn re_arming_inside_the_window_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW - 1).await;
    let res = fx.arm().await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestWindowTooShort);

    fx.advance(1).await;
    fx.arm().await?;
    let order = fx.account_f.load_order(fx.order).await;
    assert_eq!(order.interest_anchor_timestamp, fx.now);
    Ok(())
}

/// The exit budget is what the pair loses to interest over its patience span. An hour of patience
/// buys almost nothing, so a keeper skimming real value cannot clear it.
#[tokio::test]
async fn an_unwind_costlier_than_the_carry_budget_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        interest: Some(interest_config(TEST_WINDOW_SECONDS, 3_600)),
        ..Default::default()
    })
    .await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;
    let res = fx.unwind(1.04).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderInterestCostExceedsCarry
    );
    Ok(())
}

/// The two conditions are independent: an unarmed carry anchor must not suppress a price trigger
/// that did fire, and the price path keeps its own cost bound.
#[tokio::test]
async fn a_price_trigger_still_fires_while_the_carry_anchor_is_unarmed() -> anyhow::Result<()> {
    // The pair is worth ~$900, so this stop-loss is already breached at placement.
    let mut fx = setup(Params {
        stop_loss: fp!(5000),
        ..Default::default()
    })
    .await?;

    fx.advance(3_600).await;
    fx.unwind(1.0).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the price trigger should execute despite the carry anchor being unset"
    );
    Ok(())
}

/// The "sustained, not transient" property, half one: a brief spike contributes only its own
/// duration to the index, so the window's average barely moves and the order stays put.
#[tokio::test]
async fn a_brief_spike_inside_the_window_does_not_fire() -> anyhow::Result<()> {
    let mut fx = setup(spike_params()).await?;

    fx.arm().await?;
    fx.settle_borrow_rate().await?;

    // Ten minutes at ~90% utilization, then back to the near-idle baseline.
    let driver = drive_sol_rate(&fx, SPIKE_BORROW, SPIKE_COLLATERAL).await?;
    fx.advance(600).await;
    fx.settle_borrow_rate().await?;
    driver.release(&fx).await?;

    fx.advance(TEST_WINDOW).await;
    fx.settle_borrow_rate().await?;

    let res = fx.unwind(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestNotNegative);
    Ok(())
}

/// Half two: the identical peak rate, held across the whole window, does fire. Same fixture, same
/// margin, same driver; only the duration differs.
#[tokio::test]
async fn the_same_rate_sustained_across_the_window_does_fire() -> anyhow::Result<()> {
    let mut fx = setup(spike_params()).await?;

    fx.arm().await?;
    fx.settle_borrow_rate().await?;

    let _driver = drive_sol_rate(&fx, SPIKE_BORROW, SPIKE_COLLATERAL).await?;
    fx.advance(TEST_WINDOW).await;
    fx.settle_borrow_rate().await?;

    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "a rate held for the whole window should arm the exit"
    );
    Ok(())
}

/// With both conditions armed, the keeper needs only one cost bound to hold. Here the carry budget
/// is far too small for the unwind, and the price gate carries the execution instead.
#[tokio::test]
async fn both_conditions_met_lets_either_cost_bound_carry_the_execution() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        // Already breached at placement, so the price condition arms too.
        stop_loss: fp!(5000),
        // An hour of patience buys almost no exit budget.
        interest: Some(interest_config(TEST_WINDOW_SECONDS, 3_600)),
        ..Default::default()
    })
    .await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;
    fx.unwind(1.04).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the price bound should carry an execution the carry budget cannot"
    );
    Ok(())
}

/// Patience widens the carry budget but never the user's slippage ceiling, which still binds.
#[tokio::test]
async fn the_slippage_ceiling_binds_even_when_patience_would_allow_more() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        // A year of patience makes the carry budget far larger than this unwind costs.
        interest: Some(interest_config(
            TEST_WINDOW_SECONDS,
            INTEREST_MAX_PATIENCE_SECONDS,
        )),
        max_slippage_pct: 1.0,
        // A float the rate driver below can actually borrow against.
        lender_sol: SPIKE_LENDER_SOL,
        ..Default::default()
    })
    .await?;

    fx.arm().await?;
    fx.settle_borrow_rate().await?;

    // A driven rate makes the year-long carry budget far bigger than this unwind costs, leaving
    // the ceiling as the only bound that can reject it.
    let _driver = drive_sol_rate(&fx, SPIKE_BORROW, SPIKE_COLLATERAL).await?;
    fx.advance(TEST_WINDOW).await;
    fx.settle_borrow_rate().await?;

    // Against a liability grown to ~$117 by the driven rate, this pulls ~$18 more than the pair
    // was worth: past the 1% ceiling, and nowhere near the year-long carry budget.
    let res = fx.unwind(1.35).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderExecutionOverWithdrawal
    );
    Ok(())
}

/// The banks must arrive writable for the accrual. The condition fails soft, but when nothing else
/// arms the execution the keeper is still told exactly what was wrong.
#[tokio::test]
async fn read_only_order_banks_are_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;

    let res = fx.unwind_with_readonly_banks().await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderInterestBankNotWritable
    );
    Ok(())
}

/// The same unreadable carry leg must not suppress a price trigger that did fire.
#[tokio::test]
async fn an_unreadable_carry_leg_does_not_block_a_price_trigger() -> anyhow::Result<()> {
    // The pair is worth ~$900, so this stop-loss is breached from the start.
    let mut fx = setup(Params {
        stop_loss: fp!(5000),
        ..Default::default()
    })
    .await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;

    // Read-only banks leave the carry condition unarmed; the price condition carries it anyway.
    fx.unwind_readonly_full(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the price trigger should execute despite the carry leg being unreadable"
    );
    Ok(())
}

/// Freshness is a correctness requirement, not a nicety: a stale lend leg understates its realized
/// rate and would fire the trigger early. Both legs are brought to the current clock in-handler.
#[tokio::test]
async fn execution_accrues_both_order_banks() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;
    assert!(
        fx.bank_last_update(&BankMint::Usdc).await < fx.now,
        "the lend leg should be stale going in, or this proves nothing"
    );

    fx.unwind(1.0).await?;

    assert_eq!(fx.bank_last_update(&BankMint::Usdc).await, fx.now);
    assert_eq!(fx.bank_last_update(&BankMint::Sol).await, fx.now);
    Ok(())
}

/// The variable-borrow premium is a real charge that pushes a spread negative, so it counts on the
/// cost side. A pair whose base rates leave carry short of the margin clears it once premium is on.
#[tokio::test]
async fn the_variable_borrow_premium_counts_toward_the_carry_cost() -> anyhow::Result<()> {
    let mut fx = setup(premium_params()).await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;

    // Base rates alone leave the near-idle borrow leg well short of the arming margin.
    let res = fx.unwind(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestNotNegative);

    // A 25% premium on the SOL liability, collateralised by the USDC lend leg.
    let group_f = &fx.test_f.marginfi_group;
    group_f
        .try_configure_group_premium(PremiumEntry {
            collateral_tag: TAG_COLLATERAL,
            liability_tag: TAG_LIABILITY,
            rate: milli_to_u32(I80F48::from_num(0.25)),
        })
        .await?;
    group_f
        .try_configure_bank_premium(fx.test_f.get_bank(&BankMint::Usdc), TAG_COLLATERAL, true)
        .await?;
    group_f
        .try_configure_bank_premium(fx.test_f.get_bank(&BankMint::Sol), TAG_LIABILITY, true)
        .await?;
    // The snapshot is written by an oracle-carrying instruction, not by the config change itself.
    fx.account_f.try_lending_account_pulse_health().await?;

    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the premium should carry the pair past the arming margin"
    );
    Ok(())
}

/// Arm an order whose lend leg is `bank_f` and assert the anchor carries the venue's own exchange
/// rate. A native leg's multiplier is 1 and cannot distinguish the two; every integration can.
async fn assert_venue_anchor_carries_multiplier(
    test_f: &TestFixture,
    bank_f: &BankFixture,
    user: &MarginfiAccountFixture,
    multiplier: I80F48,
) -> anyhow::Result<()> {
    let sol = test_f.get_bank(&BankMint::Sol);

    // The venue fixtures pin the clock to their own state and refresh only their own feed, so
    // bring SOL's up to the same instant before anything prices it.
    let now = test_f.get_clock().await.unix_timestamp;
    test_f.set_pyth_oracle_timestamp(PYTH_SOL_FEED, now).await;

    // Someone has to supply the SOL the order's borrow leg draws on.
    let lender = test_f.create_marginfi_account().await;
    let lender_sol = sol.mint.create_token_account_and_mint_to(100.0).await;
    lender
        .try_bank_deposit(lender_sol.key, sol, 100.0, None)
        .await?;

    let user_sol = sol.mint.create_empty_token_account().await;
    user.try_bank_borrow(user_sol.key, sol, 1.0).await?;

    let order = user
        .try_place_order_with_interest(
            vec![bank_f.key, sol.key],
            OrderTrigger::StopLoss {
                threshold: fp!(1).into(),
                max_slippage: slippage(5.0),
            },
            Some(interest_config(
                TEST_WINDOW_SECONDS,
                INTEREST_DEFAULT_PATIENCE_SECONDS,
            )),
        )
        .await?;
    user.try_arm_order_interest(order).await?;

    assert_ne!(
        multiplier,
        I80F48::ONE,
        "the venue should price its position away from 1, or this proves nothing"
    );
    let share_value = I80F48::from(bank_f.load().await.asset_share_value);
    assert_eq!(
        I80F48::from(user.load_order(order).await.interest_anchor_asset_index),
        share_value * multiplier
    );
    Ok(())
}

fn sol_bank_setting() -> TestSettings {
    TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Sol,
            config: None,
        }],
        protocol_fees: false,
    }
}

#[tokio::test]
async fn a_kamino_lend_leg_anchors_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_kamino_bank(Some(sol_bank_setting())).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_kamino_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;

    // klend's collateral exchange rate: liquidity per collateral token.
    let (total_liq, total_col) = setup.load_reserve().await.scaled_supplies()?;
    assert_venue_anchor_carries_multiplier(
        &setup.test_f,
        &setup.bank_f,
        &user,
        total_liq / total_col,
    )
    .await
}

#[tokio::test]
async fn a_drift_lend_leg_anchors_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_drift_bank(Some(sol_bank_setting())).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_drift_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;

    // The mock market boots with no accrued interest, so its multiplier is exactly 1. Advance it,
    // as the drift deposit/withdraw tests do, so the anchor has something to carry.
    {
        let spot_market_key = setup.bank_f.load().await.integration_acc_1;
        let mut account = setup.test_f.try_load(&spot_market_key).await?.unwrap();
        let spot_market = bytemuck::from_bytes_mut::<MinimalSpotMarket>(
            &mut account.data[8..8 + std::mem::size_of::<MinimalSpotMarket>()],
        );
        spot_market.cumulative_deposit_interest =
            (SPOT_CUMULATIVE_INTEREST_PRECISION * 3 / 2).to_le_bytes();
        setup
            .test_f
            .context
            .borrow_mut()
            .set_account(&spot_market_key, &AccountSharedData::from(account));
    }

    // Drift's scaled balances grow by the market's cumulative deposit interest.
    let cumulative =
        u128::from_le_bytes(setup.load_spot_market().await.cumulative_deposit_interest);
    let multiplier = I80F48::from_num(cumulative)
        / I80F48::from_num(drift_mocks::constants::SPOT_CUMULATIVE_INTEREST_PRECISION);
    assert_venue_anchor_carries_multiplier(&setup.test_f, &setup.bank_f, &user, multiplier).await
}

#[tokio::test]
async fn a_juplend_lend_leg_anchors_through_the_venue_multiplier() -> anyhow::Result<()> {
    let setup = TestFixture::setup_juplend_bank(Some(sol_bank_setting())).await;
    let (user, user_token) = setup.create_user_with_liquidity(1_000.0).await;
    setup
        .test_f
        .run_juplend_deposit(&setup.bank_f, &user, user_token.key, 1_000_000_000)
        .await?;

    // The mock lending state boots at parity, so advance its exchange price the way the juplend
    // withdraw tests do, leaving the multiplier something the anchor must actually carry.
    {
        let mut account = setup.test_f.try_load(&setup.lending).await?.unwrap();
        let lending = bytemuck::from_bytes_mut::<JuplendLending>(
            &mut account.data[8..8 + std::mem::size_of::<JuplendLending>()],
        );
        lending.token_exchange_price = (EXCHANGE_PRICES_PRECISION * 3 / 2) as u64;
        setup
            .test_f
            .context
            .borrow_mut()
            .set_account(&setup.lending, &AccountSharedData::from(account));
    }

    // JupLend's fToken exchange price, which the liquidity layer advances as it earns.
    let multiplier = I80F48::from_num(setup.load_lending().await.token_exchange_price)
        / I80F48::from_num(EXCHANGE_PRICES_PRECISION);
    assert_venue_anchor_carries_multiplier(&setup.test_f, &setup.bank_f, &user, multiplier).await
}

/// Anyone can arm at the exact moment an order comes of age. Rotation stops that resetting the
/// measurement: the displaced anchor still carries a full window.
#[tokio::test]
async fn a_re_arm_at_the_moment_of_maturity_cannot_block_execution() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;

    // A third party arms the instant the order became executable.
    fx.arm().await?;

    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the displaced anchor should still carry the execution"
    );
    Ok(())
}

/// An anchor older than `INTEREST_ANCHOR_MAX_AGE_WINDOWS` stops counting, so an order nobody has
/// re-armed cannot fire on a span that reaches back into a rate regime which has since ended.
#[tokio::test]
async fn an_anchor_left_to_age_out_stops_being_executable() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.arm().await?;
    fx.advance(TEST_WINDOW * i64::from(INTEREST_ANCHOR_MAX_AGE_WINDOWS) + 1)
        .await;

    let res = fx.unwind(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestAnchorStale);

    // Re-arming restores it, but only after a fresh window has actually been measured.
    fx.arm().await?;
    let res = fx.unwind(1.0).await;
    assert_custom_error!(res.unwrap_err(), MarginfiError::OrderInterestWindowTooShort);

    fx.advance(TEST_WINDOW).await;
    fx.unwind(1.0).await?;
    assert!(fx.test_f.try_load(&fx.order).await?.is_none());
    Ok(())
}

/// The trigger walks every active balance to find its two legs and the sandwich prices them all,
/// so a nearly-full account stresses both, and the compute budget carrying them.
#[tokio::test]
async fn execution_holds_up_with_the_account_near_max_balances() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    // Unrelated deposits, none of which the order touches.
    for mint in [
        BankMint::Fixed,
        BankMint::FixedLow,
        BankMint::SolSwbPull,
        BankMint::SolSwbOrigFee,
        BankMint::SolEquivalent,
        BankMint::PyUSD,
    ] {
        let bank = fx.test_f.get_bank(&mint);
        let funded = bank.mint.create_token_account_and_mint_to(10.0).await;
        fx.account_f
            .try_bank_deposit(funded.key, bank, 10.0, None)
            .await?;
    }

    let active = fx
        .account_f
        .load()
        .await
        .lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
        .count();
    assert_eq!(active, 8, "six pads plus the order's own two legs");

    fx.arm().await?;
    fx.advance(TEST_WINDOW).await;
    fx.unwind_with_budget(1.0, 1_400_000).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the order should execute with the account nearly full"
    );
    // Every unrelated deposit is left exactly as it was.
    let after = fx.account_f.load().await;
    assert_eq!(
        after
            .lending_account
            .balances
            .iter()
            .filter(|b| b.is_active())
            .count(),
        active - 1,
        "only the borrow leg should have closed"
    );
    Ok(())
}
