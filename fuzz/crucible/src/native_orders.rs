//! Native-bank order/rebalance state machine. All financial transitions execute the SBF program.
//! The model reads typed account data and computes its own balances, bounds and maintenance health;
//! it does not call the program's reconciliation or health helpers.
use super::*;
use crate::anchor_lang::prelude::Clock;
use crate::anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use fixed::types::I80F48 as F;
use marginfi::{state as chain, types as arg};

const SCALE: u64 = 1_000_000;
const ORDER_ASSETS: u64 = 40 * SCALE;
const ORDER_DEBT: u64 = 10 * SCALE;
const REBALANCE_ASSETS: u64 = 30 * SCALE;
const COOLDOWN: u64 = 60;
const EXECUTION_FLAGS: u64 = (1 << 7) | (1 << 8);
const TOLERANCE: F = F::lit("0.000004");

#[derive(Clone)]
pub(super) struct NativeWorld {
    group: Pubkey,
    // source, interest destination, bystander collateral, order liability, second interest
    // destination at half the utilization (multi-move rebalances need a rate spread).
    banks: [Pubkey; 5],
    users: Vec<Pubkey>,
    keeper: Rc<Keypair>,
    keeper_tokens: Pubkey,
    order_user: Pubkey,
    rebalance_user: Pubkey,
    order: Option<Pubkey>,
    rebalance_order: Option<Pubkey>,
    records: Vec<Pubkey>,
    pub(super) completed_orders: u64,
    pub(super) completed_rebalances: u64,
    pub(super) completed_settlements: u64,
}

// Include owners, lamports and bytes: a rejected transaction must roll back account creation,
// closure, fees from pools, tags, sequence counters and token transfers. Only the tx fee payer's
// SOL balance is excluded; its token account is included.
type Snapshot = Vec<(Pubkey, Option<(Pubkey, u64, Vec<u8>)>)>;

fn wrapped(value: F) -> arg::WrappedI80F48 {
    arg::WrappedI80F48::from_i80f48(value)
}

fn position(user: &chain::MarginfiAccount, bank: Pubkey) -> (F, F, u16) {
    user.lending_account.balances.iter()
        .find(|b| b.active != 0 && b.bank_pk == bank)
        .map(|b| (b.asset_shares.to_i80f48(), b.liability_shares.to_i80f48(), b.tag))
        .unwrap_or((F::ZERO, F::ZERO, 0))
}

impl MarginfiFixture {
    pub(super) fn native_ensure_for_campaign(&mut self) {
        self.native_ensure_world();
    }

    fn native_load<T: bytemuck::Pod>(&self, key: Pubkey, discriminator: &[u8]) -> T {
        let account = self.ctx.read_account(&key).expect("native model: missing account");
        assert_eq!(account.owner, self.program_id, "native model: wrong owner for {key}");
        assert_eq!(&account.data[..discriminator.len()], discriminator, "native model: wrong discriminator for {key}");
        assert_eq!(account.data.len(), 8 + std::mem::size_of::<T>(), "native model: stale IDL/layout for {key}");
        bytemuck::pod_read_unaligned(&account.data[8..])
    }

    fn native_user(&self, key: Pubkey) -> chain::MarginfiAccount {
        self.native_load(key, chain::MarginfiAccount::DISCRIMINATOR)
    }

    fn native_bank(&self, key: Pubkey) -> chain::Bank {
        self.native_load(key, chain::Bank::DISCRIMINATOR)
    }

    fn native_exists(&self, key: Pubkey) -> bool {
        self.ctx.read_account(&key).map(|a| a.lamports > 0 && !a.data.is_empty()).unwrap_or(false)
    }

    fn native_lamports(&self, key: Pubkey) -> u64 {
        self.ctx.read_account(&key).map(|a| a.lamports).unwrap_or(0)
    }

    fn native_tokens(&self, key: Pubkey) -> u64 {
        let data = self.ctx.read_account(&key).expect("native model: missing token account").data;
        u64::from_le_bytes(data[64..72].try_into().unwrap())
    }

    fn native_pda(&self, seed: &[u8], keys: &[&[u8]]) -> Pubkey {
        let mut seeds = vec![seed];
        seeds.extend_from_slice(keys);
        Pubkey::find_program_address(&seeds, &self.program_id).0
    }

    fn native_ix<I: anchor_lang::InstructionData, A: anchor_lang::ToAccountMetas>(&self, ix: I, accounts: A) -> Instruction {
        scout_anchor_instruction(self.program_id, ix, accounts)
    }

    fn native_send(&mut self, ixs: Vec<Instruction>, signer: Rc<Keypair>) -> bool {
        assert!(self.ctx.pending_instructions.is_empty(), "native action inherited an unfinished batch");
        for ix in ixs {
            self.ctx.raw_call(ix).signers(&[&*signer]).add_transaction().expect("native queue");
        }
        match self.ctx.send_batch().expect("native send_batch") {
            Some(outcome) => {
                if !outcome.is_success() && std::env::var_os("FUZZ_DEBUG").is_some() {
                    eprintln!("native transaction rejected: {outcome:?}");
                }
                outcome.is_success()
            }
            None => panic!("native action sent no instructions"),
        }
    }

    fn native_new_user(&mut self, group: Pubkey) -> Pubkey {
        let key = Keypair::new();
        let user = key.pubkey();
        let outcome = self.ctx.program(self.program_id)
            .call(instruction::MarginfiAccountInitialize {})
            .accounts(accounts::MarginfiAccountInitialize {
                marginfi_group: group, marginfi_account: user,
                authority: self.payer.pubkey(), fee_payer: self.payer.pubkey(),
            }).signers(&[&*self.payer, &key]).send().expect("native initialize account");
        assert!(outcome.is_success(), "native initialize account: {outcome:?}");
        self.scout_register_subject_account(user);
        user
    }

    fn native_new_bank(&mut self, group: Pubkey, interest: bool, price: F) -> Pubkey {
        let key = Keypair::new();
        let bank = key.pubkey();
        let (lva, lv, iva, iv, fva, fv) = scout_bank_vault_pdas(self.program_id, bank);
        let mut config = scout_liquidation_bank_config();
        if interest {
            // Flat 10% borrow APR; supply APR still varies with utilization.
            config.interest_rate_config.zero_util_rate = u32::MAX / 100;
            config.interest_rate_config.hundred_util_rate = u32::MAX / 100;
        }
        let fee_state = self.native_pda(FEE_STATE_SEED, &[]);
        let outcome = self.ctx.program(self.program_id)
            .call(instruction::LendingPoolAddBank { bank_config: config })
            .accounts(scout_lending_pool_add_bank_accounts(group, self.payer.pubkey(), self.payer.pubkey(), fee_state,
                self.global_fee_wallet, self.bank_mint, bank, lva, lv, iva, iv, fva, fv, spl_token::id()))
            .signers(&[&*self.payer, &key]).send().expect("native add bank");
        assert!(outcome.is_success(), "native add bank: {outcome:?}");
        let ix = self.native_ix(instruction::LendingPoolSetOraclePrice {
            price: wrapped(price), setup: SCOUT_ORACLE_SETUP_FIXED,
        }, accounts::LendingPoolSetOraclePrice { group, governance_admin: self.payer.pubkey(), bank });
        assert!(self.native_send(vec![ix], self.payer.clone()), "native set price");
        bank
    }

    fn native_deposit_ix(&self, group: Pubkey, user: Pubkey, bank: Pubkey, amount: u64, keeper: bool) -> Instruction {
        let w = self.native_orders.as_ref();
        self.native_ix(instruction::LendingAccountDeposit { amount, deposit_up_to_limit: None }, accounts::LendingAccountDeposit {
            group, marginfi_account: user,
            authority: if keeper { w.unwrap().keeper.pubkey() } else { self.payer.pubkey() },
            bank, signer_token_account: if keeper { w.unwrap().keeper_tokens } else { self.signer_token_account },
            liquidity_vault: self.native_pda(LIQUIDITY_VAULT_SEED, &[bank.as_ref()]), token_program: spl_token::id(),
        })
    }

    fn native_observe(&self, user: Pubkey, include: &[Pubkey], exclude: &[Pubkey]) -> Vec<AccountMeta> {
        let mut keys: Vec<_> = self.native_user(user).lending_account.balances.iter()
            .filter(|b| b.active != 0).map(|b| b.bank_pk).chain(include.iter().copied())
            .filter(|key| !exclude.contains(key)).collect();
        keys.sort_unstable_by(|a, b| b.cmp(a));
        keys.dedup();
        keys.into_iter().map(|key| AccountMeta::new_readonly(key, false)).collect()
    }

    fn native_withdraw_ix(&self, user: Pubkey, bank: Pubkey, amount: u64, all: bool) -> Instruction {
        let w = self.native_orders.as_ref().unwrap();
        let mut ix = self.native_ix(instruction::LendingAccountWithdraw { amount, withdraw_all: Some(all) }, accounts::LendingAccountWithdraw {
            group: w.group, marginfi_account: user, authority: w.keeper.pubkey(), bank,
            destination_token_account: w.keeper_tokens,
            bank_liquidity_vault_authority: self.native_pda(LIQUIDITY_VAULT_AUTHORITY_SEED, &[bank.as_ref()]),
            liquidity_vault: self.native_pda(LIQUIDITY_VAULT_SEED, &[bank.as_ref()]), token_program: spl_token::id(),
        });
        ix.accounts.extend(self.native_observe(user, &[bank], &[]));
        ix
    }

    fn native_repay_ix(&self, user: Pubkey, bank: Pubkey, amount: u64, all: bool) -> Instruction {
        let w = self.native_orders.as_ref().unwrap();
        self.native_ix(instruction::LendingAccountRepay { amount, repay_all: Some(all) }, accounts::LendingAccountRepay {
            group: w.group, marginfi_account: user, authority: w.keeper.pubkey(), bank,
            signer_token_account: w.keeper_tokens,
            liquidity_vault: self.native_pda(LIQUIDITY_VAULT_SEED, &[bank.as_ref()]), token_program: spl_token::id(),
        })
    }

    // Fixture top-ups are fuzz-time preconditions, not assertions about the program: after price
    // moves or other state drift they may legitimately be rejected, so callers decide whether a
    // failure aborts setup (assert) or merely ends the action chain (return false).
    fn native_fund(&mut self, group: Pubkey, user: Pubkey, bank: Pubkey, amount: u64) -> bool {
        let ix = self.native_deposit_ix(group, user, bank, amount, false);
        self.native_send(vec![ix], self.payer.clone())
    }

    fn native_borrow(&mut self, group: Pubkey, user: Pubkey, bank: Pubkey, amount: u64) -> bool {
        let mut ix = self.native_ix(instruction::LendingAccountBorrow { amount }, accounts::LendingAccountBorrow {
            group, marginfi_account: user, authority: self.payer.pubkey(), bank,
            destination_token_account: self.signer_token_account,
            bank_liquidity_vault_authority: self.native_pda(LIQUIDITY_VAULT_AUTHORITY_SEED, &[bank.as_ref()]),
            liquidity_vault: self.native_pda(LIQUIDITY_VAULT_SEED, &[bank.as_ref()]), token_program: spl_token::id(),
        });
        ix.accounts.extend(self.native_observe(user, &[bank], &[]));
        self.native_send(vec![ix], self.payer.clone())
    }

    fn native_repay(&mut self, group: Pubkey, user: Pubkey, bank: Pubkey, amount: u64) -> bool {
        let ix = self.native_ix(instruction::LendingAccountRepay { amount, repay_all: Some(false) }, accounts::LendingAccountRepay {
            group, marginfi_account: user, authority: self.payer.pubkey(), bank,
            signer_token_account: self.signer_token_account,
            liquidity_vault: self.native_pda(LIQUIDITY_VAULT_SEED, &[bank.as_ref()]), token_program: spl_token::id(),
        });
        self.native_send(vec![ix], self.payer.clone())
    }

    fn native_ensure_world(&mut self) {
        if self.native_orders.is_some() { return; }
        let group_key = Keypair::new();
        let group = group_key.pubkey();
        let fee_state = self.native_pda(FEE_STATE_SEED, &[]);
        let outcome = self.ctx.program(self.program_id).call(instruction::MarginfiGroupInitialize {})
            .accounts(accounts::MarginfiGroupInitialize {
                marginfi_group: group, admin: self.payer.pubkey(), fee_state,
            }).signers(&[&*self.payer, &group_key]).send().expect("native group initialize");
        assert!(outcome.is_success(), "native group initialize: {outcome:?}");
        let banks = [self.native_new_bank(group, false, F::ONE), self.native_new_bank(group, true, F::ONE),
            self.native_new_bank(group, false, F::from_num(10)), self.native_new_bank(group, false, F::ONE),
            self.native_new_bank(group, true, F::ONE)];
        let lender = self.native_new_user(group);
        let utilization_user = self.native_new_user(group);
        let order_user = self.native_new_user(group);
        let rebalance_user = self.native_new_user(group);
        let keeper = Rc::new(Keypair::new());
        self.ctx.create_account().pubkey(keeper.pubkey()).owner(system_program::ID).lamports(100_000_000_000).create().unwrap();
        let keeper_tokens = self.ctx.create_token_account().pubkey(Pubkey::new_unique()).mint(self.bank_mint)
            .token_owner(keeper.pubkey()).amount(1_000_000 * SCALE).create().unwrap();
        self.native_orders = Some(NativeWorld { group, banks, users: vec![lender, utilization_user, order_user, rebalance_user],
            keeper, keeper_tokens, order_user, rebalance_user, order: None, rebalance_order: None, records: vec![],
            completed_orders: 0, completed_rebalances: 0, completed_settlements: 0 });
        assert!(self.native_fund(group, lender, banks[1], 1000 * SCALE), "setup fund interest bank");
        assert!(self.native_fund(group, lender, banks[3], 10_000 * SCALE), "setup fund liability bank");
        assert!(self.native_fund(group, lender, banks[4], 1000 * SCALE), "setup fund second interest bank");
        assert!(self.native_fund(group, utilization_user, banks[2], 200 * SCALE), "setup fund collateral");
        assert!(self.native_borrow(group, utilization_user, banks[1], 500 * SCALE), "setup borrow");
        // Half the utilization of banks[1]: the two interest banks keep a wide rate spread.
        assert!(self.native_borrow(group, utilization_user, banks[4], 250 * SCALE), "setup borrow second interest bank");
        assert!(self.native_fund(group, order_user, banks[2], SCALE), "setup fund order bystander");
        assert!(self.native_fund(group, rebalance_user, banks[0], REBALANCE_ASSETS), "setup fund rebalance source");
        assert!(self.native_fund(group, rebalance_user, banks[2], SCALE), "setup fund rebalance collateral");
        assert!(self.native_borrow(group, rebalance_user, banks[3], SCALE), "setup rebalance borrow");
    }

