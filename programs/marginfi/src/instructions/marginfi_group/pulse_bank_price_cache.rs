use crate::state::bank::BankImpl;
use crate::state::price::{OraclePriceFeedAdapter, OraclePriceType, PriceAdapter};
use crate::state::rate_limiter::{BankRateLimiterUntrackedImpl, GroupRateLimiterImpl};
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{Bank, MarginfiGroup};

/// (permissionless) Refresh the cached oracle price for a bank.
pub fn lending_pool_pulse_bank_price_cache<'info>(
    ctx: Context<'_, '_, 'info, 'info, LendingPoolPulseBankPriceCache<'info>>,
) -> MarginfiResult {
    let clock = Clock::get()?;

    let mut bank = ctx.accounts.bank.load_mut()?;
    let mut group = ctx.accounts.group.load_mut()?;

    let pf = OraclePriceFeedAdapter::try_from_bank(&bank, ctx.remaining_accounts, &clock)?;

    let price_with_confidence = pf.get_price_and_confidence_of_type(
        OraclePriceType::RealTime,
        bank.config.oracle_max_confidence,
    )?;

    bank.update_cache_price(Some(price_with_confidence))?;

    // Apply any pending untracked inflows to the group rate limiter now that we have a fresh price
    if bank.rate_limiter.untracked_inflow != 0 && group.rate_limiter.is_enabled() {
        let price: I80F48 = bank.cache.last_oracle_price.into();
        let mint_decimals = bank.mint_decimals;
        bank.rate_limiter.apply_untracked_inflow(
            &mut group.rate_limiter,
            price,
            mint_decimals,
            clock.unix_timestamp,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolPulseBankPriceCache<'info> {
    #[account(mut)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub bank: AccountLoader<'info, Bank>,
}
