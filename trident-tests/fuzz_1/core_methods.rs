use trident_fuzz::fuzzing::prelude::TridentTransactionResult;
use trident_fuzz::fuzzing::*;

use crate::invariants;
use crate::types;
use crate::FuzzTestBank;

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

        let ix = self.lending_account_repay_ix(
            amount,
            bank.address,
            bank.currency.mint,
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
            bank.currency.mint,
            user_token_account,
            marginfi_account,
            authority,
        );
        let repay_ix = self.lending_account_repay_ix(
            repay_amount,
            bank.address,
            bank.currency.mint,
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
                SPL_TOKEN_ID,
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
}
