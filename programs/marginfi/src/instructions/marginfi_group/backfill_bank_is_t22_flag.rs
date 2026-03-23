use crate::{check, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;
use marginfi_type_crate::{constants::IS_T22, types::Bank};

/// (permissionless) Backfill `IS_T22` on pre-upgrade banks.
///
/// No-op if:
/// - bank mint is a classic SPL Token mint
/// - the flag is already set
pub fn lending_pool_backfill_bank_is_t22_flag(
    ctx: Context<LendingPoolBackfillBankIsT22Flag>,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(
        bank.mint == ctx.accounts.bank_mint.key(),
        MarginfiError::InvalidBankAccount
    );

    if (bank.flags & IS_T22) != 0 {
        return Ok(());
    }

    if ctx.accounts.bank_mint.to_account_info().owner == &anchor_spl::token_2022::ID {
        bank.flags |= IS_T22;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolBackfillBankIsT22Flag<'info> {
    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,

    pub bank_mint: Box<InterfaceAccount<'info, Mint>>,
}
