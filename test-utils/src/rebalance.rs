use crate::bank::BankFixture;
use crate::marginfi_account::{MarginfiAccountFixture, RebalanceBankMeta};
use crate::prelude::*;
use crate::test::TestFixture;
use drift_mocks::state::MinimalSpotMarket;
use fixed::types::I80F48;
use juplend_mocks::state::TokenReserve;
use kamino_mocks::state::{CurvePoint, MinimalReserve};
use marginfi_type_crate::{
    constants::{REBALANCE_ORDER_SEED, REBALANCE_RECORD_SEED},
    pdas::derive_juplend_token_reserve,
    types::{RebalanceMove, WrappedI80F48},
};
use solana_sdk::{
    account::{Account, AccountSharedData},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

pub const DEPOSIT_USDC: f64 = 1_000.0;

/// Two same-mint native USDC banks plus a placed rebalance order. `src` holds the user's whole
/// deposit at 0 utilization (supply rate 0); `dst` carries a borrow so its supply rate is > 0,
/// which makes `dst_rate > src_rate` hold before the move and `dst_post >= src_post` hold after it.
pub struct RebalanceFixture {
    pub test_f: TestFixture,
    pub user: MarginfiAccountFixture,
    pub keeper: Keypair,
    pub keeper_usdc: Pubkey,
    pub src_bank_f: BankFixture,
    pub dst_bank_f: BankFixture,
    pub order_pda: Pubkey,
    pub record_pda: Pubkey,
    pub oracle_metas: Vec<AccountMeta>,
}

/// Signs `ixs` with `keeper` as fee payer and processes them in one transaction.
pub async fn process_as_keeper(
    test_f: &TestFixture,
    keeper: &Keypair,
    ixs: &[Instruction],
) -> Result<(), solana_program_test::BanksClientError> {
    let blockhash = test_f.get_latest_blockhash().await;
    let ctx = test_f.context.borrow_mut();
    let tx = Transaction::new_signed_with_payer(ixs, Some(&keeper.pubkey()), &[keeper], blockhash);
    ctx.banks_client.process_transaction(tx).await
}

pub async fn fund_keeper_for_fees(test_f: &TestFixture, keeper: &Keypair) -> anyhow::Result<()> {
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

/// Drive `dst` to ~50% utilization (a positive supply rate): a lender funds it with 1_000 USDC and a
/// SOL-collateralized borrower draws 500, then interest is accrued.
pub async fn drive_dst_utilization(test_f: &TestFixture, dst: &BankFixture) -> anyhow::Result<()> {
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, dst, 1_000.0, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(100.0)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, 100.0, None)
        .await?;
    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, dst, 500.0)
        .await?;
    test_f.marginfi_group.try_accrue_interest(dst).await?;
    Ok(())
}

/// A move of `ui_value` USD (== UI USDC amount at the $1 test oracle) from `src_index` to `dst_index`
/// (indices into the referenced-bank list).
pub fn rebalance_move(src_index: u8, dst_index: u8, ui_value: f64) -> RebalanceMove {
    RebalanceMove {
        src_index,
        dst_index,
        _pad0: [0; 6],
        amount: WrappedI80F48::from(I80F48::from_num(ui_value)),
    }
}

