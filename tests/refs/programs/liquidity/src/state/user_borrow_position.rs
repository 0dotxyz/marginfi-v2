use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::*;

use library::math::{casting::*, safe_math::*};

pub enum UserBorrowPositionStatus {
    Active,
    Paused,
    NotSet,
}

/// User borrow position
#[account(zero_copy)]
#[derive(InitSpace)]
#[repr(C, packed)]
pub struct UserBorrowPosition {
    pub protocol: Pubkey, // Protocol public key
    pub mint: Pubkey,     // Token mint public key

    // Previously packed in _userBorrowData
    pub with_interest: u8,      // Mode flag
    pub amount: u64,            // user borrow amount (normal or raw depends on with_interest)
    pub debt_ceiling: u64, // previous user debt ceiling (normal or raw depends on with_interest)
    pub last_update: u64,  // last triggered process timestamp
    pub expand_pct: u16, //expand debt ceiling percentage (in 1e2: 100% = 10_000; 1% = 100 -> max value 65_535).
    pub expand_duration: u32, // debt ceiling expand duration in seconds.(Max value 4_294_967_295; ~11_93_046 hours, ~49710 days)
    pub base_debt_ceiling: u64, // base debt ceiling: below this, there's no debt ceiling limits (normal or raw depends on with_interest)
    pub max_debt_ceiling: u64, // base debt ceiling: below this, there's no debt ceiling limits (normal or raw depends on with_interest)
    pub status: u8, // Pause status, 0 = false, 1 = true, if status = 2, then config yet to be set
}

impl UserBorrowPosition {
    pub fn init(&mut self, protocol: Pubkey, mint: Pubkey) -> Result<()> {
        self.protocol = protocol;
        self.mint = mint;
        self.last_update = Clock::get()?.unix_timestamp.cast()?;
        self.status = UserBorrowPositionStatus::NotSet as u8;

        Ok(())
    }

    pub fn configs_not_set(&self) -> bool {
        self.status == UserBorrowPositionStatus::NotSet as u8
    }

    pub fn is_active(&self) -> bool {
        self.status == UserBorrowPositionStatus::Active as u8
    }

    pub fn set_status_as_active(&mut self) -> Result<()> {
        self.status = UserBorrowPositionStatus::Active as u8;
        Ok(())
    }

    pub fn with_interest(&self) -> bool {
        self.with_interest == 1
    }

    pub fn set_interest_mode(&mut self, mode: u8) -> Result<()> {
        self.with_interest = mode;
        Ok(())
    }

    pub fn get_debt_ceiling(&self) -> Result<u128> {
        Ok(self.debt_ceiling.cast()?)
    }

    pub fn set_debt_ceiling(&mut self, new_debt_ceiling: u128) -> Result<()> {
        self.debt_ceiling = new_debt_ceiling.cast()?;
        Ok(())
    }

    pub fn get_base_debt_ceiling(&self) -> Result<u128> {
        Ok(self.base_debt_ceiling.cast()?)
    }

