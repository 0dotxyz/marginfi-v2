use crate::events::{GroupEventHeader, MarginfiGroupConfigureEvent};
use crate::prelude::MarginfiError;
use crate::state::marginfi_group::MarginfiGroupImpl;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;

/// Configure timelocked admin. Only timelocked admin can change itself after initial setup.
/// CRITICAL: No escape hatch if key is lost. Recovery requires program upgrade.
pub fn configure_timelocked_admin(
    ctx: Context<ConfigureTimelockedAdmin>,
    new_timelocked_admin: Pubkey,
    timelocked_operation_delay_seconds: u64,
) -> MarginfiResult {
    let mut marginfi_group = ctx.accounts.marginfi_group.load_mut()?;

    // After initial setup, only timelocked admin can modify timelocked admin
    if marginfi_group.has_timelocked_admin() {
        require!(
            ctx.accounts.signer.key() == marginfi_group.timelocked_admin,
            MarginfiError::Unauthorized
        );
    } else {
        require!(
            ctx.accounts.signer.key() == marginfi_group.admin,
            MarginfiError::Unauthorized
        );
    }

    require!(
        new_timelocked_admin != Pubkey::default(),
        MarginfiError::InvalidConfig
    );
    require!(
        new_timelocked_admin != marginfi_group.timelocked_admin,
        MarginfiError::InvalidConfig
    );
    require!(
        new_timelocked_admin != marginfi_group.admin,
        MarginfiError::InvalidConfig
    );
    require!(
        timelocked_operation_delay_seconds > 0,
        MarginfiError::InvalidConfig
    );
    require!(
        timelocked_operation_delay_seconds <= (i64::MAX as u64),
        MarginfiError::InvalidConfig
    );

    marginfi_group.update_timelocked_admin(new_timelocked_admin);
    marginfi_group.set_timelocked_operation_delay(timelocked_operation_delay_seconds);

    emit!(MarginfiGroupConfigureEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(ctx.accounts.signer.key())
        },
        admin: None,
        flags: marginfi_group.group_flags
    });

    Ok(())
}

#[derive(Accounts)]
pub struct ConfigureTimelockedAdmin<'info> {
    #[account(mut)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub signer: Signer<'info>,
}
