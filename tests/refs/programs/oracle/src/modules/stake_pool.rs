use anchor_lang::prelude::*;
#[allow(deprecated)]
use solana_program::borsh0_10::try_from_slice_unchecked;

use crate::constants::{FACTOR, MAX_FEE_CEILING, SECONDS_PER_HOUR};
use crate::errors::ErrorCodes;
use crate::events::LogStakePoolHighFeeDetected;
use crate::state::schema::stake_pool::{Fee, FutureEpoch, StakePool};
use crate::state::Price;
use library::math::{casting::Cast, safe_math::SafeMath};

// Gives the price of 1e15 LAMPORTS staked SOL in SOL
pub fn read_stake_pool_source(stake_pool: &AccountInfo) -> Result<Price> {
    #[allow(deprecated)]
    let stake_pool_data = try_from_slice_unchecked::<StakePool>(&stake_pool.data.borrow())?;

    let now_ts = Clock::get()?.unix_timestamp as u64;
    let epoch_begin_ts = Clock::get()?.epoch_start_timestamp as u64;
    let epoch_num = Clock::get()?.epoch;

    let elapsed_since_epoch_start = now_ts.saturating_sub(epoch_begin_ts);

    let next_epoch_to_update = stake_pool_data.last_update_epoch + 1;
    if (next_epoch_to_update < epoch_num)
        || (next_epoch_to_update == epoch_num && elapsed_since_epoch_start >= SECONDS_PER_HOUR)
    {
        return err!(ErrorCodes::StakePoolNotRefreshed);
    }

    is_fee_too_high(&stake_pool_data, &stake_pool.key(), epoch_num)?;

    let price = Price {
        price: get_exchange_rate(&stake_pool_data, FACTOR)?,
        exponent: None,
    };

    Ok(price)
}

fn get_exchange_rate(stake_pool: &StakePool, multiplier: u128) -> Result<u128> {
    let scaled_pool_lamports: u128 = multiplier.safe_mul(stake_pool.total_lamports.cast()?)?;
    let total_supply: u128 = stake_pool.pool_token_supply.cast()?;

    match scaled_pool_lamports.safe_div(total_supply) {
        Ok(val) => Ok(val),
        Err(_) => err!(ErrorCodes::InvalidPrice),
    }
}

fn check_fee(fee: &Fee) -> Result<u64> {
    if fee.denominator == 0 {
        // if denominator is 0, then the fee is 0
        return Ok(0);
    }

    let fee_scaled = fee.numerator.safe_mul(1000)?.safe_div(fee.denominator)?;

    if fee_scaled > MAX_FEE_CEILING {
        return err!(ErrorCodes::FeeTooHigh);
    }

    Ok(fee_scaled)
}

fn check_future_fee(
    future_fee: &FutureEpoch<Fee>,
    stake_pool_key: &Pubkey,
    epoch: u64,
) -> Result<()> {
    match future_fee {
        FutureEpoch::None => Ok(()),
        FutureEpoch::One(fee) | FutureEpoch::Two(fee) => {
            if fee.denominator == 0 {
                return Ok(());
            }

            let fee_scaled: u64 = fee.numerator.safe_mul(1000)?.safe_div(fee.denominator)?;

            if fee_scaled > MAX_FEE_CEILING {
                emit!(LogStakePoolHighFeeDetected {
                    stake_pool: *stake_pool_key,
                    epoch,
                });
            }

            Ok(())
        }
    }
}

fn is_fee_too_high(stake_pool: &StakePool, stake_pool_key: &Pubkey, epoch: u64) -> Result<()> {
    // Check current fees (fail if too high)
    check_fee(&stake_pool.sol_withdrawal_fee)?;
    check_fee(&stake_pool.stake_withdrawal_fee)?;
    check_fee(&stake_pool.sol_deposit_fee)?;
    check_fee(&stake_pool.stake_deposit_fee)?;

    // check future fees, if change is queued in next epoch
    check_future_fee(&stake_pool.next_sol_withdrawal_fee, stake_pool_key, epoch)?;
    check_future_fee(&stake_pool.next_stake_withdrawal_fee, stake_pool_key, epoch)?;

    Ok(())
}
