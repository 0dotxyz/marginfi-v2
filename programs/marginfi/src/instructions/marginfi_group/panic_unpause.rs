use anchor_lang::prelude::*;
use marginfi_type_crate::{constants::FEE_STATE_SEED, types::FeeState};

use crate::state::{fee_state::FeeStateImpl, panic_state::PanicStateImpl};
use crate::MarginfiError;

pub fn panic_unpause(ctx: Context<PanicUnpause>) -> Result<()> {
    let mut fee_state = ctx.accounts.fee_state.load_mut()?;
    let current_timestamp = Clock::get()?.unix_timestamp;

    require!(
        fee_state.panic_state.is_paused_flag(),
        crate::errors::MarginfiError::ProtocolNotPaused
    );

    fee_state.panic_state.unpause_if_expired(current_timestamp);

    if fee_state.panic_state.is_paused_flag() {
        fee_state.panic_state.unpause();
        msg!(
            "Protocol manually unpaused by admin at timestamp: {}",
            current_timestamp
        );
    } else {
        msg!(
            "Protocol was already auto-unpaused due to expiration at timestamp: {}",
            current_timestamp
        );
    }

    msg!(
        "Consecutive pause count reset to: {}",
        fee_state.panic_state.consecutive_pause_count
    );

    Ok(())
}

#[derive(Accounts)]
pub struct PanicUnpause<'info> {
    /// Global fee admin or the dedicated pause delegate admin.
    #[account(mut)]
    pub pause_authority: Signer<'info>,

    #[account(
        mut,
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        constraint = fee_state.load()?.is_pause_authority(pause_authority.key()) @ MarginfiError::Unauthorized
    )]
    pub fee_state: AccountLoader<'info, FeeState>,
}
