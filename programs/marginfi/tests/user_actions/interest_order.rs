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
    BANK_RATE_READING_SPACING_SECONDS, INTEREST_DEFAULT_EXIT_BUDGET_SECONDS,
    INTEREST_MAX_EXIT_BUDGET_SECONDS, INTEREST_MAX_WINDOW_SECONDS,
};
use marginfi_type_crate::types::{
    centi_to_u32, milli_to_u32, Bank, InterestTriggerConfig, OrderTrigger, PremiumEntry,
    RateReading,
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

/// `program-test` boots at timestamp 0, which a rate reading treats as never written, so tests pin
/// a real time.
const BASE_TS: i64 = 1_700_000_000;
const ASSET_DEPOSIT: f64 = 1_000.0; // USDC, $1,000 at the $1 test oracle
const LIABILITY_BORROW: f64 = 10.0; // SOL, $100 at the $10 test oracle
const SOL_PRICE: f64 = 10.0;

const TAG_COLLATERAL: u16 = 100;
const TAG_LIABILITY: u16 = 200;

// A near-idle borrow leg (1 SOL against a 1,000 SOL float) so its baseline rate is negligible and
// whatever a driver does to utilization is the only thing the window measures.
const SPIKE_LENDER_SOL: f64 = 1_000.0;
const SPIKE_BORROW_SOL: f64 = 1.0;
const SPIKE_BORROW: f64 = 800.0;
const SPIKE_COLLATERAL: f64 = 20_000.0;
/// The measurement window every fixture here configures.
const TEST_WINDOW_SECONDS: u32 = INTEREST_MAX_WINDOW_SECONDS;
const TEST_WINDOW: i64 = TEST_WINDOW_SECONDS as i64;
/// Above anything the near-idle baseline produces, below what a held ~90% utilization does.
const SPIKE_MARGIN_APR: f64 = 0.02;

/// A near-idle borrow leg at the default size, with a margin small enough that base rates alone
/// miss it and a premium alone clears it.
fn premium_params() -> Params {
    Params {
        interest: Some(InterestTriggerConfig {
            window_seconds: Some(TEST_WINDOW_SECONDS),
            exit_budget_seconds: Some(INTEREST_MAX_EXIT_BUDGET_SECONDS),
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
            exit_budget_seconds: Some(INTEREST_MAX_EXIT_BUDGET_SECONDS),
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

fn interest_config(window: u32, exit_budget: u32) -> InterestTriggerConfig {
    InterestTriggerConfig {
        window_seconds: Some(window),
        exit_budget_seconds: Some(exit_budget),
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
    /// Seconds between the banks' first readings and the order being placed.
    history_before_placement: i64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            interest: Some(interest_config(
                TEST_WINDOW_SECONDS,
                INTEREST_DEFAULT_EXIT_BUDGET_SECONDS,
            )),
            stop_loss: fp!(1),
            lender_sol: 100.0,
            borrow_sol: LIABILITY_BORROW,
            max_slippage_pct: 5.0,
            history_before_placement: 0,
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

    // The borrow priced the SOL bank, which took its first reading. The lend leg has only been
    // deposited into, which prices nothing, so it is pulsed for its own.
    test_f
        .marginfi_group
        .try_pulse_bank_price_cache(usdc)
        .await?;

    let now = BASE_TS + p.history_before_placement;
    if p.history_before_placement > 0 {
        pin_clock(&test_f, now).await;
    }

    let trigger = OrderTrigger::StopLoss {
        threshold: p.stop_loss.into(),
        max_slippage: slippage(p.max_slippage_pct),
    };
    let legs = vec![usdc.key, sol.key];
    let order = match p.interest {
        Some(interest) => {
            account_f
                .try_place_interest_order(legs, trigger, interest)
                .await?
        }
        None => account_f.try_place_order(legs, trigger).await?,
    };

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
        now,
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

    async fn pulse(&self, mint: &BankMint) -> anyhow::Result<()> {
        self.test_f
            .marginfi_group
            .try_pulse_bank_price_cache(self.test_f.get_bank(mint))
            .await?;
        Ok(())
    }

    /// Close out the borrow leg's accrual at the current rate, so the next span accrues only at
    /// whatever rate a driver then sets.
    async fn settle_borrow_rate(&self) -> anyhow::Result<()> {
        let sol = self.test_f.get_bank(&BankMint::Sol);
        self.test_f.marginfi_group.try_accrue_interest(sol).await?;
        Ok(())
    }

    async fn load_bank(&self, mint: &BankMint) -> Bank {
        self.test_f.get_bank(mint).load().await
    }

    async fn bank_last_update(&self, mint: &BankMint) -> i64 {
        self.load_bank(mint).await.last_update
    }

    async fn recorded_readings(&self, mint: &BankMint) -> usize {
        self.load_bank(mint).await.recorded_rate_readings().count()
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

#[tokio::test]
async fn interest_order_fires_once_the_pair_has_bled_for_a_window() -> anyhow::Result<()> {
    // The default stop-loss sits far below the pair's value, so only carry can fire this.
    let mut fx = setup(Params::default()).await?;

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

#[tokio::test]
async fn a_carry_order_fires_on_history_recorded_before_it_was_placed() -> anyhow::Result<()> {
    let fx = setup(Params {
        history_before_placement: TEST_WINDOW,
        ..Default::default()
    })
    .await?;

    // Placed a full window after the banks' first readings, so it is executable at once.
    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the order should execute on history older than itself"
    );
    Ok(())
}

#[tokio::test]
async fn interest_order_cannot_execute_before_its_window_elapses() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW - 1).await;
    let res = fx.unwind(1.0).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderInterestHistoryTooShort
    );
    Ok(())
}

#[tokio::test]
async fn readings_inside_the_spacing_are_not_recorded() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;
    assert_eq!(fx.recorded_readings(&BankMint::Usdc).await, 1);

    fx.advance(BANK_RATE_READING_SPACING_SECONDS - 1).await;
    fx.pulse(&BankMint::Usdc).await?;
    assert_eq!(fx.recorded_readings(&BankMint::Usdc).await, 1);

    fx.advance(1).await;
    fx.pulse(&BankMint::Usdc).await?;
    let bank = fx.load_bank(&BankMint::Usdc).await;
    assert_eq!(bank.recorded_rate_readings().count(), 2);
    // A native bank has no venue multiplier, so its reading is its share values alone.
    assert_eq!(
        *bank.newest_rate_reading().unwrap(),
        RateReading::new(
            bank.asset_share_value.into(),
            bank.liability_share_value.into(),
            fx.now
        )
        .unwrap()
    );
    Ok(())
}

#[tokio::test]
async fn an_unwind_costlier_than_the_carry_budget_is_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        interest: Some(interest_config(TEST_WINDOW_SECONDS, 3_600)),
        ..Default::default()
    })
    .await?;

    fx.advance(TEST_WINDOW).await;
    let res = fx.unwind(1.04).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderInterestCostExceedsCarry
    );
    Ok(())
}

