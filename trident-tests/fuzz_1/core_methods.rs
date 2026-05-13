use trident_fuzz::fuzzing::prelude::TridentTransactionResult;
use trident_fuzz::fuzzing::*;

use crate::constants::*;
use crate::invariants;
use crate::types;
use crate::FuzzTestBank;

use crate::FuzzTest;

impl FuzzTest {
    fn snapshot_liquidity_vaults_except(&mut self, except_bank: Pubkey) -> Vec<(Pubkey, u64)> {
        [
            self.usdc_bank.address,
            self.eth_bank.address,
            self.btc_bank.address,
        ]
        .into_iter()
        .filter(|&b| b != except_bank)
        .map(|b| {
            let v = self.bank_layout(b).liquidity_vault;
            (v, invariants::token_balance(&mut self.trident, v))
        })
        .collect()
    }

    fn assert_liquidity_balance_snapshot_unchanged(&mut self, snap: &[(Pubkey, u64)]) {
        for &(pk, before) in snap {
            invariants::assert_balance_unchanged(
                before,
                invariants::token_balance(&mut self.trident, pk),
            );
        }
    }

    fn lending_pool_accrue_bank_interest_ix(&self, bank: Pubkey) -> Instruction {
        types::marginfi::LendingPoolAccrueBankInterestInstruction::data(
            types::marginfi::LendingPoolAccrueBankInterestInstructionData::new(),
        )
        .accounts(
            types::marginfi::LendingPoolAccrueBankInterestInstructionAccounts::new(
                self.marginfi_group,
                bank,
            ),
        )
        .instruction()
    }

    /// Permissionless: accrue interest on every fuzz bank. Use after `forward_in_time` so
    /// `time_delta = clock.unix_timestamp - bank.last_update` is non-zero.
    pub fn lending_pool_accrue_all_banks(&mut self, msg: Option<&str>) {
        let bank_pks = [
            self.usdc_bank.address,
            self.eth_bank.address,
            self.btc_bank.address,
        ];
        let last_before: Vec<i64> = bank_pks
            .iter()
            .map(|&pk| invariants::bank_last_update_snapshot(&mut self.trident, pk))
            .collect();
        let ixs = vec![
            self.lending_pool_accrue_bank_interest_ix(self.usdc_bank.address),
            self.lending_pool_accrue_bank_interest_ix(self.eth_bank.address),
            self.lending_pool_accrue_bank_interest_ix(self.btc_bank.address),
        ];
        let res = self.trident.process_transaction(&ixs, msg);
        invariant!(res.is_success());
        invariants::assert_accrue_advanced_bank_last_updates(
            &mut self.trident,
            &bank_pks,
            &last_before,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lending_account_deposit(
        &mut self,
        amount: u64,
        bank: FuzzTestBank,
        user_token_account: Pubkey,
        marginfi_account: Pubkey,
        authority: Pubkey,
        msg: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&bank.mint);
        let bank_layout = self.bank_layout(bank.address);
        let user_before = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_before =
            invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank.address);

        let share_snap_before =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank.address);

        let banks = self.get_marginfi_account_banks(marginfi_account, None);
        let token_program = *mint_data.owner();
        let remaining_accounts =
            self.remaining_accounts_for_bank_risk_and_t22_transfer(bank.mint, token_program, banks);

        let ix = types::marginfi::LendingAccountDepositInstruction::data(
            types::marginfi::LendingAccountDepositInstructionData::new(amount, Some(false)),
        )
        .accounts(
            types::marginfi::LendingAccountDepositInstructionAccounts::new(
                self.marginfi_group,
                marginfi_account,
                authority,
                bank.address,
                user_token_account,
                bank_layout.liquidity_vault,
                token_program,
            ),
        )
        .remaining_accounts(remaining_accounts)
        .instruction();

        let res = self.trident.process_transaction(&[ix], msg);