    pub fn set_base_debt_ceiling(&mut self, new_base_debt_ceiling: u128) -> Result<()> {
        self.base_debt_ceiling = new_base_debt_ceiling.cast()?;
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

    pub fn get_max_debt_ceiling(&self) -> Result<u128> {
        Ok(self.max_debt_ceiling.cast()?)
    }

    pub fn set_max_debt_ceiling(&mut self, new_max_debt_ceiling: u128) -> Result<()> {
        self.max_debt_ceiling = new_max_debt_ceiling.cast()?;
        Ok(())
    }

    pub fn get_amount(&self) -> Result<u128> {
        Ok(self.amount.cast()?)
    }

    pub fn set_amount(&mut self, new_amount: u128) -> Result<()> {
        self.amount = new_amount.cast()?;
        Ok(())
    }

    pub fn calc_borrow_limit_before_operate(
        &self,
        user_borrow_position_amount: u128,
    ) -> Result<u128> {
        // @dev must support handling the case where timestamp is 0 (config is set but no interactions yet) -> base limit.
        // first tx where timestamp is 0 will enter `if (max_expanded_borrow_limit < base_borrow_limit)` because `user_borrow_position_amount` and thus
        // `max_expansion_limit` and thus `max_expanded_borrow_limit` is 0 and `baseBorrowLimit_` can not be 0.

        // calculate max expansion limit: Max amount limit can expand to since last interaction
        // userBorrow_ needs to be at least 1e15 to overflow max limit of ~1e19 in u64
        let max_expansion_limit: u128 = user_borrow_position_amount
            .safe_mul(self.expand_pct.cast()?)?
            .safe_div(FOUR_DECIMALS)?;

        // calculate max borrow limit: Max point limit can increase to since last interaction
        let max_expanded_borrow_limit: u128 =
            user_borrow_position_amount.safe_add(max_expansion_limit)?;

        // Get base borrow limit
        let base_borrow_limit: u128 = self.get_base_debt_ceiling()?;

        // If max expanded limit is less than base limit, return base limit
        if max_expanded_borrow_limit < base_borrow_limit {
            return Ok(base_borrow_limit);
        }

        // time elapsed since last borrow limit was set (in seconds)
        let time_since_last_update: u128 = Clock::get()?
            .unix_timestamp
            .safe_sub(self.last_update.cast()?)?
            .cast()?;

        let previous_borrow_limit: u128 = self.get_debt_ceiling()?;

        // Calculate expanded borrowable amount + last set borrow limit
        // Calculate borrow limit expansion since last interaction
        let mut current_borrow_limit: u128 = max_expansion_limit
            .safe_mul(time_since_last_update)?
            .safe_div(self.expand_duration.cast()?)?
            .safe_add(previous_borrow_limit)?;

        // If time elapsed is bigger than expand duration, new borrow limit would be > max expansion,
        // so set to max_expanded_borrow_limit in that case
        if current_borrow_limit > max_expanded_borrow_limit {
            current_borrow_limit = max_expanded_borrow_limit;
        }

        // Get hard max borrow limit. Above this user can never borrow (not expandable above)
        let max_borrow_limit: u128 = self.get_max_debt_ceiling()?;

        // If current limit exceeds hard max limit, cap it
        if current_borrow_limit > max_borrow_limit {
            current_borrow_limit = max_borrow_limit;
        }

        Ok(current_borrow_limit)
    }

    pub fn calc_borrow_limit_after_operate(
        &self,
        user_borrow_position_amount: u128,
        new_borrow_limit: u128,
    ) -> Result<u128> {
        // extract borrow expand percent (is in 1e2 decimals)
        let borrow_expand_pct: u128 = self.expand_pct.cast()?;

        // Calculate maximum borrow limit at full expansion
        // userBorrow_ needs to be at least 1e15 to overflow max limit of ~1e19 in u64
        let mut borrow_limit: u128 = user_borrow_position_amount.safe_add(
            user_borrow_position_amount
                .safe_mul(borrow_expand_pct)?
                .safe_div(FOUR_DECIMALS)?,
        )?;

        // Get base borrow limit
        let base_borrow_limit: u128 = self.get_base_debt_ceiling()?;

        // If below base limit, borrow limit is always base limit
        if borrow_limit < base_borrow_limit {
            return Ok(base_borrow_limit);
        }

        // Get hard max borrow limit. Above this user can never borrow (not expandable above)
        let max_borrow_limit: u128 = self.get_max_debt_ceiling()?;

        // Make sure fully expanded borrow limit is not above hard max borrow limit
        if borrow_limit > max_borrow_limit {
            borrow_limit = max_borrow_limit;
        }

        // If new borrow limit (from before operate) is > max borrow limit, set max borrow limit
        // (e.g. on a repay shrinking instantly to fully expanded borrow limit from new borrow amount. shrinking is instant)
        if new_borrow_limit > borrow_limit {
            return Ok(borrow_limit);
        }

        Ok(new_borrow_limit)
    }

    pub fn borrow_or_payback(
        &mut self,
        amount_: i128,
        borrow_exchange_price: u128,
    ) -> Result<(i128, i128)> {
        if !self.is_active() {
            return Err(ErrorCodes::UserPaused.into());
        }

        // extract user borrow amount
        let mut user_borrow_position_amount: u128 = self.get_amount()?;

        // calculate current, updated (expanded etc.) borrow limit
        let current_borrow_limit: u128 =
            self.calc_borrow_limit_before_operate(user_borrow_position_amount)?;

        // calculate updated user borrow amount
        let mut new_borrow_interest_raw: i128 = 0;
        let mut new_borrow_interest_free: i128 = 0;

        if self.with_interest() {
            // with interest
            if amount_ > 0 {
                // convert amount from normal to raw (divide by exchange price) -> round up for borrow
                new_borrow_interest_raw = amount_
                    .safe_mul(EXCHANGE_PRICES_PRECISION.cast()?)?
                    .safe_div_ceil(borrow_exchange_price.cast()?)?
                    .cast()?;

                user_borrow_position_amount =
                    user_borrow_position_amount.safe_add(new_borrow_interest_raw.cast()?)?;
            } else {
                // convert amount from normal to raw (divide by exchange price) -> round down for payback
                new_borrow_interest_raw = amount_
                    .safe_mul(EXCHANGE_PRICES_PRECISION.cast()?)?
                    .safe_div(borrow_exchange_price.cast()?)?
                    .cast()?;

                user_borrow_position_amount =
                    user_borrow_position_amount.safe_sub(new_borrow_interest_raw.abs().cast()?)?;
            }

            if new_borrow_interest_raw == 0 {
                return Err(ErrorCodes::OperateAmountsInsufficient.into());
            }
        } else {
            // without interest
            new_borrow_interest_free = amount_;
            if new_borrow_interest_free > 0 {
                // borrowing
                user_borrow_position_amount =
                    user_borrow_position_amount.safe_add(new_borrow_interest_free.cast()?)?;
            } else {
                // payback
                user_borrow_position_amount =
                    user_borrow_position_amount.safe_sub(new_borrow_interest_free.abs().cast()?)?;
            }
        }

        if amount_ > 0 && user_borrow_position_amount > current_borrow_limit {
            // if borrow, then check the user borrow amount after borrowing is below borrow limit
            return Err(ErrorCodes::BorrowLimitReached.into());
        }

        // calculate borrow limit to store as previous borrow limit in storage
        let new_borrow_limit = self
            .calc_borrow_limit_after_operate(user_borrow_position_amount, current_borrow_limit)?;

        self.last_update = Clock::get()?.unix_timestamp.cast()?;
        self.set_amount(user_borrow_position_amount)?;
        self.set_debt_ceiling(new_borrow_limit)?;

        Ok((new_borrow_interest_raw, new_borrow_interest_free))
    }
}
