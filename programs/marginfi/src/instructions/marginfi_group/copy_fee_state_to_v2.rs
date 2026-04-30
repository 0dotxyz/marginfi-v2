use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{FEE_STATE_SEED, FEE_STATE_V2_SEED},
    types::{FeeState, FeeStateV2},
};

/// Copy current FeeState fields into FeeStateV2.
pub fn copy_fee_state_to_v2(ctx: Context<CopyFeeStateToV2>) -> Result<()> {
    let fee_state = ctx.accounts.fee_state.load()?;
    let mut fee_state_v2 = ctx.accounts.fee_state_v2.load_mut()?;

    // Preserve V2 PDA identity fields.
    let v2_key = fee_state_v2.key;
    let v2_bump_seed = fee_state_v2.bump_seed;
    fee_state_v2.key = v2_key;
    fee_state_v2.bump_seed = v2_bump_seed;

    // All other fields are copied from the v1 state.
    fee_state_v2.global_fee_admin = fee_state.global_fee_admin;
    fee_state_v2.global_fee_wallet = fee_state.global_fee_wallet;
    fee_state_v2.placeholder0 = fee_state.placeholder0;
    fee_state_v2.bank_init_flat_sol_fee = fee_state.bank_init_flat_sol_fee;
    fee_state_v2.liquidation_max_fee = fee_state.liquidation_max_fee;
    fee_state_v2.program_fee_fixed = fee_state.program_fee_fixed;
    fee_state_v2.program_fee_rate = fee_state.program_fee_rate;
    fee_state_v2.panic_state = fee_state.panic_state;
    fee_state_v2.placeholder1 = fee_state.placeholder1;
    fee_state_v2.liquidation_flat_sol_fee = fee_state.liquidation_flat_sol_fee;
    fee_state_v2.order_init_flat_sol_fee = fee_state.order_init_flat_sol_fee;
    fee_state_v2.order_execution_max_fee = fee_state.order_execution_max_fee;
    fee_state_v2.pause_delegate_admin = fee_state.pause_delegate_admin;

    Ok(())
}

#[derive(Accounts)]
pub struct CopyFeeStateToV2<'info> {
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    #[account(
        mut,
        seeds = [FEE_STATE_V2_SEED.as_bytes()],
        bump
    )]
    pub fee_state_v2: AccountLoader<'info, FeeStateV2>,
}
