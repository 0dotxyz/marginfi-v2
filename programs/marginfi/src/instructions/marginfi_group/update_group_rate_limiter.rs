use crate::{check, state::rate_limiter::GroupRateLimiterImpl, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;

const MAX_RATE_LIMIT_UPDATE_LAG_SLOTS: u64 = 1_500; // ~10 minutes at ~400ms/slot

/// (admin only) Update the group rate limiter inflow/outflow state.
///
/// The group admin aggregates `RateLimitFlowEvent` events off-chain, computes the
/// USD-denominated inflows and outflows, and calls this instruction at intervals to
/// update the group rate limiter state.
///
/// This avoids requiring the group account to be writable (mut) in every user-facing
/// instruction, which would serialize all transactions for a group into a single slot.
pub fn update_group_rate_limiter(
    ctx: Context<UpdateGroupRateLimiter>,
    outflow_usd: Option<u64>,
    inflow_usd: Option<u64>,
    update_seq: u64,
    event_start_slot: u64,
    event_end_slot: u64,
) -> MarginfiResult {
    let mut group = ctx.accounts.marginfi_group.load_mut()?;
    let clock = Clock::get()?;

    check!(
        outflow_usd.is_some() || inflow_usd.is_some(),
        MarginfiError::GroupRateLimiterUpdateEmpty
    );
    check!(
        event_start_slot <= event_end_slot,
        MarginfiError::GroupRateLimiterUpdateInvalidSlotRange
    );
    check!(
        event_end_slot <= clock.slot,
        MarginfiError::GroupRateLimiterUpdateFutureSlot
    );
    check!(
        clock.slot.saturating_sub(event_end_slot) <= MAX_RATE_LIMIT_UPDATE_LAG_SLOTS,
        MarginfiError::GroupRateLimiterUpdateStale
    );
    check!(
        event_start_slot >= group.rate_limiter_last_admin_update_slot,
        MarginfiError::GroupRateLimiterUpdateOutOfOrderSlot
    );
    check!(
        event_end_slot >= group.rate_limiter_last_admin_update_slot,
        MarginfiError::GroupRateLimiterUpdateOutOfOrderSlot
    );
    check!(
        update_seq == group.rate_limiter_last_admin_update_seq.saturating_add(1),
        MarginfiError::GroupRateLimiterUpdateOutOfOrderSeq
    );

    if let Some(inflow) = inflow_usd {
        group
            .rate_limiter
            .record_inflow(inflow, clock.unix_timestamp);
        msg!("Group rate limiter inflow recorded: {} USD", inflow);
    }

    if let Some(outflow) = outflow_usd {
        group
            .rate_limiter
            .try_record_outflow(outflow, clock.unix_timestamp)?;
        msg!("Group rate limiter outflow recorded: {} USD", outflow);
    }

    group.rate_limiter_last_admin_update_slot = event_end_slot;
    group.rate_limiter_last_admin_update_seq = update_seq;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateGroupRateLimiter<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,
}
