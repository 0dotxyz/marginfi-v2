use anchor_lang::prelude::*;

use crate::constant::*;
use crate::events::*;
use crate::invokes::*;
use crate::state::*;
use crate::utils::helpers::{get_liquidity_exchange_price, update_rates};

use library::{
    math::{casting::*, safe_math::*},
    token::*,
};

fn withdraw_from_liquidity(ctx: &Context<Withdraw>, amount: u64) -> Result<u64> {
    let mint_key: Pubkey = ctx.accounts.mint.key();
    let f_token_mint_key: Pubkey = ctx.accounts.f_token_mint.key();

    let signer_seeds: &[&[&[u8]]] = &[&[
        LENDING_SEED,
        mint_key.as_ref(),
        f_token_mint_key.as_ref(),
        &[ctx.accounts.lending.bump],
    ]];

    let (supply_ex_price, _) = ctx.accounts.get_withdraw_accounts().operate_with_signer(
        OperateInstructionParams {
            supply_amount: -(amount.cast()?), // Negative for withdrawing
            borrow_amount: 0,
            withdraw_to: ctx.accounts.signer.key(), // withdraw to signer address itself, as this is withdraw instruction
            borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity address itself
        },
        signer_seeds,
    )?;

    Ok(supply_ex_price)
}

fn execute_withdraw(ctx: Context<Withdraw>, assets: u64) -> Result<u64> {
    let lending = &mut ctx.accounts.lending;

    let liquidity_exchange_price =
        get_liquidity_exchange_price(&ctx.accounts.supply_token_reserves_liquidity)?;

    // Update rates and get current token exchange price
    let token_exchange_price = update_rates(
        lending,
        &ctx.accounts.f_token_mint,
        &ctx.accounts.rewards_rate_model,
        liquidity_exchange_price,
    )?;

    // Burn shares for assets amount: assets * EXCHANGE_PRICES_PRECISION / token_exchange_price. Rounded up.
    // Note to be extra safe we do the shares burn before the withdrawFromLiquidity, even though that would return the
    // updated liquidityExchangePrice and thus save gas.
    let shares_to_burn: u64 = assets
        .cast::<u128>()?
        .safe_mul(EXCHANGE_PRICES_PRECISION)?
        .safe_div_ceil(token_exchange_price.cast()?)?
        .cast::<u64>()?;

    /*
        The `safe_div_ceil` function is designed to round up the result of multiplication followed by division.
        Given non-zero `assets` and the rounding-up behavior of this function, `shares_to_burn` will always
        be at least 1 if there's any remainder in the division.
        Thus, if `assets` is non-zero, `shares_to_burn` can never be 0. The nature of the function ensures
        that even the smallest fractional result (greater than 0) will be rounded up to 1. Hence, there's no need
        to check for a rounding error that results in 0.
    */

    // Burn the shares from the signer or owner
    burn(
        ctx.accounts.f_token_mint.to_account_info(),
        ctx.accounts.owner_token_account.to_account_info(),
        ctx.accounts.signer.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        shares_to_burn,
    )?;

    // Withdraw from liquidity directly to receiver
    withdraw_from_liquidity(&ctx, assets)?;

    emit!(LogWithdraw {
        sender: ctx.accounts.signer.key(),
        receiver: ctx.accounts.signer.key(),
        owner: ctx.accounts.signer.key(),
        assets,
        shares_burned: shares_to_burn,
    });

    Ok(shares_to_burn)
}

pub fn withdraw_internal(ctx: Context<Withdraw>, assets: u64) -> Result<u64> {
    let mut assets_to_withdraw = assets;

    if assets == u64::MAX {
        let f_token_balance = balance_of(&ctx.accounts.owner_token_account.to_account_info())?;

        assets_to_withdraw = ctx.accounts.preview_redeem(f_token_balance)?;
    }

    let shares_burned = execute_withdraw(ctx, assets_to_withdraw)?;

    Ok(shares_burned)
}

pub fn redeem_internal(ctx: Context<Withdraw>, shares: u64) -> Result<u64> {
    let mut shares_to_redeem = shares;

    if shares == u64::MAX {
        shares_to_redeem = balance_of(&ctx.accounts.owner_token_account.to_account_info())?;
    }

    let assets = ctx.accounts.preview_redeem(shares_to_redeem)?;
    withdraw_internal(ctx, assets)?;

    Ok(assets)
}
