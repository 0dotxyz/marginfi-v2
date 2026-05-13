use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::constants::*;
use crate::types;
use crate::FuzzTest;
use crate::FuzzTestBank;

use crate::types::marginfi::Bank;
use crate::types::marginfi::BankConfigCompact;
use crate::types::marginfi::BankOperationalState;
use crate::types::marginfi::InterestRateConfigCompact;

use crate::types::marginfi::MarginfiAccountInitLiqRecordInstruction;
use crate::types::marginfi::MarginfiAccountInitLiqRecordInstructionAccounts;
use crate::types::marginfi::MarginfiAccountInitLiqRecordInstructionData;
use crate::types::marginfi::OracleSetup;
use crate::types::marginfi::RatePoint;
use crate::types::marginfi::RiskTier;
use crate::types::marginfi::WrappedI80F48;

fn wrap_i80f48(value: I80F48) -> WrappedI80F48 {
    WrappedI80F48::new(value.to_bits().to_le_bytes())
}

#[derive(Clone, Copy)]
pub struct BankLayout {
    pub liquidity_vault_authority: Pubkey,
    pub liquidity_vault: Pubkey,
    pub insurance_vault_authority: Pubkey,
    pub insurance_vault: Pubkey,
    pub fee_vault_authority: Pubkey,
    pub fee_vault: Pubkey,
}

impl FuzzTest {
    pub fn init_foundation(&mut self) {
        self.trident
            .airdrop(&self.payer.pubkey(), 500 * LAMPORTS_PER_SOL);

        self.init_token_accounts();

        self.init_global_fee_state(self.payer.pubkey(), self.fee_state, None);

        self.init_marginfi_group(
            self.payer.pubkey(),
            self.marginfi_group,
            self.fee_state,
            None,
        );

        self.init_marginfi_account(
            self.marginfi_group,
            self.user_a.marginfi_account,
            self.user_a.address,
            None,
        );

        self.init_marginfi_account(
            self.marginfi_group,
            self.seeder.marginfi_account,
            self.seeder.address,
            None,
        );

        self.init_marginfi_account(
            self.marginfi_group,
            self.user_b.marginfi_account,
            self.user_b.address,
            None,
        );

        self.init_marginfi_account(
            self.marginfi_group,
            self.liquidator.marginfi_account,
            self.liquidator.address,
            None,
        );

        self.marginfi_account_init_liquidation_record(
            self.user_a.marginfi_account,
            self.payer.pubkey(),
            None,
        );

        self.init_bank(
            self.payer.pubkey(),
            self.usdc_bank,
            Self::usdc_bank_config(),
            self.marginfi_group,
            self.fee_state,
            None,
        );

        self.init_bank(
            self.payer.pubkey(),
            self.eth_bank,
            Self::eth_bank_config(),
            self.marginfi_group,
            self.fee_state,
            None,
        );

        self.init_bank(
            self.payer.pubkey(),
            self.btc_bank,
            Self::btc_bank_config(),
            self.marginfi_group,
            self.fee_state,
            None,
        );

        self.update_bank_oracle(
            self.usdc_bank,
            self.marginfi_group,
            self.payer.pubkey(),
            None,
        );

        self.update_bank_oracle(
            self.eth_bank,
            self.marginfi_group,
            self.payer.pubkey(),
            None,
        );

        self.update_bank_oracle(
            self.btc_bank,
            self.marginfi_group,
            self.payer.pubkey(),
            None,
        );
    }

    pub fn init_global_fee_state(&mut self, payer: Pubkey, fee_state: Pubkey, msg: Option<&str>) {
        let ix = types::marginfi::InitGlobalFeeStateInstruction::data(
            types::marginfi::InitGlobalFeeStateInstructionData::new(
                payer,
                payer,
                0u32,
                0u32,
                0u32,
                wrap_i80f48(I80F48!(0)),
                wrap_i80f48(I80F48!(0)),
                wrap_i80f48(I80F48!(0)),
                wrap_i80f48(I80F48!(0)),
            ),
        )
        .accounts(types::marginfi::InitGlobalFeeStateInstructionAccounts::new(
            payer, fee_state,
        ))
        .instruction();

        let res = self.trident.process_transaction(&[ix], msg);
        invariant!(res.is_success());
    }

