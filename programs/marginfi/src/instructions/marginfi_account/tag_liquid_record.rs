use crate::{
    check,
    events::LiquidationTagEvent,
    ix_utils::{get_discrim_hash, Hashable},
    prelude::*,
    state::marginfi_account::{
        check_pre_liquidation_condition_and_get_account_health, MarginfiAccountImpl,
    },
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    HealthPriceMode, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_DELEVERAGE,
    ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_RECEIVERSHIP,
};

/// (Permissionless) Tags an unhealthy account, letting the allowed liquidation premium grow over
/// time (see `tag_adjusted_premium`). Calling this instruction while the account is healthy again
/// or has no liabilities clears any existing tag instead.
/// * Fails if unhealthy and already tagged, or healthy and not tagged.
/// * A CB halt does not block tagging.
pub fn tag_liquidation_record<'info>(
    ctx: Context<'info, TagLiquidationRecord<'info>>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let group = ctx.accounts.group.load()?;

    let (health, _assets, liabs) = check_pre_liquidation_condition_and_get_account_health(
        &marginfi_account,
        &group,
        ctx.remaining_accounts,
        None,
        &mut None,
        HealthPriceMode::Live { liq_cache: None },
        true,
    )?;

    // Accounts with no liabilities cannot be meaningfully liquidated: they are never taggable,
    // and any stale tag on them can be cleared.
    if health > I80F48::ZERO || liabs == I80F48::ZERO {
        check!(
            marginfi_account.liquidation_tagged_at != 0,
            MarginfiError::HealthyAccount
        );
        marginfi_account.liquidation_tagged_at = 0;
    } else {
        check!(
            marginfi_account.liquidation_tagged_at == 0,
            MarginfiError::AccountAlreadyTagged
        );
        marginfi_account.liquidation_tagged_at = Clock::get()?.unix_timestamp;
    }

    emit!(LiquidationTagEvent {
        marginfi_account: ctx.accounts.marginfi_account.key(),
        tagged_at: marginfi_account.liquidation_tagged_at,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct TagLiquidationRecord<'info> {
    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let acc = marginfi_account.load()?;
            !acc.get_flag(ACCOUNT_IN_RECEIVERSHIP)
                && !acc.get_flag(ACCOUNT_IN_DELEVERAGE)
                && !acc.get_flag(ACCOUNT_IN_FLASHLOAN)
                && !acc.get_flag(ACCOUNT_DISABLED)
                && !acc.get_flag(ACCOUNT_IN_ORDER_EXECUTION)
        } @MarginfiError::UnexpectedLiquidationState
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub group: AccountLoader<'info, MarginfiGroup>,
}

impl Hashable for TagLiquidationRecord<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_tag_liq_record")
    }
}
