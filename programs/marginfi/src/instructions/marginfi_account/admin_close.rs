use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use marginfi_type_crate::types::{MarginfiAccount, MarginfiGroup, SECONDS_PER_DAY};

use crate::{
    check,
    events::{AccountEventHeader, AdminCloseAccountEvent},
    state::marginfi_account::MarginfiAccountImpl,
    MarginfiError, MarginfiResult,
};

/// Permissionless instruction to close accounts that are empty and have been inactive for >60
/// days. Inactivity is accepted from either the `was_active_60d` indexer flag (for memcmp
/// discovery by indexers) or `clock - last_update > 60d` (so a pulse is not required).
/// The account must also have no blocking flags (disabled, flashloan, receivership).
/// Rent is returned to the group's global fee wallet.
pub fn admin_close_account(ctx: Context<AdminCloseAccount>) -> MarginfiResult {
    let marginfi_account = ctx.accounts.marginfi_account.load()?;
    let clock = Clock::get()?;
    let elapsed = clock
        .unix_timestamp
        .saturating_sub(marginfi_account.last_update as i64);
    let is_inactive = marginfi_account.indexer_flags.was_active_60d == 0
        || elapsed > 60 * SECONDS_PER_DAY;

    check!(
        marginfi_account.indexer_flags.is_empty == 1 && is_inactive,
        MarginfiError::IllegalAction,
        "Account is not eligible for close (not empty or active within 60d)"
    );

    check!(
        marginfi_account.can_be_closed(),
        MarginfiError::IllegalAction,
        "Account cannot be closed"
    );

    emit!(AdminCloseAccountEvent {
        header: AccountEventHeader {
            signer: None,
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: ctx.accounts.group.key(),
        },
        global_fee_wallet: ctx.accounts.global_fee_wallet.key(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct AdminCloseAccount<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        close = global_fee_wallet
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// CHECK: Validated against group fee state cache
    #[account(
        mut,
        constraint = global_fee_wallet.key() == group.load()?.fee_state_cache.global_fee_wallet
            @ MarginfiError::InvalidGlobalFeeWallet
    )]
    pub global_fee_wallet: UncheckedAccount<'info>,
}