    pub fn init_marginfi_group(
        &mut self,
        payer: Pubkey,
        marginfi_group: Pubkey,
        fee_state: Pubkey,
        msg: Option<&str>,
    ) {
        let ix = types::marginfi::MarginfiGroupInitializeInstruction::data(
            types::marginfi::MarginfiGroupInitializeInstructionData::new(),
        )
        .accounts(
            types::marginfi::MarginfiGroupInitializeInstructionAccounts::new(
                marginfi_group,
                payer,
                fee_state,
            ),
        )
        .instruction();
        let res = self.trident.process_transaction(&[ix], msg);
        invariant!(res.is_success());
    }

    pub fn init_marginfi_account(
        &mut self,
        marginfi_group: Pubkey,
        marginfi_account: Pubkey,
        authority: Pubkey,
        msg: Option<&str>,
    ) {
        let ix = types::marginfi::MarginfiAccountInitializeInstruction::data(
            types::marginfi::MarginfiAccountInitializeInstructionData::new(),
        )
        .accounts(
            types::marginfi::MarginfiAccountInitializeInstructionAccounts::new(
                marginfi_group,
                marginfi_account,
                authority,
                authority,
            ),
        )
        .instruction();
        let res = self.trident.process_transaction(&[ix], msg);

        invariant!(res.is_success());
    }

    /// Permissionless: create the liquidation record PDA for a marginfi account (required before receivership liquidation).
    pub fn marginfi_account_init_liquidation_record(
        &mut self,
        marginfi_account: Pubkey,
        fee_payer: Pubkey,
        msg: Option<&str>,
    ) {
        let record = self.liquidation_record_pda(marginfi_account);
        let ix = MarginfiAccountInitLiqRecordInstruction::data(
            MarginfiAccountInitLiqRecordInstructionData::new(),
        )
        .accounts(MarginfiAccountInitLiqRecordInstructionAccounts::new(
            marginfi_account,
            fee_payer,
            record,
        ))
        .instruction();
        let res = self.trident.process_transaction(&[ix], msg);
        invariant!(res.is_success());
    }

    /// Risk/oracle tail for deposit/withdraw/borrow/repay. Must match `maybe_take_bank_mint` in the
    /// program: **prepend the bank mint only for Token-2022** (it is consumed before health). For
    /// classic SPL (`Tokenkeg`), do not prepend — otherwise the first “bank” slot is the mint and
    /// health loads it as `Bank` → `AccountOwnedByWrongProgram`.
    pub fn remaining_accounts_for_bank_risk_and_t22_transfer(
        &mut self,
        bank_mint: Pubkey,
        token_program: Pubkey,
        banks: Vec<Pubkey>,
    ) -> Vec<AccountMeta> {
        let mut remaining_accounts = Vec::new();
        if token_program == TOKEN_2022_PROGRAM_ID {
            remaining_accounts.push(AccountMeta::new_readonly(bank_mint, false));
        }

        for bank_pk in banks {
            remaining_accounts.push(AccountMeta::new_readonly(bank_pk, false));

            let bank = self
                .trident
                .get_account_with_type::<Bank>(&bank_pk, None)
                .expect("bank must exist");

            for extra_pk in Self::risk_accounts_for_bank(&bank) {
                remaining_accounts.push(AccountMeta::new_readonly(extra_pk, false));
            }
        }
        remaining_accounts
    }

