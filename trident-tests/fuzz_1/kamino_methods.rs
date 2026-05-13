use trident_fuzz::fuzzing::*;

use crate::constants;
use crate::types;
use crate::FuzzTest;

use crate::types::marginfi::KaminoInitObligationInstruction;
use crate::types::marginfi::KaminoInitObligationInstructionAccounts;
use crate::types::marginfi::KaminoInitObligationInstructionData;
use crate::types::marginfi::LendingPoolAddBankKaminoInstruction;
use crate::types::marginfi::LendingPoolAddBankKaminoInstructionAccounts;
use crate::types::marginfi::LendingPoolAddBankKaminoInstructionData;

impl FuzzTest {
    pub fn init_kamino_bank(
        &mut self,
        payer: Pubkey,
        mint: Pubkey,
        lending_market: Pubkey,
        kamino_reserve: Pubkey,
    ) {
        let mint_data = self.trident.get_account(&mint);

        let bank = self.kamino_bank_address(self.marginfi_group, mint, constants::KAMINO_BANK_SEED);

        let layout = self.bank_layout(bank);

        let obligation =
            self.kamino_obligation_pda(layout.liquidity_vault_authority, lending_market);

        let bank_config = self.default_kamino_bank_config(constants::KAMINO_PYTH_ORACLE);
        let add_ix = LendingPoolAddBankKaminoInstruction::data(
            LendingPoolAddBankKaminoInstructionData::new(bank_config, constants::KAMINO_BANK_SEED),
        )
        .accounts(LendingPoolAddBankKaminoInstructionAccounts::new(
            self.marginfi_group,
            payer,
            payer,
            mint,
            bank,
            kamino_reserve,
            obligation,
            layout.liquidity_vault_authority,
            layout.liquidity_vault,
            layout.insurance_vault_authority,
            layout.insurance_vault,
            layout.fee_vault_authority,
            layout.fee_vault,
            *mint_data.owner(),
        ))
        .remaining_accounts(vec![
            AccountMeta::new_readonly(constants::KAMINO_PYTH_ORACLE, false),
            AccountMeta::new_readonly(kamino_reserve, false),
        ])
        .instruction();

        let res = self
            .trident
            .process_transaction(&[add_ix], Some("Kamino: lending_pool_add_bank_kamino"));

        invariant!(res.is_success());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn init_kamino_obligation(
        &mut self,
        user: Pubkey,
        user_token_account: Pubkey,
        mint: Pubkey,
        lending_market: Pubkey,
        reserve: Pubkey,
        reserve_liquidity_supply: Pubkey,
        reserve_collateral_mint: Pubkey,
        reserve_collateral_supply_vault: Pubkey,
        farm_state: Pubkey,
    ) {
        let mint_data = self.trident.get_account(&mint);

        let bank = self.kamino_bank_address(self.marginfi_group, mint, constants::KAMINO_BANK_SEED);

        let layout = self.bank_layout(bank);

        let obligation =
            self.kamino_obligation_pda(layout.liquidity_vault_authority, lending_market);

        let lending_market_authority = self.get_lending_market_authority(lending_market);

        let user_meta = self.kamino_user_metadata_pda(layout.liquidity_vault_authority);

        let obligation_farm_user_state = self.get_famrs_user_state_address(farm_state, obligation);

        const NOMINAL_INIT: u64 = 100_000;
        let init_ix = KaminoInitObligationInstruction::data(
            KaminoInitObligationInstructionData::new(NOMINAL_INIT),
        )
        .accounts(KaminoInitObligationInstructionAccounts::new(
            user,
            bank,
            user_token_account,
            layout.liquidity_vault_authority,
            layout.liquidity_vault,
            obligation,
            user_meta,
            lending_market,
            lending_market_authority,
            reserve,
            mint,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_collateral_supply_vault,
            types::marginfi::program_id(),
            types::marginfi::program_id(),
            types::marginfi::program_id(),
            constants::USDC_SCOPE_PRICES,
            obligation_farm_user_state,
            farm_state,
            *mint_data.owner(),
        ))
        .instruction();
        let res = self
            .trident
            .process_transaction(&[init_ix], Some("Kamino: kamino_init_obligation"));

        invariant!(res.is_success());
    }

    #[allow(clippy::too_many_arguments)]
    pub fn deposit_to_kamino_obligation(
        &mut self,
        group: Pubkey,
        marginfi_account: Pubkey,
        user: Pubkey,
        user_token_account: Pubkey,
        mint: Pubkey,
        lending_market: Pubkey,
        reserve: Pubkey,
        reserve_liquidity_supply: Pubkey,
        reserve_collateral_mint: Pubkey,
        reserve_collateral_supply_vault: Pubkey,
        farm_state: Pubkey,
        amount: u64,
    ) {
        let mint_data = self.trident.get_account(&mint);

        let bank = self.kamino_bank_address(self.marginfi_group, mint, constants::KAMINO_BANK_SEED);

        let layout = self.bank_layout(bank);

        let obligation =
            self.kamino_obligation_pda(layout.liquidity_vault_authority, lending_market);

        let lending_market_authority = self.get_lending_market_authority(lending_market);

        let obligation_farm_user_state = self.get_famrs_user_state_address(farm_state, obligation);

        let deposit_ix = types::marginfi::KaminoDepositInstruction::data(
            types::marginfi::KaminoDepositInstructionData::new(amount),
        )
        .accounts(types::marginfi::KaminoDepositInstructionAccounts::new(
            group,
            marginfi_account,
            user,
            bank,
            user_token_account,
            layout.liquidity_vault_authority,
            layout.liquidity_vault,
            obligation,
            lending_market,
            lending_market_authority,
            reserve,
            mint,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_collateral_supply_vault,
            obligation_farm_user_state,
            farm_state,
            *mint_data.owner(),
        ))
        .instruction();
        let res = self
            .trident
            .process_transaction(&[deposit_ix], Some("Kamino: kamino_deposit"));

        invariant!(res.is_success());
    }
}
