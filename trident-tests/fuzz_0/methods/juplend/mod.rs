#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

use crate::constants;
use crate::invariants;
use crate::types;
use crate::FuzzTest;

use crate::types::marginfi::JuplendConfigCompact;
use crate::types::marginfi::OracleSetup;
use crate::types::marginfi::RiskTier;
use crate::types::marginfi::WrappedI80F48;
use crate::utils::initialize_associated_token_account;

use fixed_macro::types::I80F48;

const JUPLEND_F_TOKEN_VAULT_SEED: &[u8] = b"f_token_vault";

fn wrap_one() -> WrappedI80F48 {
    WrappedI80F48::new(I80F48!(1.0).to_bits().to_le_bytes())
}

impl FuzzTest {
    pub fn juplend_bank_address(&mut self, group: Pubkey, mint: Pubkey, bank_seed: u64) -> Pubkey {
        self.trident
            .find_program_address(
                &[group.as_ref(), mint.as_ref(), &bank_seed.to_le_bytes()],
                &types::marginfi::program_id(),
            )
            .0
    }

    pub fn juplend_f_token_vault_address(&mut self, bank: Pubkey) -> Pubkey {
        self.trident
            .find_program_address(
                &[JUPLEND_F_TOKEN_VAULT_SEED, bank.as_ref()],
                &types::marginfi::program_id(),
            )
            .0
    }

    pub fn default_juplend_bank_config(&mut self, oracle: Pubkey) -> JuplendConfigCompact {
        JuplendConfigCompact::new(
            oracle,
            wrap_one(),
            wrap_one(),
            10_000_000_000_000,
            OracleSetup::JuplendPythPull,
            RiskTier::Collateral,
            constants::PYTH_PULL_MIGRATED_CONFIG_FLAGS,
            1_000_000_000_000,
            300,
            0,
        )
    }

    pub fn init_juplend_bank(
        &mut self,
        payer: Pubkey,
        mint: Pubkey,
        juplend_lending_state: Pubkey,
        juplend_f_token_mint: Pubkey,
        oracle: Pubkey,
        message: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&mint);
        let token_program = *mint_data.owner();

        let bank =
            self.juplend_bank_address(self.marginfi_group, mint, constants::JUPITER_BANK_SEED);
        let layout = self.bank_layout(bank);
        let f_token_vault = self.juplend_f_token_vault_address(bank);

        let bank_config = self.default_juplend_bank_config(oracle);

        let add_ix = types::marginfi::LendingPoolAddBankJuplendInstruction::data(
            types::marginfi::LendingPoolAddBankJuplendInstructionData::new(
                bank_config,
                constants::JUPITER_BANK_SEED,
            ),
        )
        .accounts(
            types::marginfi::LendingPoolAddBankJuplendInstructionAccounts::new(
                self.marginfi_group,
                payer,
                payer,
                mint,
                bank,
                juplend_lending_state,
                layout.liquidity_vault_authority,
                layout.liquidity_vault,
                layout.insurance_vault_authority,
                layout.insurance_vault,
                layout.fee_vault_authority,
                layout.fee_vault,
                juplend_f_token_mint,
                f_token_vault,
                token_program,
            ),
        )
        .remaining_accounts(vec![
            AccountMeta::new_readonly(oracle, false),
            AccountMeta::new_readonly(juplend_lending_state, false),
        ])
        .instruction();

        let res = self.trident.process_transaction(&[add_ix], message);

