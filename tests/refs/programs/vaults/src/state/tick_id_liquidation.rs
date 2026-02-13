use anchor_lang::prelude::*;

use crate::errors::ErrorCodes;
use library::math::safe_math::*;

/// Tick ID liquidation data
#[account(zero_copy)]
#[repr(C, packed)]
#[derive(InitSpace)]
pub struct TickIdLiquidation {
    pub vault_id: u16, // Vault ID

    pub tick: i32,     // Tick
    pub tick_map: u32, // Tick map

    // We store 3 sets of liquidation data per ID
    // First set
    pub is_fully_liquidated_1: u8,    // Is 100% liquidated?
    pub liquidation_branch_id_1: u32, // Branch ID where this tick got liquidated
    pub debt_factor_1: u64,           // Debt factor coefficient

    // Second set
    pub is_fully_liquidated_2: u8,    // Is 100% liquidated?
    pub liquidation_branch_id_2: u32, // Branch ID where this tick got liquidated
    pub debt_factor_2: u64,           // Debt factor coefficient

    // Third set
    pub is_fully_liquidated_3: u8,    // Is 100% liquidated?
    pub liquidation_branch_id_3: u32, // Branch ID where this tick got liquidated
    pub debt_factor_3: u64,           // Debt factor coefficient
}

impl TickIdLiquidation {
    pub fn validate(&self, tick: i32, tick_id: u32) -> Result<()> {
        if self.tick != tick || self.tick_map != tick_id.safe_add(2)?.safe_div(3)? {
            return Err(error!(ErrorCodes::VaultTickIdLiquidationMismatch));
        }

        Ok(())
    }

    pub fn set_tick_status(
        &mut self,
        tick_id: u32,
        is_fully_liquidated: bool,
        liquidation_branch_id: u32,
        debt_factor: u64,
    ) {
        let index: u32 = (tick_id + 2) % 3;

        match index {
            0 => {
                self.is_fully_liquidated_1 = is_fully_liquidated as u8;
                self.liquidation_branch_id_1 = liquidation_branch_id;
                self.debt_factor_1 = debt_factor;
            }
            1 => {
                self.is_fully_liquidated_2 = is_fully_liquidated as u8;
                self.liquidation_branch_id_2 = liquidation_branch_id;
                self.debt_factor_2 = debt_factor;
            }
            _ => {
                self.is_fully_liquidated_3 = is_fully_liquidated as u8;
                self.liquidation_branch_id_3 = liquidation_branch_id;
                self.debt_factor_3 = debt_factor;
            }
        }
    }

    pub fn get_tick_status(&self, tick_id: u32) -> Result<(bool, u32, u64)> {
        // Get from tick ID liquidation data
        // Find which set of liquidation data to use based on position_tick_id
        let index: u32 = (tick_id + 2) % 3;

        let is_fully_liquidated: bool;
        let branch_id: u32;
        let connection_factor: u64;

        match index {
            0 => {
                is_fully_liquidated = self.is_fully_liquidated_1 == 1;
                branch_id = self.liquidation_branch_id_1;
                connection_factor = self.debt_factor_1;
            }
            1 => {
                is_fully_liquidated = self.is_fully_liquidated_2 == 1;
                branch_id = self.liquidation_branch_id_2;
                connection_factor = self.debt_factor_2;
            }
            _ => {
                is_fully_liquidated = self.is_fully_liquidated_3 == 1;
                branch_id = self.liquidation_branch_id_3;
                connection_factor = self.debt_factor_3;
            }
        }

        Ok((is_fully_liquidated, branch_id, connection_factor))
    }
}
