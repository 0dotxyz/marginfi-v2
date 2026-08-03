use crate::events::{GroupEventHeader, LendingPoolBankPremiumConfigureEvent};
use crate::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::PREMIUM_ACTIVE,
    types::{Bank, MarginfiGroup},
};

/// (emode admin only) Set a bank's premium tag and toggle premium accrual for its borrowers.
///
/// # Deactivation is destructive — it is a LAZY premium amnesty
///
/// `active: false` forgives unpaid premium per-balance at each balance's NEXT touch while the
/// flag is off (there is no deactivation epoch): untouched balances keep their materialized
/// receivable through a deactivate→reactivate cycle, and a full amnesty requires touching
/// every borrower (e.g. a `pulse_health` sweep) while off. Only the inactive window's accrual
/// is always forgiven (`premium_activated_at` clamp).
///
/// To stop accrual WITHOUT forgiving earned premium, retag to an unused `premium_tag` and
/// crank `pulse_health` instead — receivables stay collectible and the flag stays on.
/// Tag changes never forgive anything; only the flag transition does.
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
        if was_active {
            msg!("WARNING: premium deactivation forgives all unpaid premium on this bank");
        }
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
