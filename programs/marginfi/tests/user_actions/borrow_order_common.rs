//! The borrow-order tests' fixture: a lender-floated USDC bank, a SOL-collateralized borrower with
//! an order on it, and a keeper.

use anchor_lang::{InstructionData, ToAccountMetas};
use fixed::types::I80F48;
use fixtures::bank::BankFixture;
use fixtures::marginfi_account::MarginfiAccountFixture;
use fixtures::spl::{balance_of, TokenAccountFixture};
use fixtures::test::{DEFAULT_USDC_TEST_BANK_CONFIG, PYTH_SOL_FEED, PYTH_USDC_FEED};
use fixtures::{native, prelude::*};
use marginfi::state::{bank::BankImpl, rate::borrow_rate_at};
use marginfi_type_crate::constants::{BORROW_ORDER_RECORD_SEED, INTEREST_MIN_WINDOW_SECONDS};
use marginfi_type_crate::types::{milli_to_u32, InterestRateConfigOpt, PremiumEntry};
use solana_program_test::BanksClientError;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer as _,
    transaction::Transaction,
};

/// `program-test` boots at timestamp 0, which a rate reading treats as never written.
pub const BASE_TS: i64 = 1_700_000_000;
pub const WINDOW: i64 = INTEREST_MIN_WINDOW_SECONDS as i64;
/// USDC the lender floats, against which the order borrows.
pub const FLOAT: f64 = 10_000.0;
/// SOL the borrower posts, at the $10 test oracle.
pub const COLLATERAL: f64 = 500.0;
const TAG_STABLE: u16 = 100;
const TAG_SOL: u16 = 200;

pub fn apr(percent: f64) -> u32 {
    milli_to_u32(I80F48::from_num(percent / 100.0))
}

pub fn usdc(ui: f64) -> u64 {
    native!(ui, "USDC", f64)
}

async fn pin_clock(test_f: &TestFixture, ts: i64) {
    test_f.pin_clock(ts, &[PYTH_SOL_FEED, PYTH_USDC_FEED]).await;
}

pub struct BorrowOrderFixture {
    pub test_f: TestFixture,
    pub account_f: MarginfiAccountFixture,
    pub order: Pubkey,
    pub keeper: Keypair,
    pub keeper_usdc: Pubkey,
    /// The authority's USDC ATA, the only place a wallet fill may deliver to.
    pub owner_usdc: Pubkey,
    /// A second native USDC bank the order may redeploy into, when the params ask for one.
    pub redeploy_bank: Option<BankFixture>,
    pub now: i64,
}

pub struct Params {
    pub open_below: u32,
    /// Repay from the destination bank once the realized rate rises over this; `None` opens only.
    pub close_above: Option<u32>,
    pub amount: f64,
    pub cooldown: u32,
    pub window: u32,
    /// Lamports the keeper is paid per fill from the account's fee pool.
    pub keeper_tip: u64,
    /// Redeploy borrowed funds into a second same-mint bank; otherwise they go to the wallet.
    pub redeploy: bool,
    /// USDC a third party borrows before the first reading, so the bank charges a real rate.
    pub utilize: f64,
    /// Origination fee the bank books on top of every borrow, as a fraction.
    pub origination_fee: f64,
    /// Charge a 1% premium on USDC borrowed against SOL.
    pub premium: bool,
    /// SOL the borrower posts, at the $10 test oracle.
    pub collateral: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            open_below: apr(100.0),
            close_above: None,
            amount: 1_000.0,
            cooldown: 0,
            window: INTEREST_MIN_WINDOW_SECONDS,
            keeper_tip: 0,
            redeploy: false,
            utilize: 0.0,
            origination_fee: 0.0,
            premium: false,
            collateral: COLLATERAL,
        }
    }
}

/// A redeploying order with a close level, opened while the bank was idle.
pub fn round_trip() -> Params {
    Params {
        redeploy: true,
        close_above: Some(apr(150.0)),
        ..Default::default()
    }
}

