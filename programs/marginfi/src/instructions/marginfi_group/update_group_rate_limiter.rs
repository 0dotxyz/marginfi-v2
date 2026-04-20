use crate::{check, state::rate_limiter::GroupRateLimiterImpl, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;

const MAX_RATE_LIMIT_UPDATE_LAG_SLOTS: u64 = 1_500; // ~10 minutes at ~400ms/slot

/// (delegate_flow_admin only) Update the group rate limiter inflow/outflow state.
///
/// The delegate flow admin aggregates `RateLimitFlowEvent` events off-chain,
/// computes the USD-denominated inflows and outflows, and calls this instruction
/// at intervals to update the group rate limiter state.
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
    validate_event_slots(
        event_start_slot,
        event_end_slot,
        group.rate_limiter_last_admin_update_slot,
    )?;
    check!(
        event_end_slot <= clock.slot,
        MarginfiError::GroupRateLimiterUpdateFutureSlot
    );
    check!(
        clock.slot.saturating_sub(event_end_slot) <= MAX_RATE_LIMIT_UPDATE_LAG_SLOTS,
        MarginfiError::GroupRateLimiterUpdateStale
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

fn validate_event_slots(
    event_start_slot: u64,
    event_end_slot: u64,
    last_admin_update_slot: u64,
) -> MarginfiResult {
    check!(
        event_start_slot <= event_end_slot,
        MarginfiError::GroupRateLimiterUpdateInvalidSlotRange
    );

    // Strictly-greater enforces non-overlapping slot ranges across admin batches.
    check!(
        event_start_slot > last_admin_update_slot,
        MarginfiError::GroupRateLimiterUpdateOutOfOrderSlot
    );
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateGroupRateLimiter<'info> {
    #[account(
        mut,
        has_one = delegate_flow_admin @ MarginfiError::Unauthorized,
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub delegate_flow_admin: Signer<'info>,
}

#[cfg(test)]
mod tests {
    use super::validate_event_slots;
    use crate::MarginfiError;

    #[test]
    fn validate_event_slots_checks_range_and_non_overlapping_start() {
        let cases = [
            (111_u64, 120_u64, 110_u64, None),
            (111_u64, 111_u64, 110_u64, None),
            (500_u64, 600_u64, 0_u64, None),
            (u64::MAX, u64::MAX, u64::MAX.saturating_sub(1), None),
            (
                121_u64,
                120_u64,
                110_u64,
                Some(MarginfiError::GroupRateLimiterUpdateInvalidSlotRange),
            ),
            (
                110_u64,
                120_u64,
                110_u64,
                Some(MarginfiError::GroupRateLimiterUpdateOutOfOrderSlot),
            ),
            (
                109_u64,
                120_u64,
                110_u64,
                Some(MarginfiError::GroupRateLimiterUpdateOutOfOrderSlot),
            ),
        ];

        for (start, end, last, expected_err) in cases {
            let result = validate_event_slots(start, end, last);
            match expected_err {
                None => assert!(result.is_ok()),
                Some(err) => {
                    assert!(result.is_err());
                    assert_eq!(result.err().unwrap(), err.into());
                }
            }
        }
    }

    #[test]
    fn validate_event_slots_allows_gap_skipping_unprocessed_slots() {
        // Last settled slot is 100. A buggy updater skips events from slots 101..=103
        // and submits a batch starting at 104. This currently passes validation.
        let mut last_admin_update_slot = 100_u64;

        let first_buggy_batch = (104_u64, 104_u64);
        assert!(validate_event_slots(
            first_buggy_batch.0,
            first_buggy_batch.1,
            last_admin_update_slot
        )
        .is_ok());

        // Cursor would advance to 104, making slots 101..=103 permanently unaddressable.
        last_admin_update_slot = first_buggy_batch.1;
        assert!(validate_event_slots(105_u64, 105_u64, last_admin_update_slot).is_ok());
    }
}
