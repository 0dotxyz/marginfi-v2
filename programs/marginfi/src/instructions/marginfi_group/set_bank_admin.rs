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
    let previous_governance_admin = group.governance_admin;
    if previous_governance_admin == Pubkey::default() {
        // Legacy groups are resized with a zeroed extension. The existing fast admin may
        // bootstrap the slow authority exactly once; every later rotation is slow-admin-only.
        group.require_admin(*ctx.accounts.signer.key)?;
    } else {
        group.require_governance_admin(*ctx.accounts.signer.key)?;
    }
    group.update_governance_admin(new_bank_admin);

    emit!(SetBankAdminEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(*ctx.accounts.signer.key)
        },
        previous_bank_admin: previous_governance_admin,
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
