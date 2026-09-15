use crate::{ix_utils, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, MarginfiGroup};

/// Copy emode settings from one bank to another within the same group.
pub fn lending_pool_clone_emode(ctx: Context<LendingPoolCloneEmode>) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let source_bank = ctx.accounts.copy_from_bank.load()?;
    let mut destination_bank = ctx.accounts.copy_to_bank.load_mut()?;

    destination_bank.emode = source_bank.emode;

    msg!(
        "emode settings copied from {:?} to {:?}",
        ctx.accounts.copy_from_bank.key(),
        ctx.accounts.copy_to_bank.key()
    );

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolCloneEmode<'info> {
    #[account(has_one = governance_admin @ MarginfiError::Unauthorized)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub governance_admin: Signer<'info>,

    #[account(
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub copy_from_bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub copy_to_bank: AccountLoader<'info, Bank>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}
