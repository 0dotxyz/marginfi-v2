use crate::{ix_utils, state::marginfi_group::MarginfiGroupImpl, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{FeeState, MarginfiGroup},
};

#[derive(Accounts)]
pub struct ConfigGroupFee<'info> {
    #[account(mut)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    /// `global_fee_admin` of the FeeState
    pub global_fee_admin: Signer<'info>,

    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_admin @ MarginfiError::Unauthorized
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}

pub fn config_group_fee(ctx: Context<ConfigGroupFee>, enable_program_fee: bool) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let mut marginfi_group = ctx.accounts.marginfi_group.load_mut()?;
    let flag_before = marginfi_group.group_flags;

    marginfi_group.set_program_fee_enabled(enable_program_fee);

    msg!(
        "flag set to: {:?} was {:?}",
        marginfi_group.group_flags,
        flag_before
    );

    let clock = Clock::get()?;
    marginfi_group.fee_state_cache.last_update = clock.unix_timestamp;

    Ok(())
}
