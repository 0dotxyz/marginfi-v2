use anchor_lang::prelude::*;

use crate::utils::token::handle_transfer_or_claim;
use crate::{constants::*, errors::*, events::*, state::*};
use library::{
    math::{casting::*, safe_math::*},
    structs::TokenTransferParams,
    token::*,
};

/// @notice Pre-operate function to set interacting protocol and timestamp before calling the operate function
/// @param _mint mint to operate on
pub fn pre_operate(ctx: Context<PreOperate>, _mint: Pubkey) -> Result<()> {
    let mut token_reserve = ctx.accounts.token_reserve.load_mut()?;

    // One of the positions must be present
    if ctx.accounts.user_supply_position.is_none() && ctx.accounts.user_borrow_position.is_none() {
        return Err(ErrorCodes::InvalidParams.into());
    }

    // set interacting protocol and timestamp
    token_reserve.interacting_protocol = ctx.accounts.protocol.key();
    token_reserve.interacting_timestamp = Clock::get()?.unix_timestamp.cast()?;
    token_reserve.interacting_balance = balance_of(&ctx.accounts.vault.to_account_info())?;

    Ok(())
}

/// @notice Single function which handles supply, withdraw, borrow & payback
/// @param supply_amount if +ve then supply, if -ve then withdraw, if 0 then nothing
/// @param borrow_amount if +ve then borrow, if -ve then payback, if 0 then nothing
/// @param withdraw_to address to withdraw to
/// @param borrow_to address to borrow to
/// @param mint mint to operate on
/// @return supply_exchange_price updated supplyExchangePrice
/// @return borrow_exchange_price updated borrowExchangePrice
pub fn operate(
    ctx: Context<Operate>,
    supply_amount: i128, // Used max available in rust i128.
    borrow_amount: i128,
    withdraw_to: Pubkey,
    borrow_to: Pubkey,
    transfer_type: TransferType,
) -> Result<(u64, u64)> {
    if supply_amount.unsigned_abs() < MIN_OPERATE_AMOUNT
        && borrow_amount.unsigned_abs() < MIN_OPERATE_AMOUNT
    {
        return Err(ErrorCodes::OperateAmountsNearlyZero.into());
    }

    if supply_amount.unsigned_abs() > MAX_OPERATE || borrow_amount.unsigned_abs() > MAX_OPERATE {
        return Err(ErrorCodes::OperateAmountTooBig.into());
    }

    if (supply_amount < 0 && withdraw_to == Pubkey::default())
        || (borrow_amount > 0 && borrow_to == Pubkey::default())
    {
        return Err(ErrorCodes::UserNotDefined.into());
    }

    // operateAmountIn: deposit + payback
    let operate_amount_in: u128 = (supply_amount)
        .max(0)
        .safe_add((-borrow_amount).max(0))?
        .cast()?;

    let mut token_reserve = ctx.accounts.token_reserve.load_mut()?;
    let mint = ctx.accounts.mint.clone();

    if operate_amount_in > 0 {
        // check if the protocol interacting with LL is the same as the one in the token reserve
        if token_reserve.interacting_protocol != ctx.accounts.protocol.key()
            || token_reserve.interacting_timestamp != Clock::get()?.unix_timestamp.cast::<u64>()?
        {
            return Err(ErrorCodes::DepositExpected.into());
        }

        let last_recorded_balance: u128 = token_reserve.get_interacting_balance()?;

        let final_recorded_balance: u128 =
            balance_of(&ctx.accounts.vault.to_account_info())?.cast()?;

        // final balance - initial balance
        let net_amount_in: u128 = final_recorded_balance.safe_sub(last_recorded_balance)?;

        if net_amount_in < operate_amount_in
            || net_amount_in
                > operate_amount_in
                    .safe_mul(FOUR_DECIMALS.safe_add(MAX_INPUT_AMOUNT_EXCESS)?)?
                    .safe_div(FOUR_DECIMALS)?
        {
            return Err(ErrorCodes::TransferAmountOutOfBounds.into());
        }
    }

    let (supply_exchange_price, borrow_exchange_price) =
        token_reserve.calculate_exchange_prices()?;

    if supply_amount != 0 {
        let mut user_supply_position = ctx
            .accounts
            .user_supply_position
            .as_mut()
            .ok_or(ErrorCodes::UserSupplyPositionRequired)?
            .load_mut()?;

        if user_supply_position.mint != mint.key() {
            return Err(ErrorCodes::MintMismatch.into());
        }

        let (new_supply_interest_raw, new_supply_interest_free) =
            user_supply_position.supply_or_withdraw(supply_amount, supply_exchange_price)?;

        if new_supply_interest_free == 0 {
            token_reserve.set_new_total_supply_with_interest(new_supply_interest_raw)?;
        } else {
            token_reserve.set_new_total_supply_interest_free(new_supply_interest_free)?;
        }
    }

    if borrow_amount != 0 {
        let mut user_borrow_position = ctx
            .accounts
            .user_borrow_position
            .as_mut()
            .ok_or(ErrorCodes::UserBorrowPositionRequired)?
            .load_mut()?;

        if user_borrow_position.mint != mint.key() {
            return Err(ErrorCodes::MintMismatch.into());
        }

        let (new_borrow_interest_raw, new_borrow_interest_free) =
            user_borrow_position.borrow_or_payback(borrow_amount, borrow_exchange_price)?;

        if new_borrow_interest_free == 0 {
            token_reserve.set_new_total_borrow_with_interest(new_borrow_interest_raw)?;
        } else {
            // borrow or payback interest free -> normal amount
            token_reserve.set_new_total_borrow_interest_free(new_borrow_interest_free)?;
        }
    }

    // calculate utilization. If there is no supply, utilization must be 0 (avoid division by 0)
    // Calculate utilization over scaled values to avoid precision loss from rounding.
    let utilization: u16 = token_reserve.calc_utilization(
        supply_exchange_price,
        borrow_exchange_price,
        Some(supply_amount),
        Some(borrow_amount),
    )?;

    // for borrow operations, ensure max utilization is not reached
    if borrow_amount > 0 && utilization > token_reserve.max_utilization {
        return Err(ErrorCodes::MaxUtilizationReached.into());
    }

    let rate_model = ctx.accounts.rate_model.load()?;
    let new_borrow_rate: u16 = rate_model.calc_borrow_rate_from_utilization(utilization.cast()?)?;

    // Update the TokenReserve account with new values
    token_reserve.borrow_rate = new_borrow_rate;
    token_reserve.last_utilization = utilization;
    token_reserve.last_update_timestamp = Clock::get()?.unix_timestamp.cast()?;

    // update exchange prices
    token_reserve.supply_exchange_price = supply_exchange_price.cast()?;
    token_reserve.borrow_exchange_price = borrow_exchange_price.cast()?;

    // Reset interacting state
    token_reserve.reset_interacting_state()?;

    // Sending tokens to user at the end after updating everything
    // Only transfer to user in case of withdraw or borrow
    // Do not transfer for same amounts in same operate(): supply(+) == borrow(+), withdraw(-) == payback(-) (DEX protocol use-case)
    if supply_amount < 0 || borrow_amount > 0 {
        // Set up the amounts to transfer
        let borrow_transfer_amount: u64 = (borrow_amount).max(0).cast()?;
        let withdraw_transfer_amount: u64 = (-supply_amount).max(0).cast()?;
        let liquidity_seeds: &[&[u8]] = &[LIQUIDITY_SEED, &[ctx.accounts.liquidity.bump]];

        if withdraw_transfer_amount > 0 && borrow_transfer_amount > 0 && withdraw_to == borrow_to {
            // If user is doing borrow & withdraw together and address for both is the same
            // Then transfer tokens of borrow & withdraw together
            let total_amount: u64 = withdraw_transfer_amount.safe_add(borrow_transfer_amount)?;

            let withdraw_to_account = ctx
                .accounts
                .withdraw_to_account
                .as_ref()
                .ok_or(ErrorCodes::WithdrawToAccountRequired)?;

            let claim_amount = handle_transfer_or_claim(
                &transfer_type,
                withdraw_to,
                ctx.accounts.withdraw_claim_account.as_ref(),
                token_reserve.total_claim_amount,
                TokenTransferParams {
                    source: ctx.accounts.vault.to_account_info(),
                    destination: withdraw_to_account.to_account_info(),
                    authority: ctx.accounts.liquidity.to_account_info(), // the liquidity PDA owns the authority to transfer the tokens
                    amount: total_amount,
                    token_program: ctx.accounts.token_program.to_account_info(),
                    signer_seeds: Some(&[&liquidity_seeds]),
                    mint: ctx.accounts.mint.clone(),
                },
            )?;

            if claim_amount > 0 {
                token_reserve.add_claim_amount(claim_amount)?;
            }
        } else {
            if withdraw_transfer_amount > 0 {
                let withdraw_to_account = ctx
                    .accounts
                    .withdraw_to_account
                    .as_ref()
                    .ok_or(ErrorCodes::WithdrawToAccountRequired)?;

                let claim_amount = handle_transfer_or_claim(
                    &transfer_type,
                    withdraw_to,
                    ctx.accounts.withdraw_claim_account.as_ref(),
                    token_reserve.total_claim_amount,
                    TokenTransferParams {
                        source: ctx.accounts.vault.to_account_info(),
                        destination: withdraw_to_account.to_account_info(),
                        authority: ctx.accounts.liquidity.to_account_info(), // the liquidity PDA owns the authority to transfer the tokens
                        amount: withdraw_transfer_amount,
                        token_program: ctx.accounts.token_program.to_account_info(),
                        signer_seeds: Some(&[&liquidity_seeds]),
                        mint: ctx.accounts.mint.clone(),
                    },
                )?;

                if claim_amount > 0 {
                    token_reserve.add_claim_amount(claim_amount)?;
                }
            }

            if borrow_transfer_amount > 0 {
                let borrow_to_account = ctx
                    .accounts
                    .borrow_to_account
                    .as_ref()
                    .ok_or(ErrorCodes::BorrowToAccountRequired)?;

                let claim_amount = handle_transfer_or_claim(
                    &transfer_type,
                    borrow_to,
                    ctx.accounts.borrow_claim_account.as_ref(),
                    token_reserve.total_claim_amount,
                    TokenTransferParams {
                        source: ctx.accounts.vault.to_account_info(),
                        destination: borrow_to_account.to_account_info(),
                        authority: ctx.accounts.liquidity.to_account_info(), // the liquidity PDA owns the authority to transfer the tokens
                        amount: borrow_transfer_amount,
                        token_program: ctx.accounts.token_program.to_account_info(),
                        signer_seeds: Some(&[&liquidity_seeds]),
                        mint: ctx.accounts.mint.clone(),
                    },
                )?;

                if claim_amount > 0 {
                    token_reserve.add_claim_amount(claim_amount)?;
                }
            }
        }
    }

    emit!(LogOperate {
        user: ctx.accounts.protocol.key(),
        token: token_reserve.mint,
        supply_amount: supply_amount,
        borrow_amount: borrow_amount,
        withdraw_to: withdraw_to,
        borrow_to: borrow_to,
        supply_exchange_price: token_reserve.supply_exchange_price,
        borrow_exchange_price: token_reserve.borrow_exchange_price,
    });

    // Return exchange prices
    Ok((supply_exchange_price.cast()?, borrow_exchange_price.cast()?))
}

