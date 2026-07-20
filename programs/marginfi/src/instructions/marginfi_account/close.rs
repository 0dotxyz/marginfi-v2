use anchor_lang::prelude::*;
use marginfi_type_crate::constants::REBALANCE_FEE_POOL_SEED;
use marginfi_type_crate::types::{MarginfiAccount, ACCOUNT_FROZEN};

use crate::{check, state::marginfi_account::MarginfiAccountImpl, MarginfiError, MarginfiResult};

pub fn close_account(ctx: Context<MarginfiAccountClose>) -> MarginfiResult {
    let marginfi_account = &ctx.accounts.marginfi_account.load()?;

    if marginfi_account.get_flag(ACCOUNT_FROZEN) {
        return err!(MarginfiError::AccountFrozen);
    }

    check!(
        marginfi_account.liquidation_record == Pubkey::default(),
        MarginfiError::IllegalAction,
        "Close liquidation record before closing account"
    );

    check!(
        marginfi_account.active_orders == 0,
        MarginfiError::IllegalAction,
        "Close all active orders before closing account"
    );

    check!(
        marginfi_account.can_be_closed(),
        MarginfiError::IllegalAction,
        "Account cannot be closed"
    );

    check!(
        ctx.accounts.rebalance_fee_pool.lamports() == 0,
        MarginfiError::IllegalAction,
        "Withdraw rebalance fee pool before closing account"
    );

    Ok(())
}

#[derive(Accounts)]
pub struct MarginfiAccountClose<'info> {
    #[account(
        mut,
        has_one = authority @ MarginfiError::Unauthorized,
        close = fee_payer
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    /// CHECK: the account's rebalance fee pool PDA; validated by seeds. Must be empty (drained via
    /// `withdraw_rebalance_fee_pool`) before close so its lamports are not orphaned.
    #[account(
        seeds = [REBALANCE_FEE_POOL_SEED.as_bytes(), marginfi_account.key().as_ref()],
        bump,
    )]
    pub rebalance_fee_pool: SystemAccount<'info>,
}
