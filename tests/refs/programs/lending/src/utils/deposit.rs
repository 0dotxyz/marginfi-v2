use anchor_lang::prelude::*;

use crate::constant::*;
use crate::errors::*;
use crate::events::*;
use crate::invokes::*;
use crate::state::*;
use crate::utils::helpers::update_rates;

use library::{
    math::{casting::*, safe_math::*},
    structs::TokenTransferParams,
    token::*,
};

fn deposit_to_liquidity(ctx: &Context<Deposit>, amount: u64) -> Result<u64> {
    let mint_key: Pubkey = ctx.accounts.mint.key();
    let f_token_mint_key: Pubkey = ctx.accounts.f_token_mint.key();

    let signer_seeds: &[&[&[u8]]] = &[&[
        LENDING_SEED,
        mint_key.as_ref(),
        f_token_mint_key.as_ref(),
        &[ctx.accounts.lending.bump],
    ]];

    let accounts = ctx.accounts.get_deposit_accounts();

    // Call pre-operate to set interacting protocol and timestamp
    accounts
        .pre_operate_with_signer(PreOperateInstructionParams { mint: mint_key }, signer_seeds)?;

    // Transfer tokens to liquidity
    transfer_spl_tokens(TokenTransferParams {
        source: ctx.accounts.depositor_token_account.to_account_info(), // depositor token account
        destination: ctx.accounts.vault.to_account_info(), // vault, aka liquidity mint PDA
        authority: ctx.accounts.signer.to_account_info(),  // msg.sender
        amount,                                            // amount
        token_program: ctx.accounts.token_program.to_account_info(), // token program
        signer_seeds: None,
        mint: ctx.accounts.mint.clone(),
    })?;

    // Call operate
    let (supply_ex_price, _) = accounts.operate_with_signer(
        OperateInstructionParams {
            supply_amount: amount.cast()?,
            borrow_amount: 0,
            withdraw_to: ctx.accounts.liquidity.key(), // withdraw to liquidity address itself, as this is deposit instruction
            borrow_to: ctx.accounts.liquidity.key(), // borrow to liquidity address itself, as this is deposit instruction
        },
        signer_seeds,
    )?;

    Ok(supply_ex_price)
}

fn execute_deposit(ctx: Context<Deposit>, amount: u64) -> Result<u64> {
    // Borrow is scoped within get_supply_position_amount() and immediately dropped
    let initial_amount = ctx.accounts.get_supply_position_amount()?; // raw amount registered in the supply position

    // CPI to liquidity - safe because previous borrow was dropped
    let mut token_exchange_price: u64 = deposit_to_liquidity(&ctx, amount)?;

    // Fresh borrow after CPI completes - no conflicts
    let final_amount = ctx.accounts.get_supply_position_amount()?; // raw amount registered in the supply position

    let registered_amount_raw = final_amount.safe_sub(initial_amount)?.cast::<u128>()?;
    let registered_amount = registered_amount_raw
        .safe_mul(token_exchange_price.cast::<u128>()?)?
        .safe_div(EXCHANGE_PRICES_PRECISION)?
        .cast::<u128>()?;

    token_exchange_price = update_rates(
        &mut ctx.accounts.lending,
        &ctx.accounts.f_token_mint,
        &ctx.accounts.rewards_rate_model,
        token_exchange_price,
    )?;

    // calculate the shares to mint
    // not using previewDeposit here because we just got newTokenExchangePrice
    let shares_minted: u64 = registered_amount
        .safe_mul(EXCHANGE_PRICES_PRECISION)?
        .safe_div(token_exchange_price.cast::<u128>()?)?
        .cast::<u64>()?;

    if shares_minted == 0 {
        return Err(ErrorCodes::FTokenDepositInsignificant.into());
    }

    let signer_seeds: &[&[&[u8]]] = &[&[LENDING_ADMIN_SEED, &[ctx.accounts.lending_admin.bump]]];

    // mint the shares to the user
    mint_with_signer(
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.f_token_mint.to_account_info(),
        ctx.accounts.lending_admin.to_account_info(),
        ctx.accounts.recipient_token_account.to_account_info(),
        signer_seeds,
        shares_minted,
    )?;

    emit!(LogDeposit {
        sender: ctx.accounts.signer.key(),
        receiver: ctx.accounts.signer.key(),
        assets: amount,
        shares_minted,
    });

    Ok(shares_minted)
}

pub fn deposit_internal(ctx: Context<Deposit>, amount: u64) -> Result<u64> {
    let mut amount_to_deposit = amount;

    if amount == u64::MAX {
        amount_to_deposit = balance_of(&ctx.accounts.depositor_token_account.to_account_info())?;
    }

    let shares_minted: u64 = execute_deposit(ctx, amount_to_deposit)?;

    Ok(shares_minted)
}

pub fn mint_internal(ctx: Context<Deposit>, shares: u64) -> Result<u64> {
    let assets = if shares == u64::MAX {
        balance_of(&ctx.accounts.depositor_token_account.to_account_info())?
    } else {
        // No need to check for rounding error, previewMint rounds up.
        ctx.accounts.preview_mint(shares)?
    };

    execute_deposit(ctx, assets)?;

    Ok(assets)
}