    fn native_snapshot(&self, extra: &[Pubkey]) -> Snapshot {
        let w = self.native_orders.as_ref().unwrap();
        let mut keys = vec![w.group, self.signer_token_account, w.keeper_tokens, self.native_fee_pool()];
        keys.extend(w.users.iter().copied());
        keys.extend(w.banks);
        for bank in w.banks {
            let bank = self.native_bank(bank);
            keys.extend([bank.liquidity_vault, bank.insurance_vault, bank.fee_vault]);
        }
        keys.extend(w.order);
        keys.extend(w.rebalance_order);
        keys.extend(w.records.iter().copied());
        keys.extend_from_slice(extra);
        keys.sort_unstable(); keys.dedup();
        keys.into_iter().map(|key| {
            let state = self.ctx.read_account(&key).ok().filter(|a| a.lamports != 0 || !a.data.is_empty())
                .map(|a| (a.owner, a.lamports, a.data));
            (key, state)
        }).collect()
    }

    fn native_check_rollback(&self, before: Snapshot, extra: &[Pubkey]) {
        scout_check!("NATIVE-ROLLBACK", "rejected-tx-restores-all-tracked-accounts",
            before == self.native_snapshot(extra), "native transaction changed tracked state despite rejection");
    }

    fn native_health(&self, user: &chain::MarginfiAccount) -> F {
        let mut health = F::ZERO;
        for balance in user.lending_account.balances.iter().filter(|b| b.active != 0) {
            let bank = self.native_bank(balance.bank_pk);
            let price = bank.config.fixed_price.to_i80f48();
            let scale = F::from_num(10u64.pow(bank.mint_decimals as u32));
            // calc_value's operand order: (amount x weight) x price / scale, truncating per side.
            health += (balance.asset_shares.to_i80f48() * bank.asset_share_value.to_i80f48()
                * bank.config.asset_weight_maint.to_i80f48() * price) / scale;
            health -= (balance.liability_shares.to_i80f48() * bank.liability_share_value.to_i80f48()
                * bank.config.liability_weight_maint.to_i80f48() * price) / scale;
        }
        health
    }

    /// Maintenance health with per-bank asset-share deltas applied (negative burns, positive
    /// mints), bit-exact with calc_value's per-balance truncation. end_rebalance health-checks
    /// the POST-move balance set, so the model must too: a price-moved destination can turn an
    /// otherwise legal move unhealthy (crash_43108b5e: whole-pile move into a crashed bank).
    fn native_health_with_asset_deltas(&self, user: &chain::MarginfiAccount, deltas: &[(Pubkey, F)]) -> F {
        let mut health = F::ZERO;
        for balance in user.lending_account.balances.iter().filter(|b| b.active != 0) {
            let bank = self.native_bank(balance.bank_pk);
            let price = bank.config.fixed_price.to_i80f48();
            let scale = F::from_num(10u64.pow(bank.mint_decimals as u32));
            let delta: F = deltas.iter().filter(|(key, _)| *key == balance.bank_pk).map(|(_, d)| *d).sum();
            let a_sh = (balance.asset_shares.to_i80f48() + delta).max(F::ZERO);
            health += (a_sh * bank.asset_share_value.to_i80f48()
                * bank.config.asset_weight_maint.to_i80f48() * price) / scale;
            health -= (balance.liability_shares.to_i80f48() * bank.liability_share_value.to_i80f48()
                * bank.config.liability_weight_maint.to_i80f48() * price) / scale;
        }
        // A positive delta into a bank the account holds nothing in opens a fresh balance.
        for (key, delta) in deltas.iter().filter(|(_, d)| *d > F::ZERO) {
            if user.lending_account.balances.iter().any(|b| b.active != 0 && b.bank_pk == *key) { continue; }
            let bank = self.native_bank(*key);
            let price = bank.config.fixed_price.to_i80f48();
            let scale = F::from_num(10u64.pow(bank.mint_decimals as u32));
            health += (*delta * bank.asset_share_value.to_i80f48()
                * bank.config.asset_weight_maint.to_i80f48() * price) / scale;
        }
        health
    }

    /// Post-liquidation gates from check_post_liquidation_condition_and_get_account_health plus
    /// the repay path's OperationRepayOnly dust rule, simulated at share granularity so the
    /// accept/reject decision is bit-exact against calc_value/calc_amount and the truncating
    /// share conversions. crash_f1294cb3: a 6% seize on a barely-unhealthy account lifted
    /// maintenance health back above zero -> TooSevereLiquidation, which the model then missed.
    fn native_liquidation_gates_pass(&self, liquidatee: &chain::MarginfiAccount, asset_bank: Pubkey, liab_bank: Pubkey, seize: u64) -> bool {
        let asset_state = self.native_bank(asset_bank);
        let liab_state = self.native_bank(liab_bank);
        let asv = asset_state.asset_share_value.to_i80f48();
        let lsv = liab_state.liability_share_value.to_i80f48();
        let fee = |stored: u32| if stored == 0 { F::lit("0.025") } else { F::from_num(stored) / F::from_num(u32::MAX) };
        let final_discount = F::ONE - (fee(liab_state.liquidation_liquidator_fee) + fee(liab_state.liquidation_insurance_fee));
        // calc_value(seize, price_a, dec_a, final_discount) then calc_amount(.., price_l, dec_l).
        let seized_value = (F::from_num(seize) * final_discount) * self.native_price(asset_bank)
            / F::from_num(10u64.pow(asset_state.mint_decimals as u32));
        let repay_amt = (seized_value * F::from_num(10u64.pow(liab_state.mint_decimals as u32))) / self.native_price(liab_bank);
        let (a_shares_pre, ..) = position(liquidatee, asset_bank);
        let (_, l_shares_pre, _) = position(liquidatee, liab_bank);
        let liab_pre_amt = l_shares_pre * lsv;
        // RepayOnly rejects an over-repay beyond the dust threshold; within dust it clamps.
        if repay_amt - liab_pre_amt > F::lit("0.0001") { return false; }
        let repaid = repay_amt.min(liab_pre_amt);
        let a_shares_post = a_shares_pre - F::from_num(seize) / asv;
        let l_shares_post = l_shares_pre - repaid / lsv;
        // ExhaustedLiability: the liab leg must survive (is_empty := shares < 1).
        if l_shares_post < F::ONE { return false; }
        let mut post_health = F::ZERO;
        for balance in liquidatee.lending_account.balances.iter().filter(|b| b.active != 0) {
            let bank = self.native_bank(balance.bank_pk);
            let price = bank.config.fixed_price.to_i80f48();
            let scale = F::from_num(10u64.pow(bank.mint_decimals as u32));
            let (a_sh, l_sh) = if balance.bank_pk == asset_bank {
                (a_shares_post, balance.liability_shares.to_i80f48())
            } else if balance.bank_pk == liab_bank {
                (balance.asset_shares.to_i80f48(), l_shares_post)
            } else {
                (balance.asset_shares.to_i80f48(), balance.liability_shares.to_i80f48())
            };
            post_health += (a_sh * bank.asset_share_value.to_i80f48() * bank.config.asset_weight_maint.to_i80f48() * price) / scale;
            post_health -= (l_sh * bank.liability_share_value.to_i80f48() * bank.config.liability_weight_maint.to_i80f48() * price) / scale;
        }
        // TooSevereLiquidation / WorseHealthPostLiquidation: still unhealthy, strictly improved.
        post_health <= F::ZERO && post_health > self.native_health(liquidatee)
    }

    pub(super) fn native_place_order(&mut self, trigger: u8, slippage_bps: u16) -> bool {
        self.native_ensure_world();
        let w = self.native_orders.as_ref().unwrap().clone();
        if w.order.is_some() { return true; }
        let user = self.native_user(w.order_user);
        let assets = position(&user, w.banks[0]).0 * self.native_bank(w.banks[0]).asset_share_value.to_i80f48();
        if assets < F::from_num(ORDER_ASSETS) && !self.native_fund(w.group, w.order_user, w.banks[0], ORDER_ASSETS) { return false; }
        let debt = position(&self.native_user(w.order_user), w.banks[3]).1;
        if debt == F::ZERO && !self.native_borrow(w.group, w.order_user, w.banks[3], ORDER_DEBT) { return false; }
        let user = self.native_user(w.order_user);
        let net = self.native_pair_net(&user, w.banks[0], w.banks[3]);
        let slip = ((slippage_bps as u64 % 1001) * u32::MAX as u64 / 10_000) as u32;
        // Placement validation (state/order.rs initialize): thresholds must be > 0. Price moves
        // can push net to/below zero, so some triggers become legitimately unplaceable.
        let (trigger, valid) = match trigger % 4 {
            0 => (arg::OrderTrigger::StopLoss { threshold: wrapped(net), max_slippage: slip }, net > F::ZERO),
            1 => (arg::OrderTrigger::TakeProfit { threshold: wrapped(net), max_slippage: slip }, net > F::ZERO),
            2 => (arg::OrderTrigger::Both { stop_loss: wrapped(net - F::ONE), take_profit: wrapped(net), max_slippage: slip }, net - F::ONE > F::ZERO),
            _ => (arg::OrderTrigger::TakeProfit { threshold: wrapped(net + F::ONE), max_slippage: slip }, net + F::ONE > F::ZERO),
        };
        let mut keys = vec![w.banks[0], w.banks[3]]; keys.sort_unstable();
        let hash = solana_program::hash::hashv(&keys.iter().map(|key| key.as_ref()).collect::<Vec<_>>()).to_bytes();
        let order = self.native_pda(b"order", &[w.order_user.as_ref(), &hash]);
        let ix = self.native_ix(instruction::MarginfiAccountPlaceOrder { bank_keys: keys, trigger }, accounts::MarginfiAccountPlaceOrder {
            group: w.group, marginfi_account: w.order_user, fee_payer: self.payer.pubkey(), authority: self.payer.pubkey(), order,
            fee_state: self.native_pda(FEE_STATE_SEED, &[]), global_fee_wallet: self.global_fee_wallet,
        });
        let before = self.native_snapshot(&[]);
        let ok = self.native_send(vec![ix], self.payer.clone());
        scout_check!("NATIVE-ORDER", "placement-validation-matches-model", ok == valid,
            "native place order: net={net}, accepted={ok}, expected={valid}");
        if ok { self.native_orders.as_mut().unwrap().order = Some(order); } else { self.native_check_rollback(before, &[]); }
        ok
    }

    pub(super) fn native_execute_order(&mut self, variant: u8, amount: u16) -> bool {
        self.native_ensure_world();
        if self.native_orders.as_ref().unwrap().order.is_none() { self.native_place_order(variant % 3, 500); }
        let w = self.native_orders.as_ref().unwrap().clone();
        // A failed placement already recorded a NATIVE-ORDER violation; a bare panic here would
        // abort the campaign as a "harness bug" and mask it (campaign-42b). Stop the chain instead.
        let Some(order) = w.order else { return false; };
        let intent: chain::Order = self.native_load(order, chain::Order::DISCRIMINATOR);
        let pre = self.native_user(w.order_user);
        let scale = F::from_num(SCALE);
        let price_a = self.native_price(w.banks[0]);
        let assets = position(&pre, w.banks[0]).0 * self.native_bank(w.banks[0]).asset_share_value.to_i80f48();
        let net = self.native_pair_net(&pre, w.banks[0], w.banks[3]);
        let sl = intent.stop_loss.to_i80f48();
        let tp = intent.take_profit.to_i80f48();
        let (triggered, take_profit) = match intent.trigger {
            arg::OrderTriggerType::StopLoss => (net <= sl, false),
            arg::OrderTriggerType::TakeProfit => (net >= tp, true),
            arg::OrderTriggerType::Both => (net <= sl || net >= tp, net >= tp),
        };
        let fee: chain::FeeState = self.native_load(self.native_pda(FEE_STATE_SEED, &[]), chain::FeeState::DISCRIMINATOR);
        let retain = F::ONE - F::from_num(intent.max_slippage) / F::from_num(u32::MAX);
        let bound = if take_profit { (tp * retain).max(net * (F::ONE - fee.order_execution_max_fee.to_i80f48())) }
            else { net * retain };
        // USD bound -> native atoms of the asset bank. With positive equity bound <= net <=
        // assets*price_a holds, so a satisfying withdrawal exists whenever the trigger fired.
        // With NEGATIVE equity (post-liquidation at a crashed price) the bound is negative and
        // the formula exceeds the position: cap at the position, which satisfies the bound
        // trivially (crash_a14c183a7bb7a365).
        let max_withdraw = (assets - bound / price_a * scale).max(F::ZERO)
            .min(assets.floor()).to_num::<u64>();
        let variant = variant % 5;
        let withdrawal = if variant == 1 { max_withdraw + SCALE } else { max_withdraw * (amount as u64 % 100 + 1) / 100 };
        let record = self.native_pda(b"execute_order", &[order.as_ref()]);
        let mut start = self.native_ix(instruction::MarginfiAccountStartExecuteOrder {}, accounts::MarginfiAccountStartExecuteOrder {
            group: w.group, marginfi_account: w.order_user, fee_payer: w.keeper.pubkey(), executor: w.keeper.pubkey(),
            order, execute_record: record,
        });
        start.accounts.extend(self.native_observe(w.order_user, &[], &[]));
        let mut end = self.native_ix(instruction::MarginfiAccountEndExecuteOrder {}, accounts::MarginfiAccountEndExecuteOrder {
            group: w.group, marginfi_account: w.order_user, executor: w.keeper.pubkey(), fee_recipient: w.keeper.pubkey(), order,
            execute_record: record, fee_state: self.native_pda(FEE_STATE_SEED, &[]),
        });
        end.accounts.extend(self.native_observe(w.order_user, &[], &[w.banks[3]]));
        let mut ixs = vec![start, self.native_withdraw_ix(w.order_user, w.banks[0], withdrawal, false),
            self.native_repay_ix(w.order_user, w.banks[3], if variant == 2 { ORDER_DEBT / 2 } else { u64::MAX }, variant != 2)];
        if variant == 3 { ixs.push(self.native_withdraw_ix(w.order_user, w.banks[2], 1, false)); }
        if variant != 4 { ixs.push(end); }
        let before = self.native_snapshot(&[record]);
        let composition_ok = self.native_order_composition_ok(&w, order);
        let expected = triggered && variant == 0 && composition_ok;
        let ok = self.native_send(ixs, w.keeper.clone());
        scout_check!("NATIVE-ORDER", "execution-result-matches-independent-bound", ok == expected,
            "native order variant={variant}, triggered={triggered}, accepted={ok}, expected={expected}");
        if !ok { self.native_check_rollback(before, &[record]); return true; }
        let post = self.native_user(w.order_user);
        let post_asset_usd = position(&post, w.banks[0]).0 / scale * price_a;
        scout_check!("NATIVE-ORDER", "liability-closed-and-equity-bound", position(&post, w.banks[3]).1 == F::ZERO
            && post_asset_usd + TOLERANCE >= bound);
        scout_check!("NATIVE-ORDER", "bystander-position-and-tag-unchanged", position(&pre, w.banks[2]) == position(&post, w.banks[2]));
        scout_check!("NATIVE-HEALTH", "order-health-policy", self.native_health(&post) >= F::ZERO
            || post_asset_usd + TOLERANCE >= net);
        scout_check!("NATIVE-ORDER", "execution-closes-order-and-record", !self.native_exists(order) && !self.native_exists(record));
        self.native_orders.as_mut().unwrap().order = None;
        self.native_orders.as_mut().unwrap().completed_orders += 1;
        true
    }