pub async fn setup(p: Params) -> anyhow::Result<BorrowOrderFixture> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;
    pin_clock(&test_f, BASE_TS).await;

    let usdc = test_f.get_bank(&BankMint::Usdc);
    let sol = test_f.get_bank(&BankMint::Sol);

    if p.origination_fee > 0.0 {
        test_f
            .marginfi_group
            .try_lending_pool_configure_bank_interest_only(
                usdc,
                InterestRateConfigOpt {
                    protocol_origination_fee: Some(I80F48::from_num(p.origination_fee).into()),
                    ..Default::default()
                },
            )
            .await?;
    }
    if p.premium {
        let group_f = &test_f.marginfi_group;
        group_f
            .try_configure_group_premium(PremiumEntry {
                collateral_tag: TAG_SOL,
                liability_tag: TAG_STABLE,
                rate: apr(1.0),
            })
            .await?;
        group_f
            .try_configure_bank_premium(usdc, TAG_STABLE, true)
            .await?;
        group_f
            .try_configure_bank_premium(sol, TAG_SOL, true)
            .await?;
    }

    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = usdc.mint.create_token_account_and_mint_to(FLOAT).await;
    lender
        .try_bank_deposit(lender_usdc.key, usdc, FLOAT, None)
        .await?;

    if p.utilize > 0.0 {
        let driver_collateral = p.utilize / 10.0 * 3.0;
        let driver = test_f.create_marginfi_account().await;
        let driver_sol = sol
            .mint
            .create_token_account_and_mint_to(driver_collateral)
            .await;
        driver
            .try_bank_deposit(driver_sol.key, sol, driver_collateral, None)
            .await?;
        let driver_usdc = usdc.mint.create_empty_token_account().await;
        driver
            .try_bank_borrow(driver_usdc.key, usdc, p.utilize)
            .await?;
    }

    let account_f = test_f.create_marginfi_account().await;
    let borrower_sol = sol
        .mint
        .create_token_account_and_mint_to(p.collateral)
        .await;
    account_f
        .try_bank_deposit(borrower_sol.key, sol, p.collateral, None)
        .await?;
    let owner = test_f.context.borrow().payer.pubkey();
    let owner_usdc = TokenAccountFixture::new_from_ata(
        test_f.context.clone(),
        &usdc.mint.key,
        &owner,
        &usdc.mint.token_program,
    )
    .await
    .key;

    let redeploy_bank = if p.redeploy {
        Some(
            test_f
                .marginfi_group
                .try_lending_pool_add_bank_with_seed(
                    &test_f.usdc_mint,
                    None,
                    *DEFAULT_USDC_TEST_BANK_CONFIG,
                    901,
                )
                .await?,
        )
    } else {
        None
    };

    let keeper = Keypair::new();
    test_f.fund_keeper(&keeper).await?;
    // A full close's repay rounds up to the atom, which the keeper covers out of a small float.
    let keeper_usdc = usdc
        .mint
        .create_token_account_and_mint_to_with_owner(&keeper.pubkey(), 1.0)
        .await
        .key;

    let order = account_f.borrow_order_pda(usdc.key);
    let fx = BorrowOrderFixture {
        test_f,
        account_f,
        order,
        keeper,
        keeper_usdc,
        owner_usdc,
        redeploy_bank,
        now: BASE_TS,
    };
    fx.place(&p).await?;
    Ok(fx)
}

impl BorrowOrderFixture {
    pub fn usdc(&self) -> &BankFixture {
        self.test_f.get_bank(&BankMint::Usdc)
    }

    pub fn sol(&self) -> &BankFixture {
        self.test_f.get_bank(&BankMint::Sol)
    }

    pub fn dst(&self) -> &BankFixture {
        self.redeploy_bank.as_ref().unwrap()
    }

    pub async fn advance(&mut self, seconds: i64) {
        self.now += seconds;
        pin_clock(&self.test_f, self.now).await;
    }

    pub async fn process(
        &self,
        ixs: &[Instruction],
        signer: &Keypair,
    ) -> Result<(), BanksClientError> {
        let blockhash = self.test_f.get_latest_blockhash().await;
        let tx =
            Transaction::new_signed_with_payer(ixs, Some(&signer.pubkey()), &[signer], blockhash);
        self.test_f.banks_client().process_transaction(tx).await
    }

    pub fn payer(&self) -> Keypair {
        self.test_f.context.borrow().payer.insecure_clone()
    }

