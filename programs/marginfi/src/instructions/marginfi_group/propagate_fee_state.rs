use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{FeeState, MarginfiGroup},
};

#[derive(Accounts)]
pub struct PropagateFee<'info> {
    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// Any group, this ix is permisionless and can propagate the fee to any group
    #[account(mut)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,
}

pub fn propagate_fee(ctx: Context<PropagateFee>) -> Result<()> {
    let mut group = ctx.accounts.marginfi_group.load_mut()?;
    let fee_state = ctx.accounts.fee_state.load()?;

    group.fee_state_cache.global_fee_wallet = fee_state.global_fee_wallet;
    group.fee_state_cache.program_fee_fixed = fee_state.program_fee_fixed;
    group.fee_state_cache.program_fee_rate = fee_state.program_fee_rate;

    let clock = Clock::get()?;
    group.fee_state_cache.last_update = clock.unix_timestamp;

    group
        .panic_state_cache
        .update_from_panic_state(&fee_state.panic_state, clock.unix_timestamp);

    msg!(
        "Propagated fee and panic state to group. Panic state: paused={}",
        group.panic_state_cache.is_paused_flag()
            && !group.panic_state_cache.is_expired(clock.unix_timestamp)
    );

    Ok(())
}
