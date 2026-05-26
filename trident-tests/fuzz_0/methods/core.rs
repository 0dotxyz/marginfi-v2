use trident_fuzz::fuzzing::prelude::TridentTransactionResult;
use trident_fuzz::fuzzing::*;

use crate::invariants;
use crate::types;
use crate::FuzzTestBank;

use crate::user::User;
use crate::FuzzTest;

impl FuzzTest {
    pub(crate) fn snapshot_liquidity_vaults_except(
        &mut self,
        except_bank: Pubkey,
    ) -> Vec<(Pubkey, u64)> {
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

    pub(crate) fn assert_liquidity_balance_snapshot_unchanged(&mut self, snap: &[(Pubkey, u64)]) {
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
        let mint_data = self.trident.get_account(&bank.currency.mint);
        let bank_layout = self.bank_layout(bank.address);
        let user_before = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_before =
            invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank.address);

        let share_snap_before = invariants::marginfi_bank_share_snapshot(
            &mut self.trident,
            marginfi_account,
            bank.address,
        );

        let banks = self.get_marginfi_account_banks(marginfi_account, None);
        let token_program = *mint_data.owner();
        let remaining_accounts = self.remaining_accounts_for_bank_risk_and_t22_transfer(
            bank.currency.mint,
            token_program,
            banks,
        );

        // Randomize `deposit_up_to_limit`: when true, marginfi caps the deposit
        // at the bank's `deposit_limit`, so the actual moved amount may be
        // less than `amount`. Conservation and share-direction invariants
        // still hold, but the exact-amount equality check must be skipped.
        let deposit_up_to_limit = self.trident.random_bool();
        let ix = types::marginfi::LendingAccountDepositInstruction::data(
            types::marginfi::LendingAccountDepositInstructionData::new(
                amount,
                Some(deposit_up_to_limit),
            ),
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
            // Transfer-fee banks have non-conserving balance flows: user −
            // amount, vault + (amount − fee), fee → mint's withheld
            // balance. Skip the strict conservation / exact-amount checks
            // when the bank has a transfer fee. Direction checks (user
            // tokens drop, vault rises) and share-direction invariants
            // still hold.
            if !bank.has_transfer_fee {
                invariants::assert_deposit_balance_invariants(
                    amount,
                    user_before,
                    user_after,
                    vault_before,
                    vault_after,
                );
                if !deposit_up_to_limit {
                    invariants::assert_exact_deposit_token_leg(
                        amount,
                        user_before,
                        user_after,
                        vault_before,
                        vault_after,
                    );
                }
            } else {
                // Weakest sanity for fee banks: user balance fell, vault
                // balance rose. Both deltas non-zero when amount > 0.
                invariant!(
                    user_after <= user_before,
                    "t22-fee deposit: user must not gain. before {user_before}, after {user_after}"
                );
                invariant!(
                    vault_after >= vault_before,
                    "t22-fee deposit: vault must not lose. before {vault_before}, after {vault_after}"
                );
            }
            let share_snap_after = invariants::marginfi_bank_share_snapshot(
                &mut self.trident,
                marginfi_account,
                bank.address,
            );
            // When `deposit_up_to_limit` is true and the bank is already at
            // capacity, marginfi caps the deposit to 0 even though `amount`
            // is positive. The share invariant key off the *actually moved*
            // amount, derived from the user token delta.
            let share_amount = if deposit_up_to_limit {
                user_before.saturating_sub(user_after)
            } else {
                amount
            };
            invariants::assert_deposit_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                share_amount,
            );
            invariants::assert_balances_packed(&mut self.trident, marginfi_account);
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
        let mint_data = self.trident.get_account(&bank.currency.mint);
        let bank_layout = self.bank_layout(bank.address);
        let user_before = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_before =
            invariants::token_balance(&mut self.trident, bank_layout.liquidity_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank.address);

        let share_snap_before = invariants::marginfi_bank_share_snapshot(
            &mut self.trident,
            marginfi_account,
            bank.address,
        );

        let banks = self.get_marginfi_account_banks(marginfi_account, None);
        let token_program = *mint_data.owner();
        let remaining_accounts = self.remaining_accounts_for_bank_risk_and_t22_transfer(
            bank.currency.mint,
            token_program,
            banks,
        );

        // Randomize `withdraw_all`: when true, marginfi ignores `amount`
        // and withdraws the user's full asset position (and closes the
        // balance). The exact-amount equality check must be skipped.
        let withdraw_all = self.trident.random_bool();
        let ix = types::marginfi::LendingAccountWithdrawInstruction::data(
            types::marginfi::LendingAccountWithdrawInstructionData::new(amount, Some(withdraw_all)),
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
            if !withdraw_all {
                invariants::assert_exact_user_vault_delta_withdraw(
                    amount,
                    user_before,
                    user_after,
                    vault_before,
                    vault_after,
                );
            }
            let share_snap_after = invariants::marginfi_bank_share_snapshot(
                &mut self.trident,
                marginfi_account,
                bank.address,
            );
            // With `withdraw_all`, the actual moved amount is whatever the
            // user's position was — derive it from the token delta so the
            // share-direction invariant has the correct expectation.
            let share_amount = if withdraw_all {
                user_after.saturating_sub(user_before)
            } else {
                amount
            };
            invariants::assert_withdraw_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                share_amount,
            );
            invariants::assert_balances_packed(&mut self.trident, marginfi_account);
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

        let share_snap_before = invariants::marginfi_bank_share_snapshot(
            &mut self.trident,
            marginfi_account,
            bank.address,
        );

        let ix = self.lending_account_borrow_ix(
            amount,
            bank.address,
            bank.currency.mint,
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
            invariants::assert_balances_packed(&mut self.trident, marginfi_account);
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

        let share_snap_before = invariants::marginfi_bank_share_snapshot(
            &mut self.trident,
            marginfi_account,
            bank.address,
        );

        let bank_layout = self.bank_layout(bank.address);
        let token_program = *self.trident.get_account(&bank.currency.mint).owner();
        let banks = self.get_marginfi_account_banks(marginfi_account, Some(bank.address));
        let remaining_accounts = self.remaining_accounts_for_bank_risk_and_t22_transfer(
            bank.currency.mint,
            token_program,
            banks,
        );

        // Randomize `repay_all`: when true, marginfi ignores `amount` and
        // pays off the user's entire liability (and closes the balance).
        // The exact-amount equality check must be skipped.
        let repay_all = self.trident.random_bool();
        let repay_ix = types::marginfi::LendingAccountRepayInstruction::data(
            types::marginfi::LendingAccountRepayInstructionData::new(amount, Some(repay_all)),
        )
        .accounts(
            types::marginfi::LendingAccountRepayInstructionAccounts::new(
                self.marginfi_group,
                marginfi_account,
                authority,
                bank.address,
                source_token_account,
                bank_layout.liquidity_vault,
                token_program,
            ),
        )
        .remaining_accounts(remaining_accounts)
        .instruction();

        let res = self.trident.process_transaction(&[repay_ix], msg);

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
            if !repay_all {
                invariants::assert_repay_user_token_delta_matches_post_fee_amount(
                    amount,
                    user_before,
                    user_after,
                    vault_before,
                    vault_after,
                );
            }
            let share_snap_after = invariants::marginfi_bank_share_snapshot(
                &mut self.trident,
                marginfi_account,
                bank.address,
            );
            // With `repay_all`, the actual moved amount is whatever the
            // user's liability was — derive from the token delta.
            let share_amount = if repay_all {
                user_before.saturating_sub(user_after)
            } else {
                amount
            };
            invariants::assert_repay_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                share_amount,
            );
            invariants::assert_balances_packed(&mut self.trident, marginfi_account);
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

    pub fn lending_flashloan(
        &mut self,
        user: &User,
        inner_instructions: Vec<Instruction>,
        msg: Option<&str>,
        end_health_banks: Option<Vec<Pubkey>>,
    ) -> TridentTransactionResult {
        let banks = match end_health_banks {
            Some(mut b) => {
                b.sort_by(|a, c| c.cmp(a));
                b
            }
            None => self.get_marginfi_account_banks(user.marginfi_account, None),
        };
        let end_remaining = self.remaining_accounts_for_bank_risk_only(banks);

        let end_index = inner_instructions.len() as u64 + 1;

        let start_ix = types::marginfi::LendingAccountStartFlashloanInstruction::data(
            types::marginfi::LendingAccountStartFlashloanInstructionData::new(end_index),
        )
        .accounts(
            types::marginfi::LendingAccountStartFlashloanInstructionAccounts::new(
                user.marginfi_account,
                user.address,
            ),
        )
        .instruction();

        let end_ix = types::marginfi::LendingAccountEndFlashloanInstruction::data(
            types::marginfi::LendingAccountEndFlashloanInstructionData::new(),
        )
        .accounts(
            types::marginfi::LendingAccountEndFlashloanInstructionAccounts::new(
                user.marginfi_account,
                user.address,
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
            let extra = vec![
                user.usdc_token_account,
                user.eth_token_account,
                user.btc_token_account,
            ];
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

    #[allow(clippy::too_many_arguments)]
    pub fn lending_flashloan_borrow_repay(
        &mut self,
        borrow_amount: u64,
        repay_amount: u64,
        bank: FuzzTestBank,
        user: &User,
        msg: Option<&str>,
    ) {
        let borrow_ix = self.lending_account_borrow_ix(
            borrow_amount,
            bank.address,
            bank.currency.mint,
            user.btc_token_account,
            user.marginfi_account,
            user.address,
        );
        let repay_ix = self.lending_account_repay_ix(
            repay_amount,
            bank.address,
            bank.currency.mint,
            user.btc_token_account,
            user.marginfi_account,
            user.address,
            true,
        );
        let user_before_flashloan =
            invariants::token_balance(&mut self.trident, user.btc_token_account);
        let res = self.lending_flashloan(
            user,
            vec![borrow_ix, repay_ix],
            msg,
            Some(vec![bank.address]),
        );

        if borrow_amount == repay_amount && res.is_success() {
            let user_after_flashloan =
                invariants::token_balance(&mut self.trident, user.btc_token_account);
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
        let liab_mint_data = self.trident.get_account(&liab_bank.currency.mint);
        let liab_token_program = *liab_mint_data.owner();
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
                liab_token_program,
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
            invariants::assert_balances_packed(&mut self.trident, liquidator_marginfi_account);
            invariants::assert_balances_packed(&mut self.trident, liquidatee_marginfi_account);
        } else {
            invariants::assert_liquidation_failure_state_unchanged(&snap, &after);
        }
    }

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
            invariants::assert_balances_packed(&mut self.trident, liquidatee_marginfi_account);
        }
    }
}
