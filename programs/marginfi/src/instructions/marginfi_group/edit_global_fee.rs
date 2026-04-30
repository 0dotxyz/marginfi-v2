// Global fee admin calls this to edit fee state fields (all optional).
use crate::utils::wrapped_i80f48_to_f64;
use crate::MarginfiError;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{FeeState, WrappedI80F48},
};

pub fn edit_fee_state(
    ctx: Context<EditFeeState>,
    admin: Option<Pubkey>,
    fee_wallet: Option<Pubkey>,
    bank_init_flat_sol_fee: Option<u32>,
    liquidation_flat_sol_fee: Option<u32>,
    order_init_flat_sol_fee: Option<u32>,
    program_fee_fixed: Option<WrappedI80F48>,
    program_fee_rate: Option<WrappedI80F48>,
    liquidation_max_fee: Option<WrappedI80F48>,
    order_execution_max_fee: Option<WrappedI80F48>,
    pause_delegate_admin: Option<Pubkey>,
) -> Result<()> {
    let mut fee_state = ctx.accounts.fee_state.load_mut()?;
    if let Some(admin) = admin {
        msg!(
            "Updating global_fee_admin: {:?} -> {:?}",
            fee_state.global_fee_admin,
            admin
        );
        fee_state.global_fee_admin = admin;
    }
    if let Some(fee_wallet) = fee_wallet {
        msg!(
            "Updating global_fee_wallet: {:?} -> {:?}",
            fee_state.global_fee_wallet,
            fee_wallet
        );
        fee_state.global_fee_wallet = fee_wallet;
    }
    if let Some(bank_init_flat_sol_fee) = bank_init_flat_sol_fee {
        msg!(
            "Updating bank_init_flat_sol_fee: {:?} -> {:?}",
            fee_state.bank_init_flat_sol_fee,
            bank_init_flat_sol_fee
        );
        fee_state.bank_init_flat_sol_fee = bank_init_flat_sol_fee;
    }
    if let Some(program_fee_fixed) = program_fee_fixed {
        let old_f64: f64 = wrapped_i80f48_to_f64(fee_state.program_fee_fixed);
        let new_f64: f64 = wrapped_i80f48_to_f64(program_fee_fixed);
        msg!("Updating program_fee_fixed: {:?} -> {:?}", old_f64, new_f64);
        fee_state.program_fee_fixed = program_fee_fixed;
    }
    if let Some(program_fee_rate) = program_fee_rate {
        let old_f64: f64 = wrapped_i80f48_to_f64(fee_state.program_fee_rate);
        let new_f64: f64 = wrapped_i80f48_to_f64(program_fee_rate);
        msg!("Updating program_fee_rate: {:?} -> {:?}", old_f64, new_f64);
        fee_state.program_fee_rate = program_fee_rate;
    }
    if let Some(liquidation_max_fee) = liquidation_max_fee {
        let old_f64: f64 = wrapped_i80f48_to_f64(fee_state.liquidation_max_fee);
        let new_f64: f64 = wrapped_i80f48_to_f64(liquidation_max_fee);
        msg!(
            "Updating liquidation_max_fee: {:?} -> {:?}",
            old_f64,
            new_f64
        );
        fee_state.liquidation_max_fee = liquidation_max_fee;
    }
    if let Some(liquidation_flat_sol_fee) = liquidation_flat_sol_fee {
        msg!(
            "Updating liquidation_flat_sol_fee: {:?} -> {:?}",
            fee_state.liquidation_flat_sol_fee,
            liquidation_flat_sol_fee
        );
        fee_state.liquidation_flat_sol_fee = liquidation_flat_sol_fee;
    }
    if let Some(order_execution_max_fee) = order_execution_max_fee {
        let old_f64: f64 = wrapped_i80f48_to_f64(fee_state.order_execution_max_fee);
        let new_f64: f64 = wrapped_i80f48_to_f64(order_execution_max_fee);
        msg!(
            "Updating order_execution_max_fee: {:?} -> {:?}",
            old_f64,
            new_f64
        );
        fee_state.order_execution_max_fee = order_execution_max_fee;
    }
    if let Some(order_init_flat_sol_fee) = order_init_flat_sol_fee {
        msg!(
            "Updating order_init_flat_sol_fee: {:?} -> {:?}",
            fee_state.order_init_flat_sol_fee,
            order_init_flat_sol_fee
        );
        fee_state.order_init_flat_sol_fee = order_init_flat_sol_fee;
    }
    if let Some(pause_delegate_admin) = pause_delegate_admin {
        msg!(
            "Updating pause_delegate_admin: {:?} -> {:?}",
            fee_state.pause_delegate_admin,
            pause_delegate_admin
        );
        fee_state.pause_delegate_admin = pause_delegate_admin;
    }

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
