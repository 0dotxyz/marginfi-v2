use crate::{
    events::{GroupEventHeader, SetGovernanceAdminEvent},
    ix_utils,
    state::marginfi_group::MarginfiGroupImpl,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;

pub fn set_governance_admin(
    ctx: Context<SetGovernanceAdmin>,
    new_governance_admin: Pubkey,
) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    require_neq!(
        new_governance_admin,
        Pubkey::default(),
        MarginfiError::InvalidGovernanceAdmin
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
    group.update_governance_admin(new_governance_admin);

    emit!(SetGovernanceAdminEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(*ctx.accounts.signer.key)
        },
        previous_governance_admin,
        new_governance_admin,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SetGovernanceAdmin<'info> {
    #[account(mut)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub signer: Signer<'info>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}