pub async fn setup(
    min_improvement: I80F48,
    cooldown_seconds: u64,
) -> anyhow::Result<RebalanceFixture> {
    let test_f = TestFixture::new(Some(TestSettings::all_banks_payer_not_admin())).await;

    let src_bank_f = test_f
        .marginfi_group
        .try_lending_pool_add_bank_with_seed(
            &test_f.usdc_mint,
            None,
            *DEFAULT_USDC_TEST_BANK_CONFIG,
            100,
        )
        .await?;
    let dst_bank_f = test_f
        .marginfi_group
        .try_lending_pool_add_bank_with_seed(
            &test_f.usdc_mint,
            None,
            *DEFAULT_USDC_TEST_BANK_CONFIG,
            101,
        )
        .await?;

    // Rebalancing user: whole deposit in src, no borrows -> src utilization 0 -> src rate 0.
    let user = test_f.create_marginfi_account().await;
    let user_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(DEPOSIT_USDC)
        .await;
    user.try_bank_deposit(user_usdc.key, &src_bank_f, DEPOSIT_USDC, None)
        .await?;

    drive_dst_utilization(&test_f, &dst_bank_f).await?;
    test_f
        .marginfi_group
        .try_accrue_interest(&src_bank_f)
        .await?;

    let keeper = Keypair::new();
    fund_keeper_for_fees(&test_f, &keeper).await?;
    let keeper_usdc = test_f
        .usdc_mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;

    let allowed_banks = vec![src_bank_f.key, dst_bank_f.key];
    let order_pda = Pubkey::find_program_address(
        &[
            REBALANCE_ORDER_SEED.as_bytes(),
            user.key.as_ref(),
            test_f.usdc_mint.key.as_ref(),
        ],
        &marginfi::ID,
    )
    .0;
    let record_pda = Pubkey::find_program_address(
        &[REBALANCE_RECORD_SEED.as_bytes(), order_pda.as_ref()],
        &marginfi::ID,
    )
    .0;

    let payer = test_f.context.borrow().payer.pubkey();
    let place_ix = user
        .make_place_rebalance_order_ix(
            test_f.usdc_mint.key,
            order_pda,
            payer,
            payer,
            allowed_banks.clone(),
            Some(WrappedI80F48::from(min_improvement)),
            Some(cooldown_seconds),
            None,
            None,
        )
        .await;
    let blockhash = test_f.get_latest_blockhash().await;
    {
        let ctx = test_f.context.borrow_mut();
        let tx = Transaction::new_signed_with_payer(
            &[place_ix],
            Some(&ctx.payer.pubkey()),
            &[&ctx.payer],
            blockhash,
        );
        ctx.banks_client.process_transaction(tx).await?;
    }

    let oracle = get_oracle_id_from_feed_id(PYTH_USDC_FEED).unwrap_or(PYTH_USDC_FEED);
    let oracle_meta = AccountMeta::new_readonly(oracle, false);
    let oracle_metas = vec![oracle_meta.clone(), oracle_meta];

    Ok(RebalanceFixture {
        test_f,
        user,
        keeper,
        keeper_usdc,
        src_bank_f,
        dst_bank_f,
        order_pda,
        record_pda,
        oracle_metas,
    })
}

impl RebalanceFixture {
    /// A referenced native-USDC bank (one oracle) for the moves stream.
    pub fn bank_meta(&self, bank: Pubkey) -> RebalanceBankMeta {
        RebalanceBankMeta::new(bank, vec![self.oracle_metas[0].clone()])
    }

    /// A move of `ui_value` USD (== UI USDC amount at the $1 test oracle) from `src_index` to
    /// `dst_index` (indices into the referenced-bank list).
    pub fn usdc_move(&self, src_index: u8, dst_index: u8, ui_value: f64) -> RebalanceMove {
        rebalance_move(src_index, dst_index, ui_value)
    }

    /// Add a second same-mint USDC destination bank driven to ~50% utilization (so it clears the
    /// improvement gate against the 0%-utilization source), and extend the order's allowlist to
    /// include it. Returns its `BankFixture`.
    pub async fn add_second_dst(&self) -> anyhow::Result<BankFixture> {
        let dst2 = self
            .test_f
            .marginfi_group
            .try_lending_pool_add_bank_with_seed(
                &self.test_f.usdc_mint,
                None,
                *DEFAULT_USDC_TEST_BANK_CONFIG,
                102,
            )
            .await?;

        drive_dst_utilization(&self.test_f, &dst2).await?;

        let payer = self.test_f.context.borrow().payer.pubkey();
        let update_ix = self
            .user
            .make_update_rebalance_order_ix(
                self.order_pda,
                payer,
                Some(vec![self.src_bank_f.key, self.dst_bank_f.key, dst2.key]),
                None,
                None,
                None,
                None,
            )
            .await;
        self.process_as_payer(&[update_ix]).await?;
        Ok(dst2)
    }

