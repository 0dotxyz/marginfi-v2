use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_pack::Pack;
use spl_token::state::Mint;

use crate::constants::JUP_LEND_ACCOUNTS_COUNT;
use crate::errors::ErrorCodes;
use crate::state::{Price, Sources};

use lending::state::Lending;
use lending::utils::calculate_new_token_exchange_price;
use lending_reward_rate_model::state::LendingRewardsRateModel;
use liquidity::state::TokenReserve;

use library::math::casting::Cast;

// JupLend prices stores exchange_price in 12 decimals of precision.
const JUP_LEND_DECIMALS: u8 = 12;

// 4 consecutive accounts for JupLend Sources
pub fn validate_jup_lend_sources(sources: &[Sources]) -> Result<()> {
    if sources.len() != JUP_LEND_ACCOUNTS_COUNT {
        return err!(ErrorCodes::InvalidSourcesLength);
    }

    for source in sources.iter() {
        if !source.is_valid() {
            return err!(ErrorCodes::InvalidParams);
        }
        if !source.is_jup_lend_source() {
            return err!(ErrorCodes::InvalidSource);
        }
    }

    Ok(())
}

/// Reads and calculates fresh exchange rate from JupLend
pub fn read_jup_lend_source(
    lending_account: &AccountInfo,
    token_reserve_account: &AccountInfo,
    rate_model_account: &AccountInfo,
    f_token_mint_account: &AccountInfo,
) -> Result<Price> {
    let lending = Lending::try_deserialize(&mut lending_account.data.borrow().as_ref())?;

    let rate_model =
        LendingRewardsRateModel::try_deserialize(&mut rate_model_account.data.borrow().as_ref())?;

    let token_reserve =
        TokenReserve::try_deserialize(&mut token_reserve_account.data.borrow().as_ref())?;

    if lending.token_reserves_liquidity != *token_reserve_account.key {
        return err!(ErrorCodes::JupLendAccountMismatch);
    }
    if lending.rewards_rate_model != *rate_model_account.key {
        return err!(ErrorCodes::JupLendAccountMismatch);
    }
    if lending.f_token_mint != *f_token_mint_account.key {
        return err!(ErrorCodes::JupLendAccountMismatch);
    }

    let mint_data = f_token_mint_account.try_borrow_data()?;
    let f_token_mint = Mint::unpack_from_slice(&mint_data)?;
    let f_token_total_supply = f_token_mint.supply;

    let (liquidity_exchange_price, _) = token_reserve.calculate_exchange_prices()?;

    let token_exchange_price = calculate_new_token_exchange_price(
        liquidity_exchange_price.cast()?,
        &lending,
        &rate_model,
        f_token_total_supply,
    )?;

    if token_exchange_price == 0 {
        return err!(ErrorCodes::InvalidPrice);
    }

    Ok(Price {
        price: token_exchange_price as u128,
        exponent: Some(JUP_LEND_DECIMALS),
    })
}
