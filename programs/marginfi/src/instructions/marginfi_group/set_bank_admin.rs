use crate::{
    state::marginfi_group::{assert_bank_admin_authorized, MarginfiGroupImpl},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;

pub fn set_bank_admin(ctx: Context<SetBankAdmin>, new_bank_admin: Pubkey) -> MarginfiResult {
    let mut group = ctx.accounts.marginfi_group.load_mut()?;

    assert_bank_admin_authorized(&group, ctx.accounts.bank_admin.key)?;

    require_neq!(
        new_bank_admin,
        Pubkey::default(),
        MarginfiError::InvalidBankAdmin
    );

    group.update_bank_admin(new_bank_admin);

    Ok(())
}

#[derive(Accounts)]
pub struct SetBankAdmin<'info> {
    #[account(mut)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub bank_admin: Signer<'info>,
}