        invariant!(res.is_success());
    }

    pub fn init_juplend_position(
        &mut self,
        user: Pubkey,
        user_token_account: Pubkey,
        mint: Pubkey,
        juplend_lending_state: Pubkey,
        juplend_f_token_mint: Pubkey,
        lending_admin: Pubkey,
        supply_token_reserves_liquidity: Pubkey,
        lending_supply_position_on_liquidity: Pubkey,
        rate_model: Pubkey,
        vault: Pubkey,
        liquidity: Pubkey,
        rewards_rate_model: Pubkey,
        seed_deposit_amount: u64,
        message: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&mint);

        let bank =
            self.juplend_bank_address(self.marginfi_group, mint, constants::JUPITER_BANK_SEED);
        let layout = self.bank_layout(bank);
        let f_token_vault = self.juplend_f_token_vault_address(bank);

        let init_ix = types::marginfi::JuplendInitPositionInstruction::data(
            types::marginfi::JuplendInitPositionInstructionData::new(seed_deposit_amount),
        )
        .accounts(
            types::marginfi::JuplendInitPositionInstructionAccounts::new(
                user,
                user_token_account,
                bank,
                layout.liquidity_vault_authority,
                layout.liquidity_vault,
                mint,
                juplend_lending_state,
                juplend_f_token_mint,
                f_token_vault,
                lending_admin,
                supply_token_reserves_liquidity,
                lending_supply_position_on_liquidity,
                rate_model,
                vault,
                liquidity,
                rewards_rate_model,
                *mint_data.owner(),
            ),
        )
        .instruction();

        let res = self.trident.process_transaction(&[init_ix], message);
        invariant!(res.is_success());
    }

    pub fn deposit_to_juplend(
        &mut self,
        marginfi_account: Pubkey,
        user: Pubkey,
        user_token_account: Pubkey,
        mint: Pubkey,
        juplend_lending_state: Pubkey,
        juplend_f_token_mint: Pubkey,
        lending_admin: Pubkey,
        supply_token_reserves_liquidity: Pubkey,
        lending_supply_position_on_liquidity: Pubkey,
        rate_model: Pubkey,
        vault: Pubkey,
        liquidity: Pubkey,
        rewards_rate_model: Pubkey,
        amount: u64,
        message: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&mint);
        let token_program = *mint_data.owner();

        let bank =
            self.juplend_bank_address(self.marginfi_group, mint, constants::JUPITER_BANK_SEED);
        let layout = self.bank_layout(bank);
        let f_token_vault = self.juplend_f_token_vault_address(bank);

        let user_before = invariants::token_balance(&mut self.trident, user_token_account);
        let liquidity_vault_before =
            invariants::token_balance(&mut self.trident, layout.liquidity_vault);
        let f_token_vault_before = invariants::token_balance(&mut self.trident, f_token_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank);
        let share_snap_before =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank);

        let deposit_ix = types::marginfi::JuplendDepositInstruction::data(
            types::marginfi::JuplendDepositInstructionData::new(amount),
        )
        .accounts(types::marginfi::JuplendDepositInstructionAccounts::new(
            self.marginfi_group,
            marginfi_account,
            user,
            bank,
            user_token_account,
            layout.liquidity_vault_authority,
            layout.liquidity_vault,
            mint,
            juplend_lending_state,
            juplend_f_token_mint,
            f_token_vault,
            lending_admin,
            supply_token_reserves_liquidity,
            lending_supply_position_on_liquidity,
            rate_model,
            vault,
            liquidity,
            rewards_rate_model,
            token_program,
        ))
        .instruction();

        let res = self.trident.process_transaction(&[deposit_ix], message);

        let user_after = invariants::token_balance(&mut self.trident, user_token_account);
        let liquidity_vault_after =
            invariants::token_balance(&mut self.trident, layout.liquidity_vault);
        let f_token_vault_after = invariants::token_balance(&mut self.trident, f_token_vault);
        let share_snap_after =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank);

        if res.is_success() {
            invariants::assert_juplend_deposit_success(
                amount,
                user_before,
                user_after,
                liquidity_vault_before,
                liquidity_vault_after,
                f_token_vault_before,
                f_token_vault_after,
            );
            invariants::assert_deposit_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                amount,
            );
            invariants::assert_balances_packed(&mut self.trident, marginfi_account);
        } else {
            invariants::assert_juplend_deposit_failure_balances_unchanged(
                amount,
                user_before,
                user_after,
                liquidity_vault_before,
                liquidity_vault_after,
                f_token_vault_before,
                f_token_vault_after,
            );
            invariant!(
                share_snap_after == share_snap_before,
                "juplend deposit failure: marginfi shares changed. before: {:?}, after: {:?}",
                share_snap_before,
                share_snap_after
            );
        }

        self.assert_liquidity_balance_snapshot_unchanged(&other_vaults_snap);
    }

    pub fn withdraw_from_juplend(
        &mut self,
        marginfi_account: Pubkey,
        user: Pubkey,
        user_destination_token_account: Pubkey,
        mint: Pubkey,
        juplend_lending_state: Pubkey,
        juplend_f_token_mint: Pubkey,
        lending_admin: Pubkey,
        supply_token_reserves_liquidity: Pubkey,
        lending_supply_position_on_liquidity: Pubkey,
        rate_model: Pubkey,
        vault: Pubkey,
        liquidity: Pubkey,
        rewards_rate_model: Pubkey,
        claim_account: Pubkey,
        amount: u64,
        withdraw_all: Option<bool>,
        message: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&mint);
        let token_program = *mint_data.owner();

        let bank =
            self.juplend_bank_address(self.marginfi_group, mint, constants::JUPITER_BANK_SEED);
        let layout = self.bank_layout(bank);

        let f_token_vault = self.juplend_f_token_vault_address(bank);

        let withdraw_intermediary_ata = initialize_associated_token_account(
            &mut self.trident,
            self.payer.pubkey(),
            mint,
            layout.liquidity_vault_authority,
            token_program,
        );

        let user_before =
            invariants::token_balance(&mut self.trident, user_destination_token_account);
        let withdraw_intermediary_before =
            invariants::token_balance(&mut self.trident, withdraw_intermediary_ata);
        let liquidity_vault_before =
            invariants::token_balance(&mut self.trident, layout.liquidity_vault);
        let f_token_vault_before = invariants::token_balance(&mut self.trident, f_token_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank);
        let share_snap_before =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank);

        let banks = self.get_marginfi_account_banks(marginfi_account, Some(bank));
        let remaining =
            self.remaining_accounts_for_bank_risk_and_t22_transfer(mint, token_program, banks);

        let withdraw_ix = types::marginfi::JuplendWithdrawInstruction::data(
            types::marginfi::JuplendWithdrawInstructionData::new(amount, withdraw_all),
        )
        .accounts(types::marginfi::JuplendWithdrawInstructionAccounts::new(
            self.marginfi_group,
            marginfi_account,
            user,
            bank,
            user_destination_token_account,
            layout.liquidity_vault_authority,
            mint,
            juplend_lending_state,
            juplend_f_token_mint,
            f_token_vault,
            withdraw_intermediary_ata,
            lending_admin,
            supply_token_reserves_liquidity,
            lending_supply_position_on_liquidity,
            rate_model,
            vault,
            claim_account,
            liquidity,
            rewards_rate_model,
            token_program,
        ))
        .remaining_accounts(remaining)
        .instruction();

        let res = self.trident.process_transaction(&[withdraw_ix], message);

        let user_after =
            invariants::token_balance(&mut self.trident, user_destination_token_account);
        let withdraw_intermediary_after =
            invariants::token_balance(&mut self.trident, withdraw_intermediary_ata);
        let liquidity_vault_after =
            invariants::token_balance(&mut self.trident, layout.liquidity_vault);
        let f_token_vault_after = invariants::token_balance(&mut self.trident, f_token_vault);
        let share_snap_after =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank);

        let withdraw_all_flag = withdraw_all.unwrap_or(false);
        let share_amount_for_invariant = if withdraw_all_flag { 1 } else { amount };

        if res.is_success() {
            invariants::assert_juplend_withdraw_success(
                amount,
                withdraw_all_flag,
                user_before,
                user_after,
                withdraw_intermediary_before,
                withdraw_intermediary_after,
                liquidity_vault_before,
                liquidity_vault_after,
                f_token_vault_before,
                f_token_vault_after,
            );
            invariants::assert_withdraw_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                share_amount_for_invariant,
            );
            invariants::assert_balances_packed(&mut self.trident, marginfi_account);
        } else {
            invariants::assert_juplend_withdraw_failure_balances_unchanged(
                amount,
                user_before,
                user_after,
                withdraw_intermediary_before,
                withdraw_intermediary_after,
                liquidity_vault_before,
                liquidity_vault_after,
                f_token_vault_before,
                f_token_vault_after,
            );
            invariant!(
                share_snap_after == share_snap_before,
                "juplend withdraw failure: marginfi shares changed. before: {:?}, after: {:?}",
                share_snap_before,
                share_snap_after
            );
        }

        self.assert_liquidity_balance_snapshot_unchanged(&other_vaults_snap);
    }
}
