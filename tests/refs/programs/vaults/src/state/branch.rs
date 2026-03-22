use anchor_lang::prelude::*;
use std::{
    cell::{Ref, RefMut},
    collections::HashMap,
};

use crate::{
    constants::{FOUR_DECIMALS, X30},
    errors::ErrorCodes,
    state::{VaultState, COLD_TICK},
    structs::BranchState,
};

use library::math::{casting::*, safe_math::*, tick::TickMath};

pub enum BranchStatus {
    NotLiquidated,
    Liquidated,
    Merged,
    Closed,
}

/// Branch data structure
#[account(zero_copy)]
#[repr(C, packed)]
#[derive(InitSpace)]
pub struct Branch {
    pub vault_id: u16, // Vault ID

    pub branch_id: u32, // Branch ID

    pub status: u8,       // 0 = not liquidated, 1 = liquidated, 2 = merged, 3 = closed
    pub minima_tick: i32, // Minima tick of this branch (tick until which liquidation happened)
    pub minima_tick_partials: u32, // Partials of minima tick of branch this is connected to
    pub debt_liquidity: u64,

    // If not merged, debt factor
    // if merged, connection/adjustment debt factor
    // if closed, debt factor as 0
    pub debt_factor: u64, // Debt factor coefficient, 35 coefficient | 15 exponent

    // For all branches
    pub connected_branch_id: u32, // Branch's ID with which this branch is connected
    pub connected_minima_tick: i32, // Minima tick of branch this is connected to
}

impl Branch {
    pub fn set_connections(&mut self, branch_id: u32, minima_tick: i32) -> Result<()> {
        self.connected_branch_id = branch_id;
        self.connected_minima_tick = minima_tick;

        Ok(())
    }

    pub fn reset_branch_data(&mut self) {
        self.status = 0;
        self.minima_tick = COLD_TICK;
        self.minima_tick_partials = 0;
        self.debt_liquidity = 0;
        self.debt_factor = 0;
        self.connected_branch_id = 0;
        self.connected_minima_tick = COLD_TICK;
    }

    pub fn set_status_as_merged(&mut self) {
        self.status = BranchStatus::Merged as u8;
    }

    pub fn merge_with_base_branch(&mut self, connection_factor: u64) -> Result<()> {
        self.set_status_as_merged();

        // set new connectionFactor or debtFactor
        self.debt_factor = connection_factor;

        // deleting debt / partials / minima tick of current branch
        self.minima_tick = COLD_TICK;
        self.minima_tick_partials = 0;
        self.debt_liquidity = 0;

        Ok(())
    }

    pub fn set_state_after_absorb(&mut self, branch_data: &BranchState) {
        self.set_status_as_closed();

        self.minima_tick = branch_data.minima_tick;
        self.minima_tick_partials = branch_data.minima_tick_partials;
        self.debt_liquidity = branch_data.debt_liquidity;
        self.debt_factor = branch_data.debt_factor;
        self.connected_branch_id = branch_data.connected_branch_id;
        self.connected_minima_tick = branch_data.connected_minima_tick;
    }

    pub fn set_status_as_closed(&mut self) {
        self.status = BranchStatus::Closed as u8;
    }

    pub fn set_status_as_liquidated(&mut self) {
        self.status = BranchStatus::Liquidated as u8;
    }

    pub fn is_closed(&self) -> bool {
        self.status == BranchStatus::Closed as u8
    }

    pub fn is_merged(&self) -> bool {
        self.status == BranchStatus::Merged as u8
    }

    pub fn update_state_at_liq_end(
        &mut self,
        tick: i32,
        partials: u128,
        debt: u128,
        debt_factor: u128,
    ) -> Result<()> {
        self.set_status_as_liquidated();
        self.minima_tick = tick;
        self.minima_tick_partials = partials.cast()?;
        self.set_branch_debt(debt)?;
        self.debt_factor = debt_factor.cast()?;

        Ok(())
    }

    pub fn get_branch_debt(&self) -> Result<u128> {
        let branch_debt: u128 = self.debt_liquidity.cast()?;
        Ok(branch_debt)
    }

    pub fn set_branch_debt(&mut self, new_debt_liquidity_raw: u128) -> Result<()> {
        self.debt_liquidity = new_debt_liquidity_raw.cast()?;
        Ok(())
    }

    pub fn get_branch_debt_factor(&self) -> Result<u128> {
        let branch_debt_factor: u128 = self.debt_factor.cast()?;
        Ok(branch_debt_factor)
    }

    pub fn update_debt_liquidity(
        &mut self,
        new_debt_liquidity_raw: u128,
        minimum_branch_debt: u128,
    ) -> Result<()> {
        // Calculate branch's remaining debt
        let mut branch_debt: u128 = self.get_branch_debt()?;

        if new_debt_liquidity_raw + minimum_branch_debt > branch_debt {
            // explicitly making sure that branch debt/liquidity doesn't get super low
            branch_debt = minimum_branch_debt;
        } else {
            // Ensure branch_debt > user debt (it should be, due to margin in fetchLatestPosition)
            branch_debt = branch_debt.safe_sub(new_debt_liquidity_raw)?;
        }

        // Update branch debt
        self.set_branch_debt(branch_debt)?;

        Ok(())
    }