    pub(super) fn native_cancel_order(&mut self, unauthorized: bool) -> bool {
        self.native_ensure_world();
        if self.native_orders.as_ref().unwrap().order.is_none() { self.native_place_order(1, 500); }
        let w = self.native_orders.as_ref().unwrap().clone();
        let Some(order) = w.order else { return false; };
        let signer = if unauthorized { w.keeper.clone() } else { self.payer.clone() };
        let ix = self.native_ix(instruction::MarginfiAccountCloseOrder {}, accounts::MarginfiAccountCloseOrder {
            group: w.group, marginfi_account: w.order_user, authority: signer.pubkey(), order,
            fee_recipient: signer.pubkey(),
        });
        let before = self.native_snapshot(&[]);
        let ok = self.native_send(vec![ix], signer);
        scout_check!("NATIVE-ORDER", "cancel-authority", ok != unauthorized);
        if ok { self.native_orders.as_mut().unwrap().order = None; } else { self.native_check_rollback(before, &[]); }
        true
    }

    /// True when any of the order's tags is absent from every active balance -- the exact
    /// keeper_close_order precondition (order.rs:208-212). Balance dust does NOT make an order
    /// closable: a tagged slot that still exists still carries its tag.
    fn native_order_keeper_closable(&self, w: &NativeWorld, order_key: Pubkey) -> bool {
        let intent: chain::Order = self.native_load(order_key, chain::Order::DISCRIMINATOR);
        let user = self.native_user(w.order_user);
        intent.tags.iter().any(|tag| !user.lending_account.balances.iter().any(|b| b.active != 0 && b.tag == *tag))
    }

    /// The start_execute_order composition check (order.rs:311-314): exactly one tagged balance
    /// with an asset side and exactly one with a liability side. Sides vanish below
    /// EMPTY_BALANCE_THRESHOLD (1 share), so dust-tagged slots do not count.
    fn native_order_composition_ok(&self, w: &NativeWorld, order_key: Pubkey) -> bool {
        let intent: chain::Order = self.native_load(order_key, chain::Order::DISCRIMINATOR);
        let user = self.native_user(w.order_user);
        let (mut assets, mut liabs) = (0u32, 0u32);
        for b in user.lending_account.balances.iter().filter(|b| b.active != 0 && intent.tags.contains(&b.tag)) {
            if b.asset_shares.to_i80f48() >= F::ONE { assets += 1; }
            if b.liability_shares.to_i80f48() >= F::ONE { liabs += 1; }
        }
        assets == 1 && liabs == 1
    }

    /// Permissionless keeper close: must fail while the order's tags identify live balances and
    /// must succeed once any tag was cleared (or the tagged balance is gone).
    pub(super) fn native_keeper_close_order(&mut self) -> bool {
        self.native_ensure_world();
        if self.native_orders.as_ref().unwrap().order.is_none() { self.native_place_order(1, 500); }
        let w = self.native_orders.as_ref().unwrap().clone();
        let Some(order) = w.order else { return false; };
        let closable = self.native_order_keeper_closable(&w, order);
        let ix = self.native_ix(instruction::MarginfiAccountKeeperCloseOrder {}, accounts::MarginfiAccountKeeperCloseOrder {
            marginfi_account: w.order_user, fee_recipient: self.probe_user.pubkey(), order,
        });
        let before = self.native_snapshot(&[]);
        let rent = self.native_lamports(order);
        let recipient_before = self.native_lamports(self.probe_user.pubkey());
        let ok = self.native_send(vec![ix], w.keeper.clone());
        scout_check!("NATIVE-ORDER", "keeper-close-only-when-tag-stale", ok == closable,
            "native keeper close: accepted={ok}, closable={closable}");
        if !ok { self.native_check_rollback(before, &[]); return true; }
        scout_check!("NATIVE-ORDER", "keeper-close-pays-rent-to-recipient",
            !self.native_exists(order) && self.native_lamports(self.probe_user.pubkey()) - recipient_before == rent);
        self.native_orders.as_mut().unwrap().order = None;
        true
    }

    /// The authority clears order tags on one bank (or all); an outsider must be rejected. A
    /// cleared tag makes the order keeper-closeable, which the invariant suite then tracks.
    pub(super) fn native_set_keeper_close_flags(&mut self, unauthorized: bool, all: bool) -> bool {
        self.native_ensure_world();
        if self.native_orders.as_ref().unwrap().order.is_none() { self.native_place_order(1, 500); }
        let w = self.native_orders.as_ref().unwrap().clone();
        let Some(order) = w.order else { return false; };
        let signer = if unauthorized { w.keeper.clone() } else { self.payer.clone() };
        let bank_keys_opt = if all { None } else { Some(vec![w.banks[0]]) };
        let ix = self.native_ix(instruction::MarginfiAccountSetKeeperCloseFlags { bank_keys_opt },
            accounts::MarginfiAccountSetKeeperCloseFlags { group: w.group, marginfi_account: w.order_user, authority: signer.pubkey() });
        let before = self.native_snapshot(&[]);
        let ok = self.native_send(vec![ix], signer);
        scout_check!("NATIVE-ORDER", "set-keeper-close-flags-authority", ok != unauthorized);
        if !ok { self.native_check_rollback(before, &[]); return true; }
        scout_check!("NATIVE-ORDER", "cleared-flags-make-order-keeper-closeable", self.native_order_keeper_closable(&w, order));
        true
    }

    /// Liquidation composed with a live tagged order (the MFI-35 net). The order user always
    /// carries an order tagging banks[0] (asset) and banks[3] (liability). Variants:
    ///   0: crash banks[0], seize part of the TAGGED asset, repaying the tagged liability
    ///   1: crash banks[0], seize the UNTAGGED bystander banks[2], repaying the tagged liability
    ///   2: crash banks[0], seize more than the tagged asset holds -- OverliquidationAttempt
    ///   3: no crash -- the account is healthy, liquidation must reject
    /// Seizes stay under half the seized position, so that balance always survives. The REPAY
    /// may still grind the tagged liability to dust (or close it) when earlier price moves
    /// changed the pair ratio -- the tag then dies with the slot, exactly the MFI-35 stale-tag
    /// transition. Post-checks branch on the outcome, and the chained execute_order re-checks
    /// the program's composition gate against the model on post-liquidation state.
    pub(super) fn native_liquidate_order_user(&mut self, variant: u8, seize_pct: u8) -> bool {
        self.native_ensure_world();
        if self.native_orders.as_ref().unwrap().order.is_none() { self.native_place_order(0, 500); }
        let w = self.native_orders.as_ref().unwrap().clone();
        let Some(order) = w.order else { return false; };
        let variant = variant % 4;
        let asset_bank = if variant == 1 { w.banks[2] } else { w.banks[0] };
        let liab_bank = w.banks[3];
        let liquidator = w.users[0];
        let pre_liquidatee = self.native_user(w.order_user);
        let pre_liquidator = self.native_user(liquidator);
        let asset_pos = self.native_underlying(&pre_liquidatee, asset_bank).floor().to_num::<u64>();
        // A dust seized bank could drain fully and exercise closure instead of composition;
        // that regime is covered by the keeper-close actions. Skip it here.
        if asset_pos < 2 { return true; }
        let seize = if variant == 2 { asset_pos + SCALE } else { (asset_pos * (seize_pct as u64 % 50 + 1) / 100).max(1) };
        let original_price = self.native_price(w.banks[0]);
        if variant != 3 {
            let crashed = (original_price / F::from_num(500)).max(F::lit("0.000001"));
            if !self.native_set_price(&w, w.banks[0], crashed) { return false; }
        }
        // Liquidation requires maintenance health to be non-positive (strictly positive rejects).
        let unhealthy = self.native_health(&pre_liquidatee) <= F::ZERO;
        let expected = unhealthy && variant != 2 && asset_pos > 0
            && self.native_liquidation_gates_pass(&pre_liquidatee, asset_bank, liab_bank, seize);
        // Captured pre-tx: whether the live order's tags currently identify the pair. Mid-campaign
        // the order can already be stale (an earlier liquidation dusted a tagged balance and the
        // re-opened slot carries no tag), in which case post-liquidation composition is
        // unconstrained beyond the closable implication below.
        let composition_pre = self.native_order_composition_ok(&w, order);

        // The health parser consumes each remaining-accounts block positionally against the
        // account's balance array, and liquidation sorts BOTH accounts in-program (liquidatee
        // at the top, liquidator just before its post-health check). Both blocks are therefore
        // the active balance set in descending key order; the liquidator block additionally
        // covers the asset bank, which find_or_create adds when the liquidator lacks it.
        let liquidatee_block = self.native_observe(w.order_user, &[], &[]);
        let liquidator_block = self.native_observe(liquidator, &[asset_bank], &[]);

        let mut ix = self.native_ix(instruction::LendingAccountLiquidate {
            asset_amount: seize, liquidatee_accounts: liquidatee_block.len() as u8, liquidator_accounts: liquidator_block.len() as u8,
        }, accounts::LendingAccountLiquidate {
            group: w.group, asset_bank, liab_bank,
            liquidator_marginfi_account: liquidator, authority: self.payer.pubkey(),
            liquidatee_marginfi_account: w.order_user,
            bank_liquidity_vault_authority: self.native_pda(LIQUIDITY_VAULT_AUTHORITY_SEED, &[liab_bank.as_ref()]),
            bank_liquidity_vault: self.native_pda(LIQUIDITY_VAULT_SEED, &[liab_bank.as_ref()]),
            bank_insurance_vault: self.native_pda(INSURANCE_VAULT_SEED, &[liab_bank.as_ref()]),
            token_program: spl_token::id(),
        });
        ix.accounts.extend(liquidator_block.iter().cloned());
        ix.accounts.extend(liquidatee_block.iter().cloned());
        let before = self.native_snapshot(&[]);
        let ok = self.native_send(vec![ix], self.payer.clone());
        scout_check!("NATIVE-ORDER", "liquidation-result-matches-model", ok == expected,
            "native liquidation variant={variant}, unhealthy={unhealthy}, accepted={ok}, expected={expected}");
        if !ok {
            self.native_check_rollback(before, &[]);
            if variant != 3 { assert!(self.native_set_price(&w, w.banks[0], original_price), "native price restore"); }
            return true;
        }

        let post_liquidatee = self.native_user(w.order_user);
        let post_liquidator = self.native_user(liquidator);
        let price_a = self.native_price(asset_bank);
        let price_l = self.native_price(liab_bank);
        // Mirror liquidation_fee_fraction: a stored 0 falls back to DEFAULT_LIQUIDATION_FEE.
        let fee = |stored: u32| if stored == 0 { F::lit("0.025") } else { F::from_num(stored) / F::from_num(u32::MAX) };
        let liab_state = self.native_bank(liab_bank);
        let (f_liq, f_ins) = (fee(liab_state.liquidation_liquidator_fee), fee(liab_state.liquidation_insurance_fee));
        let lsv = liab_state.liability_share_value.to_i80f48();
        let liab_pre = position(&pre_liquidatee, liab_bank).1 * lsv;
        let seized = F::from_num(seize);
        let asset_delta = self.native_underlying(&pre_liquidatee, asset_bank) - self.native_underlying(&post_liquidatee, asset_bank);
        let liab_delta = liab_pre - position(&post_liquidatee, liab_bank).1 * lsv;
        // The repay is capped at the outstanding liability: a seize whose discounted value
        // exceeds the remaining debt grinds the tagged liability to dust (or closes it).
        scout_check!("NATIVE-ORDER", "liquidation-moves-declared-amounts",
            (asset_delta - seized).abs() <= F::from_num(3)
            && (liab_delta - (seized * price_a * (F::ONE - f_liq - f_ins) / price_l).min(liab_pre)).abs() <= F::from_num(3),
            "native liquidation flows: seized asset_delta={asset_delta} (declared {seized}), liab_delta={liab_delta}");
        // The liquidator draws down its own liab-bank deposit (never a new liability here) by
        // the discounted amount and receives exactly the seized collateral.
        let lq_paid = (position(&pre_liquidator, liab_bank).0 - position(&post_liquidator, liab_bank).0)
            * liab_state.asset_share_value.to_i80f48();
        let lq_got = self.native_underlying(&post_liquidator, asset_bank) - self.native_underlying(&pre_liquidator, asset_bank);
        scout_check!("NATIVE-ORDER", "liquidator-pays-discount-gets-seizure",
            (lq_paid - seized * price_a * (F::ONE - f_liq) / price_l).abs() <= F::from_num(3)
            && (lq_got - seized).abs() <= F::from_num(3)
            && position(&post_liquidator, liab_bank).1 == position(&pre_liquidator, liab_bank).1,
            "native liquidation liquidator: paid={lq_paid}, got={lq_got}, seized={seized}");
        // The seize stays under half the seized position, so that balance always survives with
        // its tag intact. The tagged LIABILITY may not: an over-valued seize repays it to dust
        // or closure, and its tag dies with the slot. What must never happen is a surviving
        // balance losing or changing its tag.
        let asset_tag_pre = position(&pre_liquidatee, asset_bank).2;
        let liab_tag_pre = position(&pre_liquidatee, liab_bank).2;
        let (asset_shares_post, _, asset_tag_post) = position(&post_liquidatee, asset_bank);
        let (_, liab_shares_post, liab_tag_post) = position(&post_liquidatee, liab_bank);
        scout_check!("NATIVE-ORDER", "liquidation-keeps-tags-on-surviving-balances",
            asset_shares_post >= F::ONE && asset_tag_post == asset_tag_pre
            && (liab_shares_post < F::ONE || liab_tag_post == liab_tag_pre),
            "native liquidation variant={variant}: tags pre=({asset_tag_pre},{liab_tag_pre}) post=({asset_tag_post},{liab_tag_post}), liab_shares_post={liab_shares_post}");
        // Composition is coherent exactly while both tagged balances survive -- but that
        // identity is only assertable when the order's tags still identified the pair before
        // the liquidation (see composition_pre). Coherent => not keeper-closeable; a broken
        // composition may still carry tagged dust, so keeper close stays gated off until the
        // slot is gone -- keeper-closeable is asserted by its own action.
        let tagged_pair_alive = position(&post_liquidatee, w.banks[0]).0 >= F::ONE && liab_shares_post >= F::ONE;
        let composition = self.native_order_composition_ok(&w, order);
        scout_check!("NATIVE-ORDER", "liquidation-order-lifecycle-consistency",
            (!composition_pre || composition == tagged_pair_alive)
            && (!composition || !self.native_order_keeper_closable(&w, order)),
            "native liquidation variant={variant}: composition_pre={composition_pre}, composition={composition}, pair_alive={tagged_pair_alive}");

        if variant != 3 { assert!(self.native_set_price(&w, w.banks[0], original_price), "native price restore"); }
        // MFI-35 net: run an execution attempt against post-liquidation state. Its own model
        // asserts the program's composition gate (order.rs:311-314) agrees with this one.
        self.native_execute_order(0, 99);
        true
    }

