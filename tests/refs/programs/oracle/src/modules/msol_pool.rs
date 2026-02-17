use anchor_lang::prelude::*;
#[allow(deprecated)]
use solana_program::borsh0_10::try_from_slice_unchecked;

use crate::constants::FACTOR;
use crate::errors::ErrorCodes;
use crate::state::schema::msol_pool::State;
use crate::state::Price;
use library::math::{casting::Cast, safe_math::SafeMath};

// Gives the price of 1e15 LAMPORTS staked SOL in SOL
pub fn read_msol_pool_source(msol_pool: &AccountInfo) -> Result<Price> {
    // 8 bytes of padding in the beginning of the account data is anchor default padding for discriminator
    #[allow(deprecated)]
    let stake_pool = try_from_slice_unchecked::<State>(&msol_pool.data.borrow()[8..])?;

    let price = Price {
        price: get_exchange_rate(&stake_pool, FACTOR)?,
        exponent: None,
    };

    Ok(price)
}

fn get_exchange_rate(stake_pool: &State, multiplier: u128) -> Result<u128> {
    let pending_unstake_lamports = stake_pool
        .stake_system
        .delayed_unstake_cooling_down
        .safe_add(stake_pool.emergency_cooling_down)?
        .cast()?;

    let total_controlled_lamports = stake_pool
        .validator_system
        .total_active_balance
        .safe_add(pending_unstake_lamports)?
        .safe_add(stake_pool.available_reserve_balance)?;

    let effective_staked_lamports = total_controlled_lamports
        .saturating_sub(stake_pool.circulating_ticket_balance)
        .cast::<u128>()?;

    let scaled_pool_lamports = effective_staked_lamports.safe_mul(multiplier)?;
    let total_supply: u128 = stake_pool.msol_supply.cast()?;

    match scaled_pool_lamports.safe_div(total_supply) {
        Ok(val) => Ok(val),
        Err(_) => err!(ErrorCodes::InvalidPrice),
    }
}
