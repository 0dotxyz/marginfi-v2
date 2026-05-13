use fuzz_accounts::*;
use trident_fuzz::fuzzing::*;
mod fuzz_accounts;
mod types;
use types::*;

use fixed::types::I80F48;
use fixed_macro::types::I80F48;

use crate::types::marginfi::BankOperationalState;
use crate::types::marginfi::InterestRateConfigCompact;
use crate::types::marginfi::RatePoint;
use crate::types::marginfi::RiskTier;
use crate::types::marginfi::WrappedI80F48;

pub const FEE_STATE_SEED: &str = "feestate";
pub const LIQUIDITY_VAULT_AUTHORITY_SEED: &str = "liquidity_vault_auth";
pub const LIQUIDITY_VAULT_SEED: &str = "liquidity_vault";
pub const INSURANCE_VAULT_AUTHORITY_SEED: &str = "insurance_vault_auth";
pub const INSURANCE_VAULT_SEED: &str = "insurance_vault";
pub const FEE_VAULT_AUTHORITY_SEED: &str = "fee_vault_auth";
pub const FEE_VAULT_SEED: &str = "fee_vault";
pub const LIQUIDATION_RECORD_SEED: &str = "liq_record";

pub const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
pub const SWITCHBOARD_SOMETHING: Pubkey = pubkey!("DPvVSQYhZXQ2ygfT2Qjdg6iyeQVAyiz8okj88YRjy6NN");

#[derive(FuzzTestMethods)]
struct FuzzTest {
    /// Trident client for interacting with the Solana program
    trident: Trident,
    /// Storage for all account addresses used in fuzz testing
    fuzz_accounts: AccountAddresses,
}

#[flow_executor]
impl FuzzTest {
    fn new() -> Self {
        Self {
            trident: Trident::default(),
            fuzz_accounts: AccountAddresses::default(),
        }
    }

