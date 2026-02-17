use anchor_lang::prelude::*;

use crate::constants::{EXCHANGE_PRICE_SCALE_FACTOR, FOUR_DECIMALS, SECONDS_PER_YEAR};
use crate::state::CurrentLiquidity;
use crate::{errors::ErrorCodes, events::LogUpdateExchangePrices, state::*};

use library::math::{casting::*, safe_math::*};
use liquidity::state::TokenReserve;

#[account(zero_copy)]
#[derive(InitSpace)]
#[repr(C, packed)]
pub struct VaultState {
    pub vault_id: u16, // Vault ID

    pub branch_liquidated: u8,  // Is the current active branch liquidated?
    pub topmost_tick: i32,      // value of topmost tick
    pub current_branch_id: u32, // Current branch ID
    pub total_branch_id: u32,   // Total branch ID
    pub total_supply: u64,      // Total supply
    pub total_borrow: u64,      // Total borrow
    pub total_positions: u32,   // Total positions

    pub absorbed_debt_amount: u128, // Raw debt amount
    pub absorbed_col_amount: u128,  // Raw collateral amount

    pub absorbed_dust_debt: u64, // Absorbed dust debt

    pub liquidity_supply_exchange_price: u64, // Liquidity's collateral token supply exchange price
    pub liquidity_borrow_exchange_price: u64, // Liquidity's debt token borrow exchange price
    pub vault_supply_exchange_price: u64,     // Vault's collateral token supply exchange price
    pub vault_borrow_exchange_price: u64,     // Vault's debt token borrow exchange price

    pub next_position_id: u32,      // Next position ID
    pub last_update_timestamp: u64, // Last update timestamp
}

impl VaultState {
    pub fn reset_branch_liquidated(&mut self) {
        self.branch_liquidated = 0;
    }

    pub fn set_branch_liquidated(&mut self) {
        self.branch_liquidated = 1;
    }

    pub fn reset_top_tick(&mut self) {
        self.topmost_tick = COLD_TICK;
    }

    pub fn update_state_at_liq_end(&mut self, tick: i32, branch_id: u32) -> Result<()> {
        self.set_branch_liquidated();
        self.topmost_tick = tick;
        self.current_branch_id = branch_id;

        Ok(())
    }

    pub fn get_total_supply(&self) -> Result<u128> {
        Ok(self.total_supply.cast()?)
    }

    pub fn set_total_supply(&mut self, new_total_supply: u128) -> Result<()> {
        self.total_supply = new_total_supply.cast()?;
        Ok(())
    }

    pub fn reduce_total_supply(&mut self, amount: u128) -> Result<()> {
        self.total_supply = self.total_supply.safe_sub(amount.cast()?)?;
        Ok(())
    }

    pub fn get_total_borrow(&self) -> Result<u128> {
        Ok(self.total_borrow.cast()?)
    }

    pub fn set_total_borrow(&mut self, new_total_borrow: u128) -> Result<()> {
        self.total_borrow = new_total_borrow.cast()?;
        Ok(())
    }

    pub fn reduce_total_borrow(&mut self, amount: u128) -> Result<()> {
        self.total_borrow = self.total_borrow.safe_sub(amount.cast()?)?;
        Ok(())
    }

    pub fn reset_absorbed_amounts(&mut self) {
        self.absorbed_debt_amount = 0;
        self.absorbed_col_amount = 0;
    }

    pub fn update_absorbed_dust_debt_amount(&mut self, dust_debt: u64, debt: u64) -> Result<()> {
        self.absorbed_dust_debt = self
            .absorbed_dust_debt
            .safe_add(dust_debt)?
            .safe_sub(debt)?;

        Ok(())
    }

    pub fn add_absorbed_debt_amount(&mut self, debt: u128) -> Result<()> {
        self.absorbed_debt_amount = self.absorbed_debt_amount.safe_add(debt)?;

        Ok(())
    }

    pub fn add_absorbed_col_amount(&mut self, col: u128) -> Result<()> {
        self.absorbed_col_amount = self.absorbed_col_amount.safe_add(col)?;

        Ok(())
    }

