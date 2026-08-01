use crate::{
    events::{AccountEventHeader, TransferFlagsSetEvent},
    prelude::*,
    state::marginfi_account::MarginfiAccountImpl,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{
    MarginfiAccount, ACCOUNT_TRANSFER_DISABLED, ACCOUNT_TRANSFER_SEND_DISABLED,
};

pub fn set_transfer_flags(
    ctx: Context<SetTransferFlags>,
    disable_receive: bool,
    disable_send: bool,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    if disable_receive {
        marginfi_account.set_flag(ACCOUNT_TRANSFER_DISABLED, false);
    } else {
        marginfi_account.unset_flag(ACCOUNT_TRANSFER_DISABLED, false);
    }

    if disable_send {
        marginfi_account.set_flag(ACCOUNT_TRANSFER_SEND_DISABLED, false);
    } else {
        marginfi_account.unset_flag(ACCOUNT_TRANSFER_SEND_DISABLED, false);
    }

    marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;

    emit!(TransferFlagsSetEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        disable_receive,
        disable_send,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SetTransferFlags<'info> {
    #[account(
        mut,
        constraint = {
            let acc = marginfi_account.load()?;
            acc.authority == authority.key()
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,
}
