use crate::events::{GroupEventHeader, LendingPoolBankPremiumConfigureEvent};
use crate::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::PREMIUM_ACTIVE,
    types::{Bank, MarginfiGroup},
};

/// (emode admin only) Set a bank's premium tag and toggle premium accrual for its borrowers.
pub fn lending_pool_configure_bank_premium(
    ctx: Context<LendingPoolConfigureBankPremium>,
    premium_tag: u16,
    active: bool,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    bank.premium_tag = premium_tag;
    // Note: not part of `GROUP_FLAGS` (this flag is emode-admin-gated, not group-admin-gated),
    // so it is set directly rather than through `update_flag`.
    let was_active = bank.flags & PREMIUM_ACTIVE != 0;
    if active {
        bank.flags |= PREMIUM_ACTIVE;
        // Stamp only the inactive->active TRANSITION (an idempotent re-config of an active
        // bank must not forgive pending accrual). Accrual is clamped to start here, so the
        // deactivated window can never be charged or health-projected.
        if !was_active {
            bank.premium_activated_at = Clock::get()?.unix_timestamp;
        }
    } else {
        bank.flags &= !PREMIUM_ACTIVE;
    }

    msg!(
        "premium tag set to {:?}, premium active: {:?}",
        premium_tag,
        active
    );

    emit!(LendingPoolBankPremiumConfigureEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: Some(ctx.accounts.emode_admin.key()),
        },
        bank: ctx.accounts.bank.key(),
        mint: bank.mint,
        premium_tag,
        active,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBankPremium<'info> {
    #[account(
        has_one = emode_admin @ MarginfiError::Unauthorized
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub emode_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}
