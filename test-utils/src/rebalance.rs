use crate::bank::BankFixture;
use crate::marginfi_account::{MarginfiAccountFixture, RebalanceBankMeta};
use crate::prelude::*;
use crate::test::TestFixture;
use anchor_lang::prelude::Clock;
use anchor_lang::{system_program, InstructionData, ToAccountMetas};
use drift_mocks::drift::client as drift;
use drift_mocks::state::MinimalSpotMarket;
use fixed::types::I80F48;
use juplend_mocks::state::{Lending, TokenReserve};
use kamino_mocks::state::{CurvePoint, MinimalReserve};
use marginfi_type_crate::pdas::{
    derive_drift_spot_market_vault, derive_drift_state, derive_drift_user, derive_drift_user_stats,
    DRIFT_PROGRAM_ID,
};
use marginfi_type_crate::{
    constants::{REBALANCE_ORDER_SEED, REBALANCE_RECORD_SEED},
    pdas::derive_juplend_token_reserve,
    types::{RebalanceMove, RebalanceRecord, WrappedI80F48},
};
use solana_sdk::sysvar;
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

/// Fund `bank` with 1_000 USDC of lender liquidity and draw `borrow_ui` against `sol_collateral_ui`
/// SOL collateral, then accrue, giving the bank a supply rate set by the resulting utilization.
pub async fn drive_utilization(
    test_f: &TestFixture,
    bank: &BankFixture,
    borrow_ui: f64,
    sol_collateral_ui: f64,
) -> anyhow::Result<()> {
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);
    let lender = test_f.create_marginfi_account().await;
    let lender_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(1_000.0)
        .await;
    lender
        .try_bank_deposit(lender_usdc.key, bank, 1_000.0, None)
        .await?;

    let borrower = test_f.create_marginfi_account().await;
    let borrower_sol = test_f
        .sol_mint
        .create_token_account_and_mint_to(sol_collateral_ui)
        .await;
    borrower
        .try_bank_deposit(borrower_sol.key, sol_bank_f, sol_collateral_ui, None)
        .await?;
    let borrower_usdc = test_f.usdc_mint.create_empty_token_account().await;
    borrower
        .try_bank_borrow(borrower_usdc.key, bank, borrow_ui)
        .await?;
    test_f.marginfi_group.try_accrue_interest(bank).await?;
    Ok(())
}

/// The `RebalanceRecord` PDA for the account's execution sequence `seq`.
pub fn record_pda_at(marginfi_account: Pubkey, seq: u64) -> Pubkey {
    Pubkey::find_program_address(
        &[
            REBALANCE_RECORD_SEED.as_bytes(),
            marginfi_account.as_ref(),
            &seq.to_le_bytes(),
        ],
        &marginfi::ID,
    )
    .0
}

