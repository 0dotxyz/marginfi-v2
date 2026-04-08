use anchor_lang::prelude::*;
use marginfi_type_crate::types::{MarginfiAccount, MarginfiGroup};

use crate::{
    check,
    events::{AccountEventHeader, AdminCloseAccountEvent},
    state::marginfi_account::MarginfiAccountImpl,
    MarginfiError, MarginfiResult,
};

/// Admin-only instruction to close accounts flagged as `closeable` by pulse_health.
/// The account must have no balances and no blocking flags (disabled, flashloan, receivership).
/// Rent is returned to `rent_destination`.
pub fn admin_close_account(ctx: Context<AdminCloseAccount>) -> MarginfiResult {
    let marginfi_account = ctx.accounts.marginfi_account.load()?;

    check!(
        marginfi_account.indexer_flags.closeable == 1,
        MarginfiError::IllegalAction,
        "Account is not marked as closeable"
    );

    check!(
        marginfi_account.can_be_closed(),
        MarginfiError::IllegalAction,
        "Account cannot be closed"
    );

    emit!(AdminCloseAccountEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.admin.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: ctx.accounts.group.key(),
        },
        rent_destination: ctx.accounts.rent_destination.key(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct AdminCloseAccount<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        close = rent_destination
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        constraint = group.load()?.admin == admin.key() @ MarginfiError::Unauthorized
    )]
    pub admin: Signer<'info>,

    /// CHECK: Receives the closed account's rent. Can be any account.
    #[account(mut)]
    pub rent_destination: UncheckedAccount<'info>,
}
