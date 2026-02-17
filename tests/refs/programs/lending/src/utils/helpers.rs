use anchor_lang::prelude::*;

use anchor_spl::token_interface::Mint;

use lending_reward_rate_model::state::LendingRewardsRateModel;

use crate::constant::*;
use crate::errors::*;
use crate::events::*;
use crate::state::state::*;

use library::math::{casting::*, safe_math::*};
use liquidity::state::TokenReserve;

pub fn convert_to_assets(
    lending: &Account<Lending>,
    f_token_mint: &InterfaceAccount<Mint>,
    current_rate_model: &Account<LendingRewardsRateModel>,
    liquidity_exchange_price: u64,
    shares: u64,
    round_up: bool,
) -> Result<u64> {
    let token_exchange_price = calculate_new_token_exchange_price(
        liquidity_exchange_price,
        lending,
        current_rate_model,
        f_token_mint.supply,
    )?;

    let assets = if round_up {
        shares
            .cast::<u128>()?
            .safe_mul(token_exchange_price.cast()?)?
            .safe_div_ceil(EXCHANGE_PRICES_PRECISION)?
            .cast::<u64>()?
    } else {
        shares
            .cast::<u128>()?
            .safe_mul(token_exchange_price.cast()?)?
            .safe_div(EXCHANGE_PRICES_PRECISION)?
            .cast::<u64>()?
    };

    Ok(assets)
}

pub fn total_assets(
    f_token_mint: &InterfaceAccount<Mint>,
    liquidity_exchange_price: u64,
    lending: &Account<Lending>,
    current_rate_model: &Account<LendingRewardsRateModel>,
) -> Result<u64> {
    let total_supply = f_token_mint.supply;

    let token_exchange_price = calculate_new_token_exchange_price(
        liquidity_exchange_price,
        lending,
        current_rate_model,
        f_token_mint.supply,
    )?;

    // totalAssets = (tokenExchangePrice * totalSupply) / EXCHANGE_PRICES_PRECISION
    let total_assets: u64 = token_exchange_price
        .cast::<u128>()?
        .safe_mul(total_supply.cast()?)?
        .safe_div(EXCHANGE_PRICES_PRECISION)?
        .cast::<u64>()?;

    Ok(total_assets)
}

pub fn get_liquidity_balance(
    user_supply_position_amount: u64,
    liquidity_exchange_price: u64,
) -> Result<u64> {
    // Convert to actual balance using exchange price
    let balance: u64 = user_supply_position_amount
        .cast::<u128>()?
        .safe_mul(liquidity_exchange_price.cast()?)?
        .safe_div(EXCHANGE_PRICES_PRECISION)?
        .cast::<u64>()?;

    Ok(balance)
}

pub fn get_liquidity_exchange_price<'info>(
    token_reserve: &AccountLoader<'info, TokenReserve>,
) -> Result<u64> {
    let token_reserve = token_reserve.load()?;
    let (supply_exchange_price, _) = token_reserve.calculate_exchange_prices()?;
    Ok(supply_exchange_price.cast()?)
}

