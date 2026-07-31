use crate::{
    events::{GroupEventHeader, SetBankAdminEvent},
    state::marginfi_group::MarginfiGroupImpl,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;

pub fn set_bank_admin(ctx: Context<SetBankAdmin>, new_bank_admin: Pubkey) -> MarginfiResult {
    require_neq!(
        new_bank_admin,
        Pubkey::default(),
        MarginfiError::InvalidBankAdmin
    );

    let mut group = ctx.accounts.marginfi_group.load_mut()?;
    let previous_bank_admin = group.bank_admin_or_fallback();
    group.rotate_bank_admin(new_bank_admin, *ctx.accounts.signer.key)?;

    emit!(SetBankAdminEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(*ctx.accounts.signer.key)
        },
        previous_bank_admin,
        new_bank_admin,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SetBankAdmin<'info> {
    #[account(mut)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub signer: Signer<'info>,
}