    fn native_fee_pool(&self) -> Pubkey {
        self.native_pda(b"rebalance_fee_pool", &[self.native_orders.as_ref().unwrap().rebalance_user.as_ref()])
    }

    pub(super) fn native_place_rebalance(&mut self, amount: u32, tip: u32) -> bool {
        self.native_ensure_world();
        let w = self.native_orders.as_ref().unwrap().clone();
        if w.rebalance_order.is_some() { return true; }
        if position(&self.native_user(w.rebalance_user), w.banks[0]).0 == F::ZERO
            && !self.native_fund(w.group, w.rebalance_user, w.banks[0], REBALANCE_ASSETS) { return false; }
        let order = self.native_pda(b"rebalance_order", &[w.rebalance_user.as_ref(), self.bank_mint.as_ref()]);
        // The allowlist always spans both interest banks: multi-move executions need the spread.
        let allowlist = vec![w.banks[0], w.banks[1], w.banks[4]];
        let ix = self.native_ix(instruction::MarginfiAccountPlaceRebalanceOrder {
            allowed_banks: allowlist, min_improvement: Some(wrapped(F::ZERO)), cooldown_seconds: Some(COOLDOWN),
            amount: Some(if amount % 3 == 0 { 0 } else { amount as u64 % REBALANCE_ASSETS + 1 }), keeper_tip: Some(tip as u64 % 1_000_001),
        }, accounts::MarginfiAccountPlaceRebalanceOrder {
            group: w.group, marginfi_account: w.rebalance_user, authority: self.payer.pubkey(), mint: self.bank_mint,
            rebalance_order: order, fee_payer: self.payer.pubkey(),
        });
        let ok = self.native_send(vec![ix], self.payer.clone());
        scout_check!("NATIVE-REBALANCE", "valid-placement-reaches-handler", ok);
        if ok {
            self.native_orders.as_mut().unwrap().rebalance_order = Some(order);
            self.native_rebalance_fee_pool(false, 10_000_000);
        }
        ok
    }

    pub(super) fn native_update_rebalance(&mut self, amount: u32, tip: u32) -> bool {
        self.native_ensure_world();
        if self.native_orders.as_ref().unwrap().rebalance_order.is_none() { return self.native_place_rebalance(amount, tip); }
        let w = self.native_orders.as_ref().unwrap().clone();
        let ix = self.native_ix(instruction::MarginfiAccountUpdateRebalanceOrder {
            allowed_banks: None, min_improvement: None, cooldown_seconds: None,
            amount: Some(if amount % 3 == 0 { 0 } else { amount as u64 % REBALANCE_ASSETS + 1 }), keeper_tip: Some(tip as u64 % 1_000_001),
        }, accounts::MarginfiAccountUpdateRebalanceOrder { marginfi_account: w.rebalance_user, authority: self.payer.pubkey(), rebalance_order: w.rebalance_order.unwrap() });
        let ok = self.native_send(vec![ix], self.payer.clone());
        scout_check!("NATIVE-REBALANCE", "update-policy", ok);
        ok
    }

    fn native_advance(&mut self, seconds: u64) {
        let mut clock = self.ctx.svm.get_sysvar::<Clock>();
        clock.unix_timestamp += seconds as i64;
        clock.slot += 1;
        self.ctx.set_sysvar(&clock);
    }

    pub(super) fn native_execute_rebalance(&mut self, variant: u8, amount: u32) -> bool {
        self.native_ensure_world();
        if self.native_orders.as_ref().unwrap().rebalance_order.is_none() { self.native_place_rebalance(0, 200_000); }
        let w = self.native_orders.as_ref().unwrap().clone();
        // Same campaign-42b guard as order placement: a rejected placement is already a recorded
        // violation; never unwrap it into a harness panic.
        let Some(order) = w.rebalance_order else { return false; };
        let policy: chain::RebalanceOrder = self.native_load(order, chain::RebalanceOrder::DISCRIMINATOR);
        let variant = variant % 6;
        if variant != 4 { self.native_advance(COOLDOWN); }
        if position(&self.native_user(w.users[1]), w.banks[1]).1 == F::ZERO
            && !self.native_borrow(w.group, w.users[1], w.banks[1], 500 * SCALE) { return false; }
        // Bring native share values current before measuring the transaction's own token deltas.
        for bank in [w.banks[0], w.banks[1], w.banks[4]] { self.native_accrue(w.group, bank); }
        if position(&self.native_user(w.rebalance_user), w.banks[0]).0 < F::from_num(SCALE)
            && !self.native_fund(w.group, w.rebalance_user, w.banks[0], REBALANCE_ASSETS) { return false; }
        let pre = self.native_user(w.rebalance_user);
        let source = self.native_underlying(&pre, w.banks[0]);
        let destination = self.native_underlying(&pre, w.banks[1]);
        let budget = if policy.amount == 0 { source.floor().to_num::<u64>() } else { policy.amount.min(source.floor().to_num::<u64>()) };
        let moved = 1 + amount as u64 % budget.max(1);
        let declared = if variant == 1 { moved + SCALE } else { moved };
        let declared_ui = F::from_num(declared) / F::from_num(SCALE);
        let sequence = pre.rebalance_execution_seq;
        let record = self.native_pda(b"rebalance_record", &[w.rebalance_user.as_ref(), &sequence.to_le_bytes()]);
        let mut start = self.native_ix(instruction::MarginfiAccountStartRebalance {
            moves: vec![arg::RebalanceMove { src_index: 0, dst_index: 1, _pad0: [0; 6], amount: wrapped(declared_ui) }],
            execution_seq: if variant == 5 { sequence + 1 } else { sequence },
        }, accounts::MarginfiAccountStartRebalance {
            group: w.group, marginfi_account: w.rebalance_user, rebalance_order: order, executor: w.keeper.pubkey(),
            rebalance_record: record, fee_payer: w.keeper.pubkey(),
        });
        start.accounts.extend([AccountMeta::new(w.banks[0], false), AccountMeta::new(w.banks[1], false), AccountMeta::new(w.banks[4], false)]);
        let mut end = self.native_ix(instruction::MarginfiAccountEndRebalance {}, accounts::MarginfiAccountEndRebalance {
            group: w.group, marginfi_account: w.rebalance_user, rebalance_order: order, rebalance_record: record,
            executor: w.keeper.pubkey(), fee_pool: self.native_fee_pool(),
        });
        end.accounts.extend([AccountMeta::new(w.banks[0], false), AccountMeta::new(w.banks[1], false), AccountMeta::new(w.banks[4], false)]);
        let emptied = if F::from_num(moved) == source { vec![w.banks[0]] } else { vec![] };
        end.accounts.extend(self.native_observe(w.rebalance_user, &[w.banks[1]], &emptied));
        let deposit = if variant == 2 { moved.saturating_sub(100) } else { moved };
        let mut ixs = vec![start, self.native_withdraw_ix(w.rebalance_user, w.banks[0], moved, F::from_num(moved) == source),
            self.native_deposit_ix(w.group, w.rebalance_user, w.banks[1], deposit, true)];
        if variant == 3 { ixs.push(self.native_withdraw_ix(w.rebalance_user, w.banks[2], 1, false)); }
        ixs.push(end);
        let before = self.native_snapshot(&[record]);
        let pool_before = self.native_lamports(self.native_fee_pool());
        let keeper_before = self.native_tokens(w.keeper_tokens);
        let now = self.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64;
        // end_rebalance runs a maintenance health pass on the POST-move balances; a move into a
        // price-crashed destination can reject even when the account was healthy beforehand.
        let asv0 = self.native_bank(w.banks[0]).asset_share_value.to_i80f48();
        let asv1 = self.native_bank(w.banks[1]).asset_share_value.to_i80f48();
        let burned = if F::from_num(moved) == source { position(&pre, w.banks[0]).0 }
            else { F::from_num(moved).min(source) / asv0 };
        let post_healthy = self.native_health_with_asset_deltas(&pre,
            &[(w.banks[0], -burned), (w.banks[1], F::from_num(deposit) / asv1)]) >= F::ZERO;
        let healthy = self.native_health(&pre) >= F::ZERO;
        // start_rebalance's rate gates (improvement + best-venue), modeled bit-exactly.
        let venue_ok = Self::native_venue_accepts(
            &[self.native_bank(w.banks[0]), self.native_bank(w.banks[1]), self.native_bank(w.banks[4])],
            &[w.banks[0], w.banks[1], w.banks[4]].map(|key| pre.lending_account.balances.iter().any(|b| b.active != 0 && b.bank_pk == key)),
            &[position(&pre, w.banks[0]).2, position(&pre, w.banks[1]).2, position(&pre, w.banks[4]).2],
            &[(0, 1, declared_ui)], policy.min_improvement.to_i80f48());
        let expected = (variant == 0 || variant == 4) && healthy && venue_ok && post_healthy
            && now >= policy.last_exec_timestamp + policy.cooldown_seconds;
        let ok = self.native_send(ixs, w.keeper.clone());
        scout_check!("NATIVE-REBALANCE", "execution-result-matches-model", ok == expected,
            "native rebalance variant={variant}, accepted={ok}, expected={expected}");
        if !ok { self.native_check_rollback(before, &[record]); return true; }
        let post = self.native_user(w.rebalance_user);
        let delta_src = source - self.native_underlying(&post, w.banks[0]);
        let delta_dst = self.native_underlying(&post, w.banks[1]) - destination;
        let atom_slack = F::from_num(3);
        scout_check!("NATIVE-REBALANCE", "principal-and-declared-flows", (delta_src - F::from_num(moved)).abs() <= atom_slack
            && (delta_dst - F::from_num(moved)).abs() <= atom_slack && (delta_src - delta_dst).abs() <= atom_slack);
        scout_check!("NATIVE-REBALANCE", "keeper-cannot-retain-principal", self.native_tokens(w.keeper_tokens) == keeper_before);
        scout_check!("NATIVE-REBALANCE", "bystanders-and-sequence", position(&pre, w.banks[2]) == position(&post, w.banks[2])
            && position(&pre, w.banks[3]) == position(&post, w.banks[3]) && post.rebalance_execution_seq == sequence + 1);
        scout_check!("NATIVE-HEALTH", "rebalance-maintenance-health", self.native_health(&post) >= -TOLERANCE);
        // The tip is denominated in whole-token fixed point. Convert each position before taking
        // the delta: dividing an atom-denominated delta afterward changes floor() at tiny tips.
        // The program's target denominator spans EVERY referenced bank's start amount, so the
        // second interest bank counts once the allowlist covers it.
        let scale = F::from_num(SCALE);
        let pre_total_ui = (source + destination + self.native_underlying(&pre, w.banks[4])) / scale;
        let landed_ui = self.native_underlying(&post, w.banks[1]) / scale - destination / scale;
        let target = if policy.amount == 0 { pre_total_ui } else { (F::from_num(policy.amount) / scale).min(pre_total_ui) };
        let rent = self.ctx.svm.minimum_balance_for_rent_exemption(0);
        let expected_tip = (F::from_num(policy.keeper_tip) * (landed_ui / target).min(F::ONE)).floor().to_num::<u64>()
            .min(pool_before.saturating_sub(rent));
        scout_check!("NATIVE-REBALANCE", "tip-proportional-and-rent-reserved", pool_before - self.native_lamports(self.native_fee_pool()) == expected_tip,
            "native tip: expected={expected_tip}, actual={}, landed_ui={landed_ui}, target={target}", pool_before - self.native_lamports(self.native_fee_pool()));
        if expected_tip > 0 {
            let actual: chain::RebalanceRecord = self.native_load(record, chain::RebalanceRecord::DISCRIMINATOR);
            scout_check!("NATIVE-REBALANCE", "tip-escrow-bound-to-keeper", actual.pending_tip == expected_tip && actual.executor == w.keeper.pubkey());
            self.native_orders.as_mut().unwrap().records.push(record);
        } else { scout_check!("NATIVE-REBALANCE", "untipped-record-closed", !self.native_exists(record)); }
        self.native_orders.as_mut().unwrap().completed_rebalances += 1;
        true
    }