#[tokio::test]
async fn a_price_trigger_still_fires_before_the_carry_window_elapses() -> anyhow::Result<()> {
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
        "the price trigger should execute despite the carry window being short"
    );
    Ok(())
}

#[tokio::test]
async fn a_brief_spike_inside_the_window_does_not_fire() -> anyhow::Result<()> {
    let mut fx = setup(spike_params()).await?;

    fx.settle_borrow_rate().await?;

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

#[tokio::test]
async fn the_same_rate_sustained_across_the_window_does_fire() -> anyhow::Result<()> {
    let mut fx = setup(spike_params()).await?;

    fx.settle_borrow_rate().await?;

    let _driver = drive_sol_rate(&fx, SPIKE_BORROW, SPIKE_COLLATERAL).await?;
    fx.advance(TEST_WINDOW).await;
    fx.settle_borrow_rate().await?;

    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "a rate held for the whole window should fire the exit"
    );
    Ok(())
}

#[tokio::test]
async fn both_conditions_met_lets_either_cost_bound_carry_the_execution() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        // Already breached at placement, so the price condition fires too.
        stop_loss: fp!(5000),
        // An hour's worth of loss is almost no budget at all.
        interest: Some(interest_config(TEST_WINDOW_SECONDS, 3_600)),
        ..Default::default()
    })
    .await?;

    fx.advance(TEST_WINDOW).await;
    fx.unwind(1.04).await?;

    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the price bound should carry an execution the carry budget cannot"
    );
    Ok(())
}

