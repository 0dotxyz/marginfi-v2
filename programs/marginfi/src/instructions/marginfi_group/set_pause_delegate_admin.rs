use crate::{state::fee_state::FeeStateImpl, MarginfiError};
use anchor_lang::prelude::*;
use marginfi_type_crate::{constants::FEE_STATE_SEED, types::FeeState};

pub fn set_pause_delegate_admin(
    ctx: Context<SetPauseDelegateAdmin>,
    new_pause_delegate_admin: Option<Pubkey>,
) -> Result<()> {
    let mut fee_state = ctx.accounts.fee_state.load_mut()?;
    fee_state.set_pause_delegate_admin(new_pause_delegate_admin);

    Ok(())
}

#[derive(Accounts)]
pub struct SetPauseDelegateAdmin<'info> {
    /// Admin of the global FeeState. Only this authority can grant or revoke the pause delegate.
    pub global_fee_admin: Signer<'info>,

    #[account(
        mut,
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_admin @ MarginfiError::Unauthorized
    )]
    pub fee_state: AccountLoader<'info, FeeState>,
}