    fn bump_total_branch_id(&mut self) {
        self.total_branch_id += 1;
    }

    pub fn update_branch_info_by_one(&mut self) {
        // increment total branches by 1
        self.bump_total_branch_id();

        // reset branch liquidated as new branch is initialized
        self.reset_branch_liquidated();

        // set current branch id to total branches
        self.current_branch_id = self.total_branch_id;
    }

    pub fn update_total_supply(&mut self, new_supply: u128, old_supply: u128) -> Result<()> {
        let new_supply_raw = self
            .get_total_supply()?
            .safe_add(new_supply)?
            .safe_sub(old_supply)?;

        // total supply is rounded down, when saving to storage
        self.set_total_supply(new_supply_raw)?;

        Ok(())
    }

    pub fn update_total_borrow(&mut self, new_borrow: u128, old_borrow: u128) -> Result<()> {
        let new_borrow_raw = self
            .get_total_borrow()?
            .safe_add(new_borrow)?
            .safe_sub(old_borrow)?;

        // total borrow is rounded up, when saving to storage
        self.set_total_borrow(new_borrow_raw)?;

        Ok(())
    }

    pub fn update_topmost_tick(
        &mut self,
        tick: i32,
        new_branch: &mut AccountLoader<Branch>,
    ) -> Result<()> {
        if self.is_branch_liquidated() {
            let mut new_branch = new_branch.load_mut()?;

            // To make sure we loaded the correct new branch
            if self.total_branch_id + 1 != new_branch.branch_id {
                return Err(error!(ErrorCodes::VaultNewBranchInvalid));
            }

            // Connecting new active branch with current active branch which is now base branch
            // Current top tick is now base branch's minima tick
            new_branch.set_new_branch_state(self)?;

            // Updating new vault state with new branch
            self.update_branch_info_by_one();
        }

        // Update the topmost tick in vault state as new top tick is available
        self.topmost_tick = tick;

        Ok(())
    }

    pub fn update_next_position_id(&mut self) {
        self.next_position_id += 1;
    }

    pub fn increase_total_positions(&mut self) {
        self.total_positions += 1;
    }

    pub fn decrease_total_positions(&mut self) {
        self.total_positions = self.total_positions.saturating_sub(1);
    }

    pub fn get_tick_status(&self) -> u8 {
        if self.is_branch_liquidated() {
            2
        } else {
            1
        }
    }

    pub fn is_branch_liquidated(&self) -> bool {
        self.branch_liquidated == 1
    }

    pub fn get_top_tick(&self) -> i32 {
        self.topmost_tick
    }

    fn record_last_update_timestamp(&mut self) -> Result<()> {
        self.last_update_timestamp = Clock::get()?.unix_timestamp.cast()?;
        Ok(())
    }