    /// Multi-move executions over the three-bank allowlist [banks[0], banks[1], banks[4]]
    /// (remaining-account indices 0, 1, 2):
    ///   0: consolidate 0->1 and 4->1 -- both sources pay less than the destination: succeeds
    ///   1: split 0->1 and 0->4 -- rejected by the best-venue rule whenever banks[1] still beats
    ///      banks[4] after absorbing the move; accepted when the legs truncate to zero native
    ///      atoms (dust prices no alternative venue) or rates have compressed; a == 1 also makes
    ///      the first leg zero
    ///   2: two same-direction moves 0->1 -- legal, reconciles like one flow of their sum
    ///   3: order-tagged whole move 0->1 for a self-contained second order on the rebalance
    ///      user -- succeeds iff the account holds no banks[1] balance; cleaned up in-action
    pub(super) fn native_execute_rebalance_multi(&mut self, variant: u8, amount: u32) -> bool {
        self.native_ensure_world();
        if self.native_orders.as_ref().unwrap().rebalance_order.is_none() { self.native_place_rebalance(0, 200_000); }
        let w = self.native_orders.as_ref().unwrap().clone();
        let Some(order) = w.rebalance_order else { return false; };
        let policy: chain::RebalanceOrder = self.native_load(order, chain::RebalanceOrder::DISCRIMINATOR);
        let variant = variant % 4;
        self.native_advance(COOLDOWN);
        // Both interest venues need utilization so their supply rates stay ordered 1 > 4 > 0.
        for (bank, amt) in [(w.banks[1], 500 * SCALE), (w.banks[4], 250 * SCALE)] {
            if position(&self.native_user(w.users[1]), bank).1 == F::ZERO
                && !self.native_borrow(w.group, w.users[1], bank, amt) { return false; }
        }
        for bank in [w.banks[0], w.banks[1], w.banks[4]] { self.native_accrue(w.group, bank); }
        if position(&self.native_user(w.rebalance_user), w.banks[0]).0 < F::from_num(SCALE)
            && !self.native_fund(w.group, w.rebalance_user, w.banks[0], REBALANCE_ASSETS) { return false; }
        if variant == 0 && position(&self.native_user(w.rebalance_user), w.banks[4]).0 < F::from_num(SCALE)
            && !self.native_fund(w.group, w.rebalance_user, w.banks[4], 4 * SCALE) { return false; }

        // Variant 3 setup: a classic order tagging the rebalance user's banks[0] asset and
        // banks[3] liability. Scoped to this action: closed and untagged before returning.
        let mut rb_order: Option<Pubkey> = None;
        if variant == 3 {
            if position(&self.native_user(w.rebalance_user), w.banks[3]).1 == F::ZERO
                && !self.native_borrow(w.group, w.rebalance_user, w.banks[3], SCALE) { return false; }
            let mut keys = vec![w.banks[0], w.banks[3]]; keys.sort_unstable();
            let hash = solana_program::hash::hashv(&keys.iter().map(|key| key.as_ref()).collect::<Vec<_>>()).to_bytes();
            let key = self.native_pda(b"order", &[w.rebalance_user.as_ref(), &hash]);
            if !self.native_exists(key) {
                let ix = self.native_ix(instruction::MarginfiAccountPlaceOrder {
                    bank_keys: keys, trigger: arg::OrderTrigger::StopLoss { threshold: wrapped(F::lit("0.000001")), max_slippage: 0 },
                }, accounts::MarginfiAccountPlaceOrder {
                    group: w.group, marginfi_account: w.rebalance_user, fee_payer: self.payer.pubkey(), authority: self.payer.pubkey(),
                    order: key, fee_state: self.native_pda(FEE_STATE_SEED, &[]), global_fee_wallet: self.global_fee_wallet,
                });
                let ok = self.native_send(vec![ix], self.payer.clone());
                scout_check!("NATIVE-REBALANCE", "tagged-fixture-order-placement", ok);
                if !ok { return false; }
            }
            rb_order = Some(key);
        }

        let pre = self.native_user(w.rebalance_user);
        let scale = F::from_num(SCALE);
        let pre0 = self.native_underlying(&pre, w.banks[0]);
        let pre1 = self.native_underlying(&pre, w.banks[1]);
        let pre4 = self.native_underlying(&pre, w.banks[4]);
        let cap = if policy.amount == 0 { u64::MAX } else { policy.amount };
        let budget0 = pre0.floor().to_num::<u64>().min(cap);
        let a = 1 + amount as u64 % budget0.max(1);
        let budget4 = pre4.floor().to_num::<u64>().min(cap.saturating_sub(a));
        let b = 1 + (amount as u64 / 97) % budget4.max(1);
        let tagged_src_tag = position(&pre, w.banks[0]).2;
        let has_b1_balance = pre.lending_account.balances.iter().any(|bal| bal.active != 0 && bal.bank_pk == w.banks[1]);
        let (moves, legs_withdraw, legs_deposit): (Vec<arg::RebalanceMove>, Vec<(Pubkey, u64, bool)>, Vec<(Pubkey, u64)>) = match variant {
            0 => (vec![
                    Self::native_move(0, 1, a, scale), Self::native_move(2, 1, b, scale)],
                vec![(w.banks[0], a, F::from_num(a) == pre0), (w.banks[4], b, F::from_num(b) == pre4)],
                vec![(w.banks[1], a + b)]),
            1 => (vec![Self::native_move(0, 1, a / 2, scale), Self::native_move(0, 2, a - a / 2, scale)],
                vec![(w.banks[0], a, F::from_num(a) == pre0)],
                vec![(w.banks[1], a / 2), (w.banks[4], a - a / 2)]),
            2 => (vec![Self::native_move(0, 1, a / 2, scale), Self::native_move(0, 1, a - a / 2, scale)],
                vec![(w.banks[0], a, F::from_num(a) == pre0)],
                vec![(w.banks[1], a)]),
            _ => {
                // Tagged src: the declared amount is only used for rate math; the legs must move
                // the whole balance (withdraw_all) into a bank the account holds nothing in.
                let whole = pre0.floor().to_num::<u64>();
                (vec![Self::native_move(0, 1, whole, scale)],
                    vec![(w.banks[0], u64::MAX, true)],
                    vec![(w.banks[1], whole)])
            }
        };
        let venue_moves: Vec<(usize, usize, F)> = moves.iter()
            .map(|m| (m.src_index as usize, m.dst_index as usize, m.amount.to_i80f48())).collect();
        // end_rebalance health-checks the post-move balance set: model it at share granularity.
        let health_deltas: Vec<(Pubkey, F)> = legs_withdraw.iter().map(|&(bank, amt, all)| {
            let asv = self.native_bank(bank).asset_share_value.to_i80f48();
            let cur = position(&pre, bank).0;
            let burned = if all { cur } else { F::from_num(amt).min(cur * asv) / asv };
            (bank, -burned)
        }).chain(legs_deposit.iter().map(|&(bank, amt)| {
            (bank, F::from_num(amt) / self.native_bank(bank).asset_share_value.to_i80f48())
        })).collect();
        let post_healthy = self.native_health_with_asset_deltas(&pre, &health_deltas) >= F::ZERO;
        let sequence = pre.rebalance_execution_seq;
        let record = self.native_pda(b"rebalance_record", &[w.rebalance_user.as_ref(), &sequence.to_le_bytes()]);
        let mut start = self.native_ix(instruction::MarginfiAccountStartRebalance {
            moves, execution_seq: sequence,
        }, accounts::MarginfiAccountStartRebalance {
            group: w.group, marginfi_account: w.rebalance_user, rebalance_order: order, executor: w.keeper.pubkey(),
            rebalance_record: record, fee_payer: w.keeper.pubkey(),
        });
        start.accounts.extend([AccountMeta::new(w.banks[0], false), AccountMeta::new(w.banks[1], false), AccountMeta::new(w.banks[4], false)]);
        let mut end = self.native_ix(instruction::MarginfiAccountEndRebalance {}, accounts::MarginfiAccountEndRebalance {
            group: w.group, marginfi_account: w.rebalance_user, rebalance_order: order, rebalance_record: record,
            executor: w.keeper.pubkey(), fee_pool: self.native_fee_pool(),
        });
        end.accounts.extend([AccountMeta::new(w.banks[0], false), AccountMeta::new(w.banks[1], false), AccountMeta::new(w.banks[4], false)]);
        let emptied: Vec<Pubkey> = legs_withdraw.iter().filter(|l| l.2).map(|l| l.0).collect();
        // The end ix health parse is positional: remaining accounts must equal the account's
        // post-tx active banks exactly. Only variant 1 creates a banks[4] balance mid-tx.
        let observe_include: &[Pubkey] = if variant == 1 { &[w.banks[1], w.banks[4]] } else { &[w.banks[1]] };
        end.accounts.extend(self.native_observe(w.rebalance_user, observe_include, &emptied));
        let mut ixs = vec![start];
        for (bank, amt, all) in legs_withdraw { ixs.push(self.native_withdraw_ix(w.rebalance_user, bank, amt, all)); }
        for (bank, amt) in legs_deposit { ixs.push(self.native_deposit_ix(w.group, w.rebalance_user, bank, amt, true)); }
        ixs.push(end);

        let healthy = self.native_health(&pre) >= F::ZERO;
        let cooldown_ok = self.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64
            >= policy.last_exec_timestamp + policy.cooldown_seconds;
        let within_budget = cap == u64::MAX || match variant { 0 => a + b, 2 => a, 3 => pre0.floor().to_num::<u64>(), _ => a } <= cap + 3;
        // Rate gates: per-move improvement and the best-venue rule, modeled bit-exactly.
        let venue_ok = Self::native_venue_accepts(
            &[self.native_bank(w.banks[0]), self.native_bank(w.banks[1]), self.native_bank(w.banks[4])],
            &[w.banks[0], w.banks[1], w.banks[4]].map(|key| pre.lending_account.balances.iter().any(|b| b.active != 0 && b.bank_pk == key)),
            &[position(&pre, w.banks[0]).2, position(&pre, w.banks[1]).2, position(&pre, w.banks[4]).2],
            &venue_moves, policy.min_improvement.to_i80f48());
        // Every move must carry a positive amount (rebalance.rs initialize): the double-tap
        // variant's first leg is zero when a == 1, and the split variant's can be too.
        let expected = cooldown_ok && healthy && within_budget && venue_ok && post_healthy
            && match variant { 1 | 2 => a >= 2, 3 => !has_b1_balance, _ => true };
        let before = self.native_snapshot(&[record]);
        let pool_before = self.native_lamports(self.native_fee_pool());
        let keeper_before = self.native_tokens(w.keeper_tokens);
        let ok = self.native_send(ixs, w.keeper.clone());
        scout_check!("NATIVE-REBALANCE", "multi-execution-result-matches-model", ok == expected,
            "native multi rebalance variant={variant}, accepted={ok}, expected={expected}");
        if !ok {
            self.native_check_rollback(before, &[record]);
            if let Some(key) = rb_order { self.native_scrub_rb_order(&w, key); }
            return true;
        }
        let post = self.native_user(w.rebalance_user);
        let d0 = pre0 - self.native_underlying(&post, w.banks[0]);
        let d1 = self.native_underlying(&post, w.banks[1]) - pre1;
        let d4 = pre4 - self.native_underlying(&post, w.banks[4]);
        // 3 atoms of dust per move touching the bank, matching REBALANCE_CONSERVATION_DUST_ATOMS.
        let (e0, e1, e4) = match variant {
            0 => (F::from_num(a), F::from_num(a + b), F::from_num(b)),
            1 => (F::from_num(a), F::from_num(a / 2), -F::from_num(a - a / 2)),
            2 => (F::from_num(a), F::from_num(a), F::ZERO),
            _ => (pre0, pre0, F::ZERO),
        };
        let slack = F::from_num(6);
        scout_check!("NATIVE-REBALANCE", "multi-principal-and-declared-flows", (d0 - e0).abs() <= slack
            && (d1 - e1).abs() <= slack && (d4 - e4).abs() <= slack,
            "native multi flows: d0={d0} e0={e0}, d1={d1} e1={e1}, d4={d4} e4={e4}");
        scout_check!("NATIVE-REBALANCE", "keeper-cannot-retain-principal", self.native_tokens(w.keeper_tokens) == keeper_before);
        scout_check!("NATIVE-REBALANCE", "multi-bystanders-and-sequence", position(&pre, w.banks[2]) == position(&post, w.banks[2])
            && position(&pre, w.banks[3]).1 == position(&post, w.banks[3]).1 && post.rebalance_execution_seq == sequence + 1);
        scout_check!("NATIVE-HEALTH", "rebalance-maintenance-health", self.native_health(&post) >= -TOLERANCE);
        if variant == 3 {
            // The whole tagged pile moved and the tag followed it to the destination balance.
            let moved_tag = position(&post, w.banks[1]).2;
            scout_check!("NATIVE-REBALANCE", "tagged-move-carries-tag-and-moves-whole", position(&post, w.banks[0]).0 == F::ZERO
                && tagged_src_tag != 0 && moved_tag == tagged_src_tag);
            if let Some(key) = rb_order {
                // The order's tagged asset changed banks but still identifies one asset + one liability.
                let intent: chain::Order = self.native_load(key, chain::Order::DISCRIMINATOR);
                let (mut assets, mut liabs) = (0u32, 0u32);
                for bal in post.lending_account.balances.iter().filter(|bal| bal.active != 0 && intent.tags.contains(&bal.tag)) {
                    if bal.asset_shares.to_i80f48() >= F::ONE { assets += 1; }
                    if bal.liability_shares.to_i80f48() >= F::ONE { liabs += 1; }
                }
                scout_check!("NATIVE-ORDER", "tagged-move-preserves-order-composition", assets == 1 && liabs == 1);
            }
        }
        // Program-exact tip math (reconcile + end_rebalance): total_moved sums the POSITIVE
        // per-bank deltas (destination gains) in whole-token UI units, converted per bank; the
        // denominator sums every referenced bank's start amount the same way. Source drains
        // differ from destination gains by deposit rounding dust, which flips floor() when the
        // fraction sits at the cap (whole-position tagged moves hit this).
        let moved_ui = [w.banks[0], w.banks[1], w.banks[4]].into_iter()
            .map(|bank| (self.native_underlying(&post, bank) / scale - self.native_underlying(&pre, bank) / scale).max(F::ZERO))
            .fold(F::ZERO, |acc, delta| acc + delta);
        let pre_total_ui = [w.banks[0], w.banks[1], w.banks[4]].into_iter()
            .map(|bank| self.native_underlying(&pre, bank) / scale)
            .fold(F::ZERO, |acc, amount| acc + amount);
        let target = if policy.amount == 0 { pre_total_ui } else { (F::from_num(policy.amount) / scale).min(pre_total_ui) };
        let rent = self.ctx.svm.minimum_balance_for_rent_exemption(0);
        let expected_tip = (F::from_num(policy.keeper_tip) * (moved_ui / target).min(F::ONE)).floor().to_num::<u64>()
            .min(pool_before.saturating_sub(rent));
        scout_check!("NATIVE-REBALANCE", "multi-tip-proportional-and-rent-reserved", pool_before - self.native_lamports(self.native_fee_pool()) == expected_tip,
            "native multi tip: expected={expected_tip}, actual={}", pool_before - self.native_lamports(self.native_fee_pool()));
        if expected_tip > 0 {
            let actual: chain::RebalanceRecord = self.native_load(record, chain::RebalanceRecord::DISCRIMINATOR);
            scout_check!("NATIVE-REBALANCE", "multi-tip-escrow-bound-to-keeper", actual.pending_tip == expected_tip && actual.executor == w.keeper.pubkey());
            self.native_orders.as_mut().unwrap().records.push(record);
        } else { scout_check!("NATIVE-REBALANCE", "multi-untipped-record-closed", !self.native_exists(record)); }
        self.native_orders.as_mut().unwrap().completed_rebalances += 1;
        if let Some(key) = rb_order { self.native_scrub_rb_order(&w, key); }
        true
    }