    pub fn set_new_branch_state(&mut self, vault_state: &VaultState) -> Result<()> {
        // reset data if any
        self.reset_branch_data();

        // Connecting new active branch with current active branch which is now base branch
        self.connected_branch_id = vault_state.current_branch_id;

        // Current top tick is now base branch's minima tick
        self.connected_minima_tick = vault_state.topmost_tick;

        Ok(())
    }
}

pub struct BranchAccounts<'info> {
    pub accounts: Vec<AccountLoader<'info, Branch>>,
    pub indices: HashMap<u32, usize>,
}

impl<'info> BranchAccounts<'info> {
    fn get_index(&self, branch_id: u32) -> Result<&usize> {
        match self.indices.get(&branch_id) {
            Some(index) => Ok(index),
            None => {
                msg!("Branch not found: branch_id = {}", branch_id);
                Err(error!(ErrorCodes::VaultBranchNotFound))
            }
        }
    }

    pub fn get(&self, branch_id: u32) -> Result<AccountLoader<'info, Branch>> {
        let index = self.get_index(branch_id)?;
        let account = self.accounts[*index].clone();

        Ok(account)
    }

    pub fn load(&self, branch_id: u32) -> Result<Ref<Branch>> {
        let index = self.get_index(branch_id)?;
        let loaded = self.accounts[*index].load()?;

        Ok(loaded)
    }

    pub fn load_mut(&self, branch_id: u32) -> Result<RefMut<Branch>> {
        let index = self.get_index(branch_id)?;
        let loaded = self.accounts[*index].load_mut()?;

        Ok(loaded)
    }
}

pub fn get_branches_from_remaining_accounts<'info>(
    remaining_accounts: &&'info [AccountInfo<'info>],
    remaining_accounts_indices: &Vec<u8>,
    vault_id: u16,
) -> Result<Box<BranchAccounts<'info>>> {
    // remaining_accounts_indices[0] is oracle sources length
    // remaining_accounts_indices[1] is branches length

    let branches_length: usize = remaining_accounts_indices[1].cast::<usize>()?;
    let start_index: usize = remaining_accounts_indices[0].cast()?;
    let end_index: usize = start_index + branches_length;

    if remaining_accounts.len() < end_index {
        return Err(error!(ErrorCodes::VaultLiquidateRemainingAccountsTooShort));
    }

    let mut branch_accounts = Box::new(BranchAccounts {
        accounts: Vec::with_capacity(branches_length),
        indices: HashMap::with_capacity(branches_length),
    });

    // Fill the vector with actual accounts
    for account in remaining_accounts.iter().take(end_index).skip(start_index) {
        if *account.owner != crate::ID {
            return Err(error!(ErrorCodes::VaultBranchOwnerNotValid));
        }

        // Load accounts as lazy loaded accounts
        let branch = AccountLoader::<Branch>::try_from(account)?;
        branch_accounts.accounts.push(branch);
    }

    branch_accounts.indices = get_branch_indices(&branch_accounts.accounts, vault_id)?;

    Ok(branch_accounts)
}

pub fn get_branch_indices<'info>(
    branch_accounts: &Vec<AccountLoader<'info, Branch>>,
    vault_id: u16,
) -> Result<HashMap<u32, usize>> {
    Ok(branch_accounts
        .iter()
        .enumerate()
        .map(|(idx, b)| {
            let loaded = b.load()?;
            if loaded.vault_id != vault_id {
                return Err(error!(ErrorCodes::VaultBranchVaultIdMismatch));
            }
            Ok((loaded.branch_id, idx))
        })
        .collect::<Result<_>>()?)
}

pub fn get_current_partials_ratio(minima_tick_partials: u32, ratio: u128) -> Result<(u128, u128)> {
    // Liquidated tick - has partials
    let ratio_one_less = ratio
        .safe_mul(FOUR_DECIMALS)?
        .safe_div(TickMath::TICK_SPACING)?;

    let length = ratio.safe_sub(ratio_one_less)?;

    // Get partials from branch data
    let partials = minima_tick_partials.cast()?;

    // Calculate current ratio with partials
    let current_ratio = ratio_one_less.safe_add((length.safe_mul(partials)?).safe_div(X30)?)?;

    Ok((current_ratio, partials))
}

pub fn get_tick_partials(ratio_one_less: u128, final_ratio: u128) -> Result<u128> {
    let ratio = ratio_one_less
        .safe_mul(TickMath::TICK_SPACING)?
        .safe_div(FOUR_DECIMALS)?;

    let length = ratio.safe_sub(ratio_one_less)?;
    let partials = (final_ratio.safe_sub(ratio_one_less)?)
        .safe_mul(X30)?
        .safe_div(length)?;

    Ok(partials)
}
