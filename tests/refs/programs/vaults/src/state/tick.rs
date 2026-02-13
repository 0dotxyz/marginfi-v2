use anchor_lang::prelude::*;
use std::{
    cell::{Ref, RefMut},
    collections::HashMap,
};

use library::math::casting::*;

use crate::errors::ErrorCodes;

/// Tick data structure
#[account(zero_copy)]
#[repr(C, packed)]
#[derive(InitSpace)]
pub struct Tick {
    pub vault_id: u16, // Vault ID

    pub tick: i32,         // Tick key in the original mapping
    pub is_liquidated: u8, // If 1 then liquidated else not liquidated
    pub total_ids: u32,    // Total IDs. ID should start from 1. Increases every time when a tick gets liquidated as each liquidation must be 
                           // stored uniquely, see connection for this with tick_id_liquidation

    pub raw_debt: u64, // Raw debt (if not liquidated)

    pub is_fully_liquidated: u8,    // Is 100% liquidated?
    pub liquidation_branch_id: u32, // Branch ID where this tick got liquidated
    pub debt_factor: u64,           // Debt factor coefficient, 35 coefficient | 15 exponent
}

impl Tick {
    pub fn validate(&self, tick: i32) -> Result<()> {
        let self_tick = self.tick;
        if self_tick != tick {
            msg!("Tick mismatch: expected {} but got {}", tick, self_tick);
            return Err(error!(ErrorCodes::VaultTickMismatch));
        }

        Ok(())
    }

    pub fn is_liquidated(&self) -> bool {
        self.is_liquidated == 1
    }

    pub fn is_fully_liquidated(&self) -> bool {
        self.is_fully_liquidated == 1
    }

    pub fn set_fully_liquidated(&mut self) {
        self.set_liquidated(0, 0);
        self.is_fully_liquidated = 1;
    }

    pub fn set_liquidated(&mut self, branch_id: u32, debt_factor: u64) {
        self.raw_debt = 0;
        self.is_liquidated = 1;
        self.debt_factor = debt_factor;
        self.liquidation_branch_id = branch_id;
    }

    pub fn get_raw_debt(&self) -> Result<u128> {
        Ok(self.raw_debt.cast()?)
    }

    pub fn set_raw_debt(&mut self, new_raw_debt: u128) -> Result<()> {
        self.raw_debt = new_raw_debt.cast()?;
        Ok(())
    }

    pub fn get_tick_status(&self) -> Result<(bool, u32, u64)> {
        Ok((
            self.is_fully_liquidated(),
            self.liquidation_branch_id,
            self.debt_factor,
        ))
    }
}

pub struct TickAccounts<'info> {
    pub accounts: Vec<AccountLoader<'info, Tick>>,
    pub indices: HashMap<i32, usize>,
}

impl<'info> TickAccounts<'info> {
    fn get_index(&self, tick: i32) -> Result<&usize> {
        match self.indices.get(&tick) {
            Some(index) => Ok(index),
            None => {
                msg!("Tick not found: tick = {}", tick);
                Err(error!(ErrorCodes::VaultTickNotFound))
            }
        }
    }

    pub fn load(&self, tick: i32) -> Result<Ref<Tick>> {
        let index = self.get_index(tick)?;
        let loaded = self.accounts[*index].load()?;

        Ok(loaded)
    }

    pub fn load_mut(&self, tick: i32) -> Result<RefMut<Tick>> {
        let index = self.get_index(tick)?;
        let loaded = self.accounts[*index].load_mut()?;

        Ok(loaded)
    }
}

pub fn get_ticks_from_remaining_accounts<'info>(
    remaining_accounts: &'info [AccountInfo<'info>],
    remaining_accounts_indices: &Vec<u8>,
    vault_id: u16,
) -> Result<Box<TickAccounts<'info>>> {
    let total_ticks_length: usize = remaining_accounts_indices[2].cast::<usize>()?;

    // remaining_accounts_indices[0] is oracle sources length
    // remaining_accounts_indices[1] is branches length
    // remaining_accounts_indices[2] is ticks length
    // remaining_accounts_indices[3] is tick has debt length

    let start_index: usize = remaining_accounts_indices[0].cast::<usize>()?
        + remaining_accounts_indices[1].cast::<usize>()?;

    let end_index: usize = start_index + total_ticks_length;

    let mut tick_accounts = Box::new(TickAccounts {
        accounts: Vec::with_capacity(total_ticks_length),
        indices: HashMap::with_capacity(total_ticks_length),
    });

    if remaining_accounts.len() < end_index {
        return Err(error!(ErrorCodes::VaultLiquidateRemainingAccountsTooShort));
    }

    for account in remaining_accounts.iter().take(end_index).skip(start_index) {
        if *account.owner != crate::ID {
            return Err(error!(ErrorCodes::VaultTickOwnerNotValid));
        }

        let tick_data = AccountLoader::<'info, Tick>::try_from(account)?;
        tick_accounts.accounts.push(tick_data);
    }

    tick_accounts.indices = get_tick_accounts_indices(&tick_accounts.accounts, vault_id)?;

    Ok(tick_accounts)
}

fn get_tick_accounts_indices<'info>(
    tick_accounts: &Vec<AccountLoader<'info, Tick>>,
    vault_id: u16,
) -> Result<HashMap<i32, usize>> {
    tick_accounts
        .iter()
        .enumerate()
        .map(|(idx, t)| {
            let tick_data = t.load()?;
            if tick_data.vault_id != vault_id {
                return Err(error!(ErrorCodes::VaultTickVaultIdMismatch));
            }
            Ok((tick_data.tick, idx))
        })
        .collect()
}

pub fn get_next_ref_tick(
    minima_tick: i32,
    next_tick: i32,
    liquidation_tick: i32,
) -> Result<(i32, u8)> {
    // Fetching refTick. refTick is the biggest tick of these 3:
    // 1. Next tick with liquidity (from tickHasDebt)
    // 2. Minima tick of current branch
    // 3. Liquidation threshold tick

    if minima_tick > next_tick && minima_tick > liquidation_tick {
        Ok((minima_tick, 2))
    } else if next_tick > liquidation_tick {
        // next tick will be next tick from perfect tick
        Ok((next_tick, 1))
    } else {
        // next tick is threshold tick
        Ok((liquidation_tick, 3))
    }
}