    fn native_move(src: u8, dst: u8, atoms: u64, scale: F) -> arg::RebalanceMove {
        arg::RebalanceMove { src_index: src, dst_index: dst, _pad0: [0; 6], amount: wrapped(F::from_num(atoms) / scale) }
    }

    /// Exact replica of the type-crate's u32->I80F48 conversions (interest_rate.rs):
    /// centi is `util / u32::MAX` computed on raw bits; milli scales it to the 0-1000% APR range.
    fn native_u32_to_centi(util: u32) -> F {
        F::from_bits((i128::from(util) << F::FRAC_NBITS) / i128::from(u32::MAX))
    }

    fn native_u32_to_milli(rate: u32) -> F {
        F::from_bits(Self::native_u32_to_centi(rate).to_bits() * 10)
    }

    /// interest_rate.rs lerp: linear interpolation with the same clamping and rounding.
    fn native_lerp(x0: F, y0: F, x1: F, y1: F, x: F) -> F {
        if x1 <= x0 || x <= x0 { return y0; }
        if x >= x1 { return y1; }
        assert!(y1 >= y0, "curve rates must be non-decreasing");
        y0 + (y1 - y0) * ((x - x0) / (x1 - x0))
    }

    /// NativeRateModel::rate_at (state/rate.rs) replicated over the generated account view:
    /// the supply APR a native bank pays after `extra_native` more atoms are deposited.
    fn native_rate_at(bank: &chain::Bank, extra_native: u64) -> F {
        let cfg = &bank.config.interest_rate_config;
        assert_eq!(cfg.curve_type, 1, "INTEREST_CURVE_SEVEN_POINT");
        let assets = bank.total_asset_shares.to_i80f48() * bank.asset_share_value.to_i80f48()
            + F::from_num(extra_native);
        let liab = bank.total_liability_shares.to_i80f48() * bank.liability_share_value.to_i80f48();
        if assets == F::ZERO || liab == F::ZERO { return F::ZERO; }
        let u = liab / assets;
        let (zero, hundred) = (Self::native_u32_to_milli(cfg.zero_util_rate), Self::native_u32_to_milli(cfg.hundred_util_rate));
        let (mut prev_util, mut prev_rate) = (F::ZERO, zero);
        for point in cfg.points.iter().filter(|p| p.util != 0) {
            let (pu, pr) = (Self::native_u32_to_centi(point.util), Self::native_u32_to_milli(point.rate));
            if u <= pu { return Self::native_lerp(prev_util, prev_rate, pu, pr, u) * u; }
            prev_util = pu; prev_rate = pr;
        }
        Self::native_lerp(prev_util, prev_rate, F::ONE, hundred, u) * u
    }

    /// Bit-exact model of start_rebalance's rate gates (rebalance.rs:935-999): each move's
    /// destination must beat its source's current rate by min_improvement, and no allowlisted
    /// alternative may pay more than the destination after absorbing the move's tokens
    /// (RebalanceNotBestVenue). All native-world banks carry the harness's flat curve and no
    /// active deposit limit, so capacity never binds. `moves` carries (src_index, dst_index,
    /// amount) with amount the exact RebalanceMove UI value; dust legs that truncate to zero
    /// native atoms price no alternative.
    fn native_venue_accepts(banks: &[chain::Bank], has_balance: &[bool], tags: &[u16], moves: &[(usize, usize, F)], min_imp: F) -> bool {
        let n = banks.len();
        let exp10 = F::from_num(10u64.pow(banks[0].mint_decimals as u32));
        let to_native = |ui: F| -> u64 { (ui * exp10).to_num::<u64>() };
        for bank in banks {
            assert!(bank.config.deposit_limit == u64::MAX, "venue model assumes no active deposit limit");
        }
        let rates: Vec<F> = banks.iter().map(|bank| Self::native_rate_at(bank, 0)).collect();
        let mut inflow = vec![F::ZERO; n];
        for &(s, d, amount) in moves {
            if moves.iter().any(|&(_, od, _)| od == s) { return false; } // RebalanceBankSourceAndDestination
            inflow[d] += amount;
        }
        let inflow_native: Vec<u64> = inflow.iter().map(|&ui| to_native(ui)).collect();
        let landed: Vec<F> = (0..n).map(|i| if inflow_native[i] == 0 { rates[i] } else { Self::native_rate_at(&banks[i], inflow_native[i]) }).collect();
        let mut tagged_dst = vec![false; n];
        for &(s, d, _) in moves {
            if tags[s] != 0 { tagged_dst[d] = true; }
        }
        for &(s, d, amount) in moves {
            if landed[d] <= rates[s] + min_imp { return false; } // RebalanceNotImproving
            let tagged = tags[s] != 0;
            // Tagged moves price every alternative against the destination's whole inflow;
            // capacity is unbounded, so the tagged branch's `amount > capacity` skip never fires.
            let amount_native = if tagged { inflow_native[d] } else { to_native(amount) };
            for i in 0..n {
                // capacity == F::MAX: the `inflow >= capacity` skip never fires.
                if i == d || landed[i] <= landed[d] || tags[i] != 0 || tagged_dst[i] { continue; }
                if tagged && (inflow[i] > F::ZERO || has_balance[i]) { continue; }
                let extra_native = inflow_native[i] + amount_native;
                if extra_native == inflow_native[i] { continue; }
                if Self::native_rate_at(&banks[i], extra_native) > landed[d] { return false; }
            }
        }
        true
    }

    /// Tear down the tagged variant's second order: authority-close it, then clear every balance
    /// tag the placement and any tag-carrying move left on the rebalance user.
    fn native_scrub_rb_order(&mut self, w: &NativeWorld, order: Pubkey) {
        let close = self.native_ix(instruction::MarginfiAccountCloseOrder {}, accounts::MarginfiAccountCloseOrder {
            group: w.group, marginfi_account: w.rebalance_user, authority: self.payer.pubkey(), order,
            fee_recipient: self.payer.pubkey(),
        });
        let ok = self.native_send(vec![close], self.payer.clone());
        scout_check!("NATIVE-REBALANCE", "tagged-fixture-order-close", ok);
        let clear = self.native_ix(instruction::MarginfiAccountSetKeeperCloseFlags { bank_keys_opt: None },
            accounts::MarginfiAccountSetKeeperCloseFlags { group: w.group, marginfi_account: w.rebalance_user, authority: self.payer.pubkey() });
        let ok = self.native_send(vec![clear], self.payer.clone());
        scout_check!("NATIVE-REBALANCE", "tagged-fixture-flags-cleared", ok);
        if ok {
            let user = self.native_user(w.rebalance_user);
            scout_check!("NATIVE-REBALANCE", "tagged-fixture-leaves-no-tags",
                user.lending_account.balances.iter().all(|b| b.tag == 0));
        }
    }

    fn native_underlying(&self, user: &chain::MarginfiAccount, bank: Pubkey) -> F {
        position(user, bank).0 * self.native_bank(bank).asset_share_value.to_i80f48()
    }

    fn native_price(&self, bank: Pubkey) -> F {
        self.native_bank(bank).config.fixed_price.to_i80f48()
    }

    /// USD net equity of the order pair at the Equity requirement (weight 1: every native-world
    /// bank is Collateral tier). banks[0]/banks[3] never accrue interest, but read the share
    /// values anyway so the model stays honest if that changes. Each side is priced exactly as
    /// the program's calc_value does -- (shares x share_value) x price, divided by 10^decimals,
    /// truncating PER SIDE -- because placement pins the trigger threshold to this value and
    /// start_execute_order's `net <= sl` gate sits exactly on the boundary, where a one-ulp
    /// difference in truncation order flips the trigger (crash_e2d46821925f64d2).
    fn native_pair_net(&self, user: &chain::MarginfiAccount, asset_bank: Pubkey, liab_bank: Pubkey) -> F {
        let scale = F::from_num(SCALE);
        let (a, d) = (position(user, asset_bank).0, position(user, liab_bank).1);
        let asv = self.native_bank(asset_bank).asset_share_value.to_i80f48();
        let lsv = self.native_bank(liab_bank).liability_share_value.to_i80f48();
        (a * asv * self.native_price(asset_bank)) / scale - (d * lsv * self.native_price(liab_bank)) / scale
    }

    fn native_set_price(&mut self, w: &NativeWorld, bank: Pubkey, new: F) -> bool {
        let ix = self.native_ix(instruction::LendingPoolSetOraclePrice { price: wrapped(new), setup: SCOUT_ORACLE_SETUP_FIXED },
            accounts::LendingPoolSetOraclePrice { group: w.group, governance_admin: self.payer.pubkey(), bank });
        let before = self.native_snapshot(&[]);
        let ok = self.native_send(vec![ix], self.payer.clone());
        scout_check!("NATIVE-ORDER", "fixed-price-admin-write-applies", ok);
        if !ok { self.native_check_rollback(before, &[]); return false; }
        scout_check!("NATIVE-ORDER", "fixed-price-matches-admin-write", self.native_price(bank) == new,
            "native price move: bank {bank} expected {new}, found {}", self.native_price(bank));
        true
    }

    /// Move a bank's Fixed oracle price by +/-[0, 95]%. This is what actually fires order
    /// triggers honestly (thresholds were anchored to earlier prices) and what pushes accounts
    /// across liquidation health boundaries. The harness banks never enable the circuit breaker,
    /// so no move trips the CB price gate in this world.
    pub(super) fn native_move_price(&mut self, bank_index: u8, pct: u8, down: bool) -> bool {
        self.native_ensure_world();
        let w = self.native_orders.as_ref().unwrap().clone();
        let bank = w.banks[bank_index as usize % w.banks.len()];
        let old = self.native_price(bank);
        let pct = F::from_num(pct % 96);
        let new = (if down { old * (F::from_num(100) - pct) } else { old * (F::from_num(100) + pct) } / F::from_num(100))
            .max(F::lit("0.000001"));
        self.native_set_price(&w, bank, new)
    }

    fn native_accrue(&mut self, group: Pubkey, bank: Pubkey) {
        let ix = self.native_ix(instruction::LendingPoolAccrueBankInterest {}, accounts::LendingPoolAccrueBankInterest { group, bank });
        assert!(self.native_send(vec![ix], self.payer.clone()), "native accrue");
    }

