use crate::state::bank::BankImpl;
use crate::state::price::OraclePriceFeedAdapter;
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, MarginfiGroup};

/// (permissionless) Refresh the cached oracle price for a bank.
pub fn lending_pool_pulse_bank_price_cache<'info>(
    ctx: Context<'_, '_, 'info, 'info, LendingPoolPulseBankPriceCache<'info>>,
) -> MarginfiResult {
    let clock = Clock::get()?;

    let mut bank = ctx.accounts.bank.load_mut()?;

    let price_for_cache = OraclePriceFeedAdapter::get_price_and_confidence_for_cache(
        &bank,
        ctx.remaining_accounts,
        &clock,
    )?;

    bank.update_cache_price(Some(price_for_cache))?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolPulseBankPriceCache<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub bank: AccountLoader<'info, Bank>,
}
