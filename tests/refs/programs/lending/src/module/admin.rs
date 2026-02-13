use anchor_lang::prelude::*;
use std::collections::HashMap;

use crate::constant::*;
use crate::errors::ErrorCodes;
use crate::events::*;
use crate::state::*;
use crate::utils::{
    helpers::{get_liquidity_balance, get_liquidity_exchange_price, total_assets, update_rates},
    rebalance::execute_rebalance,
};

use library::math::{casting::*, safe_math::*};
use library::structs::AddressBool;
use liquidity::ID as LIQUIDITY_PROGRAM_ID;

pub fn init_lending_admin(
    ctx: Context<InitLendingAdmin>,
    liquidity_program: Pubkey,
    rebalancer: Pubkey,
    authority: Pubkey,
) -> Result<()> {
    let lending_admin = &mut ctx.accounts.lending_admin;

    // Ensure that the liquidity program is the same as the one in the liquidity program at compile time
    if liquidity_program != LIQUIDITY_PROGRAM_ID {
        return Err(error!(ErrorCodes::FTokenLiquidityProgramMismatch));
    }

    if rebalancer == Pubkey::default() || authority == Pubkey::default() {
        return Err(error!(ErrorCodes::FTokenInvalidParams));
    }

    lending_admin.authority = authority;
    lending_admin.liquidity_program = liquidity_program;
    lending_admin.rebalancer = rebalancer;
    lending_admin.bump = ctx.bumps.lending_admin;
    lending_admin.next_lending_id = 1;

    // Whitelist governance account as auth_user for modified OnlyAuths check
    lending_admin.auths.push(authority);

    Ok(())
}

pub fn init_lending(
    ctx: Context<InitLending>,
    symbol: String,
    liquidity_program: Pubkey,
) -> Result<()> {
    let lending = &mut ctx.accounts.lending;

    lending.lending_id = ctx.accounts.lending_admin.next_lending_id;
    ctx.accounts.lending_admin.next_lending_id = lending.lending_id.safe_add(1)?;

    lending.mint = ctx.accounts.mint.key();
    lending.f_token_mint = ctx.accounts.f_token_mint.key();
    lending.decimals = ctx.accounts.mint.decimals;
    lending.bump = ctx.bumps.lending;

    // As this is lending, we only need to consider supply exchange price
    lending.liquidity_exchange_price =
        get_liquidity_exchange_price(&ctx.accounts.token_reserves_liquidity)?;

    lending.token_exchange_price = EXCHANGE_PRICES_PRECISION.cast()?;

    lending.last_update_timestamp = Clock::get()?.unix_timestamp.cast()?;
    lending.token_reserves_liquidity = ctx.accounts.token_reserves_liquidity.key();

    // Deriving this f_token PDA address on liquidity program
    lending.supply_position_on_liquidity = Pubkey::find_program_address(
        &[
            USER_SUPPLY_POSITION_SEED,
            ctx.accounts.mint.key().as_ref(),
            lending.key().as_ref(), // lending is the signer for CPI
        ],
        &liquidity_program,
    )
    .0;

    ctx.accounts.initialize_token_metadata(symbol)?;

    Ok(())
}

pub fn update_auths(context: Context<UpdateAuths>, auth_status: Vec<AddressBool>) -> Result<()> {
    let mut auth_map: HashMap<Pubkey, bool> = context
        .accounts
        .lending_admin
        .auths
        .iter()
        .map(|addr: &Pubkey| (*addr, true))
        .collect();

    let default_pubkey: Pubkey = Pubkey::default();

    for auth in auth_status.iter() {
        if auth.addr == default_pubkey
            || (auth.addr == context.accounts.lending_admin.authority && !auth.value)
        {
            return Err(ErrorCodes::FTokenInvalidParams.into());
        }

        auth_map.insert(auth.addr, auth.value);
    }

    context.accounts.lending_admin.auths = auth_map
        .into_iter()
        .filter(|(_, value)| *value)
        .map(|(addr, _)| addr)
        .collect();

    if context.accounts.lending_admin.auths.len() > MAX_AUTH_COUNT {
        return Err(ErrorCodes::FTokenMaxAuthCountReached.into());
    }

    emit!(LogUpdateAuths {
        auth_status: auth_status.clone(),
    });

    Ok(())
}

