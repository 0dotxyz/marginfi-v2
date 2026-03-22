use anchor_lang::prelude::*;

use crate::constants::{
    LOWER_DECIMALS_OPERATE, MAX_TOKEN_DECIMALS, MINIMUM_BRANCH_DEBT,
    MINIMUM_BRANCH_DEBT_LOWER_DECIMALS, MINIMUM_TICK_DEBT, MINIMUM_TICK_DEBT_LOWER_DECIMALS,
    MIN_DEBT, MIN_DEBT_LOWER_DECIMALS,
};
use crate::errors::ErrorCodes;

use library::math::casting::*;
use library::math::safe_math::*;

pub fn get_minimum_tick_debt(decimals: u8) -> Result<u128> {
    let minimum_tick_debt = if decimals >= LOWER_DECIMALS_OPERATE {
        MINIMUM_TICK_DEBT
    } else {
        MINIMUM_TICK_DEBT_LOWER_DECIMALS
    };

    Ok(scale_amounts(minimum_tick_debt.cast()?, decimals)?.cast()?)
}

pub fn get_minimum_debt(decimals: u8) -> Result<u128> {
    let minimum_debt = if decimals >= LOWER_DECIMALS_OPERATE {
        MIN_DEBT
    } else {
        MIN_DEBT_LOWER_DECIMALS
    };

    Ok(scale_amounts(minimum_debt.cast()?, decimals)?.cast()?)
}

pub fn get_minimum_branch_debt(decimals: u8) -> Result<u128> {
    let minimum_branch_debt = if decimals >= LOWER_DECIMALS_OPERATE {
        MINIMUM_BRANCH_DEBT
    } else {
        MINIMUM_BRANCH_DEBT_LOWER_DECIMALS
    };

    Ok(scale_amounts(minimum_branch_debt.cast()?, decimals)?.cast()?)
}

fn get_scale(decimals: u8) -> Result<u128> {
    if decimals <= MAX_TOKEN_DECIMALS {
        Ok(10u128.pow((MAX_TOKEN_DECIMALS - decimals).cast()?))
    } else {
        return Err(error!(ErrorCodes::VaultInvalidDecimals));
    }
}

pub fn scale_amounts(amount: i128, decimals: u8) -> Result<i128> {
    let scale: u128 = get_scale(decimals)?;
    Ok(amount.safe_mul(scale.cast()?)?)
}

pub fn unscale_amounts(amount: i128, decimals: u8) -> Result<i128> {
    let scale: u128 = get_scale(decimals)?;
    Ok(amount.safe_div(scale.cast()?)?)
}

pub fn unscale_amounts_up(amount: i128, decimals: u8) -> Result<i128> {
    let scale: u128 = get_scale(decimals)?;
    Ok(amount.safe_div_ceil(scale.cast()?)?)
}