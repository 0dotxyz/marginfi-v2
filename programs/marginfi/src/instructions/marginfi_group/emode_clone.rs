use crate::{check, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, MarginfiGroup};

/// Copy emode settings from one bank to another within the same group.
pub fn lending_pool_clone_emode(ctx: Context<LendingPoolCloneEmode>) -> MarginfiResult {
    let group = ctx.accounts.group.load()?;

    check!(
        ctx.accounts.signer.key() == group.admin || ctx.accounts.signer.key() == group.emode_admin,
        MarginfiError::Unauthorized
    );

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
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub signer: Signer<'info>,

    #[account(
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub copy_from_bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub copy_to_bank: AccountLoader<'info, Bank>,
}
