use crate::{state::bank::BankImpl, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, MarginfiGroup};

pub fn lending_pool_accrue_bank_interest(
    ctx: Context<LendingPoolAccrueBankInterest>,
) -> MarginfiResult {
    let clock = Clock::get()?;
    let mut bank = ctx.accounts.bank.load_mut()?;
    let group = &ctx.accounts.group.load()?;

    bank.accrue_interest(
        clock.unix_timestamp,
        group,
        #[cfg(not(feature = "client"))]
        ctx.accounts.bank.key(),
    )?;

    // Note: reusing the InterestRateCalc that accrue_interest builds (rather than rebuilding it here)
    // was measured to save ~0 CU — the calc is cheap struct construction — so it's intentionally not
    // threaded through, to keep accrue/cache decoupled.
    bank.update_bank_cache(group)?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolAccrueBankInterest<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub bank: AccountLoader<'info, Bank>,
}