    pub async fn tx_fee(&self) -> u64 {
        let blockhash = self.test_f.get_latest_blockhash().await;
        let message = Transaction::new_signed_with_payer(
            &[self.start_open_ix()],
            Some(&self.keeper.pubkey()),
            &[&self.keeper],
            blockhash,
        )
        .message;
        self.test_f
            .banks_client()
            .get_fee_for_message(message)
            .await
            .unwrap()
            .unwrap()
    }

    pub async fn lamports(&self, key: Pubkey) -> u64 {
        self.test_f.banks_client().get_balance(key).await.unwrap()
    }

    async fn place(&self, p: &Params) -> Result<(), BanksClientError> {
        self.place_on(self.usdc().key, p).await
    }

    pub async fn place_on(&self, bank: Pubkey, p: &Params) -> Result<(), BanksClientError> {
        let payer = self.payer();
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::PlaceBorrowOrder {
                group: self.test_f.marginfi_group.key,
                marginfi_account: self.account_f.key,
                authority: payer.pubkey(),
                bank,
                destination_bank: self.redeploy_bank.as_ref().map(|b| b.key),
                borrow_order: self.account_f.borrow_order_pda(bank),
                fee_state: self.test_f.marginfi_group.fee_state,
                global_fee_wallet: self.test_f.marginfi_group.fee_wallet,
                fee_payer: payer.pubkey(),
                system_program: anchor_lang::system_program::ID,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountPlaceBorrowOrder {
                amount: usdc(p.amount),
                open_below_apr: p.open_below,
                close_above_apr: p.close_above,
                cooldown_seconds: Some(p.cooldown),
                window_seconds: Some(p.window),
                keeper_tip: Some(p.keeper_tip),
            }
            .data(),
        };
        self.process(&[ix], &payer).await
    }

