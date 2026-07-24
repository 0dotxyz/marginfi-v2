use crate::{
    check,
    events::LiquidationTagEvent,
    ix_utils::{get_discrim_hash, Hashable},
    prelude::*,
    state::marginfi_account::{
        any_balance_bank_is_cb_halted, check_pre_liquidation_condition_and_get_account_health,
        MarginfiAccountImpl,
    },
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    HealthPriceMode, LiquidationRecord, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED,
    ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION,
    ACCOUNT_IN_RECEIVERSHIP,
};

/// (Permissionless) Tags an unhealthy account, letting the allowed liquidation premium grow over
/// time (see `tag_adjusted_premium`). Calling this instruction while the account is healthy again
/// or has no liabilities clears any existing tag instead.
/// * Fails if unhealthy and already tagged, or healthy and not tagged.
/// * Fails if tagging while any balance bank is CB-halted (liquidation is admin-only then).
pub fn tag_liquidation_record<'info>(
    ctx: Context<'info, TagLiquidationRecord<'info>>,
) -> MarginfiResult {
    let marginfi_account = ctx.accounts.marginfi_account.load()?;
    let mut liq_record = ctx.accounts.liquidation_record.load_mut()?;
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
        check!(liq_record.tagged_at != 0, MarginfiError::HealthyAccount);
        liq_record.tagged_at = 0;
    } else {
        check!(
            liq_record.tagged_at == 0,
            MarginfiError::LiquidationRecordAlreadyTagged
        );
        // While any balance bank is CB-halted, liquidation is admin-only, so the premium-growth
        // clock must not start.
        check!(
            !any_balance_bank_is_cb_halted(&marginfi_account, ctx.remaining_accounts)?,
            MarginfiError::CircuitBreakerAdminOnly
        );
        liq_record.tagged_at = Clock::get()?.unix_timestamp;
    }

    emit!(LiquidationTagEvent {
        marginfi_account: ctx.accounts.marginfi_account.key(),
        tagged_at: liq_record.tagged_at,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct TagLiquidationRecord<'info> {
    #[account(
        has_one = liquidation_record @ MarginfiError::InvalidLiquidationRecord,
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

    /// The associated liquidation record PDA for the given `marginfi_account`
    #[account(mut)]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    pub group: AccountLoader<'info, MarginfiGroup>,
}

impl Hashable for TagLiquidationRecord<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_tag_liq_record")
    }
}