#[tokio::test]
async fn the_slippage_ceiling_binds_even_when_the_budget_would_allow_more() -> anyhow::Result<()> {
    let mut fx = setup(Params {
        // A year's worth of loss makes the budget far larger than this unwind costs.
        interest: Some(interest_config(
            TEST_WINDOW_SECONDS,
            INTEREST_MAX_EXIT_BUDGET_SECONDS,
        )),
        max_slippage_pct: 1.0,
        // A float the rate driver below can actually borrow against.
        lender_sol: SPIKE_LENDER_SOL,
        ..Default::default()
    })
    .await?;

    fx.settle_borrow_rate().await?;

    let _driver = drive_sol_rate(&fx, SPIKE_BORROW, SPIKE_COLLATERAL).await?;
    fx.advance(TEST_WINDOW).await;
    fx.settle_borrow_rate().await?;

    // This pulls ~$35 more than the ~$100 liability was worth: past the 1% ceiling, and nowhere
    // near the year of driven-rate loss the carry budget allows.
    let res = fx.unwind(1.35).await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderExecutionOverWithdrawal
    );
    Ok(())
}

#[tokio::test]
async fn read_only_order_banks_are_rejected() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW).await;

    let res = fx.unwind_with_readonly_banks().await;
    assert_custom_error!(
        res.unwrap_err(),
        MarginfiError::OrderInterestBankNotWritable
    );
    Ok(())
}

#[tokio::test]
async fn an_unreadable_carry_leg_does_not_block_a_price_trigger() -> anyhow::Result<()> {
    // The pair is worth ~$900, so this stop-loss is breached from the start.
    let mut fx = setup(Params {
        stop_loss: fp!(5000),
        ..Default::default()
    })
    .await?;

    fx.advance(TEST_WINDOW).await;

    fx.unwind_readonly_full(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the price trigger should execute despite the carry leg being unreadable"
    );
    Ok(())
}

#[tokio::test]
async fn execution_accrues_both_order_banks() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

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

#[tokio::test]
async fn the_variable_borrow_premium_counts_toward_the_carry_cost() -> anyhow::Result<()> {
    let mut fx = setup(premium_params()).await?;

    fx.advance(TEST_WINDOW).await;

    // Base rates alone leave the near-idle borrow leg well short of the trigger margin.
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
        "the premium should carry the pair past the trigger margin"
    );
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
    let multiplier = I80F48::from_num(cumulative)
        / I80F48::from_num(drift_mocks::constants::SPOT_CUMULATIVE_INTEREST_PRECISION);
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

#[tokio::test]
async fn a_pulse_at_the_moment_of_maturity_cannot_displace_the_measurement() -> anyhow::Result<()> {
    let mut fx = setup(Params::default()).await?;

    fx.advance(TEST_WINDOW).await;

    // A third party prices both banks the instant the order became executable.
    fx.pulse(&BankMint::Usdc).await?;
    fx.pulse(&BankMint::Sol).await?;

    fx.unwind(1.0).await?;
    assert!(
        fx.test_f.try_load(&fx.order).await?.is_none(),
        "the older readings should still carry the execution"
    );
    Ok(())
}

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
