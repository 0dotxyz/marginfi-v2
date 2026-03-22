use crate::{
    check, errors::MarginfiError, state::marginfi_group::MarginfiGroupImpl, MarginfiResult,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;

pub fn configure_deleverage_withdrawal_limit(
    ctx: Context<ConfigureDeleverageWithdrawalLimit>,
    daily_withdrawal_limit: u32,
) -> MarginfiResult {
    let mut marginfi_group = ctx.accounts.marginfi_group.load_mut()?;

    check!(
        daily_withdrawal_limit > 0,
        MarginfiError::ZeroWithdrawalLimit
    );

    msg!(
        "daily withdrawal limit set to: {:?} was {:?}",
        daily_withdrawal_limit,
        marginfi_group.deleverage_withdraw_window_cache.daily_limit
    );

    let clock = Clock::get()?;
    marginfi_group.deleverage_withdraw_window_cache.daily_limit = daily_withdrawal_limit;
    marginfi_group
        .deleverage_withdraw_window_cache
        .last_daily_reset_timestamp = clock.unix_timestamp;

    Ok(())
}

#[derive(Accounts)]
pub struct ConfigureDeleverageWithdrawalLimit<'info> {
    #[account(
        mut,
        constraint = marginfi_group.load()?.is_admin_or_limit_admin(admin.key()) @ MarginfiError::Unauthorized
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,
}