/// Drive `dst` to ~50% utilization (a positive supply rate).
pub async fn drive_dst_utilization(test_f: &TestFixture, dst: &BankFixture) -> anyhow::Result<()> {
    drive_utilization(test_f, dst, 500.0, 100.0).await
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
    let record_pda = record_pda_at(user.key, 0);

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

    /// Add `n` more same-mint destination banks, each driven to the same utilization as `dst` so a
    /// move into any of them clears the best-venue check, and extend the allowlist to cover them all.
    pub async fn add_dst_banks(&self, n: usize) -> anyhow::Result<Vec<BankFixture>> {
        let mut extra = Vec::with_capacity(n);
        for i in 0..n {
            let bank = self
                .test_f
                .marginfi_group
                .try_lending_pool_add_bank_with_seed(
                    &self.test_f.usdc_mint,
                    None,
                    *DEFAULT_USDC_TEST_BANK_CONFIG,
                    110 + i as u64,
                )
                .await?;
            drive_dst_utilization(&self.test_f, &bank).await?;
            extra.push(bank);
        }

        let mut allowed = vec![self.src_bank_f.key, self.dst_bank_f.key];
        allowed.extend(extra.iter().map(|b| b.key));
        let payer = self.test_f.context.borrow().payer.pubkey();
        let update_ix = self
            .user
            .make_update_rebalance_order_ix(
                self.order_pda,
                payer,
                Some(allowed),
                None,
                None,
                None,
                None,
            )
            .await;
        self.process_as_payer(&[update_ix]).await?;
        Ok(extra)
    }

    /// Add a same-mint destination bank drawn to `borrow_ui` against the 1_000 USDC seeded into it,
    /// so its rate sits wherever that utilization puts it, and extend the allowlist to cover it.
    pub async fn add_dst_bank_at(&self, borrow_ui: f64) -> anyhow::Result<BankFixture> {
        let bank = self
            .test_f
            .marginfi_group
            .try_lending_pool_add_bank_with_seed(
                &self.test_f.usdc_mint,
                None,
                *DEFAULT_USDC_TEST_BANK_CONFIG,
                104,
            )
            .await?;
        drive_utilization(&self.test_f, &bank, borrow_ui, 100.0).await?;

        let payer = self.test_f.context.borrow().payer.pubkey();
        let update_ix = self
            .user
            .make_update_rebalance_order_ix(
                self.order_pda,
                payer,
                Some(vec![self.src_bank_f.key, self.dst_bank_f.key, bank.key]),
                None,
                None,
                None,
                None,
            )
            .await;
        self.process_as_payer(&[update_ix]).await?;
        Ok(bank)
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
        let execution_seq = self.user.load().await.rebalance_execution_seq;
        let record_pda = record_pda_at(self.user.key, execution_seq);
        let ref_banks = vec![self.bank_meta(src), self.bank_meta(dst)];
        let moves = vec![rebalance_move(0, 1, DEPOSIT_USDC)];
        let start_ix = self
            .user
            .make_rebalance_start_ix(
                ref_banks.clone(),
                moves,
                execution_seq,
                self.order_pda,
                record_pda,
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
                record_pda,
                self.keeper.pubkey(),
            )
            .await;
        vec![start_ix, withdraw_ix, deposit_ix, end_ix]
    }

    /// Like `build_sandwich` but deposits only `deposit_ui` into `dst` while still withdrawing the full
    /// source and declaring the full move: the keeper pockets the shortfall. Conservation must reject it.
    pub async fn build_skim_sandwich(
        &self,
        src: Pubkey,
        dst: Pubkey,
        deposit_ui: f64,
    ) -> Vec<Instruction> {
        let execution_seq = self.user.load().await.rebalance_execution_seq;
        let record_pda = record_pda_at(self.user.key, execution_seq);
        let ref_banks = vec![self.bank_meta(src), self.bank_meta(dst)];
        let moves = vec![rebalance_move(0, 1, DEPOSIT_USDC)];
        let start_ix = self
            .user
            .make_rebalance_start_ix(
                ref_banks.clone(),
                moves,
                execution_seq,
                self.order_pda,
                record_pda,
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
                deposit_ui,
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
                record_pda,
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

    /// The keeper-signed `settle_rebalance_tip` ix for the standard `[src, dst]` referenced set.
    pub async fn build_settle(&self, src: Pubkey, dst: Pubkey) -> Instruction {
        self.build_settle_as(src, dst, self.keeper.pubkey()).await
    }

    /// `settle_rebalance_tip` with an explicit `caller`, so tests can settle from a third party and
    /// assert the tip and record rent still reach the recorded executor.
    pub async fn build_settle_as(&self, src: Pubkey, dst: Pubkey, caller: Pubkey) -> Instruction {
        let ref_banks = vec![self.bank_meta(src), self.bank_meta(dst)];
        let seq = self.user.load().await.rebalance_execution_seq - 1;
        self.user
            .make_rebalance_settle_ix(
                ref_banks,
                record_pda_at(self.user.key, seq),
                self.keeper.pubkey(),
                caller,
            )
            .await
    }

    /// Pin the clock's unix timestamp to `now` and refresh the native oracle to match.
    pub async fn pin_clock(&self, now: i64) {
        {
            let ctx = self.test_f.context.borrow_mut();
            let mut clock: Clock = ctx.banks_client.get_sysvar().await.unwrap();
            clock.unix_timestamp = now;
            ctx.set_sysvar(&clock);
        }
        self.test_f
            .set_pyth_oracle_timestamp(self.oracle_metas[0].pubkey, now)
            .await;
    }

    /// Advance the pinned clock by `secs` and refresh the native oracle timestamp so its price stays
    /// fresh for post-advance reads.
    pub async fn advance_clock(&self, secs: i64) {
        let now = {
            let ctx = self.test_f.context.borrow_mut();
            let mut clock: Clock = ctx.banks_client.get_sysvar().await.unwrap();
            clock.unix_timestamp = clock.unix_timestamp.saturating_add(secs);
            let now = clock.unix_timestamp;
            ctx.set_sysvar(&clock);
            now
        };
        self.test_f
            .set_pyth_oracle_timestamp(self.oracle_metas[0].pubkey, now)
            .await;
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

    /// A bank's `asset_share_value`, the per-share yield index a settlement compares across banks.
    pub async fn share_value(&self, bank: &BankFixture) -> I80F48 {
        I80F48::from(bank.load().await.asset_share_value)
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

    /// The keeper tip escrowed into the record at `end_rebalance`, read from chain.
    pub async fn record_pending_tip(&self) -> u64 {
        let ctx = self.test_f.context.borrow_mut();
        let acc = ctx
            .banks_client
            .get_account(self.record_pda)
            .await
            .unwrap()
            .unwrap();
        bytemuck::from_bytes::<RebalanceRecord>(
            &acc.data[8..8 + core::mem::size_of::<RebalanceRecord>()],
        )
        .pending_tip
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

    /// Deposits `amount_native` into the Drift dst spot market through a native Drift lender,
    /// raising the market's `deposit_balance`.
    pub async fn seed_drift_liquidity(&self, amount_native: u64) -> anyhow::Result<()> {
        let bank_state = self.drift_bank.load().await;
        let spot_market: MinimalSpotMarket =
            load_and_deserialize(self.test_f.context.clone(), &bank_state.integration_acc_1).await;
        let lender = self.test_f.payer();
        let user = derive_drift_user(&lender, 0).0;
        let user_stats = derive_drift_user_stats(&lender).0;
        let state = derive_drift_state().0;
        let source = self
            .mint
            .create_token_account_and_mint_to(amount_native as f64 / 1_000_000.0)
            .await;

        let ix = |accounts: Vec<AccountMeta>, data: Vec<u8>| Instruction {
            program_id: DRIFT_PROGRAM_ID,
            accounts,
            data,
        };
        let mut ixs = vec![
            ix(
                drift::accounts::InitializeUserStats {
                    user_stats,
                    state,
                    authority: lender,
                    payer: lender,
                    rent: sysvar::rent::ID,
                    system_program: system_program::ID,
                }
                .to_account_metas(Some(true)),
                drift::args::InitializeUserStats {}.data(),
            ),
            ix(
                drift::accounts::InitializeUser {
                    user,
                    user_stats,
                    state,
                    authority: lender,
                    payer: lender,
                    rent: sysvar::rent::ID,
                    system_program: system_program::ID,
                }
                .to_account_metas(Some(true)),
                drift::args::InitializeUser {
                    sub_account_id: 0,
                    name: [0u8; 32],
                }
                .data(),
            ),
            ix(
                drift::accounts::Deposit {
                    state,
                    user,
                    user_stats,
                    authority: lender,
                    spot_market_vault: derive_drift_spot_market_vault(spot_market.market_index).0,
                    user_token_account: source.key,
                    token_program: self.mint.token_program,
                }
                .to_account_metas(Some(true)),
                drift::args::Deposit {
                    market_index: spot_market.market_index,
                    amount: amount_native,
                    reduce_only: false,
                }
                .data(),
            ),
        ];
        // Drift loads its risk maps from the remaining accounts (oracles first, then writable spot
        // markets); the deposited market must appear there or it resolves against an empty map.
        if let Some(last) = ixs.last_mut() {
            last.accounts
                .push(AccountMeta::new(bank_state.integration_acc_1, false));
        }
        let blockhash = self.test_f.get_latest_blockhash().await;
        let ctx = self.test_f.context.borrow_mut();
        let payer = ctx.payer.pubkey();
        let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer), &[&ctx.payer], blockhash);
        ctx.banks_client.process_transaction(tx).await?;
        Ok(())
    }

    /// Doubles the Drift spot market's cumulative interest, so one unit of scaled balance is worth
    /// two native units. Call before any deposit into the market, so positions are opened at the
    /// scaled rate. Used to give a referenced bank a venue multiplier above 1.
    pub async fn double_drift_exchange_rate(&self) {
        let spot_market_key = self.drift_bank.load().await.integration_acc_1;
        let mut acct = self
            .test_f
            .try_load(&spot_market_key)
            .await
            .unwrap()
            .unwrap();
        let m = bytemuck::from_bytes_mut::<MinimalSpotMarket>(&mut acct.data[8..]);
        // Existing depositors' claims double with the rate, so the vault is topped up by their
        // current value or Drift rejects the next deposit on its vault invariant.
        let decimals = self.mint.mint.decimals;
        let claims = u128::from_le_bytes(m.deposit_balance)
            .saturating_mul(u128::from_le_bytes(m.cumulative_deposit_interest))
            / 10u128.pow(19 - decimals as u32);
        let vault = m.vault;
        m.cumulative_deposit_interest =
            (u128::from_le_bytes(m.cumulative_deposit_interest) * 2).to_le_bytes();
        m.cumulative_borrow_interest =
            (u128::from_le_bytes(m.cumulative_borrow_interest) * 2).to_le_bytes();
        self.test_f
            .context
            .borrow_mut()
            .set_account(&spot_market_key, &AccountSharedData::from(acct));
        self.mint
            .clone()
            .mint_to(&vault, claims as f64 / 10f64.powi(decimals as i32))
            .await;
    }

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
        let record_pda = record_pda_at(self.user.key, 0);

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

    /// Kamino's reward accounts for start/end: its reserve's `LendingMarket`.
    pub async fn kamino_rewards(&self) -> Vec<Pubkey> {
        let reserve_key = self.kamino_bank.load().await.integration_acc_1;
        let reserve =
            load_and_deserialize::<MinimalReserve>(self.test_f.context.clone(), &reserve_key).await;
        vec![reserve.lending_market]
    }

    /// JupLend's reward accounts for start/end: `[rewards_rate_model, f_token_mint]`.
    pub async fn juplend_rewards(&self) -> Vec<Pubkey> {
        let lending_key = self.juplend_bank.load().await.integration_acc_1;
        let lending =
            load_and_deserialize::<Lending>(self.test_f.context.clone(), &lending_key).await;
        vec![lending.rewards_rate_model, lending.f_token_mint]
    }

    pub async fn process(
        &self,
        ixs: &[Instruction],
    ) -> Result<(), solana_program_test::BanksClientError> {
        process_as_keeper(&self.test_f, &self.keeper, ixs).await
    }
}