    /// Remaining accounts for `check_account_init_health` / end-flashloan (bank + risk/oracle per
    /// active balance). No leading mint — unlike SPL deposit/withdraw/borrow/repay.
    /// Remaining accounts for `LendingAccountLiquidate`:
    /// 1. Token-2022 **liability** mint (consumed by `maybe_take_bank_mint`)
    /// 2. **Primary oracle groups** for `asset_bank` then `liab_bank` (same order as
    ///    `programs/marginfi/fuzz` — `liquidate.rs` indexes the liab oracle at
    ///    `remaining[asset_len..asset_len+liab_len]` where each len is
    ///    `get_remaining_accounts_per_bank - 1`)
    /// 3. Bank+oracle groups for the liquidator: existing active banks **plus** this ix’s
    ///    `liab_bank` and `asset_bank` (the liquidator receives asset + liability here; health
    ///    runs with an empty slice if these are omitted → `InvalidBankAccount`).
    /// 4. Bank+oracle groups for the liquidatee (suffix counted by `liquidatee_accounts`)
    pub fn remaining_accounts_for_liquidation(
        &mut self,
        asset_bank: Pubkey,
        liab_bank: Pubkey,
        liquidator_marginfi_account: Pubkey,
        liquidatee_marginfi_account: Pubkey,
    ) -> (Vec<AccountMeta>, u8, u8) {
        let liab_bank_state = self
            .trident
            .get_account_with_type::<Bank>(&liab_bank, None)
            .expect("liab bank");
        let mut metas = vec![AccountMeta::new_readonly(liab_bank_state.mint, false)];

        let asset_bank_state = self
            .trident
            .get_account_with_type::<Bank>(&asset_bank, None)
            .expect("asset bank");
        for pk in Self::risk_accounts_for_bank(&asset_bank_state) {
            metas.push(AccountMeta::new_readonly(pk, false));
        }
        for pk in Self::risk_accounts_for_bank(&liab_bank_state) {
            metas.push(AccountMeta::new_readonly(pk, false));
        }

        let mut liquidator_banks =
            self.get_marginfi_account_banks(liquidator_marginfi_account, None);
        for pk in [liab_bank, asset_bank] {
            if !liquidator_banks.contains(&pk) {
                liquidator_banks.push(pk);
            }
        }
        liquidator_banks.sort_by(|a, b| b.cmp(a));

        let liq_start = metas.len();
        for bank_pk in &liquidator_banks {
            metas.push(AccountMeta::new_readonly(*bank_pk, false));
            let bank = self
                .trident
                .get_account_with_type::<Bank>(bank_pk, None)
                .expect("bank must exist");
            for extra_pk in Self::risk_accounts_for_bank(&bank) {
                metas.push(AccountMeta::new_readonly(extra_pk, false));
            }
        }
        let liquidator_accounts = (metas.len() - liq_start) as u8;

        let liquidatee_banks = self.get_marginfi_account_banks(liquidatee_marginfi_account, None);
        let le_start = metas.len();
        for bank_pk in &liquidatee_banks {
            metas.push(AccountMeta::new_readonly(*bank_pk, false));
            let bank = self
                .trident
                .get_account_with_type::<Bank>(bank_pk, None)
                .expect("bank must exist");
            for extra_pk in Self::risk_accounts_for_bank(&bank) {
                metas.push(AccountMeta::new_readonly(extra_pk, false));
            }
        }
        let liquidatee_accounts = (metas.len() - le_start) as u8;

        (metas, liquidatee_accounts, liquidator_accounts)
    }

    pub fn remaining_accounts_for_bank_risk_only(
        &mut self,
        banks: Vec<Pubkey>,
    ) -> Vec<AccountMeta> {
        let mut remaining_accounts = Vec::new();
        for bank_pk in banks {
            remaining_accounts.push(AccountMeta::new(bank_pk, false));
            let bank = self
                .trident
                .get_account_with_type::<Bank>(&bank_pk, None)
                .expect("bank must exist");
            for extra_pk in Self::risk_accounts_for_bank(&bank) {
                remaining_accounts.push(AccountMeta::new(extra_pk, false));
            }
        }
        remaining_accounts
    }

    /// Remaining accounts for `EndLiquidation` / cached health: **one account per active bank only**.
    /// `get_health_components` with `HealthPriceMode::Cached` advances by one slot per balance; oracles
    /// must not appear between banks (unlike `StartLiquidation`, which uses `Live` and bank+oracle groups).
    pub fn remaining_accounts_for_bank_risk_banks_only(
        &mut self,
        banks: Vec<Pubkey>,
    ) -> Vec<AccountMeta> {
        banks
            .into_iter()
            .map(|bank_pk| AccountMeta::new(bank_pk, false))
            .collect()
    }

