use anchor_lang::prelude::*;
use std::ops::Neg;

use crate::constants::*;
use crate::errors::*;

use library::math::{casting::*, safe_math::*};

pub enum UserSupplyPositionStatus {
    Active,
    Paused,
    NotSet,
}

/// User supply position
#[account(zero_copy)]
#[derive(InitSpace)]
#[repr(C, packed)]
pub struct UserSupplyPosition {
    pub protocol: Pubkey, // Protocol public key
    pub mint: Pubkey,     // Token mint public key

    pub with_interest: u8,          // Mode flag, 0 = false, 1 = true
    pub amount: u64,                // user supply amount (normal or raw depends on with_interest)
    pub withdrawal_limit: u128, // previous user withdrawal limit (normal or raw depends on with_interest)
    pub last_update: u64,       // last triggered process timestamp
    pub expand_pct: u16, //expand withdrawal limit percentage (in 1e2: 100% = 10_000; 1% = 100 -> max value 65_535).
    pub expand_duration: u64, // withdrawal limit expand duration in seconds.(Max value 4_294_967_295; ~11_93_046 hours, ~49710 days)
    pub base_withdrawal_limit: u64, // base withdrawal limit: below this, 100% withdrawals can be done (normal or raw depends on with_interest)
    pub status: u8, // Pause status, 0 = false, 1 = true, if status = 2, then config yet to be set
}

impl UserSupplyPosition {
    pub fn init(&mut self, protocol: Pubkey, mint: Pubkey) -> Result<()> {
        self.protocol = protocol;
        self.mint = mint;
        self.last_update = Clock::get()?.unix_timestamp.cast()?;
        self.status = UserSupplyPositionStatus::NotSet as u8;

        Ok(())
    }

    pub fn configs_not_set(&self) -> bool {
        self.status == UserSupplyPositionStatus::NotSet as u8
    }

    pub fn is_active(&self) -> bool {
        self.status == UserSupplyPositionStatus::Active as u8
    }

    pub fn set_status_as_active(&mut self) -> Result<()> {
        self.status = UserSupplyPositionStatus::Active as u8;
        Ok(())
    }

    pub fn with_interest(&self) -> bool {
        self.with_interest == 1
    }

    pub fn set_interest_mode(&mut self, mode: u8) -> Result<()> {
        self.with_interest = mode;
        Ok(())
    }

    pub fn get_withdrawal_limit(&self) -> Result<u128> {
        Ok(self.withdrawal_limit)
    }

    pub fn set_withdrawal_limit(&mut self, new_withdrawal_limit: u128) -> Result<()> {
        if new_withdrawal_limit > u64::MAX as u128 {
            return Err(ErrorCodes::ValueOverflow.into());
        }
        self.withdrawal_limit = new_withdrawal_limit;
        Ok(())
    }

    pub fn get_base_withdrawal_limit(&self) -> Result<u128> {
        Ok(self.base_withdrawal_limit.cast()?)
    }

    pub fn set_base_withdrawal_limit(&mut self, new_base_withdrawal_limit: u128) -> Result<()> {
        self.base_withdrawal_limit = new_base_withdrawal_limit.cast()?;
        Ok(())
    }

    pub fn set_expand_duration(&mut self, new_expand_duration: u128) -> Result<()> {
        self.expand_duration = new_expand_duration.cast()?;
        Ok(())
    }

    pub fn set_expand_pct(&mut self, new_expand_pct: u128) -> Result<()> {
        self.expand_pct = new_expand_pct.cast()?;
        Ok(())
    }

    pub fn get_amount(&self) -> Result<u128> {
        Ok(self.amount.cast()?)
    }

    pub fn set_amount(&mut self, new_amount: u128) -> Result<()> {
        self.amount = new_amount.cast()?;
        Ok(())
    }

    pub fn calc_withdrawal_limit_before_operate(&self) -> Result<(u128, u128)> {
        let last_withdrawal_limit: u128 = self.get_withdrawal_limit()?;
        let user_supply_position_amount: u128 = self.get_amount()?;

        if last_withdrawal_limit == 0 {
            return Ok((0, user_supply_position_amount));
        }

        // extract max withdrawable percent of user supply and
        // calculate maximum withdrawable amount expandPercentage of user supply at full expansion duration elapsed
        // e.g.: if 10% expandPercentage, meaning 10% is withdrawable after full expandDuration has elapsed.
        let user_supply_expand_pct: u128 = self.expand_pct.cast()?;

        // userSupply_ needs to be at least 1e15 to overflow max limit of ~1e19 in u64 (no token in existence where this is possible).
        let max_withdrawable_amount: u128 = user_supply_position_amount
            .safe_mul(user_supply_expand_pct)?
            .safe_div(FOUR_DECIMALS)?;

        let time_since_last_update: u128 = (Clock::get()?.unix_timestamp)
            .safe_sub(self.last_update.cast()?)?
            .cast()?;

        // calculate withdrawable amount of expandPercent that is elapsed of expandDuration.
        // e.g. if 60% of expandDuration has elapsed, then user should be able to withdraw 6% of user supply, down to 94%.
        // Note: no explicit check for this needed, it is covered by setting minWithdrawalLimit_ if needed.

        let withdrawal_amount: u128 = max_withdrawable_amount
            .safe_mul(time_since_last_update)?
            .safe_div(self.expand_duration.cast()?)?;

        let mut current_withdrawal_limit: u128 =
            last_withdrawal_limit.saturating_sub(withdrawal_amount);

        let minimum_withdrawal_amount: u128 =
            user_supply_position_amount.safe_sub(max_withdrawable_amount)?;

        if minimum_withdrawal_amount > current_withdrawal_limit {
            current_withdrawal_limit = minimum_withdrawal_amount;
        }

        Ok((current_withdrawal_limit, user_supply_position_amount))
    }

