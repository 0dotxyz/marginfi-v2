// Global fee admin calls this to edit the fee rate or the fee wallet.
use crate::utils::wrapped_i80f48_to_f64;
use crate::MarginfiError;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{FeeState, WrappedI80F48},
};

pub fn edit_fee_state(
    ctx: Context<EditFeeState>,
    admin: Pubkey,
    fee_wallet: Pubkey,
    bank_init_flat_sol_fee: u32,
    liquidation_flat_sol_fee: u32,
    order_init_flat_sol_fee: u32,
    program_fee_fixed: WrappedI80F48,
    program_fee_rate: WrappedI80F48,
    liquidation_max_fee: WrappedI80F48,
    order_execution_max_fee: WrappedI80F48,
) -> Result<()> {
    let mut fee_state = ctx.accounts.fee_state.load_mut()?;
    fee_state.global_fee_admin = admin;
    fee_state.global_fee_wallet = fee_wallet;
    fee_state.bank_init_flat_sol_fee = bank_init_flat_sol_fee;
    fee_state.program_fee_fixed = program_fee_fixed;
    fee_state.program_fee_rate = program_fee_rate;
    fee_state.liquidation_max_fee = liquidation_max_fee;
    fee_state.liquidation_flat_sol_fee = liquidation_flat_sol_fee;
    fee_state.order_execution_max_fee = order_execution_max_fee;
    fee_state.order_init_flat_sol_fee = order_init_flat_sol_fee;

    let fixed_f64: f64 = wrapped_i80f48_to_f64(fee_state.program_fee_fixed);
    let rate_f64: f64 = wrapped_i80f48_to_f64(fee_state.program_fee_rate);
    let liq_f64: f64 = wrapped_i80f48_to_f64(fee_state.liquidation_max_fee);
    let ord_f64: f64 = wrapped_i80f48_to_f64(fee_state.order_execution_max_fee);
    msg!("admin set to: {:?} fee wallet: {:?}", admin, fee_wallet);
    msg!(
        "flat sol: {:?} fixed: {:?} rate: {:?}",
        fee_state.bank_init_flat_sol_fee,
        fixed_f64,
        rate_f64
    );
    msg!(
        "liquidation max fee: {:?}, flat fee: {:?}",
        liq_f64,
        fee_state.liquidation_flat_sol_fee
    );
    msg!(
        "order execution max fee: {:?}, flat fee: {:?}",
        ord_f64,
        fee_state.order_init_flat_sol_fee
    );

    Ok(())
}

#[derive(Accounts)]
pub struct EditFeeState<'info> {
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
}