    fn risk_accounts_for_bank(bank: &Bank) -> Vec<Pubkey> {
        // These are the "extra" risk/oracle accounts required *after* the bank account itself.
        //
        // This must match what the on-chain risk engine expects, which is ultimately driven by
        // `bank.config.oracle_setup` and the `bank.config.oracle_keys[]` slots (see
        // `OraclePriceFeedAdapter::validate_bank_config` and `get_remaining_accounts_per_bank`).
        match bank.config.oracle_setup {
            // Fixed: bank only
            OracleSetup::Fixed => vec![],

            // Standard oracles: bank + oracle
            OracleSetup::PythPushOracle | OracleSetup::SwitchboardPull => {
                vec![bank.config.oracle_keys[0]]
            }

            // Staked: bank + oracle + (lst_mint) + (sol_pool)
            OracleSetup::StakedWithPythPush => vec![
                bank.config.oracle_keys[0],
                bank.config.oracle_keys[1],
                bank.config.oracle_keys[2],
            ],

            // Integrations (bank + oracle + reserve/spot-market/lending-state)
            OracleSetup::KaminoPythPush
            | OracleSetup::KaminoSwitchboardPull
            | OracleSetup::DriftPythPull
            | OracleSetup::DriftSwitchboardPull
            | OracleSetup::SolendPythPull
            | OracleSetup::SolendSwitchboardPull
            | OracleSetup::JuplendPythPull
            | OracleSetup::JuplendSwitchboardPull => {
                vec![bank.config.oracle_keys[0], bank.config.oracle_keys[1]]
            }

            // Fixed integrations (bank + integration account; no oracle)
            OracleSetup::FixedKamino | OracleSetup::FixedDrift | OracleSetup::FixedJuplend => {
                vec![bank.config.oracle_keys[1]]
            }

            // Deprecated or unset: treat as bank + primary oracle key (best-effort).
            OracleSetup::None | OracleSetup::PythLegacy | OracleSetup::SwitchboardV2 => {
                vec![bank.config.oracle_keys[0]]
            }
        }
    }

    pub fn bank_layout(&mut self, bank: Pubkey) -> BankLayout {
        BankLayout {
            liquidity_vault_authority: self
                .trident
                .find_program_address(
                    &[LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(), bank.as_ref()],
                    &types::marginfi::program_id(),
                )
                .0,
            liquidity_vault: self
                .trident
                .find_program_address(
                    &[LIQUIDITY_VAULT_SEED.as_bytes(), bank.as_ref()],
                    &types::marginfi::program_id(),
                )
                .0,
            insurance_vault_authority: self
                .trident
                .find_program_address(
                    &[INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(), bank.as_ref()],
                    &types::marginfi::program_id(),
                )
                .0,
            insurance_vault: self
                .trident
                .find_program_address(
                    &[INSURANCE_VAULT_SEED.as_bytes(), bank.as_ref()],
                    &types::marginfi::program_id(),
                )
                .0,
            fee_vault_authority: self
                .trident
                .find_program_address(
                    &[FEE_VAULT_AUTHORITY_SEED.as_bytes(), bank.as_ref()],
                    &types::marginfi::program_id(),
                )
                .0,
            fee_vault: self
                .trident
                .find_program_address(
                    &[FEE_VAULT_SEED.as_bytes(), bank.as_ref()],
                    &types::marginfi::program_id(),
                )
                .0,
        }
    }

    fn usdc_bank_config() -> BankConfigCompact {
        BankConfigCompact::new(
            // https://solscan.io/account/2s37akK2eyBbp8DZgCm7RtsaEz8eJP3Nxd4urLHQv7yB#accountData
            wrap_i80f48(I80F48!(1.0)),
            wrap_i80f48(I80F48!(1.0)),
            // Mainnet USDC-style liability weights (~1.1 init, ~1.05 maint).
            wrap_i80f48(I80F48!(1.1)),
            wrap_i80f48(I80F48!(1.05)),
            u64::MAX / 4,
            InterestRateConfigCompact::new(
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                0,
                0,
                [RatePoint::new(0, 0); 5],
            ),
            BankOperationalState::Operational,
            200_000_000_000_000,
            RiskTier::Collateral,
            0,
            1,
            [0; 5],
            200_000_000,
            300,
            0,
        )
    }

    fn eth_bank_config() -> BankConfigCompact {
        BankConfigCompact::new(
            // https://solscan.io/account/BkUyfXjbBBALcfZvw76WAFRvYQ21xxMWWeoPtJrUqG3z#accountData
            wrap_i80f48(I80F48!(0.5)),
            wrap_i80f48(I80F48!(0.65)),
            wrap_i80f48(I80F48!(1.85)),
            wrap_i80f48(I80F48!(1.6)),
            u64::MAX / 4,
            InterestRateConfigCompact::new(
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                0,
                0,
                [RatePoint::new(0, 0); 5],
            ),
            BankOperationalState::Operational,
            u64::MAX / 4,
            RiskTier::Collateral,
            0,
            0,
            [0; 5],
            u64::MAX / 4,
            300,
            0,
        )
    }