    pub fn calc_withdrawal_limit_after_operate(
        &self,
        user_supply_position_amount: u128,
        new_withdrawal_limit: u128,
    ) -> Result<u128> {
        // temp_ => base withdrawal limit. below this, maximum withdrawals are allowed
        let base_withdrawal_limit: u128 = self.get_base_withdrawal_limit()?;

        // if user supply is below base limit then max withdrawals are allowed
        if user_supply_position_amount < base_withdrawal_limit {
            return Ok(0);
        }

        // minimum withdrawal limit: userSupply - max withdrawable limit (userSupply * expandPercent))
        // userSupply_ needs to be at least 1e15 to overflow max limit of ~1e19 in u64 (no token in existence where this is possible).
        // subtraction can not underflow as maxWithdrawableLimit_ is a percentage amount (<=100%) of userSupply_
        let minimum_withdrawal_limit: u128 = user_supply_position_amount.safe_sub(
            user_supply_position_amount
                .safe_mul(self.expand_pct.cast()?)?
                .safe_div(FOUR_DECIMALS)?,
        )?;

        if minimum_withdrawal_limit > new_withdrawal_limit {
            return Ok(minimum_withdrawal_limit);
        }

        Ok(new_withdrawal_limit)
    }

    pub fn supply_or_withdraw(
        &mut self,
        amount_: i128,
        supply_exchange_price: u128,
    ) -> Result<(i128, i128)> {
        if !self.is_active() {
            return Err(ErrorCodes::UserPaused.into());
        }

        let (current_withdrawal_limit, mut user_supply_position_amount) =
            self.calc_withdrawal_limit_before_operate()?;

        let mut new_supply_interest_raw: i128 = 0;
        let mut new_supply_interest_free: i128 = 0;

        if self.with_interest() {
            if amount_ > 0 {
                // convert amount from normal to raw (divide by exchange price) -> round down for deposit
                new_supply_interest_raw = amount_
                    .safe_mul(EXCHANGE_PRICES_PRECISION.cast()?)?
                    .safe_div(supply_exchange_price.cast()?)?;

                user_supply_position_amount =
                    user_supply_position_amount.safe_add(new_supply_interest_raw.cast()?)?;
            } else {
                // convert amount from normal to raw (divide by exchange price) -> round up for withdraw
                new_supply_interest_raw = amount_
                    .abs()
                    .safe_mul(EXCHANGE_PRICES_PRECISION.cast()?)?
                    .safe_div_ceil(supply_exchange_price.cast()?)?
                    .neg();

                user_supply_position_amount =
                    user_supply_position_amount.safe_sub(new_supply_interest_raw.abs().cast()?)?;
            }

            if new_supply_interest_raw == 0 {
                return Err(ErrorCodes::OperateAmountsInsufficient.into());
            }
        } else {
            new_supply_interest_free = amount_;
            if new_supply_interest_free > 0 {
                user_supply_position_amount =
                    user_supply_position_amount.safe_add(new_supply_interest_free.cast()?)?;
            } else {
                // if withdrawal is more than user's supply then rust will throw here
                user_supply_position_amount =
                    user_supply_position_amount.safe_sub(new_supply_interest_free.abs().cast()?)?;
            }
        }

        if amount_ < 0 && user_supply_position_amount < current_withdrawal_limit {
            return Err(ErrorCodes::WithdrawalLimitReached.into());
        }

        let new_withdrawal_limit = self.calc_withdrawal_limit_after_operate(
            user_supply_position_amount,
            current_withdrawal_limit,
        )?;

        self.set_amount(user_supply_position_amount)?;
        self.set_withdrawal_limit(new_withdrawal_limit)?;
        self.last_update = Clock::get()?.unix_timestamp.cast()?;

        Ok((new_supply_interest_raw, new_supply_interest_free))
    }
}