    #[init]
    fn start(&mut self) {
        let fee_state = self
            .trident
            .find_program_address(&[FEE_STATE_SEED.as_bytes()], &marginfi::program_id())
            .0;

        let payer = self.trident.random_keypair();

        self.trident
            .airdrop(&payer.pubkey(), 500 * LAMPORTS_PER_SOL);

        let init_fee_state = marginfi::InitGlobalFeeStateInstruction::data(
            marginfi::InitGlobalFeeStateInstructionData::new(
                payer.pubkey(),
                payer.pubkey(),
                0,
                0,
                0,
                WrappedI80F48::from(I80F48!(0)),
                WrappedI80F48::from(I80F48!(0)),
                WrappedI80F48::from(I80F48!(0)),
                WrappedI80F48::from(I80F48!(0)),
            ),
        )
        .accounts(marginfi::InitGlobalFeeStateInstructionAccounts::new(
            payer.pubkey(),
            fee_state,
        ))
        .instruction();

        let res = self
            .trident
            .process_transaction(&[init_fee_state], Some("Init Global Fee State"));

        invariant!(res.is_success());

        let marginfi_group = self.trident.random_keypair();

        let ix = types::marginfi::MarginfiGroupInitializeInstruction::data(
            marginfi::MarginfiGroupInitializeInstructionData::new(),
        )
        .accounts(marginfi::MarginfiGroupInitializeInstructionAccounts::new(
            marginfi_group.pubkey(),
            payer.pubkey(),
            fee_state,
        ))
        .instruction();

        let res = self
            .trident
            .process_transaction(&[ix], Some("Marginfi Group Initialize"));

        invariant!(res.is_success());

        let bank_mint = self.trident.random_keypair().pubkey();
        let bank = self.trident.random_keypair().pubkey();

        let liquidity_vault_authority = self
            .trident
            .find_program_address(
                &[LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(), bank.as_ref()],
                &marginfi::program_id(),
            )
            .0;

        let liquidity_vault = self
            .trident
            .find_program_address(
                &[LIQUIDITY_VAULT_SEED.as_bytes(), bank.as_ref()],
                &marginfi::program_id(),
            )
            .0;

        let insurance_vault_authority = self
            .trident
            .find_program_address(
                &[INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(), bank.as_ref()],
                &marginfi::program_id(),
            )
            .0;

        let insurance_vault = self
            .trident
            .find_program_address(
                &[INSURANCE_VAULT_SEED.as_bytes(), bank.as_ref()],
                &marginfi::program_id(),
            )
            .0;

        let fee_vault_authority = self
            .trident
            .find_program_address(
                &[FEE_VAULT_AUTHORITY_SEED.as_bytes(), bank.as_ref()],
                &marginfi::program_id(),
            )
            .0;

        let fee_vault = self
            .trident
            .find_program_address(
                &[FEE_VAULT_SEED.as_bytes(), bank.as_ref()],
                &marginfi::program_id(),
            )
            .0;

        let bank_mint_ix = self.trident.initialize_mint_2022(
            &payer.pubkey(),
            &bank_mint,
            6,
            &payer.pubkey(),
            None,
            &[],
        );

        let res = self
            .trident
            .process_transaction(&bank_mint_ix, Some("Initialize Mint"));

        assert!(res.is_success());

        let bank_config = types::marginfi::BankConfigCompact::new(
            WrappedI80F48::from(I80F48!(0.0)),
            WrappedI80F48::from(I80F48!(0.0)),
            WrappedI80F48::from(I80F48!(1)),
            WrappedI80F48::from(I80F48!(1)),
            0,
            InterestRateConfigCompact::new(
                WrappedI80F48::from(I80F48!(0)),
                WrappedI80F48::from(I80F48!(0)),
                WrappedI80F48::from(I80F48!(0)),
                WrappedI80F48::from(I80F48!(0)),
                WrappedI80F48::from(I80F48!(0)),
                0,
                0,
                [RatePoint::new(0, 0); 5],
            ),
            BankOperationalState::Operational,
            0,
            RiskTier::Collateral,
            1,
            0,
            [0; 5],
            0,
            100,
            100,
        );

        let lending_pool_add_bank = types::marginfi::LendingPoolAddBankInstruction::data(
            types::marginfi::LendingPoolAddBankInstructionData::new(bank_config),
        )
        .accounts(types::marginfi::LendingPoolAddBankInstructionAccounts::new(
            marginfi_group.pubkey(),
            payer.pubkey(),
            payer.pubkey(),
            fee_state,
            payer.pubkey(),
            bank_mint,
            bank,
            liquidity_vault_authority,
            liquidity_vault,
            insurance_vault_authority,
            insurance_vault,
            fee_vault_authority,
            fee_vault,
            TOKEN_2022_PROGRAM_ID,
        ))
        .instruction();

        let res = self
            .trident
            .process_transaction(&[lending_pool_add_bank], Some("Lending Pool Add Bank"));

        assert!(res.is_success());

        let configure_bank_oracle =
            types::marginfi::LendingPoolConfigureBankOracleInstruction::data(
                marginfi::LendingPoolConfigureBankOracleInstructionData::new(
                    4,
                    SWITCHBOARD_SOMETHING,
                ),
            )
            .accounts(
                marginfi::LendingPoolConfigureBankOracleInstructionAccounts::new(
                    marginfi_group.pubkey(),
                    payer.pubkey(),
                    bank,
                ),
            )
            .remaining_accounts(vec![AccountMeta::new_readonly(
                SWITCHBOARD_SOMETHING,
                false,
            )])
            .instruction();

        let res = self
            .trident
            .process_transaction(&[configure_bank_oracle], Some("Configure Bank Oracle"));

        assert!(res.is_success());

        let marginfi_account = self.trident.random_keypair().pubkey();

        let authority = self.trident.random_keypair().pubkey();

        let marginfi_account_init = marginfi::MarginfiAccountInitializeInstruction::data(
            marginfi::MarginfiAccountInitializeInstructionData::new(),
        )
        .accounts(marginfi::MarginfiAccountInitializeInstructionAccounts::new(
            marginfi_group.pubkey(),
            marginfi_account,
            authority,
            payer.pubkey(),
        ))
        .instruction();

        let res = self.trident.process_transaction(
            &[marginfi_account_init],
            Some("Marginfi Account Initialize"),
        );

        assert!(res.is_success());

        let liquidation_record = self
            .trident
            .find_program_address(
                &[
                    LIQUIDATION_RECORD_SEED.as_bytes(),
                    marginfi_account.as_ref(),
                ],
                &marginfi::program_id(),
            )
            .0;

        let init_liq_record = marginfi::MarginfiAccountInitLiqRecordInstruction::data(
            marginfi::MarginfiAccountInitLiqRecordInstructionData::new(),
        )
        .accounts(
            marginfi::MarginfiAccountInitLiqRecordInstructionAccounts::new(
                marginfi_account,
                payer.pubkey(),
                liquidation_record,
            ),
        )
        .instruction();

        let res = self
            .trident
            .process_transaction(&[init_liq_record], Some("Init Liquidation Record"));

        assert!(res.is_success());
    }

    #[flow]
    fn flow1(&mut self) {
        // Perform logic which is meant to be fuzzed
        // This flow is selected randomly from other flows
    }

    #[flow]
    fn flow2(&mut self) {
        // Perform logic which is meant to be fuzzed
        // This flow is selected randomly from other flows
    }

    #[end]
    fn end(&mut self) {
        // Perform any cleanup here, this method will be executed
        // at the end of each iteration
    }
}

fn main() {
    FuzzTest::fuzz(10, 100);
}

// jto-usd
