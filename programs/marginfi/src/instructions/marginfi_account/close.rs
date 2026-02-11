use anchor_lang::prelude::*;
use marginfi_type_crate::types::{MarginfiAccount, ACCOUNT_FROZEN};

use crate::{check, state::marginfi_account::MarginfiAccountImpl, MarginfiError, MarginfiResult};

pub fn close_account(ctx: Context<MarginfiAccountClose>) -> MarginfiResult {
    let marginfi_account = &ctx.accounts.marginfi_account.load()?;

    if marginfi_account.get_flag(ACCOUNT_FROZEN) {
        return err!(MarginfiError::AccountFrozen);
    }

    check!(
        marginfi_account.can_be_closed(),
        MarginfiError::IllegalAction,
        "Account cannot be closed"
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
}
