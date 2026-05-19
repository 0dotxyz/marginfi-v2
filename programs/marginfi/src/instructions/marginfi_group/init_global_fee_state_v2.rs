use anchor_lang::prelude::*;
use marginfi_type_crate::{constants::FEE_STATE_V2_SEED, types::FeeStateV2};

/// Runs once per program to initialize the V2 fee state account.
pub fn initialize_fee_state_v2(ctx: Context<InitFeeStateV2>) -> Result<()> {
    let mut fee_state_v2 = ctx.accounts.fee_state_v2.load_init()?;
    fee_state_v2.key = ctx.accounts.fee_state_v2.key();
    fee_state_v2.bump_seed = ctx.bumps.fee_state_v2;

    Ok(())
}

#[derive(Accounts)]
pub struct InitFeeStateV2<'info> {
    /// Pays the init fee
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        seeds = [FEE_STATE_V2_SEED.as_bytes()],
        bump,
        payer = payer,
        space = 8 + FeeStateV2::LEN,
    )]
    pub fee_state_v2: AccountLoader<'info, FeeStateV2>,

    pub system_program: Program<'info, System>,
}
