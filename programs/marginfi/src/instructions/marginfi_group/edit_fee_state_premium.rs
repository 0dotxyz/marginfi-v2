// Global fee admin calls this to edit the variable-borrow premium fields on the fee state.
use crate::ix_utils;
use crate::MarginfiError;
use anchor_lang::prelude::*;
use marginfi_type_crate::{constants::FEE_STATE_SEED, types::FeeState};

pub fn edit_fee_state_premium(
    ctx: Context<EditFeeStatePremium>,
    premium_wallet: Pubkey,
) -> Result<()> {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let mut fee_state = ctx.accounts.fee_state.load_mut()?;
    msg!(
        "Updating premium_wallet: {:?} -> {:?}",
        fee_state.premium_wallet,
        premium_wallet
    );
    fee_state.premium_wallet = premium_wallet;

    Ok(())
}

#[derive(Accounts)]
pub struct EditFeeStatePremium<'info> {
    /// Admin of the global FeeState
    pub global_fee_admin: Signer<'info>,

    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        mut,
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_admin @ MarginfiError::Unauthorized
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}
