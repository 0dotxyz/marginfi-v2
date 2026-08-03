use crate::{
    check,
    events::{AccountEventHeader, MarginfiAccountFlagUpdateEvent},
    prelude::*,
    state::marginfi_account::MarginfiAccountImpl,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{
    MarginfiAccount, ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED,
    ACCOUNT_POSITION_TRANSFER_SEND_DISABLED,
};

pub fn set_position_transfer_flags(
    ctx: Context<SetPositionTransferFlags>,
    disable_send: Option<bool>,
    disable_receive: Option<bool>,
) -> MarginfiResult {
    let mut account = ctx.accounts.marginfi_account.load_mut()?;

    check!(
        account.authority == ctx.accounts.authority.key(),
        MarginfiError::Unauthorized
    );

    if let Some(v) = disable_send {
        if v {
            account.set_flag(ACCOUNT_POSITION_TRANSFER_SEND_DISABLED, true);
        } else {
            account.unset_flag(ACCOUNT_POSITION_TRANSFER_SEND_DISABLED, true);
        }
    }

    if let Some(v) = disable_receive {
        if v {
            account.set_flag(ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED, true);
        } else {
            account.unset_flag(ACCOUNT_POSITION_TRANSFER_RECEIVE_DISABLED, true);
        }
    }

    account.last_update = Clock::get()?.unix_timestamp as u64;

    emit!(MarginfiAccountFlagUpdateEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: account.authority,
            marginfi_group: account.group,
        },
        disable_position_transfer_send: disable_send,
        disable_position_transfer_receive: disable_receive,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SetPositionTransferFlags<'info> {
    #[account(mut)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,
}
