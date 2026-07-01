#![allow(clippy::too_many_arguments)]

pub mod utils;

use trident_fuzz::fuzzing::*;

use crate::constants;
use crate::invariants;
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
        oracle: Pubkey,
        message: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&mint);

        let bank = self.kamino_bank_address(self.marginfi_group, mint, constants::KAMINO_BANK_SEED);

        let layout = self.bank_layout(bank);

        let obligation =
            self.kamino_obligation_pda(layout.liquidity_vault_authority, lending_market);

        let bank_config = self.default_kamino_bank_config(oracle);
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
            AccountMeta::new_readonly(oracle, false),
            AccountMeta::new_readonly(kamino_reserve, false),
        ])
        .instruction();

        let res = self.trident.process_transaction(&[add_ix], message);

        invariant!(res.is_success());
    }

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
        init_amount: u64,
        message: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&mint);

        let bank = self.kamino_bank_address(self.marginfi_group, mint, constants::KAMINO_BANK_SEED);

        let layout = self.bank_layout(bank);

        let obligation =
            self.kamino_obligation_pda(layout.liquidity_vault_authority, lending_market);

        let lending_market_authority = self.get_lending_market_authority(lending_market);

        let user_meta = self.kamino_user_metadata_pda(layout.liquidity_vault_authority);

        let obligation_farm_user_state = self.get_farms_user_state_address(farm_state, obligation);

        let init_ix = KaminoInitObligationInstruction::data(
            KaminoInitObligationInstructionData::new(init_amount),
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
            obligation_farm_user_state,
            farm_state,
            *mint_data.owner(),
        ))
        .instruction();
        let res = self.trident.process_transaction(&[init_ix], message);

        invariant!(res.is_success());
    }

    pub fn deposit_to_kamino_obligation(
        &mut self,
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
        scope_prices: Option<Pubkey>,
        amount: u64,
        message: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&mint);

        let bank = self.kamino_bank_address(self.marginfi_group, mint, constants::KAMINO_BANK_SEED);

        let layout = self.bank_layout(bank);

        let obligation =
            self.kamino_obligation_pda(layout.liquidity_vault_authority, lending_market);

        let user_before = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_before = invariants::token_balance(&mut self.trident, layout.liquidity_vault);
        let reserve_supply_before =
            invariants::token_balance(&mut self.trident, reserve_liquidity_supply);
        let collateral_dest_before =
            invariants::token_balance(&mut self.trident, reserve_collateral_supply_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank);
        let share_snap_before =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank);

        let refresh_reserve = self.refresh_reserve(reserve, lending_market, scope_prices);
        let refresh_obligation = self.refresh_obligation(lending_market, obligation);

        let lending_market_authority = self.get_lending_market_authority(lending_market);

        let obligation_farm_user_state = self.get_farms_user_state_address(farm_state, obligation);

        // `refresh_reserve` is wired in via a dedicated preceding ix below,
        // so the deposit ix's own refresh flag stays off.
        let deposit_ix = types::marginfi::KaminoDepositInstruction::data(
            types::marginfi::KaminoDepositInstructionData::new(amount, Some(false)),
        )
        .accounts(types::marginfi::KaminoDepositInstructionAccounts::new(
            self.marginfi_group,
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
            .process_transaction(&[refresh_reserve, refresh_obligation, deposit_ix], message);

        let user_after = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_after = invariants::token_balance(&mut self.trident, layout.liquidity_vault);
        let reserve_supply_after =
            invariants::token_balance(&mut self.trident, reserve_liquidity_supply);
        let collateral_dest_after =
            invariants::token_balance(&mut self.trident, reserve_collateral_supply_vault);

        if res.is_success() {
            invariants::assert_kamino_deposit_success_liquidity_leg(
                amount,
                user_before,
                user_after,
                vault_before,
                vault_after,
                reserve_supply_before,
                reserve_supply_after,
            );
            invariants::assert_kamino_deposit_success_collateral_destination(
                amount,
                collateral_dest_before,
                collateral_dest_after,
            );
            let share_snap_after =
                invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank);
            invariants::assert_deposit_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                amount,
            );
            invariants::assert_balances_packed(&mut self.trident, marginfi_account);
            self.forward_slot_based_on_reserve(reserve);
        } else {
            invariants::assert_kamino_deposit_failure_balances_unchanged(
                user_before,
                user_after,
                vault_before,
                vault_after,
                reserve_supply_before,
                reserve_supply_after,
                collateral_dest_before,
                collateral_dest_after,
            );
        }
        self.assert_liquidity_balance_snapshot_unchanged(&other_vaults_snap);
    }

    pub fn withdraw_from_kamino_obligation(
        &mut self,
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
        scope_prices: Option<Pubkey>,
        amount: u64,
        flags: Option<u8>,
        message: Option<&str>,
    ) {
        let mint_data = self.trident.get_account(&mint);

        let bank = self.kamino_bank_address(self.marginfi_group, mint, constants::KAMINO_BANK_SEED);

        let layout = self.bank_layout(bank);

        let obligation =
            self.kamino_obligation_pda(layout.liquidity_vault_authority, lending_market);

        let user_before = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_before = invariants::token_balance(&mut self.trident, layout.liquidity_vault);
        let reserve_supply_before =
            invariants::token_balance(&mut self.trident, reserve_liquidity_supply);
        let collateral_dest_before =
            invariants::token_balance(&mut self.trident, reserve_collateral_supply_vault);
        let other_vaults_snap = self.snapshot_liquidity_vaults_except(bank);
        let share_snap_before =
            invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank);

        let refresh_reserve = self.refresh_reserve(reserve, lending_market, scope_prices);
        let refresh_obligation = self.refresh_obligation(lending_market, obligation);

        let lending_market_authority = self.get_lending_market_authority(lending_market);

        let obligation_farm_user_state = self.get_farms_user_state_address(farm_state, obligation);

        let banks = self.get_marginfi_account_banks(marginfi_account, Some(bank));
        let remaining =
            self.remaining_accounts_for_bank_risk_and_t22_transfer(mint, *mint_data.owner(), banks);

        let withdraw_ix = types::marginfi::KaminoWithdrawInstruction::data(
            types::marginfi::KaminoWithdrawInstructionData::new(amount, flags),
        )
        .accounts(types::marginfi::KaminoWithdrawInstructionAccounts::new(
            self.marginfi_group,
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
        .remaining_accounts(remaining)
        .instruction();

        let res = self
            .trident
            .process_transaction(&[refresh_reserve, refresh_obligation, withdraw_ix], message);

        let user_after = invariants::token_balance(&mut self.trident, user_token_account);
        let vault_after = invariants::token_balance(&mut self.trident, layout.liquidity_vault);
        let reserve_supply_after =
            invariants::token_balance(&mut self.trident, reserve_liquidity_supply);
        let collateral_src_after =
            invariants::token_balance(&mut self.trident, reserve_collateral_supply_vault);

        // Decode the flags bitmap to recover the boolean for downstream
        // invariants (mirrors `kamino_withdraw`'s constant `WITHDRAW_ALL_FLAG`).
        let withdraw_all_flag = flags.unwrap_or(0) & constants::WITHDRAW_ALL_FLAG != 0;
        let liquidity_received = user_after.saturating_sub(user_before);

        if res.is_success() {
            invariants::assert_kamino_withdraw_success_liquidity_leg(
                user_before,
                user_after,
                vault_before,
                vault_after,
                reserve_supply_before,
                reserve_supply_after,
            );
            invariants::assert_kamino_withdraw_success_collateral_source(
                withdraw_all_flag,
                amount,
                collateral_dest_before,
                collateral_src_after,
                liquidity_received,
            );
            let share_invariant_amount = if withdraw_all_flag {
                u64::from(liquidity_received > 0)
            } else if amount > 0 {
                1
            } else {
                0
            };
            let share_snap_after =
                invariants::marginfi_bank_share_snapshot(&mut self.trident, marginfi_account, bank);
            invariants::assert_withdraw_success_share_invariants(
                &share_snap_before,
                &share_snap_after,
                share_invariant_amount,
            );
            invariants::assert_balances_packed(&mut self.trident, marginfi_account);
            self.forward_slot_based_on_reserve(reserve);
        } else {
            invariants::assert_kamino_withdraw_failure_balances_unchanged(
                user_before,
                user_after,
                vault_before,
                vault_after,
                reserve_supply_before,
                reserve_supply_after,
                collateral_dest_before,
                collateral_src_after,
            );
        }
        self.assert_liquidity_balance_snapshot_unchanged(&other_vaults_snap);
    }
    pub fn refresh_obligation(
        &mut self,
        lending_market: Pubkey,
        obligation: Pubkey,
    ) -> Instruction {
        let obligation_raw_data = self.trident.get_account(&obligation);

        let obligation_data = klend_interface::from_account_data::<
            klend_interface::state::Obligation,
        >(obligation_raw_data.data())
        .expect("Obligation magically not available");

        let deposits = obligation_data
            .deposits
            .iter()
            .filter(|d| d.deposit_reserve != Pubkey::default())
            .map(|d| d.deposit_reserve)
            .collect::<Vec<_>>();

        let borrows = obligation_data
            .borrows
            .iter()
            .filter(|b| b.borrow_reserve != Pubkey::default())
            .map(|b| b.borrow_reserve)
            .collect::<Vec<_>>();

        let remaining_accounts = deposits
            .iter()
            .chain(borrows.iter())
            .map(|r| AccountMeta::new(*r, false))
            .collect::<Vec<_>>();

        klend_interface::instructions::refresh_obligation(
            klend_interface::instructions::RefreshObligationAccounts {
                lending_market,
                obligation,
            },
            remaining_accounts,
        )
    }
    pub fn refresh_reserve(
        &mut self,
        reserve: Pubkey,
        lending_market: Pubkey,
        scope_prices: Option<Pubkey>,
    ) -> Instruction {
        klend_interface::instructions::refresh_reserve(
            klend_interface::instructions::RefreshReserveAccounts {
                reserve,
                lending_market,
                pyth_oracle: None,
                switchboard_price_oracle: None,
                switchboard_twap_oracle: None,
                scope_prices,
            },
        )
    }

    pub fn forward_slot_based_on_reserve(&mut self, reserve: Pubkey) {
        let reserve_data = self.trident.get_account(&reserve);

        let reserve = klend_interface::from_account_data::<klend_interface::state::Reserve>(
            reserve_data.data(),
        )
        .expect("Reserve magically not available");

        self.trident.warp_to_slot(reserve.last_update.slot);
    }
}
