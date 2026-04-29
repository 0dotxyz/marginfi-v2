use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
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

    if (bank.flags & IS_T22) != 0 {
        return Ok(());
    }

    if ctx.accounts.mint.owner == &anchor_spl::token_2022::ID {
        bank.flags |= IS_T22;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolBackfillBankIsT22Flag<'info> {
    #[account(
        mut,
        has_one = mint @ MarginfiError::InvalidBankAccount
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: Constrained by `has_one = mint`.
    pub mint: UncheckedAccount<'info>,
}
