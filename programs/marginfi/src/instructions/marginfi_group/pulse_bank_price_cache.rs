use crate::state::bank::BankImpl;
use crate::state::price::{OraclePriceFeedAdapter, OraclePriceType, PriceAdapter};
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, MarginfiGroup};

/// (permissionless) Refresh the cached oracle price for a bank.
pub fn lending_pool_pulse_bank_price_cache<'info>(
    ctx: Context<'_, '_, 'info, 'info, LendingPoolPulseBankPriceCache<'info>>,
) -> MarginfiResult {
    let clock = Clock::get()?;

    let mut bank = ctx.accounts.bank.load_mut()?;

    let pf = OraclePriceFeedAdapter::try_from_bank(&bank, ctx.remaining_accounts, &clock)?;

    let price_with_confidence = pf.get_price_and_confidence_of_type(
        OraclePriceType::RealTime,
        bank.config.oracle_max_confidence,
    )?;

    bank.update_cache_price(Some(price_with_confidence))?;

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