    /// Add a second same-mint USDC SOURCE bank at 0% utilization (rate 0), give the user a `deposit`
    /// position in it, and extend the order allowlist to `[src, dst, src2]`. For consolidation (N->1)
    /// tests: the user then holds value in two low-rate sources to sweep into the higher-rate `dst`.
    pub async fn add_second_src(&self, deposit: f64) -> anyhow::Result<BankFixture> {
        let src2 = self
            .test_f
            .marginfi_group
            .try_lending_pool_add_bank_with_seed(
                &self.test_f.usdc_mint,
                None,
                *DEFAULT_USDC_TEST_BANK_CONFIG,
                103,
            )
            .await?;
        let user_usdc = self
            .test_f
            .usdc_mint
            .create_token_account_and_mint_to(deposit)
            .await;
        self.user
            .try_bank_deposit(user_usdc.key, &src2, deposit, None)
            .await?;
        self.test_f
            .marginfi_group
            .try_accrue_interest(&src2)
            .await?;

        let payer = self.test_f.context.borrow().payer.pubkey();
        let update_ix = self
            .user
            .make_update_rebalance_order_ix(
                self.order_pda,
                payer,
                Some(vec![self.src_bank_f.key, self.dst_bank_f.key, src2.key]),
                None,
                None,
                None,
                None,
            )
            .await;
        self.process_as_payer(&[update_ix]).await?;
        Ok(src2)
    }

    /// The keeper-signed sandwich: start -> withdraw all of `src` -> deposit into `dst` -> end.
    /// One full-position move from referenced bank 0 (`src`) to bank 1 (`dst`).
    pub async fn build_sandwich(&self, src: Pubkey, dst: Pubkey) -> Vec<Instruction> {
        let ref_banks = vec![self.bank_meta(src), self.bank_meta(dst)];
        let moves = vec![self.usdc_move(0, 1, DEPOSIT_USDC)];
        let start_ix = self
            .user
            .make_rebalance_start_ix(
                ref_banks.clone(),
                moves,
                self.order_pda,
                self.record_pda,
                self.keeper.pubkey(),
                self.keeper.pubkey(),
            )
            .await;
        let withdraw_ix = self
            .user
            .make_withdraw_ix_with_authority(
                self.keeper_usdc,
                &self.src_bank_f,
                DEPOSIT_USDC,
                Some(true),
                self.keeper.pubkey(),
            )
            .await;
        let deposit_ix = self
            .user
            .make_deposit_ix_with_authority(
                self.keeper_usdc,
                &self.dst_bank_f,
                DEPOSIT_USDC,
                None,
                self.keeper.pubkey(),
            )
            .await;
        let end_ix = self
            .user
            .make_rebalance_end_ix(
                ref_banks,
                vec![src],
                self.order_pda,
                self.record_pda,
                self.keeper.pubkey(),
            )
            .await;
        vec![start_ix, withdraw_ix, deposit_ix, end_ix]
    }

    pub async fn process(
        &self,
        ixs: &[Instruction],
    ) -> Result<(), solana_program_test::BanksClientError> {
        process_as_keeper(&self.test_f, &self.keeper, ixs).await
    }

    pub async fn asset_shares(&self, bank: Pubkey) -> I80F48 {
        let acct = self.user.load().await;
        acct.lending_account
            .balances
            .iter()
            .find(|b| b.bank_pk == bank)
            .map(|b| I80F48::from(b.asset_shares))
            .unwrap_or(I80F48::ZERO)
    }

    pub fn fee_pool(&self) -> Pubkey {
        self.user.rebalance_fee_pool_pda()
    }

    pub async fn lamports_of(&self, key: Pubkey) -> u64 {
        let ctx = self.test_f.context.borrow_mut();
        ctx.banks_client
            .get_account(key)
            .await
            .unwrap()
            .map(|a| a.lamports)
            .unwrap_or(0)
    }