    pub async fn update(
        &self,
        amount: Option<u64>,
        close_above_apr: Option<u32>,
    ) -> Result<(), BanksClientError> {
        let payer = self.payer();
        let ix = Instruction {
            program_id: marginfi::ID,
            accounts: marginfi::accounts::UpdateBorrowOrder {
                marginfi_account: self.account_f.key,
                authority: payer.pubkey(),
                borrow_order: self.order,
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountUpdateBorrowOrder {
                amount,
                open_below_apr: None,
                close_above_apr,
                cooldown_seconds: None,
                window_seconds: None,
                keeper_tip: None,
            }
            .data(),
        };
        self.process(&[ix], &payer).await
    }

    fn record(&self) -> Pubkey {
        Pubkey::find_program_address(
            &[BORROW_ORDER_RECORD_SEED.as_bytes(), self.order.as_ref()],
            &marginfi::ID,
        )
        .0
    }

    fn start_accounts(&self) -> marginfi::accounts::StartBorrowOrderFill {
        marginfi::accounts::StartBorrowOrderFill {
            group: self.test_f.marginfi_group.key,
            marginfi_account: self.account_f.key,
            borrow_order: self.order,
            bank: self.usdc().key,
            executor: self.keeper.pubkey(),
            borrow_order_record: self.record(),
            fee_payer: self.keeper.pubkey(),
            instruction_sysvar: solana_instructions_sysvar::id(),
            system_program: anchor_lang::system_program::ID,
        }
    }

    pub fn start_open_ix(&self) -> Instruction {
        Instruction {
            program_id: marginfi::ID,
            accounts: self.start_accounts().to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountStartBorrowOrderOpen {}.data(),
        }
    }

    pub fn start_close_ix(&self) -> Instruction {
        Instruction {
            program_id: marginfi::ID,
            accounts: self.start_accounts().to_account_metas(Some(true)),
            data: marginfi::instruction::MarginfiAccountStartBorrowOrderClose {}.data(),
        }
    }

    pub async fn borrow_ix_for(
        &self,
        account: &MarginfiAccountFixture,
        bank: &BankFixture,
        destination: Pubkey,
        ui: f64,
    ) -> Instruction {
        account
            .make_borrow_ix_with_authority(destination, bank, ui, self.keeper.pubkey())
            .await
    }

    pub async fn borrow_ix(&self, bank: &BankFixture, destination: Pubkey, ui: f64) -> Instruction {
        self.borrow_ix_for(&self.account_f, bank, destination, ui)
            .await
    }

    pub async fn deposit_ix(&self, bank: &BankFixture, ui: f64) -> Instruction {
        self.account_f
            .make_deposit_ix_with_authority(self.keeper_usdc, bank, ui, None, self.keeper.pubkey())
            .await
    }

    pub async fn withdraw_ix(&self, bank: &BankFixture, ui: f64) -> Instruction {
        self.account_f
            .make_withdraw_ix_with_authority(self.keeper_usdc, bank, ui, None, self.keeper.pubkey())
            .await
    }

    pub async fn repay_ix(&self, bank: &BankFixture, ui: f64, all: bool) -> Instruction {
        self.account_f
            .make_repay_ix_with_authority(
                self.keeper_usdc,
                bank,
                ui,
                all.then_some(true),
                self.keeper.pubkey(),
            )
            .await
    }

    /// `opened` and `closed` are the balances the legs create or empty mid-transaction, which the
    /// health set has to include or leave out.
    async fn end_ix(&self, open: bool, opened: Vec<Pubkey>, closed: Vec<Pubkey>) -> Instruction {
        let accounts = marginfi::accounts::EndBorrowOrderFill {
            group: self.test_f.marginfi_group.key,
            marginfi_account: self.account_f.key,
            borrow_order: self.order,
            bank: self.usdc().key,
            destination_bank: self.redeploy_bank.as_ref().map(|b| b.key),
            executor: self.keeper.pubkey(),
            borrow_order_record: self.record(),
            fee_pool: self.account_f.rebalance_fee_pool_pda(),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(Some(true));
        let data = if open {
            marginfi::instruction::MarginfiAccountEndBorrowOrderOpen {}.data()
        } else {
            marginfi::instruction::MarginfiAccountEndBorrowOrderClose {}.data()
        };
        let mut end = Instruction {
            program_id: marginfi::ID,
            accounts,
            data,
        };
        end.accounts.extend(
            self.account_f
                .load_observation_account_metas(opened, closed)
                .await,
        );
        end
    }

    pub async fn end_open_ix(&self, opened: Vec<Pubkey>) -> Instruction {
        self.end_ix(true, opened, vec![]).await
    }

    pub async fn end_close_ix(&self, closed: Vec<Pubkey>) -> Instruction {
        self.end_ix(false, vec![], closed).await
    }

    pub async fn fill(&self, ui_amount: f64) -> Result<(), BanksClientError> {
        self.fill_native(usdc(ui_amount)).await
    }

    pub async fn fill_native(&self, amount: u64) -> Result<(), BanksClientError> {
        let usdc = self.usdc();
        let ui = amount as f64 / 1e6;
        let mut ixs = vec![self.start_open_ix()];
        let mut opened = vec![usdc.key];
        match &self.redeploy_bank {
            Some(dst) => {
                ixs.push(self.borrow_ix(usdc, self.keeper_usdc, ui).await);
                ixs.push(self.deposit_ix(dst, ui).await);
                opened.push(dst.key);
            }
            None => ixs.push(self.borrow_ix(usdc, self.owner_usdc, ui).await),
        }
        ixs.push(self.end_open_ix(opened).await);
        self.process(&ixs, &self.keeper).await
    }

    /// Withdraw `ui` USDC from the destination and repay it, or the whole debt with `all`.
    pub async fn close(&self, ui: f64, all: bool) -> Result<(), BanksClientError> {
        let usdc = self.usdc();
        let closed = if all { vec![usdc.key] } else { vec![] };
        let ixs = [
            self.start_close_ix(),
            self.withdraw_ix(self.dst(), ui).await,
            self.repay_ix(usdc, ui, all).await,
            self.end_close_ix(closed).await,
        ];
        self.process(&ixs, &self.keeper).await
    }

    async fn sync(&self) {
        self.test_f
            .marginfi_group
            .try_pulse_bank_price_cache(self.usdc())
            .await
            .unwrap();
    }

    /// The largest open that still prices under the order's level.
    pub async fn max_fill(&self) -> u64 {
        self.sync().await;
        let order = self.order_state().await;
        let bank = self.usdc().load().await;
        let group = self.test_f.marginfi_group.load().await;
        let fits = |x: u64| order.opens_at(borrow_rate_at(&bank, &group, x).unwrap());
        let (mut lo, mut hi) = (0u64, order.remaining());
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }

    pub async fn owner_usdc_balance(&self) -> u64 {
        balance_of(self.test_f.context.clone(), self.owner_usdc).await
    }

    pub async fn order_state(&self) -> marginfi_type_crate::types::BorrowOrder {
        self.account_f.load_borrow_order(self.order).await
    }

    /// The account's USDC debt as of now, and its shares.
    pub async fn debt(&self) -> (I80F48, I80F48) {
        self.sync().await;
        let account = self.account_f.load().await;
        let bank = self.usdc().load().await;
        let shares = account
            .lending_account
            .get_balance(&self.usdc().key)
            .map(|b| I80F48::from(b.liability_shares))
            .unwrap_or(I80F48::ZERO);
        (bank.get_liability_amount(shares).unwrap(), shares)
    }

    pub async fn user_withdraws_from_destination(&self, ui: f64) -> Result<(), BanksClientError> {
        let own = self.usdc().mint.create_empty_token_account().await.key;
        self.account_f
            .try_bank_withdraw(own, self.dst(), ui, None)
            .await
    }

    pub async fn user_tops_up_destination(&self, ui: f64) -> Result<(), BanksClientError> {
        let own = self.usdc().mint.create_token_account_and_mint_to(ui).await;
        self.account_f
            .try_bank_deposit(own.key, self.dst(), ui, None)
            .await
    }

    pub async fn redeployed(&self) -> I80F48 {
        let account = self.account_f.load().await;
        let shares = account
            .lending_account
            .get_balance(&self.dst().key)
            .map(|b| I80F48::from(b.asset_shares))
            .unwrap_or(I80F48::ZERO);
        self.dst().load().await.get_asset_amount(shares).unwrap()
    }

    /// What `filled` reads after repaying `repaid` USDC.
    pub async fn filled_after_repaying(&self, repaid: u64) -> u64 {
        let order = self.order_state().await;
        let bank = self.usdc().load().await;
        let repaid_shares = bank.get_liability_shares(I80F48::from_num(repaid)).unwrap();
        let held = I80F48::from(order.liability_shares);
        let principal = I80F48::from_num(order.filled) * (held - repaid_shares) / held;
        principal.round().to_num::<u64>()
    }

    /// A close that repays `ui` USDC out of the keeper's own float and withdraws nothing.
    pub async fn close_from_own_funds(&self, ui: f64) -> Result<(), BanksClientError> {
        let ixs = [
            self.start_close_ix(),
            self.repay_ix(self.usdc(), ui, false).await,
            self.end_close_ix(vec![]).await,
        ];
        self.process(&ixs, &self.keeper).await
    }

    /// A third party borrows `fraction` of the float, funded to repay in full later.
    pub async fn spike_utilization(
        &self,
        fraction: f64,
    ) -> anyhow::Result<(MarginfiAccountFixture, Pubkey)> {
        let usdc = self.usdc();
        let driver = self.test_f.create_marginfi_account().await;
        let driver_sol = self
            .sol()
            .mint
            .create_token_account_and_mint_to(3_000.0)
            .await;
        driver
            .try_bank_deposit(driver_sol.key, self.sol(), 3_000.0, None)
            .await?;
        let driver_usdc = usdc.mint.create_token_account_and_mint_to(FLOAT).await;
        driver
            .try_bank_borrow(driver_usdc.key, usdc, FLOAT * fraction)
            .await?;
        Ok((driver, driver_usdc.key))
    }

    /// Open the round trip, then hold 90% utilization for a window (252% on the test curve).
    pub async fn open_then_spike(&mut self) -> anyhow::Result<(MarginfiAccountFixture, Pubkey)> {
        self.advance(WINDOW).await;
        self.fill(1_000.0).await?;
        let driver = self.spike_utilization(0.8).await?;
        self.advance(WINDOW).await;
        Ok(driver)
    }
}
