use anchor_lang::prelude::*;

use crate::invokes::*;
use crate::state::*;

use library::math::casting::*;
use library::structs::TokenTransferParams;
use library::token::*;

pub fn execute_rebalance(ctx: &Context<Rebalance>, amount: u64) -> Result<u64> {
    let accounts = ctx.accounts.get_rebalance_accounts();
    let mint_key: Pubkey = ctx.accounts.mint.key();
    let f_token_mint_key: Pubkey = ctx.accounts.f_token_mint.key();

    let signer_seeds: &[&[&[u8]]] = &[&[
        LENDING_SEED,
        mint_key.as_ref(),
        f_token_mint_key.as_ref(),
        &[ctx.accounts.lending.bump],
    ]];

    // Call pre-operate to set interacting protocol and timestamp
    accounts
        .pre_operate_with_signer(PreOperateInstructionParams { mint: mint_key }, signer_seeds)?;

    transfer_spl_tokens(TokenTransferParams {
        source: ctx.accounts.depositor_token_account.to_account_info(), // msg.sender
        destination: ctx.accounts.vault.to_account_info(), // vault, aka liquidity mint PDA
        authority: ctx.accounts.signer.to_account_info(),  // msg.sender
        amount,                                            // amount
        token_program: ctx.accounts.token_program.to_account_info(), // token program
        signer_seeds: None,
        mint: ctx.accounts.mint.clone(),
    })?;

    // During rebalance, we are not withdrawing or borrowing, so we can pass the lending PDA and their associated ATA
    // This let us save on two extra accounts, hence reusing existing to build the correct accounts context.
    let (supply_ex_price, _) = ctx.accounts.get_rebalance_accounts().operate_with_signer(
        OperateInstructionParams {
            supply_amount: amount.cast()?,
            borrow_amount: 0,
            withdraw_to: ctx.accounts.liquidity.key(), // withdraw to liquidity address itself, as this is rebalance instruction
            borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity address itself
        },
        signer_seeds,
    )?;

    Ok(supply_ex_price)
}
