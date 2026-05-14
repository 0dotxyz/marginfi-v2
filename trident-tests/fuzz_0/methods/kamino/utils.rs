use trident_fuzz::fuzzing::*;

use crate::constants;
use crate::types::marginfi::KaminoConfigCompact;
use crate::FuzzTest;

use crate::types::marginfi::BankOperationalState;
use crate::types::marginfi::OracleSetup;
use crate::types::marginfi::RiskTier;
use crate::types::marginfi::WrappedI80F48;
use fixed_macro::types::I80F48;

fn wrap_one() -> WrappedI80F48 {
    WrappedI80F48::new(I80F48!(1.0).to_bits().to_le_bytes())
}

impl FuzzTest {
    pub fn kamino_bank_address(&mut self, group: Pubkey, mint: Pubkey, bank_seed: u64) -> Pubkey {
        self.trident
            .find_program_address(
                &[group.as_ref(), mint.as_ref(), &bank_seed.to_le_bytes()],
                &crate::types::marginfi::program_id(),
            )
            .0
    }
    pub fn kamino_obligation_pda(
        &mut self,
        liquidity_vault_authority: Pubkey,
        lending_market: Pubkey,
    ) -> Pubkey {
        self.trident
            .find_program_address(
                &[
                    &[0u8],
                    &[0u8],
                    liquidity_vault_authority.as_ref(),
                    lending_market.as_ref(),
                    solana_sdk::system_program::ID.as_ref(),
                    solana_sdk::system_program::ID.as_ref(),
                ],
                &constants::KLEND,
            )
            .0
    }
    pub fn kamino_user_metadata_pda(&mut self, obligation_owner: Pubkey) -> Pubkey {
        self.trident
            .find_program_address(
                &[b"user_meta", obligation_owner.as_ref()],
                &constants::KLEND,
            )
            .0
    }
    pub fn default_kamino_bank_config(&mut self, oracle: Pubkey) -> KaminoConfigCompact {
        KaminoConfigCompact::new(
            oracle,
            wrap_one(),
            wrap_one(),
            10_000_000_000_000,
            OracleSetup::KaminoPythPush,
            BankOperationalState::Operational,
            RiskTier::Collateral,
            constants::PYTH_PULL_MIGRATED_CONFIG_FLAGS,
            1_000_000_000_000,
            300,
            0,
        )
    }

    pub fn get_lending_market_authority(&mut self, lending_market: Pubkey) -> Pubkey {
        self.trident
            .find_program_address(
                &[
                    constants::KLEND_LENDING_MARKET_AUTH,
                    lending_market.as_ref(),
                ],
                &constants::KLEND,
            )
            .0
    }

    pub fn get_famrs_user_state_address(
        &mut self,
        farm_state: Pubkey,
        delegatee: Pubkey,
    ) -> Pubkey {
        self.trident
            .find_program_address(
                &[
                    constants::KFARMS_BASE_SEED_USER_STATE,
                    farm_state.as_ref(),
                    delegatee.as_ref(),
                ],
                &constants::KFARMS,
            )
            .0
    }
}

