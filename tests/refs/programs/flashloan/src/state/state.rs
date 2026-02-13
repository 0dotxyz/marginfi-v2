use anchor_lang::prelude::*;

use crate::constants::{FLASHLOAN_FEE_MAX, FOUR_DECIMALS};
use crate::errors::ErrorCodes;
use library::math::{casting::*, safe_math::*};
use liquidity::ID as LIQUIDITY_PROGRAM_ID;

#[account]
#[derive(InitSpace)]
pub struct FlashloanAdmin {
    pub authority: Pubkey,         // Governance account
    pub liquidity_program: Pubkey, // Address of liquidity program
    pub status: bool,              // is protocol active or not
    pub flashloan_fee: u16,        // IN BASIS_POINTS , 1e4 = 100%

    pub flashloan_timestamp: u64,     // timestamp of flashloan
    pub is_flashloan_active: bool,    // is flashloan active or not
    pub active_flashloan_amount: u64, // active flashloan amount

    pub bump: u8,
}

impl FlashloanAdmin {
    pub fn init(
        &mut self,
        authority: Pubkey,
        flashloan_fee: u16,
        liquidity_program: Pubkey,
        bump: u8,
    ) -> Result<()> {
        if flashloan_fee > FLASHLOAN_FEE_MAX {
            return Err(ErrorCodes::FlashloanFeeTooHigh.into());
        }

        let default_pubkey: Pubkey = Pubkey::default();

        if liquidity_program == default_pubkey || authority == default_pubkey {
            return Err(ErrorCodes::FlashloanInvalidParams.into());
        }

        if liquidity_program != LIQUIDITY_PROGRAM_ID {
            return Err(error!(ErrorCodes::FlashloanInvalidParams));
        }

        self.status = true;
        self.authority = authority;
        self.flashloan_fee = flashloan_fee;
        self.liquidity_program = liquidity_program;
        self.bump = bump;

        Ok(())
    }

    pub fn pause_protocol(&mut self) -> Result<()> {
        if !self.status {
            return Err(ErrorCodes::FlashloanInvalidParams.into());
        }

        self.status = false;
        Ok(())
    }

    pub fn activate_protocol(&mut self) -> Result<()> {
        if self.status {
            return Err(ErrorCodes::FlashloanInvalidParams.into());
        }

        self.status = true;
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        !self.status
    }

    pub fn set_flashloan_fee(&mut self, flashloan_fee: u16) -> Result<()> {
        if flashloan_fee > FLASHLOAN_FEE_MAX {
            return Err(ErrorCodes::FlashloanFeeTooHigh.into());
        }

        self.flashloan_fee = flashloan_fee;
        Ok(())
    }

    pub fn set_flashloan_as_active(&mut self, amount: u64) -> Result<()> {
        if self.is_flashloan_active {
            return Err(ErrorCodes::FlashloanAlreadyActive.into());
        }

        self.flashloan_timestamp = Clock::get()?.unix_timestamp as u64;
        self.is_flashloan_active = true;
        self.active_flashloan_amount = amount;
        Ok(())
    }

    pub fn set_flashloan_as_inactive(&mut self) -> Result<()> {
        if !self.is_flashloan_active {
            return Err(ErrorCodes::FlashloanAlreadyInactive.into());
        }

        self.is_flashloan_active = false;
        self.active_flashloan_amount = 0;
        Ok(())
    }

    fn calculate_flashloan_fee(&self, amount: u64) -> Result<u64> {
        let flashloan_fee: u128 = self.flashloan_fee.cast()?;
        let flashloan_fee_amount: u128 = amount
            .cast::<u128>()?
            .safe_mul(flashloan_fee)?
            .safe_div_ceil(FOUR_DECIMALS)?; // round up for flashloan fee

        Ok(flashloan_fee_amount.cast()?)
    }

    pub fn get_expected_payback_amount(&self, amount: u64) -> Result<u64> {
        let flashloan_fee_amount = self.calculate_flashloan_fee(amount)?;
        let expected_payback_amount = amount.safe_add(flashloan_fee_amount)?;

        Ok(expected_payback_amount)
    }
}
