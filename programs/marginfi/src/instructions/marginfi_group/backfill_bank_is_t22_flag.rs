use crate::{
    check_eq,
    constants::{MAINNET_PROGRAM_ID, STAGING_ID},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{BANK_SEED_KNOWN, IS_T22},
    types::{Bank, MarginfiGroup},
};

/// (permissionless) Backfill `IS_T22` on pre-upgrade banks.
///
/// No-op if:
/// - bank mint is a classic SPL Token mint
/// - the flag is already set
///
/// Also supports backfilling `bank_seed` in the same call:
/// - pass `None` to skip seed backfill
/// - pass `Some(seed)` (including `Some(0)`) to backfill it on pre-upgrade seeded banks
pub fn lending_pool_backfill_bank_is_t22_flag(
    ctx: Context<LendingPoolBackfillBankIsT22Flag>,
    bank_seed: Option<u64>,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    if let Some(bank_seed) = bank_seed {
        // Note: This enables localnet to derive mainnet keys for regression tests, even though the
        // account when loaded as a fixture is "owned" by the localnet program.
        let seed_derivation_program = if crate::ID == STAGING_ID || *ctx.program_id == STAGING_ID {
            STAGING_ID
        } else {
            MAINNET_PROGRAM_ID
        };

        let (derived_bank, _) = Pubkey::find_program_address(
            &[
                ctx.accounts.group.key().as_ref(),
                ctx.accounts.mint.key().as_ref(),
                &bank_seed.to_le_bytes(),
            ],
            &seed_derivation_program,
        );
        check_eq!(
            derived_bank,
            ctx.accounts.bank.key(),
            MarginfiError::InvalidBankAccount
        );
        bank.bank_seed = bank_seed;
        bank.flags |= BANK_SEED_KNOWN;
    }

    if (bank.flags & IS_T22) == 0 && ctx.accounts.mint.owner == &anchor_spl::token_2022::ID {
        bank.flags |= IS_T22;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolBackfillBankIsT22Flag<'info> {
    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = mint @ MarginfiError::InvalidMint
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: Constrained by `has_one = group`.
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// CHECK: Constrained by `has_one = mint`.
    pub mint: UncheckedAccount<'info>,
}