    pub async fn process_as_payer(
        &self,
        ixs: &[Instruction],
    ) -> Result<(), solana_program_test::BanksClientError> {
        let blockhash = self.test_f.get_latest_blockhash().await;
        let ctx = self.test_f.context.borrow_mut();
        let payer = ctx.payer.pubkey();
        let tx = Transaction::new_signed_with_payer(ixs, Some(&payer), &[&ctx.payer], blockhash);
        ctx.banks_client.process_transaction(tx).await
    }

    pub async fn set_keeper_tip(
        &self,
        tip: u64,
    ) -> Result<(), solana_program_test::BanksClientError> {
        let payer = self.test_f.context.borrow().payer.pubkey();
        let ix = self
            .user
            .make_update_rebalance_order_ix(
                self.order_pda,
                payer,
                None,
                None,
                None,
                None,
                Some(tip),
            )
            .await;
        self.process_as_payer(&[ix]).await
    }

    pub async fn top_up_pool(
        &self,
        amount: u64,
    ) -> Result<(), solana_program_test::BanksClientError> {
        let payer = self.test_f.context.borrow().payer.pubkey();
        let ix = self
            .user
            .make_top_up_rebalance_fee_pool_ix(payer, amount)
            .await;
        self.process_as_payer(&[ix]).await
    }

    /// Switch the order from uncapped (the default) to a bounded `amount` of native tokens.
    pub async fn set_amount(
        &self,
        amount: u64,
    ) -> Result<(), solana_program_test::BanksClientError> {
        let payer = self.test_f.context.borrow().payer.pubkey();
        let update_ix = self
            .user
            .make_update_rebalance_order_ix(
                self.order_pda,
                payer,
                None,
                None,
                None,
                Some(amount),
                None,
            )
            .await;
        self.process_as_payer(&[update_ix]).await
    }
}

/// The user's src-venue deposit (native units, 6-decimal USDC). The keeper redeposits this full amount
/// into the dst venue; value is strictly conserved and the keeper is paid a separate SOL tip.
pub const VENUE_DEPOSIT_NATIVE: u64 = 100_000_000; // 100 USDC
/// 50% borrow utilization engineered onto the Drift dst spot market: enough to make its supply rate
/// clearly beat the 0%-utilization source while staying positive after the dst deposit grows it.
pub const DRIFT_DST_BORROW_NUM: u128 = 1;
pub const DRIFT_DST_BORROW_DEN: u128 = 2;

/// One `TestFixture` hosting Kamino, Drift and JupLend banks all on the SAME mint `M` (the baked-mint
/// Kamino reserve mint, the only one that cannot be relocated). Built by extending the Kamino fixture
/// with a Drift and a JupLend bank for `M`. Both cross-venue tests reuse it: the shared mint is what
/// lets one rebalance order move a position between two different venues.
pub struct MultiVenueFixture {
    pub test_f: TestFixture,
    pub user: MarginfiAccountFixture,
    pub mint: MintFixture,
    pub keeper: Keypair,
    pub keeper_token: Pubkey,
    pub oracle: Pubkey,
    pub kamino_bank: BankFixture,
    pub drift_bank: BankFixture,
    pub juplend_bank: BankFixture,
}