/// @notice Initialize claim account
/// @param mint mint to claim
/// @param user user to claim to
pub fn init_claim_account(
    ctx: Context<InitClaimAccount>,
    mint: Pubkey,
    user: Pubkey,
) -> Result<()> {
    let claim_account = &mut ctx.accounts.claim_account.load_init()?;
    claim_account.init(user, mint)?;

    Ok(())
}

/// @notice Claim function to claim tokens from the liquidity pool
/// @param mint mint to claim
/// @param recipient address to claim to
pub fn claim(ctx: Context<Claim>, recipient: Pubkey) -> Result<()> {
    let user_claim = &mut ctx.accounts.claim_account.load_mut()?;
    let mut token_reserve = ctx.accounts.token_reserve.load_mut()?;

    let amount = user_claim.balance();
    if amount == 0 {
        return Err(ErrorCodes::NoAmountToClaim.into());
    }

    token_reserve.reduce_claim_amount(amount)?;
    user_claim.reset_balance()?;

    let liquidity_seeds: &[&[u8]] = &[LIQUIDITY_SEED, &[ctx.accounts.liquidity.bump]];
    transfer_spl_tokens(TokenTransferParams {
        source: ctx.accounts.vault.to_account_info(),
        destination: ctx.accounts.recipient_token_account.to_account_info(),
        authority: ctx.accounts.liquidity.to_account_info(),
        amount: amount,
        token_program: ctx.accounts.token_program.to_account_info(),
        signer_seeds: Some(&[&liquidity_seeds]),
        mint: ctx.accounts.mint.clone(),
    })?;

    emit!(LogClaim {
        user: ctx.accounts.user.key(),
        token: ctx.accounts.mint.key(),
        recipient,
        amount: amount,
    });

    Ok(())
}
