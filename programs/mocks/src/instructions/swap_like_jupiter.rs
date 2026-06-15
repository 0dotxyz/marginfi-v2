use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::{pool_auth_signer_seeds, state::PoolAuth};

#[derive(Accounts)]
pub struct SwapLikeJupiter<'info> {
    pub user_authority: Signer<'info>,

    /// PDA authority of the pools
    /// CHECK: this is a mock program, security doesn't matter
    pub pool_auth: Account<'info, PoolAuth>,

    #[account(mut)]
    pub pool_a: Account<'info, TokenAccount>,

    #[account(mut)]
    pub pool_b: Account<'info, TokenAccount>,

    #[account(mut)]
    pub source_a: Account<'info, TokenAccount>,

    #[account(mut)]
    pub destination_b: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

impl<'info> SwapLikeJupiter<'info> {
    pub fn swap_like_jup(
        ctx: Context<'info, SwapLikeJupiter<'info>>,
        amt_a: u64,
        amt_b: u64,
    ) -> Result<()> {
        let pool_auth = &ctx.accounts.pool_auth;

        let a_before = ctx.accounts.source_a.amount;
        msg!("User a before: {:?} transfer {:?}", a_before, amt_a);
        token::transfer(ctx.accounts.transfer_a_to_pool(), amt_a)?;

        let b_before = ctx.accounts.pool_b.amount;
        msg!("Pool b before: {:?} transfer {:?}", b_before, amt_b);
        token::transfer(
            ctx.accounts
                .transfer_b_to_user()
                .with_signer(&[pool_auth_signer_seeds!(pool_auth)]),
            amt_b,
        )?;

        Ok(())
    }
}

impl<'info> SwapLikeJupiter<'info> {
    fn transfer_a_to_pool(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.key(),
            Transfer {
                from: self.source_a.to_account_info(),
                to: self.pool_a.to_account_info(),
                authority: self.user_authority.to_account_info(),
            },
        )
    }
    fn transfer_b_to_user(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        CpiContext::new(
            self.token_program.key(),
            Transfer {
                from: self.pool_b.to_account_info(),
                to: self.destination_b.to_account_info(),
                authority: self.pool_auth.to_account_info(),
            },
        )
    }
}