        let user_after = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_after = invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);

        if res.is_success() {
            invariants::assert_deposit_balance_invariants(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
            invariants::assert_exact_deposit_token_leg(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
            let share_snap_after = invariants::marginfi_bank_share_snapshot(
                &mut self.trident,
                marginfi_account,
                bank.address,
            );
            invariants::assert_deposit_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                amount,
            );
        } else {
            invariants::assert_no_balance_change(
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
        }
        self.assert_liquidity_balance_snapshot_unchanged(&other_vaults_snap);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lending_account_withdraw(
        &mut self,
        amount: u64,
        bank: FuzzTestBank,
        user_token_account: Pubkey,
        marginfi_account: Pubkey,
        authority: Pubkey,
        msg: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&bank.mint);
        let bank_layout = self.bank_layout(bank.address);
        let user_before = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_before =
            invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank.address);

        let share_snap_before =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank.address);

        let banks = self.get_marginfi_account_banks(marginfi_account, None);
        let token_program = *mint_data.owner();
        let remaining_accounts =
            self.remaining_accounts_for_bank_risk_and_t22_transfer(bank.mint, token_program, banks);

        let ix = types::marginfi::LendingAccountWithdrawInstruction::data(
            types::marginfi::LendingAccountWithdrawInstructionData::new(amount, Some(false)),
        )
        .accounts(
            types::marginfi::LendingAccountWithdrawInstructionAccounts::new(
                self.marginfi_group,
                marginfi_account,
                authority,
                bank.address,
                user_token_account,
                bank_layout.liquidity_vault_authority,
                bank_layout.liquidity_vault,
                token_program,
            ),
        )
        .remaining_accounts(remaining_accounts)
        .instruction();

        let res = self.trident.process_transaction(&[ix], msg);

        let user_after = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_after = invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);

        if res.is_success() {
            invariants::assert_withdraw_balance_invariants(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
            invariants::assert_exact_user_vault_delta_withdraw(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
            let share_snap_after = invariants::marginfi_bank_share_snapshot(
                &mut self.trident,
                marginfi_account,
                bank.address,
            );
            invariants::assert_withdraw_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                amount,
            );
        } else {
            invariants::assert_no_balance_change(
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
        }
        self.assert_liquidity_balance_snapshot_unchanged(&other_vaults_snap);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lending_account_borrow_ix(
        &mut self,
        amount: u64,
        bank: Pubkey,
        bank_mint: Pubkey,
        destination_token_account: Pubkey,
        marginfi_account: Pubkey,
        authority: Pubkey,
    ) -> Instruction {
        let bank_layout = self.bank_layout(bank);
        let token_program = *self.trident.get_account(&bank_mint).owner();
        let banks = self.get_marginfi_account_banks(marginfi_account, Some(bank));
        let remaining_accounts =
            self.remaining_accounts_for_bank_risk_and_t22_transfer(bank_mint, token_program, banks);

        types::marginfi::LendingAccountBorrowInstruction::data(
            types::marginfi::LendingAccountBorrowInstructionData::new(amount),
        )
        .accounts(
            types::marginfi::LendingAccountBorrowInstructionAccounts::new(
                self.marginfi_group,
                marginfi_account,
                authority,
                bank,
                destination_token_account,
                bank_layout.liquidity_vault_authority,
                bank_layout.liquidity_vault,
                token_program,
            ),
        )
        .remaining_accounts(remaining_accounts)
        .instruction()
    }

    /// `pre_commit_interacting_bank`: set `true` when this instruction is serialized before an
    /// earlier instruction in the same transaction opens the position (e.g. repay after borrow in
    /// a flashloan bundle).
    #[allow(clippy::too_many_arguments)]
    pub fn lending_account_repay_ix(
        &mut self,
        amount: u64,
        bank: Pubkey,
        bank_mint: Pubkey,
        source_token_account: Pubkey,
        marginfi_account: Pubkey,
        authority: Pubkey,
        pre_commit_interacting_bank: bool,
    ) -> Instruction {
        let bank_layout = self.bank_layout(bank);
        let token_program = *self.trident.get_account(&bank_mint).owner();
        let banks = self.get_marginfi_account_banks(
            marginfi_account,
            if pre_commit_interacting_bank {
                Some(bank)
            } else {
                None
            },
        );
        let remaining_accounts =
            self.remaining_accounts_for_bank_risk_and_t22_transfer(bank_mint, token_program, banks);

        types::marginfi::LendingAccountRepayInstruction::data(
            types::marginfi::LendingAccountRepayInstructionData::new(amount, Some(false)),
        )
        .accounts(
            types::marginfi::LendingAccountRepayInstructionAccounts::new(
                self.marginfi_group,
                marginfi_account,
                authority,
                bank,
                source_token_account,
                bank_layout.liquidity_vault,
                token_program,
            ),
        )
        .remaining_accounts(remaining_accounts)
        .instruction()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lending_account_borrow(
        &mut self,
        amount: u64,
        bank: FuzzTestBank,
        destination_token_account: Pubkey,
        marginfi_account: Pubkey,
        authority: Pubkey,
        msg: Option<&str>,
    ) {
        let bank_layout = self.bank_layout(bank.address);
        let user_before = invariants::token_balance(&mut self.trident, destination_token_account);
        let vault_before =
            invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank.address);

        let share_snap_before =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank.address);

        let ix = self.lending_account_borrow_ix(
            amount,
            bank.address,
            bank.mint,
            destination_token_account,
            marginfi_account,
            authority,
        );

        let res = self.trident.process_transaction(&[ix], msg);

        let user_after = invariants::token_balance(&mut self.trident, destination_token_account);
        let vault_after = invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);

        if res.is_success() {
            invariants::assert_borrow_balance_invariants(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
            invariants::assert_exact_user_vault_delta_withdraw(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
            let share_snap_after = invariants::marginfi_bank_share_snapshot(
                &mut self.trident,
                marginfi_account,
                bank.address,
            );
            invariants::assert_borrow_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                amount,
            );
        } else {
            invariants::assert_no_balance_change(
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
        }
        self.assert_liquidity_balance_snapshot_unchanged(&other_vaults_snap);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lending_account_repay(
        &mut self,
        amount: u64,
        bank: FuzzTestBank,
        source_token_account: Pubkey,
        marginfi_account: Pubkey,
        authority: Pubkey,
        msg: Option<&str>,
    ) {
        let bank_layout = self.bank_layout(bank.address);
        let user_before = invariants::token_balance(&mut self.trident, source_token_account);
        let vault_before =
            invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank.address);

        let share_snap_before =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank.address);

        let ix = self.lending_account_repay_ix(
            amount,
            bank.address,
            bank.mint,
            source_token_account,
            marginfi_account,
            authority,
            false,
        );

        let res = self.trident.process_transaction(&[ix], msg);

        let user_after = invariants::token_balance(&mut self.trident, source_token_account);
        let vault_after = invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);

        if res.is_success() {
            invariants::assert_repay_balance_invariants(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
            invariants::assert_repay_user_token_delta_matches_post_fee_amount(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
            let share_snap_after = invariants::marginfi_bank_share_snapshot(
                &mut self.trident,
                marginfi_account,
                bank.address,
            );
            invariants::assert_repay_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                amount,
            );
        } else {
            invariants::assert_no_balance_change(
                user_before,
                user_after,
                vault_before,
                vault_after,
            );
        }
        self.assert_liquidity_balance_snapshot_unchanged(&other_vaults_snap);
    }

    /// `[start_flashloan(end_index), ...inner_instructions, end_flashloan]`.
    ///
    /// `end_health_banks`: banks to pass into `end_flashloan` health/risk `remaining_accounts`
    /// (bank + oracle per bank). Use **`None`** when the final active-bank list matches the
    /// account **before** this transaction (e.g. empty inner). Use **`Some(vec![…])`** when inner
    /// ixs touch banks that are not yet active on-chain at ix-build time but can still be **active**
    /// after the tx (e.g. borrow→repay may leave an active empty slot — still needs that bank’s
    /// risk accounts).
    pub fn lending_flashloan(
        &mut self,
        marginfi_account: Pubkey,
        authority: Pubkey,
        inner_instructions: Vec<Instruction>,
        msg: Option<&str>,
        end_health_banks: Option<Vec<Pubkey>>,
    ) -> TridentTransactionResult {
        let banks = match end_health_banks {
            Some(mut b) => {
                b.sort_by(|a, c| c.cmp(a));
                b
            }
            None => self.get_marginfi_account_banks(marginfi_account, None),
        };
        let end_remaining = self.remaining_accounts_for_bank_risk_only(banks);

        let end_index = inner_instructions.len() as u64 + 1;

        let start_ix = types::marginfi::LendingAccountStartFlashloanInstruction::data(
            types::marginfi::LendingAccountStartFlashloanInstructionData::new(end_index),
        )
        .accounts(
            types::marginfi::LendingAccountStartFlashloanInstructionAccounts::new(
                marginfi_account,
                authority,
            ),
        )
        .instruction();

        let end_ix = types::marginfi::LendingAccountEndFlashloanInstruction::data(
            types::marginfi::LendingAccountEndFlashloanInstructionData::new(),
        )
        .accounts(
            types::marginfi::LendingAccountEndFlashloanInstructionAccounts::new(
                marginfi_account,
                authority,
            ),
        )
        .remaining_accounts(end_remaining)
        .instruction();

        let empty_snap = if inner_instructions.is_empty() {
            let vaults = [
                self.bank_layout(self.usdc_bank.address).liquidity_vault,
                self.bank_layout(self.eth_bank.address).liquidity_vault,
                self.bank_layout(self.btc_bank.address).liquidity_vault,
            ];
            let extra = if authority == self.user_a.address {
                vec![
                    self.user_a.usdc_token_account,
                    self.user_a.eth_token_account,
                ]
            } else if authority == self.user_b.address {
                vec![self.user_b.btc_token_account]
            } else if authority == self.seeder.address {
                vec![
                    self.seeder.usdc_token_account,
                    self.seeder.eth_token_account,
                    self.seeder.btc_token_account,
                ]
            } else {
                vec![]
            };
            Some(invariants::flashloan_empty_balance_snapshot(
                &mut self.trident,
                &vaults,
                &extra,
            ))
        } else {
            None
        };

        let mut ixs = Vec::with_capacity(2 + inner_instructions.len());
        ixs.push(start_ix);
        ixs.extend(inner_instructions);
        ixs.push(end_ix);

        let res = self.trident.process_transaction(&ixs, msg);

        if let Some(ref snap) = empty_snap {
            invariants::assert_token_snapshot_unchanged(&mut self.trident, snap);
        }

        res
    }

    /// Flashloan with inner borrow (`borrow_amount`) then repay (`repay_amount`) on `bank`.
    ///
    /// For a closed loop, amounts must match; otherwise the transaction should revert once
    /// end-flashloan health runs (or earlier if SPL repay exceeds the wallet).
    #[allow(clippy::too_many_arguments)]
    pub fn lending_flashloan_borrow_repay(
        &mut self,
        borrow_amount: u64,
        repay_amount: u64,
        bank: FuzzTestBank,
        marginfi_account: Pubkey,
        authority: Pubkey,
        user_token_account: Pubkey,
        msg: Option<&str>,
    ) {
        let borrow_ix = self.lending_account_borrow_ix(
            borrow_amount,
            bank.address,
            bank.mint,
            user_token_account,
            marginfi_account,
            authority,
        );
        let repay_ix = self.lending_account_repay_ix(
            repay_amount,
            bank.address,
            bank.mint,
            user_token_account,
            marginfi_account,
            authority,
            true,
        );
        let user_before_flashloan =
            invariants::token_balance(&mut self.trident, user_token_account);
        let res = self.lending_flashloan(
            marginfi_account,
            authority,
            vec![borrow_ix, repay_ix],
            msg,
            Some(vec![bank.address]),
        );

        if borrow_amount == repay_amount && res.is_success() {
            let user_after_flashloan =
                invariants::token_balance(&mut self.trident, user_token_account);
            invariants::assert_flashloan_closed_loop_user_unchanged(
                user_before_flashloan,
                user_after_flashloan,
            );
        }

        if borrow_amount != repay_amount {
            invariant!(
                !res.is_success(),
                "mismatched borrow/repay should revert the flashloan tx"
            );
        }
    }

    /// Permissionless-style liquidation: liquidator signs; liquidatee must be unhealthy.
    ///
    /// Does **not** use `start_liquidation` / `end_liquidation` (those are a separate receivership
    /// flow with withdraw/repay-only inner instructions).
    #[allow(clippy::too_many_arguments)]
    pub fn lending_account_liquidate(
        &mut self,
        asset_amount: u64,
        asset_bank: FuzzTestBank,
        liab_bank: FuzzTestBank,
        liquidator_marginfi_account: Pubkey,
        liquidator_authority: Pubkey,
        liquidatee_marginfi_account: Pubkey,
        msg: Option<&str>,
    ) {
        let liab_layout = self.bank_layout(liab_bank.address);
        let (remaining_accounts, liquidatee_accounts, liquidator_accounts) = self
            .remaining_accounts_for_liquidation(
                asset_bank.address,
                liab_bank.address,
                liquidator_marginfi_account,
                liquidatee_marginfi_account,
            );

        let snap = invariants::liquidation_balance_snapshot(
            &mut self.trident,
            liab_layout.liquidity_vault,
            liab_layout.insurance_vault,
            liquidatee_marginfi_account,
            liquidator_marginfi_account,
            asset_bank.address,
            liab_bank.address,
        );

        let ix = types::marginfi::LendingAccountLiquidateInstruction::data(
            types::marginfi::LendingAccountLiquidateInstructionData::new(
                asset_amount,
                liquidatee_accounts,
                liquidator_accounts,
            ),
        )
        .accounts(
            types::marginfi::LendingAccountLiquidateInstructionAccounts::new(
                self.marginfi_group,
                asset_bank.address,
                liab_bank.address,
                liquidator_marginfi_account,
                liquidator_authority,
                liquidatee_marginfi_account,
                liab_layout.liquidity_vault_authority,
                liab_layout.liquidity_vault,
                liab_layout.insurance_vault,
                TOKEN_2022_PROGRAM_ID,
            ),
        )
        .remaining_accounts(remaining_accounts)
        .instruction();

        let res = self.trident.process_transaction(&[ix], msg);

        let after = invariants::liquidation_balance_snapshot(
            &mut self.trident,
            liab_layout.liquidity_vault,
            liab_layout.insurance_vault,
            liquidatee_marginfi_account,
            liquidator_marginfi_account,
            asset_bank.address,
            liab_bank.address,
        );

        if res.is_success() {
            invariants::assert_liquidation_success_share_invariants(&snap, &after, asset_amount);
        } else {
            invariants::assert_liquidation_failure_state_unchanged(&snap, &after);
        }
    }

    /// Receivership liquidation: `start_liquidation` → optional middle instructions → `end_liquidation` (last).
    ///
    /// Allowed middle ixs are marginfi withdraw/repay (and integration withdraws); see `validate_ixes_exclusive` in the program.
    ///
    /// **Signing:** Trident’s `process_transaction` uses a single fee-payer keypair. Pass that pubkey as
    /// `liquidation_receiver` and `global_fee_wallet` (matches `init_global_fee_state` in this harness).
    pub fn lending_account_receivership_liquidation(
        &mut self,
        liquidatee_marginfi_account: Pubkey,
        liquidation_receiver: Pubkey,
        global_fee_wallet: Pubkey,
        middle_ixs: &[Instruction],
        msg: Option<&str>,
    ) {
        let record = self.liquidation_record_pda(liquidatee_marginfi_account);
        let liq_banks = self.get_marginfi_account_banks(liquidatee_marginfi_account, None);
        let health_remaining_start = self.remaining_accounts_for_bank_risk_only(liq_banks.clone());
        let health_remaining_end = self.remaining_accounts_for_bank_risk_banks_only(liq_banks);

        let start_ix = types::marginfi::StartLiquidationInstruction::data(
            types::marginfi::StartLiquidationInstructionData::new(),
        )
        .accounts(types::marginfi::StartLiquidationInstructionAccounts::new(
            liquidatee_marginfi_account,
            record,
            liquidation_receiver,
        ))
        .remaining_accounts(health_remaining_start)
        .instruction();

        let end_ix = types::marginfi::EndLiquidationInstruction::data(
            types::marginfi::EndLiquidationInstructionData::new(),
        )
        .accounts(types::marginfi::EndLiquidationInstructionAccounts::new(
            liquidatee_marginfi_account,
            record,
            liquidation_receiver,
            self.fee_state,
            global_fee_wallet,
        ))
        .remaining_accounts(health_remaining_end)
        .instruction();

        let mut ixs = vec![start_ix];
        ixs.extend_from_slice(middle_ixs);
        ixs.push(end_ix);
        let res = self.trident.process_transaction(&ixs, msg);
        if res.is_success() {
            invariants::assert_receivership_cleared_after_success(
                &mut self.trident,
                liquidatee_marginfi_account,
                record,
            );
        }
    }

    // pub fn kamino_deposit(
    //     &mut self,
    //     kamino_reserve: Pubkey,
    //     amount: u64,
    //     marginfi_account: Pubkey,
    //     authority: Pubkey,
    //     signer_token_account: Pubkey,
    //     msg: Option<&str>,
    // ) {
    //     let (bank, rsv) = kamino_fork::kamino_marginfi_bank_and_reserve(
    //         &mut self.trident,
    //         self.marginfi_group,
    //         kamino_reserve,
    //         kamino_fork::KAMINO_BANK_SEED,
    //     );
    //     let mint = rsv.liquidity_mint;
    //     let layout = self.bank_layout(bank);
    //     let obligation = kamino_fork::kamino_obligation_pda(layout.liquidity_vault_authority);
    //     let lma = kamino_fork::kamino_lending_market_authority_pda();
    //     let ph = kamino_fork::KAMINO_IX_OPTIONAL_PLACEHOLDER;

    //     let ix = KaminoDepositInstruction::data(KaminoDepositInstructionData::new(amount))
    //         .accounts(KaminoDepositInstructionAccounts::new(
    //             self.marginfi_group,
    //             marginfi_account,
    //             authority,
    //             bank,
    //             signer_token_account,
    //             layout.liquidity_vault_authority,
    //             layout.liquidity_vault,
    //             obligation,
    //             kamino_fork::KAMINO_LENDING_MARKET,
    //             lma,
    //             kamino_reserve,
    //             mint,
    //             rsv.reserve_liquidity_supply,
    //             rsv.reserve_collateral_mint,
    //             rsv.reserve_collateral_supply_vault,
    //             ph,
    //             ph,
    //             rsv.liquidity_token_program,
    //         ))
    //         .instruction();
    //     let _ = self.trident.process_transaction(&[ix], msg);
    // }

    // /// Withdraw Kamino collateral via marginfi. `collateral_amount` is in **collateral** token units.
    // pub fn kamino_withdraw(
    //     &mut self,
    //     kamino_reserve: Pubkey,
    //     collateral_amount: u64,
    //     withdraw_all: Option<bool>,
    //     marginfi_account: Pubkey,
    //     authority: Pubkey,
    //     destination_token_account: Pubkey,
    //     msg: Option<&str>,
    // ) {
    //     let (bank, rsv) = kamino_fork::kamino_marginfi_bank_and_reserve(
    //         &mut self.trident,
    //         self.marginfi_group,
    //         kamino_reserve,
    //         kamino_fork::KAMINO_BANK_SEED,
    //     );
    //     let mint = rsv.liquidity_mint;
    //     let layout = self.bank_layout(bank);
    //     let obligation = kamino_fork::kamino_obligation_pda(layout.liquidity_vault_authority);
    //     let lma = kamino_fork::kamino_lending_market_authority_pda();
    //     let ph = kamino_fork::KAMINO_IX_OPTIONAL_PLACEHOLDER;

    //     let banks = self.get_marginfi_account_banks(marginfi_account, Some(bank));
    //     let remaining = self.remaining_accounts_for_bank_risk_and_t22_transfer(mint, banks);

    //     let ix = KaminoWithdrawInstruction::data(KaminoWithdrawInstructionData::new(
    //         collateral_amount,
    //         withdraw_all,
    //     ))
    //     .accounts(KaminoWithdrawInstructionAccounts::new(
    //         self.marginfi_group,
    //         marginfi_account,
    //         authority,
    //         bank,
    //         destination_token_account,
    //         layout.liquidity_vault_authority,
    //         layout.liquidity_vault,
    //         obligation,
    //         kamino_fork::KAMINO_LENDING_MARKET,
    //         lma,
    //         kamino_reserve,
    //         mint,
    //         rsv.reserve_liquidity_supply,
    //         rsv.reserve_collateral_mint,
    //         rsv.reserve_collateral_supply_vault,
    //         ph,
    //         ph,
    //         rsv.liquidity_token_program,
    //     ))
    //     .remaining_accounts(remaining)
    //     .instruction();
    //     let _ = self.trident.process_transaction(&[ix], msg);
    // }
}