    pub(super) fn native_settle_rebalance(&mut self, advance: bool, refund: bool) -> bool {
        self.native_ensure_world();
        let w = self.native_orders.as_ref().unwrap().clone();
        let Some(&record) = w.records.first() else { return true; };
        let captured: chain::RebalanceRecord = self.native_load(record, chain::RebalanceRecord::DISCRIMINATOR);
        if refund { self.native_clear_utilization(); }
        if advance {
            let now = self.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64;
            self.native_advance((captured.move_timestamp + captured.settle_delay).saturating_sub(now));
        }
        let mut ix = self.native_ix(instruction::MarginfiAccountSettleRebalanceTip {}, accounts::MarginfiAccountSettleRebalanceTip {
            group: w.group, marginfi_account: w.rebalance_user, rebalance_record: record,
            executor: w.keeper.pubkey(), fee_pool: self.native_fee_pool(), caller: self.payer.pubkey(),
        });
        // The referenced banks must be replayed in the recorded order (require_keys_eq per slot).
        let ref_keys: Vec<Pubkey> = captured.ref_banks[..captured.ref_bank_count as usize].iter().map(|r| r.bank).collect();
        ix.accounts.extend(ref_keys.iter().map(|key| AccountMeta::new(*key, false)));
        let before = self.native_snapshot(&[]);
        let pool_before = self.native_lamports(self.native_fee_pool());
        let keeper_before = self.native_lamports(w.keeper.pubkey());
        let record_lamports = self.native_lamports(record);
        let now = self.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64;
        let expected = now >= captured.move_timestamp + captured.settle_delay;
        let ok = self.native_send(vec![ix], self.payer.clone());
        scout_check!("NATIVE-REBALANCE", "settlement-delay", ok == expected);
        if !ok { self.native_check_rollback(before, &[]); return true; }
        // Realized iff EVERY move's destination yield index out-grew its source's over the window
        // (cross-multiplied, mirroring settle_rebalance_tip). Native venues: index = asset_share_value.
        let cur: Vec<F> = ref_keys.iter().map(|key| self.native_bank(*key).asset_share_value.to_i80f48()).collect();
        let move_idx: Vec<F> = captured.move_yield_index[..captured.ref_bank_count as usize].iter().map(|y| y.to_i80f48()).collect();
        let mut realized = true;
        for m in captured.moves[..captured.move_count as usize].iter() {
            let (s, d) = (m.src_index as usize, m.dst_index as usize);
            if cur[d] * move_idx[s] <= cur[s] * move_idx[d] { realized = false; break; }
        }
        let rent = self.ctx.svm.minimum_balance_for_rent_exemption(0);
        let refund = !realized && pool_before >= rent;
        scout_check!("NATIVE-REBALANCE", "settlement-pays-recorded-keeper-or-refunds-pool",
            self.native_lamports(w.keeper.pubkey()) - keeper_before == record_lamports - if refund { captured.pending_tip } else { 0 }
            && self.native_lamports(self.native_fee_pool()) - pool_before == if refund { captured.pending_tip } else { 0 }
            && !self.native_exists(record));
        self.native_orders.as_mut().unwrap().records.remove(0);
        self.native_orders.as_mut().unwrap().completed_settlements += 1;
        true
    }

    fn native_clear_utilization(&mut self) {
        let w = self.native_orders.as_ref().unwrap().clone();
        let user = w.users[1];
        let bank = w.banks[1];
        let ix = self.native_ix(instruction::LendingAccountRepay { amount: u64::MAX, repay_all: Some(true) }, accounts::LendingAccountRepay {
            group: w.group, marginfi_account: user, authority: self.payer.pubkey(), bank,
            signer_token_account: self.signer_token_account, liquidity_vault: self.native_pda(LIQUIDITY_VAULT_SEED, &[bank.as_ref()]), token_program: spl_token::id(),
        });
        if position(&self.native_user(user), bank).1 > F::ZERO {
            assert!(self.native_send(vec![ix], self.payer.clone()), "native utilization repay");
        }
    }

    pub(super) fn native_cancel_rebalance(&mut self, unauthorized: bool) -> bool {
        self.native_ensure_world();
        let w = self.native_orders.as_ref().unwrap().clone();
        let Some(order) = w.rebalance_order else { return true; };
        let signer = if unauthorized { w.keeper.clone() } else { self.payer.clone() };
        let ix = self.native_ix(instruction::MarginfiAccountCloseRebalanceOrder {}, accounts::MarginfiAccountCloseRebalanceOrder {
            marginfi_account: w.rebalance_user, authority: Some(signer.pubkey()), fee_recipient: signer.pubkey(), rebalance_order: order,
        });
        let before = self.native_snapshot(&[]);
        let ok = self.native_send(vec![ix], signer);
        scout_check!("NATIVE-REBALANCE", "cancel-authority", ok != unauthorized);
        if ok { self.native_orders.as_mut().unwrap().rebalance_order = None; } else { self.native_check_rollback(before, &[]); }
        true
    }

    pub(super) fn native_rebalance_fee_pool(&mut self, withdraw: bool, amount: u32) -> bool {
        self.native_ensure_world();
        let w = self.native_orders.as_ref().unwrap().clone();
        let pool = self.native_fee_pool();
        let before = self.native_lamports(pool);
        let recipient_before = self.native_lamports(w.keeper.pubkey());
        let rent = self.ctx.svm.minimum_balance_for_rent_exemption(0);
        let amount = amount as u64 % 20_000_001;
        let paid = if before.saturating_sub(amount.min(before)) < rent { before } else { amount.min(before) };
        let expected = if withdraw { before - paid } else { before + amount + rent.saturating_sub(before) };
        let ix = if withdraw {
            self.native_ix(instruction::MarginfiAccountWithdrawRebalanceFeePool { amount }, accounts::MarginfiAccountWithdrawRebalanceFeePool {
                marginfi_account: w.rebalance_user, authority: self.payer.pubkey(), fee_pool: pool,
                destination: w.keeper.pubkey(),
            })
        } else {
            self.native_ix(instruction::MarginfiAccountTopUpRebalanceFeePool { amount }, accounts::MarginfiAccountTopUpRebalanceFeePool {
                marginfi_account: w.rebalance_user, fee_pool: pool, payer: self.payer.pubkey(),
            })
        };
        let snapshot = self.native_snapshot(&[]);
        let ok = self.native_send(vec![ix], self.payer.clone());
        scout_check!("NATIVE-REBALANCE", "authorized-fee-pool-operation", ok);
        if !ok { self.native_check_rollback(snapshot, &[]); return true; }
        let after = self.native_lamports(pool);
        scout_check!("NATIVE-REBALANCE", "fee-pool-conserves-lamports-and-reserves-rent", after == expected
            && self.native_lamports(w.keeper.pubkey()) - recipient_before == if withdraw { paid } else { 0 });
        true
    }

    pub(super) fn native_check_invariants(&mut self) {
        let Some(w) = self.native_orders.as_ref().cloned() else { return; };
        for bank_key in w.banks {
            let bank = self.native_bank(bank_key);
            let mut assets = F::ZERO;
            let mut liabilities = F::ZERO;
            for user in &w.users {
                let p = position(&self.native_user(*user), bank_key);
                assets += p.0; liabilities += p.1;
            }
            // No custom message: the default names the property and site, which the mutation
            // self-tests match on.
            scout_check!("NATIVE-ACCOUNTING", "bank-shares-equal-all-live-positions", (assets - bank.total_asset_shares.to_i80f48()).abs() <= TOLERANCE
                && (liabilities - bank.total_liability_shares.to_i80f48()).abs() <= TOLERANCE);
            let claims = bank.total_asset_shares.to_i80f48() * bank.asset_share_value.to_i80f48();
            let loans = bank.total_liability_shares.to_i80f48() * bank.liability_share_value.to_i80f48();
            let fees = bank.collected_insurance_fees_outstanding.to_i80f48() + bank.collected_group_fees_outstanding.to_i80f48()
                + bank.collected_program_fees_outstanding.to_i80f48() + bank.collected_premium_outstanding.to_i80f48();
            scout_check!("NATIVE-ACCOUNTING", "vault-plus-loans-cover-claims-and-fees", F::from_num(self.native_tokens(bank.liquidity_vault)) + loans + F::from_num(4) >= claims + fees);
        }
        for (user, expected_orders) in [(w.order_user, u8::from(w.order.is_some())), (w.rebalance_user, u8::from(w.rebalance_order.is_some()))] {
            let account = self.native_user(user);
            scout_check!("NATIVE-ORDER", "active-order-count-matches-model", account.active_orders == expected_orders);
            scout_check!("NATIVE-ORDER", "execution-flags-never-escape-transaction", account.account_flags & EXECUTION_FLAGS == 0);
        }
        if let Some(order) = w.order {
            let actual: chain::Order = self.native_load(order, chain::Order::DISCRIMINATOR);
            let user = self.native_user(w.order_user);
            let tags = [position(&user, w.banks[0]).2, position(&user, w.banks[3]).2];
            // A live order is either composition-coherent (tags identify exactly one asset and
            // one liability) or stale/keeper-closeable (e.g. after set_keeper_close_flags or a
            // liquidation that closed a tagged balance). Only the coherent state asserts identity.
            if self.native_order_composition_ok(&w, order) {
                scout_check!("NATIVE-ORDER", "live-order-tags-identify-both-positions", actual.marginfi_account == w.order_user
                    && tags[0] != 0 && tags[1] != 0 && tags[0] != tags[1]
                    && actual.tags.iter().all(|tag| tags.contains(tag)));
            }
        }
        if let Some(order) = w.rebalance_order {
            let actual: chain::RebalanceOrder = self.native_load(order, chain::RebalanceOrder::DISCRIMINATOR);
            scout_check!("NATIVE-REBALANCE", "persistent-order-identity", actual.marginfi_account == w.rebalance_user && actual.mint == self.bank_mint);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> MarginfiFixture {
        assert!(!__scout_check_observe_mode(), "regressions require enforcing checks");
        for property in ["NATIVE-ACCOUNTING", "NATIVE-ORDER", "NATIVE-REBALANCE", "NATIVE-ROLLBACK", "NATIVE-HEALTH"] {
            assert!(__scout_check_selected(property), "regressions require unmuted {property}");
        }
        clear_iteration_state();
        let mut f = MarginfiFixture::setup_base();
        assert!(f.action_init_global_fee_state(0, 0));
        clear_iteration_state();
        f
    }

    fn check(f: &mut MarginfiFixture) {
        f.native_check_invariants();
        let violation = take_violation();
        assert!(violation.is_none(), "native invariant violation: {violation:?}");
    }

    #[test]
    fn native_order_trigger_and_execution_matrix() {
        let template = fixture();
        for trigger in 0..4 {
            for variant in 0..5 {
                clear_iteration_state();
                let mut f = template.clone();
                assert!(f.native_place_order(trigger, 500));
                assert!(f.native_execute_order(variant, 99));
                check(&mut f);
                assert_eq!(f.native_orders.as_ref().unwrap().completed_orders, u64::from(trigger != 3 && variant == 0));
            }
        }
    }

    #[test]
    fn native_order_cancel_and_reopen() {
        let mut f = fixture();
        for _ in 0..3 {
            assert!(f.native_place_order(1, 0));
            assert!(f.native_cancel_order(true)); check(&mut f);
            assert!(f.native_cancel_order(false)); check(&mut f);
            assert!(f.native_place_order(0, 1000));
            assert!(f.native_execute_order(0, 99)); check(&mut f);
        }
    }

    #[test]
    fn native_rebalance_execution_matrix() {
        let template = fixture();
        for variant in 0..6 {
            clear_iteration_state();
            let mut f = template.clone();
            assert!(f.native_place_rebalance(0, 200_000));
            assert!(f.native_execute_rebalance(variant, 2 * SCALE as u32));
            check(&mut f);
            assert_eq!(f.native_orders.as_ref().unwrap().completed_rebalances, u64::from(variant == 0));
        }
    }

    #[test]
    fn native_rebalance_tip_delay_and_permissionless_settlement() {
        let mut f = fixture();
        assert!(f.native_execute_rebalance(0, 10 * SCALE as u32)); check(&mut f);
        assert!(!f.native_orders.as_ref().unwrap().records.is_empty());
        assert!(f.native_settle_rebalance(false, false)); check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_settlements, 0);
        assert!(f.native_settle_rebalance(true, false)); check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_settlements, 1);
    }

    #[test]
    fn native_rebalance_update_cancel_and_fee_pool() {
        let mut f = fixture();
        assert!(f.native_place_rebalance(0, 0));
        assert!(f.native_update_rebalance(5 * SCALE as u32, 100_000));
        assert!(f.native_execute_rebalance(0, SCALE as u32)); check(&mut f);
        assert!(f.native_cancel_rebalance(true)); check(&mut f);
        assert!(f.native_cancel_rebalance(false)); check(&mut f);
        assert!(f.native_settle_rebalance(true, false)); check(&mut f);
        assert!(f.native_rebalance_fee_pool(true, 20_000_000)); check(&mut f);
        assert!(f.native_rebalance_fee_pool(false, 2_000_000)); check(&mut f);
    }

    #[test]
    fn native_rebalance_refunds_when_no_yield_is_realized() {
        let mut f = fixture();
        assert!(f.native_execute_rebalance(0, 10 * SCALE as u32)); check(&mut f);
        let pool_before = f.native_lamports(f.native_fee_pool());
        assert!(f.native_settle_rebalance(true, true)); check(&mut f);
        assert!(f.native_lamports(f.native_fee_pool()) > pool_before);
    }

    #[test]
    fn native_tip_rounding_after_policy_and_pool_changes() {
        let mut f = fixture();
        assert!(f.native_rebalance_fee_pool(true, 65535));
        assert!(f.native_cancel_order(false));
        assert!(f.native_rebalance_fee_pool(false, 65535));
        assert!(f.native_update_rebalance(65536, 2));
        assert!(f.native_execute_rebalance(93, 2551585282)); check(&mut f);
        assert!(f.native_execute_rebalance(100, 65536)); check(&mut f);
    }