    fn btc_bank_config() -> BankConfigCompact {
        BankConfigCompact::new(
            // Mainnet-style BTC weights from WrappedI80F48 LE bytes:
            // asset init 00..80.. (0.5), asset maint 66..a6.. (~0.65),
            // liab init 9a..d9 01 (~1.85), liab maint 9a..99 01 (~1.6).
            // https://solscan.io/account/BKsfDJCMbYep6gr9pq8PsmJbb5XGLHbAJzUV8vmorz7a#accountData
            wrap_i80f48(I80F48!(0.5)),
            wrap_i80f48(I80F48!(0.65)),
            wrap_i80f48(I80F48!(1.85)),
            wrap_i80f48(I80F48!(1.6)),
            u64::MAX / 4,
            InterestRateConfigCompact::new(
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                0,
                0,
                [RatePoint::new(0, 0); 5],
            ),
            BankOperationalState::Operational,
            u64::MAX / 4,
            RiskTier::Collateral,
            0,
            0,
            [0; 5],
            u64::MAX / 4,
            300,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn init_bank(
        &mut self,
        payer: Pubkey,
        bank: FuzzTestBank,
        bank_config: BankConfigCompact,
        marginfi_group: Pubkey,
        fee_state: Pubkey,
        msg: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&bank.mint);

        let layout = self.bank_layout(bank.address);

        let ix = types::marginfi::LendingPoolAddBankInstruction::data(
            types::marginfi::LendingPoolAddBankInstructionData::new(bank_config),
        )
        .accounts(types::marginfi::LendingPoolAddBankInstructionAccounts::new(
            marginfi_group,
            payer,
            payer,
            fee_state,
            payer,
            bank.mint,
            bank.address,
            layout.liquidity_vault_authority,
            layout.liquidity_vault,
            layout.insurance_vault_authority,
            layout.insurance_vault,
            layout.fee_vault_authority,
            layout.fee_vault,
            *mint_data.owner(),
        ))
        .instruction();

        let res = self.trident.process_transaction(&[ix], msg);
        invariant!(res.is_success());
    }

    pub fn update_bank_oracle(
        &mut self,
        bank: FuzzTestBank,
        marginfi_group: Pubkey,
        admin: Pubkey,
        msg: Option<&str>,
    ) {
        let remaining_accounts = vec![AccountMeta::new_readonly(bank.oracle_setup.1, false)];
        let ix = types::marginfi::LendingPoolConfigureBankOracleInstruction::data(
            types::marginfi::LendingPoolConfigureBankOracleInstructionData::new(
                bank.oracle_setup.0 as u8,
                bank.oracle_setup.1,
            ),
        )
        .accounts(
            types::marginfi::LendingPoolConfigureBankOracleInstructionAccounts::new(
                marginfi_group,
                admin,
                bank.address,
            ),
        )
        .remaining_accounts(remaining_accounts)
        .instruction();

        let res = self.trident.process_transaction(&[ix], msg);
        invariant!(res.is_success());
    }

    pub fn init_token_accounts(&mut self) {
        self.initialize_mint(
            self.payer.pubkey(),
            self.usdc_bank.mint,
            6,
            self.usdc_bank.mint_authority,
        );
        self.initialize_mint_2022(
            self.payer.pubkey(),
            self.eth_bank.mint,
            6,
            self.eth_bank.mint_authority,
        );
        self.initialize_mint_2022(
            self.payer.pubkey(),
            self.btc_bank.mint,
            8,
            self.btc_bank.mint_authority,
        );

        self.init_token_accounts_and_mint_to();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn init_token_accounts_and_mint_to(&mut self) {
        for user in &[self.user_a, self.user_b, self.seeder, self.liquidator] {
            self.trident.airdrop(&user.address, 500 * LAMPORTS_PER_SOL);

            self.initialize_token_account(
                user.address,
                user.usdc_token_account,
                self.usdc_bank.mint,
                user.address,
            );

            self.mint_to(
                user.usdc_token_account,
                self.usdc_bank.mint,
                self.usdc_bank.mint_authority,
                u64::MAX / 100,
            );
            self.initialize_token_account_2022(
                user.address,
                user.eth_token_account,
                self.eth_bank.mint,
                user.address,
                &[],
            );
            self.mint_to_2022(
                user.eth_token_account,
                self.eth_bank.mint,
                self.eth_bank.mint_authority,
                u64::MAX / 100,
            );
            self.initialize_token_account_2022(
                user.address,
                user.btc_token_account,
                self.btc_bank.mint,
                user.address,
                &[],
            );

            self.mint_to_2022(
                user.btc_token_account,
                self.btc_bank.mint,
                self.btc_bank.mint_authority,
                u64::MAX / 100,
            );
        }
    }
}