    pub fn set_top_tick<'info>(
        &mut self,
        branch_accounts: &mut BranchAccounts,
        tick_has_debt_accounts: &Box<TickHasDebtAccounts<'info>>,
    ) -> Result<i32> {
        let branch_id = self.current_branch_id;
        let mut current_branch = branch_accounts.load_mut(branch_id)?;

        // Get base branch minima tick
        let base_branch_minima_tick = current_branch.connected_minima_tick;

        let top_tick = self.get_top_tick();

        let next_top_tick_not_liquidated: i32 =
            tick_has_debt_accounts.fetch_next_top_tick(top_tick)?;

        // Choose the higher tick as the new top tick
        let new_top_tick = if base_branch_minima_tick > next_top_tick_not_liquidated {
            base_branch_minima_tick
        } else {
            next_top_tick_not_liquidated
        };

        if new_top_tick == COLD_TICK {
            // Last user left the vault
            self.reset_top_tick();
            self.reset_branch_liquidated();
        } else if new_top_tick == next_top_tick_not_liquidated {
            // New top tick exists in current non-liquidated branch
            self.topmost_tick = new_top_tick;
            self.reset_branch_liquidated();
        } else {
            // if this happens that means base branch exists & is the next top tick
            // Remove current non liquidated branch as active.
            // Not deleting here as it's going to get initialize again whenever a new top tick comes

            self.set_branch_liquidated();
            // Set base branch as current
            self.topmost_tick = current_branch.connected_minima_tick; // new current top tick = base branch minima tick
            self.current_branch_id = current_branch.connected_branch_id;

            // Clear the current branch data
            current_branch.reset_branch_data();

            // Reduce total branch ID by 1
            self.total_branch_id = branch_id - 1;
        }

        Ok(new_top_tick)
    }

    fn get_exchange_prices(&self) -> Result<(u128, u128, u128, u128)> {
        let vault_supply_ex_price: u128 = self.vault_supply_exchange_price.cast()?;
        let vault_borrow_ex_price: u128 = self.vault_borrow_exchange_price.cast()?;
        let old_liq_supply_ex_price: u128 = self.liquidity_supply_exchange_price.cast()?;
        let old_liq_borrow_ex_price: u128 = self.liquidity_borrow_exchange_price.cast()?;

        Ok((
            vault_supply_ex_price,
            vault_borrow_ex_price,
            old_liq_supply_ex_price,
            old_liq_borrow_ex_price,
        ))
    }

    pub fn load_exchange_prices(
        &self,
        vault_config: &VaultConfig,
        supply_token_reserves_account: &AccountLoader<TokenReserve>,
        borrow_token_reserves_account: &AccountLoader<TokenReserve>,
    ) -> Result<(u128, u128, u128, u128)> {
        let supply_token_reserves = supply_token_reserves_account.load()?;
        let borrow_token_reserves = borrow_token_reserves_account.load()?;

        // latest vault exchange prices with old liquidity exchange prices
        let (
            mut vault_supply_ex_price,
            mut vault_borrow_ex_price,
            old_liq_supply_ex_price,
            old_liq_borrow_ex_price,
        ) = self.get_exchange_prices()?;

        let (liq_supply_ex_price, _) = supply_token_reserves.calculate_exchange_prices()?;
        let (_, liq_borrow_ex_price) = borrow_token_reserves.calculate_exchange_prices()?;

        if liq_supply_ex_price < old_liq_supply_ex_price
            || liq_borrow_ex_price < old_liq_borrow_ex_price
        {
            // new liquidity exchange price is < than the old one. liquidity exchange price should only ever increase.
            // If not, something went wrong and avoid proceeding with unknown outcome.
            return Err(error!(ErrorCodes::VaultLiquidityExchangePriceUnexpected));
        }

        let vault_supply_ex_price_old = vault_supply_ex_price;

        // liquidity Exchange Prices always increases in next block. Hence subtraction with old will never be negative
        // uint64 * 1e18 is the max the number that could be
        // Calculating increase in supply exchange price w.r.t last stored liquidity's exchange price
        let liq_supply_increase_in_percent: u128 = liq_supply_ex_price
            .safe_mul(EXCHANGE_PRICE_SCALE_FACTOR)?
            .safe_div(old_liq_supply_ex_price)?;

        // It's extremely hard the exchange prices to overflow even in 100 years but if it does it's not an
        // issue here as we are not updating on storage last stored vault's supply token exchange price
        vault_supply_ex_price = vault_supply_ex_price
            .safe_mul(liq_supply_increase_in_percent)?
            .safe_div(EXCHANGE_PRICE_SCALE_FACTOR)?;

        let time_diff = Clock::get()?
            .unix_timestamp
            .cast::<u128>()?
            .safe_sub(self.last_update_timestamp.cast()?)?;

        if vault_config.supply_rate_magnifier != 0 {
            let supply_rate_change: u128 = vault_supply_ex_price_old
                .safe_mul(time_diff)?
                .safe_mul(vault_config.supply_rate_magnifier.abs().cast()?)?
                .safe_div(FOUR_DECIMALS)?
                .safe_div(SECONDS_PER_YEAR)?;

            if vault_config.supply_rate_magnifier > 0 {
                vault_supply_ex_price = vault_supply_ex_price.safe_add(supply_rate_change)?;
            } else {
                vault_supply_ex_price = vault_supply_ex_price.safe_sub(supply_rate_change)?;
            }
        }

        let vault_borrow_ex_price_old = vault_borrow_ex_price;

        // Calculating increase in borrow exchange price w.r.t last stored liquidity's exchange price
        let liq_borrow_increase_in_percent: u128 = liq_borrow_ex_price
            .safe_mul(EXCHANGE_PRICE_SCALE_FACTOR)?
            .safe_div(old_liq_borrow_ex_price)?;

        vault_borrow_ex_price = vault_borrow_ex_price
            .safe_mul(liq_borrow_increase_in_percent)?
            .safe_div_ceil(EXCHANGE_PRICE_SCALE_FACTOR)?;

        if vault_config.borrow_rate_magnifier != 0 {
            let borrow_rate_change: u128 = vault_borrow_ex_price_old
                .safe_mul(time_diff)?
                .safe_mul(vault_config.borrow_rate_magnifier.abs().cast()?)?
                .safe_div(FOUR_DECIMALS)?
                .safe_div(SECONDS_PER_YEAR)?;

            if vault_config.borrow_rate_magnifier > 0 {
                vault_borrow_ex_price = vault_borrow_ex_price.safe_add(borrow_rate_change)?;
            } else {
                vault_borrow_ex_price = vault_borrow_ex_price.safe_sub(borrow_rate_change)?;
            }
        }

        Ok((
            liq_supply_ex_price,
            liq_borrow_ex_price,
            vault_supply_ex_price,
            vault_borrow_ex_price,
        ))
    }

    pub fn update_exchange_prices(
        &mut self,
        vault_config: &VaultConfig,
        supply_token_reserves: &AccountLoader<TokenReserve>,
        borrow_token_reserves: &AccountLoader<TokenReserve>,
    ) -> Result<()> {
        let (
            liq_supply_ex_price,
            liq_borrow_ex_price,
            vault_supply_ex_price,
            vault_borrow_ex_price,
        ) =
            self.load_exchange_prices(vault_config, supply_token_reserves, borrow_token_reserves)?;

        // Update in storage
        self.liquidity_supply_exchange_price = liq_supply_ex_price.cast()?;
        self.liquidity_borrow_exchange_price = liq_borrow_ex_price.cast()?;
        self.vault_supply_exchange_price = vault_supply_ex_price.cast()?;
        self.vault_borrow_exchange_price = vault_borrow_ex_price.cast()?;
        self.record_last_update_timestamp()?;

        emit!(LogUpdateExchangePrices {
            liquidity_supply_exchange_price: self.liquidity_supply_exchange_price,
            liquidity_borrow_exchange_price: self.liquidity_borrow_exchange_price,
            vault_supply_exchange_price: self.vault_supply_exchange_price,
            vault_borrow_exchange_price: self.vault_borrow_exchange_price,
        });

        Ok(())
    }

    pub fn absorb_dust_amount_for_liquidate(
        &mut self,
        current_data: &mut CurrentLiquidity,
    ) -> Result<()> {
        let absorbed_debt: u128 = self.absorbed_debt_amount;
        let absorbed_col: u128 = self.absorbed_col_amount;

        if absorbed_debt > current_data.debt_remaining {
            // Removing collateral in equal proportion as debt
            current_data.total_col_liq = absorbed_col
                .safe_mul(current_data.debt_remaining)?
                .safe_div(absorbed_debt)?;

            // Update absorbed amounts
            self.absorbed_col_amount = absorbed_col.safe_sub(current_data.total_col_liq)?;

            // Update debt
            current_data.total_debt_liq = current_data.debt_remaining;
            self.absorbed_debt_amount = absorbed_debt.safe_sub(current_data.debt_remaining)?;

            current_data.debt_remaining = 0;
        } else {
            // Clean out all absorbed debt and collateral
            self.reset_absorbed_amounts();

            current_data.debt_remaining = current_data.debt_remaining.safe_sub(absorbed_debt)?;
            current_data.total_debt_liq = absorbed_debt;
            current_data.total_col_liq = absorbed_col;
        }

        Ok(())
    }
}