pub async fn setup_multi_venue_fixture() -> anyhow::Result<MultiVenueFixture> {
    let kamino = TestFixture::setup_kamino_bank(None).await;
    let mint = kamino.bank_f.mint.clone();
    let (drift_bank, _, _) = kamino.test_f.add_drift_bank_for_mint(&mint, 0, 777).await;
    let (juplend_bank, _, _) = kamino.test_f.add_juplend_bank_for_mint(&mint, 888).await;

    let user = kamino.test_f.create_marginfi_account().await;
    let keeper = Keypair::new();
    fund_keeper_for_fees(&kamino.test_f, &keeper).await?;
    let keeper_token = mint
        .create_empty_token_account_with_owner(&keeper.pubkey())
        .await
        .key;
    let oracle = get_oracle_id_from_feed_id(PYTH_USDC_FEED).unwrap_or(PYTH_USDC_FEED);
    // The Kamino fixture pins the clock to the reserve's price timestamp, far from genesis; stamp the
    // shared USDC Pyth feed to that same `now` so the rebalance value path's price reads non-stale.
    // The harness clock does not advance between txs, so a single stamp covers the whole test.
    let now = kamino.test_f.get_clock().await.unix_timestamp;
    kamino.test_f.set_pyth_oracle_timestamp(oracle, now).await;

    Ok(MultiVenueFixture {
        test_f: kamino.test_f,
        user,
        mint,
        keeper,
        keeper_token,
        oracle,
        kamino_bank: kamino.bank_f,
        drift_bank,
        juplend_bank,
    })
}

impl MultiVenueFixture {
    /// Flattens the Kamino reserve's borrow-rate curve to zero (borrow rate 0 at every utilization
    /// knot), making its supply rate ~0 regardless of the reserve's utilization. Touches only the
    /// rate curve — never the balances — so the Kamino `refresh_reserve` exchange-rate math, which
    /// reads liquidity/collateral, stays consistent. Used to make Kamino a low-rate source.
    pub async fn set_kamino_rate_zero(&self) {
        let reserve_key = self.kamino_bank.load().await.integration_acc_1;
        let mut acct = self.test_f.try_load(&reserve_key).await.unwrap().unwrap();
        let r = bytemuck::from_bytes_mut::<MinimalReserve>(&mut acct.data[8..]);
        let mut points = [CurvePoint {
            utilization_rate_bps: 0,
            borrow_rate_bps: 0,
        }; 11];
        for (i, p) in points.iter_mut().enumerate() {
            p.utilization_rate_bps = i as u32 * 1_000; // 0..10_000 bps, strictly increasing
        }
        r.config.borrow_rate_curve.points = points;
        r.config.protocol_take_rate_pct = 0;
        self.test_f
            .context
            .borrow_mut()
            .set_account(&reserve_key, &AccountSharedData::from(acct));
    }

    /// Drives the Drift dst spot market to a non-trivial borrow utilization by writing only the borrow
    /// side (`borrow_balance`/`cumulative_borrow_interest` mirror the deposit side scaled by
    /// `num/den`), so its supply rate clearly beats the 0%-utilization source. Touching only the
    /// borrow side leaves the deposit-side accounting the venue deposit leg relies on untouched.
    pub async fn set_drift_borrow_utilization(&self, num: u128, den: u128) {
        let spot_market_key = self.drift_bank.load().await.integration_acc_1;
        let ts = self.test_f.get_clock().await.unix_timestamp;
        let mut acct = self
            .test_f
            .try_load(&spot_market_key)
            .await
            .unwrap()
            .unwrap();
        let m = bytemuck::from_bytes_mut::<MinimalSpotMarket>(&mut acct.data[8..]);
        let deposit_balance = u128::from_le_bytes(m.deposit_balance);
        m.borrow_balance = (deposit_balance * num / den).to_le_bytes();
        m.cumulative_borrow_interest = m.cumulative_deposit_interest;
        m.last_interest_ts = ts as u64;
        self.test_f
            .context
            .borrow_mut()
            .set_account(&spot_market_key, &AccountSharedData::from(acct));
    }

