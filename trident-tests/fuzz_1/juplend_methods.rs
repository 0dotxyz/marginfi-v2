#![allow(clippy::too_many_arguments)]

use trident_fuzz::fuzzing::*;

use crate::constants;
use crate::types;
use crate::FuzzTest;

use crate::types::marginfi::JuplendConfigCompact;
use crate::types::marginfi::OracleSetup;
use crate::types::marginfi::RiskTier;
use crate::types::marginfi::WrappedI80F48;

use fixed_macro::types::I80F48;

const JUPLEND_F_TOKEN_VAULT_SEED: &[u8] = b"f_token_vault";

fn wrap_one() -> WrappedI80F48 {
    WrappedI80F48::new(I80F48!(1.0).to_bits().to_le_bytes())
}

impl FuzzTest {
    // ============================================================================================
    // PDA helpers (mirrors on-chain seeds)
    // ============================================================================================

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

    // ============================================================================================
    // Config helper
    // ============================================================================================

    pub fn default_juplend_bank_config(&mut self, oracle: Pubkey) -> JuplendConfigCompact {
        JuplendConfigCompact::new(
            oracle,
            wrap_one(),
            wrap_one(),
            10_000_000_000_000, // deposit_limit
            OracleSetup::JuplendPythPull,
            RiskTier::Collateral,
            constants::PYTH_PULL_MIGRATED_CONFIG_FLAGS,
            1_000_000_000_000, // total_asset_value_init_limit
            300,               // oracle_max_age
            0,                 // oracle_max_confidence
        )
    }

    // ============================================================================================
    // Juplend integration templates
    // ============================================================================================

    /// Create a Juplend bank in the marginfi group.
    ///
    /// You provide:
    /// - the underlying mint (e.g. USDC)
    /// - the Juplend `Lending` state (`integration_acc_1`)
    /// - the Juplend fToken mint (from the lending state)
    /// - oracle accounts required by your chosen `OracleSetup` (remaining accounts)
    ///
    /// After this, you must run:
    /// - `juplend_init_position` (seed deposit; flips bank from Paused -> Operational)
    /// - create the withdraw intermediary ATA (`integration_acc_3`) via ATA program (we provide a helper)
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
                constants::LIQUIDITY_PROGRAM,
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
            constants::LIQUIDITY_PROGRAM,
            rewards_rate_model,
            token_program,
        ))
        .instruction();

        let res = self.trident.process_transaction(&[deposit_ix], message);
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

        let withdraw_intermediary_ata = self.initialize_associated_token_account(
            self.payer.pubkey(),
            mint,
            layout.liquidity_vault_authority,
            token_program,
        );

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
            constants::LIQUIDITY_PROGRAM,
            rewards_rate_model,
            token_program,
        ))
        .remaining_accounts(remaining)
        .instruction();

        let res = self.trident.process_transaction(&[withdraw_ix], message);
    }
}
