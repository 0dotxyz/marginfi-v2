use crate::{
    state::rate_limiter::{BankRateLimiterImpl, GroupRateLimiterImpl},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, MarginfiGroup};

/// Configure bank-level rate limits for withdraw/borrow operations.
///
/// Rate limits track net outflow (outflows - inflows) in native tokens.
/// Deposits and repays offset withdraws and borrows respectively.
///
/// Setting a limit to 0 disables rate limiting for that window.
/// Both hourly and daily windows can be configured independently.
///
/// Only the group admin can configure rate limits.
pub fn configure_bank_rate_limits(
    ctx: Context<ConfigureBankRateLimits>,
    hourly_max_outflow: Option<u64>,
    daily_max_outflow: Option<u64>,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;
    let clock = Clock::get()?;

    if let Some(hourly) = hourly_max_outflow {
        bank.rate_limiter
            .configure_hourly(hourly, clock.unix_timestamp);
        msg!(
            "Bank hourly rate limit configured: {} native tokens",
            hourly
        );
    }

    if let Some(daily) = daily_max_outflow {
        bank.rate_limiter
            .configure_daily(daily, clock.unix_timestamp);
        msg!("Bank daily rate limit configured: {} native tokens", daily);
    }

    Ok(())
}

#[derive(Accounts)]
pub struct ConfigureBankRateLimits<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}

/// Configure group-level rate limits for aggregate withdraw/borrow operations.
///
/// Rate limits track net outflow in USD.
/// Deposits and repays offset withdraws and borrows respectively.
///
/// Example: $10M daily limit = 10_000_000
///
/// Setting a limit to 0 disables rate limiting for that window.
/// Both hourly and daily windows can be configured independently.
///
/// Only the group admin can configure rate limits.
pub fn configure_group_rate_limits(
    ctx: Context<ConfigureGroupRateLimits>,
    hourly_max_outflow_usd: Option<u64>,
    daily_max_outflow_usd: Option<u64>,
) -> MarginfiResult {
    let mut group = ctx.accounts.marginfi_group.load_mut()?;
    let clock = Clock::get()?;

    if let Some(hourly) = hourly_max_outflow_usd {
        group
            .rate_limiter
            .configure_hourly(hourly, clock.unix_timestamp);
        msg!("Group hourly rate limit configured: {} USD", hourly);
    }

    if let Some(daily) = daily_max_outflow_usd {
        group
            .rate_limiter
            .configure_daily(daily, clock.unix_timestamp);
        msg!("Group daily rate limit configured: {} USD", daily);
    }

    Ok(())
}

#[derive(Accounts)]
pub struct ConfigureGroupRateLimits<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,
}
