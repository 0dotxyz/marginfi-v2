//! A USDC lend against a SOL borrow with a standing interest-trigger order on the pair, and the
//! drivers that move the SOL rate: the fixture the interest-order tests run against.

use fixed::types::I80F48;
use fixed_macro::types::I80F48 as fp;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::prelude::*;
use fixtures::test::{
    PYTH_PUSH_SOL_FULLV_FEED, PYTH_PUSH_SOL_PARTV_FEED, PYTH_PYUSD_FEED, PYTH_SOL_EQUIVALENT_FEED,
    PYTH_SOL_FEED, PYTH_USDC_FEED,
};
use marginfi_type_crate::constants::{
    INTEREST_DEFAULT_EXIT_BUDGET_SECONDS, INTEREST_MAX_EXIT_BUDGET_SECONDS,
    INTEREST_MAX_WINDOW_SECONDS,
};
use marginfi_type_crate::types::{
    centi_to_u32, milli_to_u32, Bank, InterestTriggerConfig, OrderTrigger,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_program_test::BanksClientError;
use solana_sdk::{
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

pub const TAG_COLLATERAL: u16 = 100;
pub const TAG_LIABILITY: u16 = 200;

// A near-idle borrow leg (1 SOL against a 1,000 SOL float) so its baseline rate is negligible and
// whatever a driver does to utilization is the only thing the window measures.
pub const SPIKE_LENDER_SOL: f64 = 1_000.0;
const SPIKE_BORROW_SOL: f64 = 1.0;
pub const SPIKE_BORROW: f64 = 800.0;
pub const SPIKE_COLLATERAL: f64 = 20_000.0;
/// The measurement window every fixture here configures.
pub const TEST_WINDOW_SECONDS: u32 = INTEREST_MAX_WINDOW_SECONDS;
pub const TEST_WINDOW: i64 = TEST_WINDOW_SECONDS as i64;
/// Above anything the near-idle baseline produces, below what a held ~90% utilization does.
const SPIKE_MARGIN_APR: f64 = 0.02;

/// A near-idle borrow leg at the default size, with a margin small enough that base rates alone
/// miss it and a premium alone clears it.
pub fn premium_params() -> Params {
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
pub fn spike_params() -> Params {
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

pub fn interest_config(window: u32, exit_budget: u32) -> InterestTriggerConfig {
    InterestTriggerConfig {
        window_seconds: Some(window),
        exit_budget_seconds: Some(exit_budget),
        min_negative_apr: None,
    }
}

/// Pin the clock and republish every Pyth feed a fixture here can touch, so a padded account's
/// unrelated balances stay priceable across the long steps these tests take.
async fn pin_clock(test_f: &TestFixture, ts: i64) {
    test_f
        .pin_clock(
            ts,
            &[
                PYTH_SOL_FEED,
                PYTH_USDC_FEED,
                PYTH_SOL_EQUIVALENT_FEED,
                PYTH_PYUSD_FEED,
                PYTH_PUSH_SOL_FULLV_FEED,
                PYTH_PUSH_SOL_PARTV_FEED,
            ],
        )
        .await;
}

pub struct InterestFixture {
    pub test_f: TestFixture,
    pub account_f: MarginfiAccountFixture,
    pub order: Pubkey,
    pub keeper: Keypair,
    pub keeper_sol: Pubkey,
    pub keeper_usdc: Pubkey,
    pub now: i64,
    /// Sizes the unwind withdrawal, so a fixture with a smaller borrow is unwound to scale.
    pub borrow_sol: f64,
}

pub struct Params {
    pub interest: Option<InterestTriggerConfig>,
    /// Far below the pair's value unless a test wants the price condition live.
    pub stop_loss: I80F48,
    /// SOL the lender supplies. A large float against a small borrow leaves the bank near zero
    /// utilization, so its rate is whatever a test drives it to and nothing else.
    pub lender_sol: f64,
    pub borrow_sol: f64,
    pub max_slippage_pct: f64,
    /// Seconds between the banks' first readings and the order being placed.
    pub history_before_placement: i64,
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
pub struct RateDriver {
    pub account: MarginfiAccountFixture,
    pub sol_account: Pubkey,
}

impl RateDriver {
    /// Repay the whole borrow, returning the bank to its baseline utilization.
    pub async fn release(&self, fx: &InterestFixture) -> anyhow::Result<()> {
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
pub async fn setup(p: Params) -> anyhow::Result<InterestFixture> {
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
    test_f.fund_keeper(&keeper).await?;
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
pub async fn drive_sol_rate(
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
    pub async fn advance(&mut self, seconds: i64) {
        self.now += seconds;
        pin_clock(&self.test_f, self.now).await;
    }

    pub async fn pulse(&self, mint: &BankMint) -> anyhow::Result<()> {
        self.test_f
            .marginfi_group
            .try_pulse_bank_price_cache(self.test_f.get_bank(mint))
            .await?;
        Ok(())
    }

    /// Close out the borrow leg's accrual at the current rate, so the next span accrues only at
    /// whatever rate a driver then sets.
    pub async fn settle_borrow_rate(&self) -> anyhow::Result<()> {
        let sol = self.test_f.get_bank(&BankMint::Sol);
        self.test_f.marginfi_group.try_accrue_interest(sol).await?;
        Ok(())
    }

    pub async fn load_bank(&self, mint: &BankMint) -> Bank {
        self.test_f.get_bank(mint).load().await
    }

    pub async fn bank_last_update(&self, mint: &BankMint) -> i64 {
        self.load_bank(mint).await.last_update
    }

    pub async fn recorded_readings(&self, mint: &BankMint) -> usize {
        self.load_bank(mint).await.recorded_rate_readings().count()
    }

    /// Repay the SOL liability from the keeper's own tokens, pull `scale` times the covering USDC
    /// back out, and close. `scale` is against the ORIGINAL borrow, not the accrued liability.
    pub async fn unwind(&self, scale: f64) -> std::result::Result<(), BanksClientError> {
        self.unwind_with_budget(scale, 0).await
    }

    /// `compute_units` of 0 leaves the default budget; a fuller account needs more than 200k.
    pub async fn unwind_with_budget(
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
    pub async fn unwind_with_readonly_banks(&self) -> std::result::Result<(), BanksClientError> {
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
    pub async fn unwind_readonly_full(
        &self,
        scale: f64,
    ) -> std::result::Result<(), BanksClientError> {
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
        self.test_f.banks_client().process_transaction(tx).await
    }
}
