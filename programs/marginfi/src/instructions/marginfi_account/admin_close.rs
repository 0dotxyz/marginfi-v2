use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use marginfi_type_crate::types::{
    MarginfiAccount, MarginfiGroup, ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_ORDER_EXECUTION,
    SECONDS_PER_DAY,
};

use crate::{
    check,
    events::{AccountEventHeader, AdminCloseAccountEvent},
    state::marginfi_account::MarginfiAccountImpl,
    MarginfiError, MarginfiResult,
};

/// Permissionless instruction to close legacy or new accounts that are empty and inactive for >60
/// days. Eligibility is computed from direct account invariants (balances/flags/timestamps), not
/// indexer flags, so pre-flag accounts remain safely closeable.
pub fn admin_close_account(ctx: Context<AdminCloseAccount>) -> MarginfiResult {
    let marginfi_account = ctx.accounts.marginfi_account.load()?;
    let clock = Clock::get()?;
    let elapsed = clock
        .unix_timestamp
        .saturating_sub(marginfi_account.last_update as i64);
    let is_inactive = elapsed > 60 * SECONDS_PER_DAY;

    check!(
        marginfi_account.can_be_closed() && is_inactive,
        MarginfiError::IllegalAction,
        "Account is not eligible for close (not empty or active within 60d)"
    );

    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_DELEVERAGE)
            && !marginfi_account.get_flag(ACCOUNT_IN_ORDER_EXECUTION)
            && marginfi_account.active_orders == 0
            && marginfi_account.liquidation_record == Pubkey::default(),
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
