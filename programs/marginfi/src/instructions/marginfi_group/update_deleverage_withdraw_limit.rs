use crate::{check, state::marginfi_group::MarginfiGroupImpl, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::MarginfiGroup;

const MAX_DELEVERAGE_WITHDRAW_LIMIT_UPDATE_LAG_SLOTS: u64 = 1_500; // ~10 minutes at ~400ms/slot

/// (admin only) Update the deleverage daily withdraw counter with aggregated outflow.
///
/// The group admin aggregates `DeleverageWithdrawFlowEvent` events off-chain and calls this
/// instruction at intervals to update the on-chain deleverage daily withdraw counter.
///
/// This avoids requiring the group account to be writable (mut) in every withdraw instruction.
pub fn update_deleverage_withdraw_limit(
    ctx: Context<UpdateDeleverageWithdrawLimit>,
    outflow_usd: u32,
    update_seq: u64,
    event_start_slot: u64,
    event_end_slot: u64,
) -> MarginfiResult {
    let mut group = ctx.accounts.marginfi_group.load_mut()?;
    let clock = Clock::get()?;

    check!(
        outflow_usd > 0,
        MarginfiError::DeleverageWithdrawLimitUpdateEmpty
    );
    validate_event_slots(
        event_start_slot,
        event_end_slot,
        group.deleverage_withdraw_last_admin_update_slot,
    )?;
    check!(
        event_end_slot <= clock.slot,
        MarginfiError::DeleverageWithdrawLimitUpdateFutureSlot
    );
    check!(
        clock.slot.saturating_sub(event_end_slot) <= MAX_DELEVERAGE_WITHDRAW_LIMIT_UPDATE_LAG_SLOTS,
        MarginfiError::DeleverageWithdrawLimitUpdateStale
    );
    check!(
        update_seq
            == group
                .deleverage_withdraw_last_admin_update_seq
                .saturating_add(1),
        MarginfiError::DeleverageWithdrawLimitUpdateOutOfOrderSeq
    );

    group.update_withdrawn_equity(I80F48::from_num(outflow_usd), clock.unix_timestamp)?;
    msg!(
        "Deleverage withdraw limit outflow recorded: {} USD",
        outflow_usd
    );

    group.deleverage_withdraw_last_admin_update_slot = event_end_slot;
    group.deleverage_withdraw_last_admin_update_seq = update_seq;

    Ok(())
}

fn validate_event_slots(
    event_start_slot: u64,
    event_end_slot: u64,
    last_admin_update_slot: u64,
) -> MarginfiResult {
    check!(
        event_start_slot <= event_end_slot,
        MarginfiError::DeleverageWithdrawLimitUpdateInvalidSlotRange
    );

    // Strictly-greater enforces non-overlapping slot ranges across admin batches.
    check!(
        event_start_slot > last_admin_update_slot,
        MarginfiError::DeleverageWithdrawLimitUpdateOutOfOrderSlot
    );
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateDeleverageWithdrawLimit<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,
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
                Some(MarginfiError::DeleverageWithdrawLimitUpdateInvalidSlotRange),
            ),
            (
                110_u64,
                120_u64,
                110_u64,
                Some(MarginfiError::DeleverageWithdrawLimitUpdateOutOfOrderSlot),
            ),
            (
                109_u64,
                120_u64,
                110_u64,
                Some(MarginfiError::DeleverageWithdrawLimitUpdateOutOfOrderSlot),
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
}