pub fn calculate_new_token_exchange_price(
    new_liquidity_exchange_price: u64,
    lending: &Lending,
    current_rate_model: &LendingRewardsRateModel,
    f_token_total_supply: u64,
) -> Result<u64> {
    let old_token_exchange_price: u64 = lending.token_exchange_price;
    let old_liquidity_exchange_price: u64 = lending.liquidity_exchange_price;

    if new_liquidity_exchange_price < old_liquidity_exchange_price {
        return Err(ErrorCodes::FTokenLiquidityExchangePriceUnexpected.into());
    }

    let curr_timestamp: u128 = Clock::get()?.unix_timestamp.cast()?;
    let total_assets = old_token_exchange_price
        .cast::<u128>()?
        .safe_mul(f_token_total_supply.cast()?)?
        .safe_div(EXCHANGE_PRICES_PRECISION)?
        .cast()?;

    let (
        mut current_rewards_rate,
        current_start_time,
        current_end_time,
        next_start_time,
        next_end_time,
        mut next_rewards_rate,
    ) = current_rate_model.get_rate(total_assets)?;

    let mut last_update_timestamp: u64 = lending.last_update_timestamp;
    let mut total_rewards_return: u128 = 0;

    // Process current rewards period if applicable
    if current_start_time > 0
        && current_rewards_rate > 0
        && last_update_timestamp < current_end_time
    {
        if current_rewards_rate > MAX_REWARDS_RATE {
            current_rewards_rate = 0;
        }

        // Ensure we don't start before the actual rewards start time
        if last_update_timestamp < current_start_time {
            last_update_timestamp = current_start_time;
        }

        let current_period_end = if curr_timestamp < current_end_time.cast()? {
            curr_timestamp // Still in current rewards period
        } else {
            current_end_time.cast()? // Current rewards have ended, but we process up to end_time
        };

        // Only process if there's actually time to account for
        if current_period_end > last_update_timestamp.cast()? {
            let time_diff: u128 = current_period_end - last_update_timestamp.cast::<u128>()?;
            let current_rewards_return = current_rewards_rate
                .cast::<u128>()?
                .safe_mul(time_diff)?
                .safe_div(SECONDS_PER_YEAR)?;

            total_rewards_return = total_rewards_return.safe_add(current_rewards_return)?;

            // Update the tracking timestamp
            last_update_timestamp = current_period_end.cast()?;
        }
    }

    // Process next rewards period if applicable
    // This handles the case where current rewards ended and next rewards have started
    if next_start_time > 0
        && next_end_time > 0
        && next_rewards_rate > 0
        && curr_timestamp > next_start_time.cast()?
    {
        if next_rewards_rate > MAX_REWARDS_RATE.cast()? {
            next_rewards_rate = 0;
        }

        // Determine the start of the next period we need to process
        let next_period_start = if last_update_timestamp < next_start_time {
            next_start_time.cast()?
        } else {
            last_update_timestamp.cast()?
        };

        // Determine the end of the next period we need to process
        let next_period_end = if curr_timestamp < next_end_time.cast()? {
            curr_timestamp // Still in next rewards period
        } else {
            next_end_time.cast()? // Next rewards have also ended
        };

        // Only process if there's actually time to account for
        if next_period_end > next_period_start {
            let time_diff = next_period_end - next_period_start;
            let next_rewards_return = next_rewards_rate
                .cast::<u128>()?
                .safe_mul(time_diff)?
                .safe_div(SECONDS_PER_YEAR)?;

            total_rewards_return = total_rewards_return.safe_add(next_rewards_return)?;
        }
    }

    // Calculate total return including both liquidity gains and rewards
    let liquidity_return_percent = new_liquidity_exchange_price
        .safe_sub(old_liquidity_exchange_price)?
        .cast::<u128>()?
        .safe_mul(RETURN_PERCENT_PRECISION)?
        .safe_div(old_liquidity_exchange_price.cast()?)?;

    let total_return_in_percent = total_rewards_return.safe_add(liquidity_return_percent)?;

    // Calculate new token exchange price
    let new_token_exchange_price: u64 = old_token_exchange_price.safe_add(
        old_token_exchange_price
            .cast::<u128>()?
            .safe_mul(total_return_in_percent)?
            .safe_div(RETURN_PERCENT_PRECISION)?
            .cast()?,
    )?;

    Ok(new_token_exchange_price)
}

pub fn update_rates(
    lending: &mut Account<Lending>,
    f_token_mint: &InterfaceAccount<Mint>,
    current_rate_model: &Account<LendingRewardsRateModel>,
    liquidity_exchange_price: u64,
) -> Result<u64> {
    let token_exchange_price = calculate_new_token_exchange_price(
        liquidity_exchange_price,
        lending,
        current_rate_model,
        f_token_mint.supply,
    )?;

    lending.token_exchange_price = token_exchange_price;
    lending.liquidity_exchange_price = liquidity_exchange_price;
    lending.last_update_timestamp = Clock::get()?.unix_timestamp.cast()?;

    emit!(LogUpdateRates {
        token_exchange_price,
        liquidity_exchange_price,
    });

    Ok(token_exchange_price)
}
