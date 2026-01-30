/// Admin-only instruction to toggle `ACCOUNT_FROZEN` on a marginfi account.
///
/// Behavior:
/// - When frozen, the account authority is blocked from major actions (borrow/deposit/withdraw/repay/transfer/etc.) with `AccountFrozen`.
/// - The group admin retains access to operate the account while frozen (for remediation/seizure).
/// - Setting `frozen = false` clears the flag and returns control to the authority under normal auth rules.
pub fn set_account_freeze(ctx: Context<SetAccountFreeze>, frozen: bool) -> MarginfiResult {
    let group = ctx.accounts.group.load()?;
    check_eq!(
        group.admin,
        ctx.accounts.admin.key(),
        MarginfiError::Unauthorized
    );
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    if frozen {
        marginfi_account.set_flag(ACCOUNT_FROZEN, true);
    } else {
        marginfi_account.unset_flag(ACCOUNT_FROZEN, true);
    }
    marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;

    emit!(MarginfiAccountFreezeEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.admin.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: ctx.accounts.group.key(),
        },
        frozen,
    });

    Ok(())
}

use crate::{
    check_eq,
    events::{AccountEventHeader, MarginfiAccountFreezeEvent},
    prelude::*,
    state::marginfi_account::MarginfiAccountImpl,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{MarginfiAccount, MarginfiGroup, ACCOUNT_FROZEN};

#[derive(Accounts)]
pub struct SetAccountFreeze<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        constraint = group.load()?.admin == admin.key() @ MarginfiError::Unauthorized
    )]
    pub admin: Signer<'info>,
}