    /// Stamps the JupLend dst `TokenReserve` rate fields so its supply rate is high
    /// (`borrow_rate × utilization`, no fee), making JupLend a high-rate destination for the start
    /// gate. Leaves the supply/borrow totals and exchange prices as the venue seeded them, and stamps
    /// `last_update_timestamp` to the current (pinned) clock so the reserve reads fresh without
    /// breaking the deposit leg's `now - last_update` interest math.
    pub async fn set_juplend_rate_high(&self) {
        let key = derive_juplend_token_reserve(&self.mint.key).0;
        let now = self.test_f.get_clock().await.unix_timestamp as u64;
        let mut acct = self.test_f.try_load(&key).await.unwrap().unwrap();
        let size = std::mem::size_of::<TokenReserve>();
        let tr = bytemuck::from_bytes_mut::<TokenReserve>(&mut acct.data[8..8 + size]);
        tr.borrow_rate = 1_000; // 10%
        tr.last_utilization = 8_000; // 80%
        tr.fee_on_interest = 0;
        tr.supply_exchange_price = 1_000_000_000_000;
        tr.borrow_exchange_price = 1_000_000_000_000;
        tr.total_supply_with_interest = 1_000_000;
        tr.total_borrow_with_interest = 1_000_000;
        tr.last_update_timestamp = now;
        self.test_f
            .context
            .borrow_mut()
            .set_account(&key, &AccountSharedData::from(acct));
    }

    /// Places the rebalance order on mint `M`, allowing both venue banks. Returns the order/record PDAs.
    pub async fn place_order(
        &self,
        src_bank: Pubkey,
        dst_bank: Pubkey,
        min_improvement: I80F48,
    ) -> anyhow::Result<(Pubkey, Pubkey)> {
        let order_pda = Pubkey::find_program_address(
            &[
                REBALANCE_ORDER_SEED.as_bytes(),
                self.user.key.as_ref(),
                self.mint.key.as_ref(),
            ],
            &marginfi::ID,
        )
        .0;
        let record_pda = Pubkey::find_program_address(
            &[REBALANCE_RECORD_SEED.as_bytes(), order_pda.as_ref()],
            &marginfi::ID,
        )
        .0;

        let payer = self.test_f.context.borrow().payer.pubkey();
        let place_ix = self
            .user
            .make_place_rebalance_order_ix(
                self.mint.key,
                order_pda,
                payer,
                payer,
                vec![src_bank, dst_bank],
                Some(WrappedI80F48::from(min_improvement)),
                Some(0),
                None,
                None,
            )
            .await;
        let blockhash = self.test_f.get_latest_blockhash().await;
        {
            let ctx = self.test_f.context.borrow_mut();
            let tx = Transaction::new_signed_with_payer(
                &[place_ix],
                Some(&ctx.payer.pubkey()),
                &[&ctx.payer],
                blockhash,
            );
            ctx.banks_client.process_transaction(tx).await?;
        }
        Ok((order_pda, record_pda))
    }

    /// Reads the user's asset shares in `bank` (zero if no active balance).
    pub async fn asset_shares(&self, bank: Pubkey) -> I80F48 {
        let acct = self.user.load().await;
        acct.lending_account
            .balances
            .iter()
            .find(|b| b.bank_pk == bank)
            .map(|b| I80F48::from(b.asset_shares))
            .unwrap_or(I80F48::ZERO)
    }

    /// Per-Kamino-bank oracle slice for start/end: `[oracle, reserve]` (oracle first, venue last).
    pub async fn kamino_slice(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.oracle, false),
            AccountMeta::new_readonly(self.kamino_bank.load().await.integration_acc_1, false),
        ]
    }

    /// Per-Drift-bank oracle slice for start/end: `[oracle, spot_market]`.
    pub async fn drift_slice(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.oracle, false),
            AccountMeta::new_readonly(self.drift_bank.load().await.integration_acc_1, false),
        ]
    }

    /// Per-JupLend-bank oracle slice for start/end: `[oracle, lending]`. The `TokenReserve` is passed
    /// separately via the start/end `*_token_reserve` argument, not in this slice.
    pub async fn juplend_slice(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(self.oracle, false),
            AccountMeta::new_readonly(self.juplend_bank.load().await.integration_acc_1, false),
        ]
    }

    pub async fn process(
        &self,
        ixs: &[Instruction],
    ) -> Result<(), solana_program_test::BanksClientError> {
        process_as_keeper(&self.test_f, &self.keeper, ixs).await
    }
}