    #[test]
    fn native_invariants_detect_corrupted_accounting_flags_and_order_counts() {
        let mut f = fixture();
        f.native_ensure_world();
        let w = f.native_orders.as_ref().unwrap().clone();
        check(&mut f);
        let mut bank = f.native_bank(w.banks[0]);
        bank.total_asset_shares = wrapped(bank.total_asset_shares.to_i80f48() + F::ONE);
        f.ctx.update_account(&w.banks[0], |data| data[8..].copy_from_slice(bytemuck::bytes_of(&bank))).unwrap();
        f.native_check_invariants();
        assert!(take_violation().unwrap().contains("NATIVE-ACCOUNTING"));
        bank.total_asset_shares = wrapped(bank.total_asset_shares.to_i80f48() - F::ONE);
        f.ctx.update_account(&w.banks[0], |data| data[8..].copy_from_slice(bytemuck::bytes_of(&bank))).unwrap();
        let mut user = f.native_user(w.order_user);
        user.active_orders = 1;
        f.ctx.update_account(&w.order_user, |data| data[8..].copy_from_slice(bytemuck::bytes_of(&user))).unwrap();
        f.native_check_invariants();
        assert!(take_violation().unwrap().contains("active-order-count"));
        user.active_orders = 0;
        user.account_flags |= 1 << 7;
        f.ctx.update_account(&w.order_user, |data| data[8..].copy_from_slice(bytemuck::bytes_of(&user))).unwrap();
        f.native_check_invariants();
        assert!(take_violation().unwrap().contains("execution-flags"));
        user.account_flags &= !(1 << 7);
        f.ctx.update_account(&w.order_user, |data| data[8..].copy_from_slice(bytemuck::bytes_of(&user))).unwrap();

        // Vault solvency: drain one token from a liquidity vault (SPL amount at bytes 64..72).
        let vault = f.native_pda(LIQUIDITY_VAULT_SEED, &[w.banks[0].as_ref()]);
        let vault_bytes = f.ctx.read_account(&vault).unwrap().data;
        let amount = u64::from_le_bytes(vault_bytes[64..72].try_into().unwrap());
        f.ctx.update_account(&vault, |data| data[64..72].copy_from_slice(&(amount - SCALE).to_le_bytes())).unwrap();
        f.native_check_invariants();
        assert!(take_violation().unwrap().contains("vault-plus-loans-cover-claims-and-fees"));
        f.ctx.update_account(&vault, |data| data.copy_from_slice(&vault_bytes)).unwrap();

        // Order tag identity: a live order whose stored account pointer is wrong must trip the
        // identity check even while the composition still matches the balances.
        assert!(f.native_place_order(0, 500));
        let order = f.native_orders.as_ref().unwrap().order.unwrap();
        let mut placed: chain::Order = f.native_load(order, chain::Order::DISCRIMINATOR);
        let original_account = placed.marginfi_account;
        placed.marginfi_account = Pubkey::new_unique();
        f.ctx.update_account(&order, |data| data[8..].copy_from_slice(bytemuck::bytes_of(&placed))).unwrap();
        f.native_check_invariants();
        assert!(take_violation().unwrap().contains("live-order-tags-identify-both-positions"));
        placed.marginfi_account = original_account;
        f.ctx.update_account(&order, |data| data[8..].copy_from_slice(bytemuck::bytes_of(&placed))).unwrap();

        // Rebalance order identity: the stored mint must stay the group's native mint.
        assert!(f.native_place_rebalance(0, 200_000));
        let rb_order = f.native_orders.as_ref().unwrap().rebalance_order.unwrap();
        let mut policy: chain::RebalanceOrder = f.native_load(rb_order, chain::RebalanceOrder::DISCRIMINATOR);
        let original_mint = policy.mint;
        policy.mint = Pubkey::new_unique();
        f.ctx.update_account(&rb_order, |data| data[8..].copy_from_slice(bytemuck::bytes_of(&policy))).unwrap();
        f.native_check_invariants();
        assert!(take_violation().unwrap().contains("persistent-order-identity"));
        policy.mint = original_mint;
        f.ctx.update_account(&rb_order, |data| data[8..].copy_from_slice(bytemuck::bytes_of(&policy))).unwrap();

        check(&mut f);
    }

    #[test]
    fn native_rebalance_multi_move_matrix() {
        let template = fixture();
        for variant in 0..4 {
            clear_iteration_state();
            let mut f = template.clone();
            assert!(f.native_place_rebalance(0, 200_000));
            assert!(f.native_execute_rebalance_multi(variant, 6 * SCALE as u32));
            check(&mut f);
            // The split variant is rejected on the fresh fixture: banks[1] still beats banks[4]
            // after absorbing the move (RebalanceNotBestVenue). The others complete.
            assert_eq!(f.native_orders.as_ref().unwrap().completed_rebalances, u64::from(variant != 1));
        }
    }

    #[test]
    fn native_split_move_with_dust_legs_skips_venue_pricing() {
        // Campaign crash_5f6aa417a3004b3c (seed 42, iteration 164): a split whose legs truncate
        // to zero native atoms prices no alternative venue (rebalance.rs: a bank whose room for
        // the move rounds below one native unit is skipped), so the best-venue rule never fires
        // and the execution is accepted.
        let mut f = fixture();
        assert!(f.native_place_rebalance(0, 200_000));
        assert!(f.native_execute_rebalance_multi(1, 1)); // amount=1 -> a = 2 -> two 1-atom legs
        check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_rebalances, 1);
    }

    #[test]
    fn native_multi_move_rejects_zero_amount_leg() {
        // Campaign crash_16dd8c7e1d3fca68: the double-tap variant with a == 1 makes the first
        // leg a zero-amount move; the program rejects zero-amount moves (rebalance.rs
        // initialize) and the rejection must roll back cleanly.
        let mut f = fixture();
        assert!(f.native_place_rebalance(0, 200_000));
        assert!(f.native_execute_rebalance_multi(2, 0)); // amount=0 -> a = 1 -> first leg is 0
        check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_rebalances, 0);
    }

    #[test]
    fn native_order_executes_with_negative_equity_after_liquidation() {
        // Campaign crash_a14c183a7bb7a365 (seed 42, iteration 3752): an 80% price crash plus a
        // liquidation at the crashed price leaves the order pair's equity negative. The
        // stop-loss bound is then negative, the naive withdraw bound exceeds the position, and
        // the uncapped withdrawal is rejected for insufficient funds. The withdrawal must be
        // capped at the position; the equity bound stays trivially satisfiable.
        let mut f = fixture();
        assert!(f.native_place_order(0, 500));
        assert!(f.native_move_price(0, 80, true)); // banks[0]: 1.0 -> 0.2
        assert!(f.native_liquidate_order_user(0, 25)); // chains native_execute_order(0, 99)
        check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_orders, 1);
    }

    #[test]
    fn native_liquidation_composes_with_order_tags() {
        let template = fixture();
        for variant in 0..4 {
            clear_iteration_state();
            let mut f = template.clone();
            assert!(f.native_liquidate_order_user(variant, 25));
            check(&mut f);
            let w = f.native_orders.as_ref().unwrap();
            match variant {
                // Seizing the crashed tagged asset lowers net equity: the stop-loss fires and
                // the chained execution closes the order on post-liquidation state.
                0 => {
                    assert_eq!(w.completed_orders, 1);
                    assert!(w.order.is_none());
                }
                // Seizing the untagged bystander repays debt only: net equity rises, the
                // stop-loss no longer fires, and the chained execution must reject cleanly.
                1 => {
                    assert_eq!(w.completed_orders, 0);
                    assert!(w.order.is_some());
                }
                // Over-seize and healthy-account variants reject before the chained execution.
                _ => {
                    assert_eq!(w.completed_orders, 0);
                    assert!(w.order.is_some());
                }
            }
        }
    }

    #[test]
    fn native_liquidation_too_severe_rejected_at_health_boundary() {
        // Campaign crash_f1294cb3b665fae1 (seed 42, iteration 66092): on a barely-unhealthy
        // account a large-but-legal seize lifts maintenance health back above zero and the
        // program rejects with TooSevereLiquidation (marginfi_account.rs:1818). The model must
        // predict that rejection; a small seize that keeps health <= 0 is still accepted.
        let mut f = fixture();
        assert!(f.native_place_order(0, 500));
        let w = f.native_orders.as_ref().unwrap().clone();
        // Repay part of the debt so the account is only marginally unhealthy once the
        // liquidation action crashes banks[0] to 1/500: health = 0.9*(0.08 + 10) - 9.2 = -0.128.
        assert!(f.native_repay(w.group, w.order_user, w.banks[3], ORDER_DEBT * 8 / 100));
        // A 50% seize of the $10 bystander repays 4.75 of debt -> post health +0.122: rejected.
        assert!(f.native_liquidate_order_user(1, 49));
        let user = f.native_user(w.order_user);
        assert_eq!(position(&user, w.banks[2]).0, F::from_num(SCALE));
        assert_eq!(position(&user, w.banks[3]).1, F::from_num(9_200_000));
        // A 1% seize repays 0.095 -> post health -0.123 (still <= 0, improved): accepted.
        assert!(f.native_liquidate_order_user(1, 0));
        check(&mut f);
        let user = f.native_user(w.order_user);
        assert_eq!(position(&user, w.banks[2]).0, F::from_num(SCALE - SCALE / 100));
    }

    #[test]
    fn native_rebalance_into_crashed_destination_rejected_on_post_health() {
        // Campaign crash_43108b5ef9df5a65 (seed 42, iteration 36506): after price moves the
        // account can be maintenance-healthy pre-move yet unhealthy post-move, because
        // end_rebalance health-checks the POST-move balance set (WorseHealthPostExecution via
        // check_account_maint_health). The model must price the destination at its moved price.
        let mut f = fixture();
        assert!(f.native_place_rebalance(0, 200_000));
        let w = f.native_orders.as_ref().unwrap().clone();
        // Pile on debt so the banks[0] pile dominates health: 0.9*(30 + 10) - 15 = 21 > 0.
        assert!(f.native_borrow(w.group, w.rebalance_user, w.banks[3], 14 * SCALE));
        assert!(f.native_move_price(1, 95, true)); // banks[1]: 1.0 -> 0.05
        // Moving the whole 30-token pile into the crashed venue: post assets 0.9*(1.5 + 10)
        // = 10.35 < 15 liabilities -> rejected.
        assert!(f.native_execute_rebalance(0, 30 * SCALE as u32 - 1));
        check(&mut f);
        let user = f.native_user(w.rebalance_user);
        assert_eq!(position(&user, w.banks[0]).0, F::from_num(30 * SCALE));
        assert_eq!(position(&user, w.banks[1]).0, F::ZERO);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_rebalances, 0);
        // A 1-token move keeps post health at 0.9*(29 + 0.05 + 10) - 15 = 20.145 > 0: accepted.
        assert!(f.native_execute_rebalance(0, SCALE as u32 - 1));
        check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_rebalances, 1);
    }

    #[test]
    fn native_tagged_move_then_settle_and_untag() {
        let mut f = fixture();
        assert!(f.native_place_rebalance(0, 200_000));
        // The tagged move lands only while the rebalance user holds no banks[1] balance, so run
        // it before any ordinary execution; then verify the account rebalances normally after the
        // order was closed and its tags scrubbed.
        assert!(f.native_execute_rebalance_multi(3, 0));
        check(&mut f);
        assert!(f.native_execute_rebalance(0, SCALE as u32));
        check(&mut f);
        assert!(f.native_execute_rebalance_multi(0, 2 * SCALE as u32));
        check(&mut f);
        assert!(f.native_settle_rebalance(true, false));
        check(&mut f);
    }

    #[test]
    fn native_price_move_drives_triggers_honestly() {
        let mut f = fixture();
        // StopLoss anchored at the current net: triggered immediately, untriggered by a rally.
        assert!(f.native_place_order(0, 500));
        assert!(f.native_move_price(0, 20, false)); // asset +20%: net 30 -> 38, above the SL threshold
        assert!(f.native_execute_order(0, 99)); // must be rejected: trigger no longer met
        check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_orders, 0);
        assert!(f.native_move_price(0, 50, true)); // crash back below: net ~14 <= 30
        assert!(f.native_execute_order(0, 99)); // triggered for real now
        check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_orders, 1);
        // TakeProfit one unit above net: only a price move can light it.
        assert!(f.native_place_order(3, 500));
        assert!(f.native_execute_order(0, 99)); // rejected: trigger not met
        check(&mut f);
        assert!(f.native_move_price(0, 10, false)); // +10% lifts net past the threshold
        assert!(f.native_execute_order(0, 99));
        check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_orders, 2);
    }

    #[test]
    fn native_keeper_close_only_after_flag_clearing() {
        let mut f = fixture();
        assert!(f.native_place_order(1, 500));
        assert!(f.native_keeper_close_order()); // rejected: both tags identify live balances
        check(&mut f);
        assert!(f.native_orders.as_ref().unwrap().order.is_some());
        assert!(f.native_set_keeper_close_flags(true, true)); // unauthorized: rejected
        check(&mut f);
        assert!(f.native_set_keeper_close_flags(false, false)); // clear the asset leg's tag
        check(&mut f);
        assert!(f.native_keeper_close_order()); // stale now: permissionless close succeeds
        check(&mut f);
        assert!(f.native_orders.as_ref().unwrap().order.is_none());
        // The account can place and execute again on the same pair afterwards.
        assert!(f.native_place_order(0, 1000));
        assert!(f.native_execute_order(0, 99));
        check(&mut f);
        assert_eq!(f.native_orders.as_ref().unwrap().completed_orders, 1);
    }

    #[test]
    fn native_randomized_sequences() {
        let template = fixture();
        for seed in [1u64, 7, 42, 2026] {
            clear_iteration_state();
            let mut f = template.clone();
            let mut rng = seed;
            for _ in 0..40 {
                rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17;
                let byte = (rng >> 32) as u8;
                match rng % 14 {
                    0 => f.native_place_order(byte, rng as u16),
                    1 => f.native_execute_order(byte, rng as u16),
                    2 => f.native_cancel_order(byte % 2 == 0),
                    3 => f.native_place_rebalance(rng as u32, (rng >> 16) as u32),
                    4 => f.native_update_rebalance(rng as u32, (rng >> 16) as u32),
                    5 => f.native_execute_rebalance(byte, rng as u32),
                    6 => f.native_settle_rebalance(byte % 2 == 0, false),
                    7 => f.native_cancel_rebalance(byte % 2 == 0),
                    8 => f.native_rebalance_fee_pool(byte % 2 == 0, rng as u32),
                    9 => f.native_move_price(byte, (rng >> 8) as u8, byte % 2 == 0),
                    10 => f.native_keeper_close_order(),
                    11 => f.native_set_keeper_close_flags(byte % 2 == 0, rng % 3 == 0),
                    12 => f.native_execute_rebalance_multi(byte, rng as u32),
                    _ => f.native_liquidate_order_user(byte, (rng >> 8) as u8),
                };
                check(&mut f);
            }
        }
    }
}
