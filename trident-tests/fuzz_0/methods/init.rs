use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use trident_fuzz::fuzzing::*;

use crate::constants::*;
use crate::solana_amount;
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
use crate::usdc_amount;
use crate::utils::initialize_token_account;
use crate::utils::mint_to;

/// Hardcoded transfer-fee parameters for the runtime-initialized T22 bank.
/// 1% basis points, 1B native-unit maximum fee — values small enough that
/// the seeder's bootstrap deposit (1e9 units below) reliably succeeds while
/// still exercising marginfi's `transfer_checked_with_fee` codepath.
const T22_TRANSFER_FEE_BPS: u16 = 100;
const T22_MAX_FEE: u64 = 1_000_000_000;
const T22_MINT_DECIMALS: u8 = 8;
const T22_SEEDER_DEPOSIT_AMOUNT: u64 = 1_000_000_000;

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
            .airdrop(&self.payer.pubkey(), solana_amount!(500));

        self.init_token_accounts();

        self.init_global_fee_state(self.payer.pubkey(), self.fee_state, None);

        self.init_marginfi_group(
            self.payer.pubkey(),
            self.marginfi_group,
            self.fee_state,
            None,
        );

        // ================================================================================================
        // Init Marginfi Accounts for Users
        for user in self
            .users
            .clone()
            .iter()
            .chain([&self.seeder.clone(), &self.liquidator.clone()])
        {
            // ================================================================================================
            // Init Marginfi Account for User
            self.init_marginfi_account(
                self.marginfi_group,
                user.marginfi_account,
                user.address,
                None,
            );

            // ================================================================================================
            // Init Liquidation Record for User
            self.marginfi_account_init_liquidation_record(
                user.marginfi_account,
                self.payer.pubkey(),
                None,
            );
        }

        // ================================================================================================
        // Init Bank for USDC
        self.init_bank(
            self.payer.pubkey(),
            self.usdc_bank,
            Self::usdc_bank_config(),
            self.marginfi_group,
            self.fee_state,
            None,
        );

        // ================================================================================================
        // Init Bank for WETH
        self.init_bank(
            self.payer.pubkey(),
            self.eth_bank,
            Self::eth_bank_config(),
            self.marginfi_group,
            self.fee_state,
            None,
        );

        // ================================================================================================
        // Init Bank for cbBTC
        self.init_bank(
            self.payer.pubkey(),
            self.btc_bank,
            Self::btc_bank_config(),
            self.marginfi_group,
            self.fee_state,
            None,
        );

        // ================================================================================================
        // Init Token-2022 + TransferFeeConfig bank, then seed a deposit so
        // every sequence exercises marginfi's T22-with-fee LendingAccountDeposit
        // codepath at least once.
        self.init_t22_with_fee_bank_and_seed();

        // ================================================================================================
        // Init the Isolated-risk-tier bank. Positions on this bank carry
        // `asset_weight = 0` and mixing with default positions trips
        // `IsolatedAccountIllegalState`. Per-user token accounts get
        // minted here; the dedicated `flow_isolated_deposit` exercises
        // them, and any random cross-bank op the harness happens to issue
        // afterwards drives the isolated-violation path.
        self.init_isolated_bank_and_mint();

        // ================================================================================================
        // Init Kamino Bank for USDC
        self.init_kamino_bank(
            self.payer.pubkey(),
            self.usdc_bank.currency.mint,
            self.kamino_main_lending_market,
            self.kamino_usdc_reserve,
            self.kamino_oracle,
            None,
        );

        // ================================================================================================
        // Init Juplend Bank for USDC
        self.init_juplend_bank(
            self.payer.pubkey(),
            self.usdc_bank.currency.mint,
            self.juplend_usdc_lending_state,
            self.juplend_usdc_f_token_mint,
            self.juplend_oracle,
            None,
        );

        // ================================================================================================
        // Init Kamino Obligation
        self.init_kamino_obligation(
            self.seeder.address,
            self.seeder.usdc_token_account,
            self.usdc_bank.currency.mint,
            self.kamino_main_lending_market,
            self.kamino_usdc_reserve,
            self.kamino_usdc_reserve_liquidity_supply,
            self.kamino_usdc_reserve_collateral_mint,
            self.kamino_usdc_reserve_collateral_supply_vault,
            self.kamino_usdc_reserve_farm_state,
            usdc_amount!(10_000),
            None,
        );

        // ================================================================================================
        // Init Juplend Position
        self.init_juplend_position(
            self.seeder.address,
            self.seeder.usdc_token_account,
            self.usdc_bank.currency.mint,
            self.juplend_usdc_lending_state,
            self.juplend_usdc_f_token_mint,
            self.juplend_lending_state_admin,
            self.juplend_usdc_supply_token_reserves_liquidity,
            self.juplend_usdc_lending_supply_position_on_liquidity,
            self.juplend_usdc_rate_model,
            self.juplend_usdc_vault,
            self.juplend_usdc_liquidity,
            self.juplend_usdc_rewards_rate_model,
            usdc_amount!(10_000),
            None,
        );

        // ================================================================================================
        // Update Bank Oracle for USDC
        self.update_bank_oracle(
            self.usdc_bank,
            self.marginfi_group,
            self.payer.pubkey(),
            None,
        );

        // ================================================================================================
        // Update Bank Oracle for WETH
        self.update_bank_oracle(
            self.eth_bank,
            self.marginfi_group,
            self.payer.pubkey(),
            None,
        );

        // ================================================================================================
        // Update Bank Oracle for cbBTC
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

    pub fn remaining_accounts_for_bank_risk_and_t22_transfer(
        &mut self,
        bank_mint: Pubkey,
        token_program: Pubkey,
        banks: Vec<Pubkey>,
    ) -> Vec<AccountMeta> {
        let mut remaining_accounts = Vec::new();
        if token_program == SPL_TOKEN_2022_ID {
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
        let liab_mint_data = self.trident.get_account(&liab_bank_state.mint);
        let liab_token_program = *liab_mint_data.owner();

        // Liquidation uses the liab bank's token program for SPL transfers.
        // For Token-2022, the mint must be the first remaining account (consumed by `maybe_take_bank_mint`).
        let mut metas = Vec::new();
        if liab_token_program == SPL_TOKEN_2022_ID {
            metas.push(AccountMeta::new_readonly(liab_bank_state.mint, false));
        }

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
        match bank.config.oracle_setup {
            OracleSetup::Fixed => vec![],
            OracleSetup::PythPushOracle | OracleSetup::SwitchboardPull => {
                vec![bank.config.oracle_keys[0]]
            }

            OracleSetup::StakedWithPythPush => vec![
                bank.config.oracle_keys[0],
                bank.config.oracle_keys[1],
                bank.config.oracle_keys[2],
            ],

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

            OracleSetup::FixedKamino | OracleSetup::FixedDrift | OracleSetup::FixedJuplend => {
                vec![bank.config.oracle_keys[1]]
            }

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

    /// Isolated-tier bank: `asset_weight = 0` (positions contribute zero
    /// collateral value) and `RiskTier::Isolated` (an account holding a
    /// position on this bank can hold at most one liability). Mixing
    /// isolated + default positions trips `IsolatedAccountIllegalState`.
    fn isolated_bank_config() -> BankConfigCompact {
        BankConfigCompact::new(
            wrap_i80f48(I80F48!(0.0)),  // asset_weight_init = 0
            wrap_i80f48(I80F48!(0.0)),  // asset_weight_maint = 0
            wrap_i80f48(I80F48!(1.5)),  // liability_weight_init
            wrap_i80f48(I80F48!(1.25)), // liability_weight_maint
            u64::MAX / 4,
            InterestRateConfigCompact::new(
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                wrap_i80f48(I80F48!(0.0)),
                0,
                0,
                // asset_weight = 0 on isolated banks means accrual semantics
                // don't really matter; keep the curve flat.
                core::array::from_fn(|_| RatePoint::new(0, 0)),
            ),
            BankOperationalState::Operational,
            u64::MAX / 4,
            RiskTier::Isolated,
            0,
            0,
            [0; 5],
            u64::MAX / 4,
            300,
            0,
        )
    }

    fn usdc_bank_config() -> BankConfigCompact {
        // Mainnet USDC rate curve points (verbatim from upstream
        // marginfi-v2-fuzz-tests / 0.1.9 update).
        let points = [
            RatePoint::new(2_147_483_648, 8_589_935),
            RatePoint::new(3_435_973_836, 17_179_869),
            RatePoint::new(3_865_470_566, 21_474_836),
            RatePoint::new(4_080_218_930, 32_212_255),
            RatePoint::new(4_209_067_949, 42_949_673),
        ];
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
                64_424_509,
                points,
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
        // Mainnet ETH rate curve (verbatim from upstream 0.1.9 update).
        let points = [
            RatePoint::new(3_435_973_836, 42_949_672),
            RatePoint::new(0, 0),
            RatePoint::new(0, 0),
            RatePoint::new(0, 0),
            RatePoint::new(0, 0),
        ];
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
                1_288_490_188,
                points,
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
        // Mainnet BTC rate curve (verbatim from upstream 0.1.9 update —
        // same shape as ETH).
        let points = [
            RatePoint::new(3_435_973_836, 42_949_672),
            RatePoint::new(0, 0),
            RatePoint::new(0, 0),
            RatePoint::new(0, 0),
            RatePoint::new(0, 0),
        ];
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
                1_288_490_188,
                points,
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
        let mint_data = self.trident.get_account(&bank.currency.mint);

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
            bank.currency.mint,
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
        // self.initialize_mint(
        //     self.payer.pubkey(),
        //     self.usdc_bank.currency.mint,
        //     self.usdc_bank.currency.decimals,
        //     self.usdc_bank.currency.mint_authority,
        // );
        // self.initialize_mint_2022(
        //     self.payer.pubkey(),
        //     self.eth_bank.currency.mint,
        //     self.eth_bank.currency.decimals,
        //     self.eth_bank.currency.mint_authority,
        // );
        // self.initialize_mint_2022(
        //     self.payer.pubkey(),
        //     self.btc_bank.currency.mint,
        //     self.btc_bank.currency.decimals,
        //     self.btc_bank.currency.mint_authority,
        // );

        self.init_token_accounts_and_mint_to();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn init_token_accounts_and_mint_to(&mut self) {
        self.users
            .iter()
            .chain([&self.seeder, &self.liquidator])
            .for_each(|user| {
                self.trident.airdrop(&user.address, solana_amount!(500));

                initialize_token_account(
                    &mut self.trident,
                    user.address,
                    user.usdc_token_account,
                    self.usdc_bank.currency.mint,
                    user.address,
                );

                mint_to(
                    &mut self.trident,
                    user.usdc_token_account,
                    self.usdc_bank.currency.mint,
                    self.usdc_bank.currency.mint_authority,
                    user.initial_usdc_amount,
                );
                initialize_token_account(
                    &mut self.trident,
                    user.address,
                    user.eth_token_account,
                    self.eth_bank.currency.mint,
                    user.address,
                );
                mint_to(
                    &mut self.trident,
                    user.eth_token_account,
                    self.eth_bank.currency.mint,
                    self.eth_bank.currency.mint_authority,
                    user.initial_eth_amount,
                );
                initialize_token_account(
                    &mut self.trident,
                    user.address,
                    user.btc_token_account,
                    self.btc_bank.currency.mint,
                    user.address,
                );

                mint_to(
                    &mut self.trident,
                    user.btc_token_account,
                    self.btc_bank.currency.mint,
                    self.btc_bank.currency.mint_authority,
                    user.initial_btc_amount,
                );
            });
    }

    /// Bring up the Token-2022 + TransferFeeConfig bank: initialize the
    /// mint with the extension, mint the seeder's stake, init the bank,
    /// then run a single `LendingAccountDeposit` so every fuzz sequence
    /// exercises marginfi's `transfer_checked_with_fee` codepath through
    /// the deposit ix.
    pub fn init_t22_with_fee_bank_and_seed(&mut self) {
        let payer = self.payer.pubkey();
        let mint = self.t22_bank.currency.mint;
        let mint_authority = self.t22_mint_authority;

        // 1) Initialize the T22 mint with TransferFeeConfig.
        let mint_ixs = self.trident.initialize_mint_2022(
            &payer,
            &mint,
            T22_MINT_DECIMALS,
            &mint_authority,
            None,
            &[MintExtension::TransferFeeConfig {
                transfer_fee_config_authority: Some(payer),
                withdraw_withheld_authority: Some(payer),
                transfer_fee_basis_points: T22_TRANSFER_FEE_BPS,
                maximum_fee: T22_MAX_FEE,
            }],
        );
        let res = self.trident.process_transaction(&mint_ixs, None);
        invariant!(res.is_success());

        // 2) Initialize every actor's T22 token account (seeder + users +
        //    liquidator) and mint each their initial T22 balance. The seeder
        //    uses these in step 4 below; the 4 users use them in
        //    `flow_t22_deposit` / `flow_t22_withdraw`.
        for actor in self
            .users
            .clone()
            .iter()
            .chain([&self.seeder.clone(), &self.liquidator.clone()])
        {
            let acc_ixs = self.trident.initialize_token_account_2022(
                &payer,
                &actor.t22_token_account,
                &mint,
                &actor.address,
                &[],
            );
            let res = self.trident.process_transaction(&acc_ixs, None);
            invariant!(res.is_success());

            let mint_to_ix = self.trident.mint_to_2022(
                &actor.t22_token_account,
                &mint,
                &mint_authority,
                actor.initial_t22_amount,
            );
            let res = self.trident.process_transaction(&[mint_to_ix], None);
            invariant!(res.is_success());
        }

        // 3) Init the marginfi bank backed by the new T22 mint.
        // Reuse `usdc_bank_config()`: weights/limits aren't the point here,
        // we just need a bank we can deposit into through the standard ix.
        self.init_bank(
            payer,
            self.t22_bank,
            Self::usdc_bank_config(),
            self.marginfi_group,
            self.fee_state,
            Some("Init T22-with-fee bank"),
        );

        // 4) Seeder deposit — drives the T22 transfer-fee codepath in
        //    `LendingAccountDeposit`. `has_transfer_fee = true` on the bank
        //    tells `lending_account_deposit` to relax exact-amount /
        //    conservation invariants for this tx.
        self.lending_account_deposit(
            T22_SEEDER_DEPOSIT_AMOUNT,
            self.t22_bank,
            self.seeder.t22_token_account,
            self.seeder.marginfi_account,
            self.seeder.address,
            Some("Seeder deposit — T22 transfer-fee bank"),
        );
    }

    /// Bring up the Isolated-risk-tier bank: initialize the classic SPL
    /// Token mint, init per-actor token accounts, mint each their initial
    /// balance, and init the marginfi bank with `RiskTier::Isolated` and
    /// `asset_weight = 0`. No automatic deposit — `flow_isolated_deposit`
    /// drives the codepath at fuzz time. Cross-bank interactions with
    /// other banks trigger `IsolatedAccountIllegalState` naturally.
    pub fn init_isolated_bank_and_mint(&mut self) {
        let payer = self.payer.pubkey();
        let mint = self.isolated_bank.currency.mint;
        let mint_authority = self.isolated_mint_authority;

        // 1) Initialize the classic SPL Token mint.
        crate::utils::initialize_mint(
            &mut self.trident,
            payer,
            mint,
            8,
            mint_authority,
        );

        // 2) Per-actor token account + initial mint.
        for actor in self
            .users
            .clone()
            .iter()
            .chain([&self.seeder.clone(), &self.liquidator.clone()])
        {
            crate::utils::initialize_token_account(
                &mut self.trident,
                payer,
                actor.isolated_token_account,
                mint,
                actor.address,
            );
            crate::utils::mint_to(
                &mut self.trident,
                actor.isolated_token_account,
                mint,
                mint_authority,
                actor.initial_isolated_amount,
            );
        }

        // 3) Init the marginfi bank with the isolated config.
        self.init_bank(
            payer,
            self.isolated_bank,
            Self::isolated_bank_config(),
            self.marginfi_group,
            self.fee_state,
            Some("Init isolated-tier bank"),
        );
    }
}