pub fn set_rewards_rate_model(ctx: Context<SetRewardsRateModel>, _mint: Pubkey) -> Result<()> {
    let lending = &mut ctx.accounts.lending;

    if lending.rewards_rate_model != Pubkey::default() {
        return Err(ErrorCodes::FTokenRewardsRateModelAlreadySet.into());
    }

    lending.rewards_rate_model = ctx.accounts.new_rewards_rate_model.key();

    Ok(emit!(LogUpdateRewards {
        rewards_rate_model: ctx.accounts.new_rewards_rate_model.key(),
    }))
}

pub fn rebalance(ctx: Context<Rebalance>) -> Result<()> {
    let mut liquidity_exchange_price =
        get_liquidity_exchange_price(&ctx.accounts.supply_token_reserves_liquidity)?;

    let assets = total_assets(
        &ctx.accounts.f_token_mint,
        liquidity_exchange_price,
        &ctx.accounts.lending,
        &ctx.accounts.rewards_rate_model,
    )?;

    let user_supply_position_amount = ctx
        .accounts
        .lending_supply_position_on_liquidity
        .load()?
        .amount;

    let liquidity_balance =
        get_liquidity_balance(user_supply_position_amount, liquidity_exchange_price)?;

    // calculating difference in assets. if liquidity balance is bigger it'll throw which is an expected behaviour
    let assets_delta: u64 = assets.safe_sub(liquidity_balance)?;

    liquidity_exchange_price = execute_rebalance(&ctx, assets_delta)?;

    update_rates(
        &mut ctx.accounts.lending,
        &ctx.accounts.f_token_mint,
        &ctx.accounts.rewards_rate_model,
        liquidity_exchange_price,
    )?;

    Ok(emit!(LogRebalance { assets }))
}

pub fn update_rebalancer(ctx: Context<UpdateRebalancer>, new_rebalancer: Pubkey) -> Result<()> {
    if new_rebalancer == Pubkey::default() {
        return Err(ErrorCodes::FTokenInvalidParams.into());
    }

    let lending_admin = &mut ctx.accounts.lending_admin;
    lending_admin.rebalancer = new_rebalancer;
    Ok(emit!(LogUpdateRebalancer { new_rebalancer }))
}

pub fn update_authority(
    context: Context<UpdateAuthority>,
    new_authority: Pubkey,
) -> Result<()> {
    if context.accounts.signer.key() != context.accounts.lending_admin.authority {
        // second check on top of context.rs to be extra sure
        return Err(ErrorCodes::FTokenOnlyAuthority.into());
    }

    if new_authority != GOVERNANCE_MS {
        return Err(ErrorCodes::FTokenInvalidParams.into());
    }

    let old_authority = context.accounts.lending_admin.authority.clone();

    context.accounts.lending_admin.authority = new_authority;

    let mut auth_map: HashMap<Pubkey, bool> = context
        .accounts
        .lending_admin
        .auths
        .iter()
        .map(|addr: &Pubkey| (*addr, true))
        .collect();

    auth_map.remove(&old_authority);
    auth_map.insert(new_authority, true);

    context.accounts.lending_admin.auths = auth_map
        .into_iter()
        .filter(|(_, value)| *value)
        .map(|(addr, _)| addr)
        .collect();

    emit!(LogUpdateAuthority {
        new_authority: new_authority,
    });

    Ok(())
}

pub fn update_rate(ctx: Context<UpdateRate>) -> Result<()> {
    let liquidity_exchange_price =
        get_liquidity_exchange_price(&ctx.accounts.supply_token_reserves_liquidity)?;

    update_rates(
        &mut ctx.accounts.lending,
        &ctx.accounts.f_token_mint,
        &ctx.accounts.rewards_rate_model,
        liquidity_exchange_price,
    )?;

    Ok(())
}
