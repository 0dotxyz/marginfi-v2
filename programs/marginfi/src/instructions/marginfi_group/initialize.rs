use crate::events::{GroupEventHeader, MarginfiGroupCreateEvent};
use crate::state::marginfi_group::MarginfiGroupImpl;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{FeeState, MarginfiGroup},
};

pub fn initialize_group(ctx: Context<MarginfiGroupInitialize>) -> MarginfiResult {
    let marginfi_group = &mut ctx.accounts.marginfi_group.load_init()?;

    marginfi_group.set_initial_configuration(ctx.accounts.admin.key());

    msg!(
        "Group admin: {:?} flags: {:?}",
        marginfi_group.admin,
        marginfi_group.group_flags
    );

    let fee_state = ctx.accounts.fee_state.load()?;

    // The fuzzer should ignore this because the "Clock" mock sysvar doesn't load until after the
    // group is init. Eventually we might fix the fuzzer to load the clock first...
    #[cfg(not(feature = "client"))]
    {
        let clock = Clock::get()?;
        marginfi_group.fee_state_cache.last_update = clock.unix_timestamp;
    }
    marginfi_group.fee_state_cache.global_fee_wallet = fee_state.global_fee_wallet;
    marginfi_group.fee_state_cache.program_fee_fixed = fee_state.program_fee_fixed;
    marginfi_group.fee_state_cache.program_fee_rate = fee_state.program_fee_rate;
    marginfi_group.banks = 0;

    let cache = marginfi_group.fee_state_cache;
    msg!(
        "global fee wallet: {:?}, fixed fee: {:?}, program free {:?}",
        cache.global_fee_wallet,
        cache.program_fee_fixed,
        cache.program_fee_rate
    );

    emit!(MarginfiGroupCreateEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(*ctx.accounts.admin.key)
        },
    });

    Ok(())
}

#[derive(Accounts)]
pub struct MarginfiGroupInitialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + std::mem::size_of::<MarginfiGroup>(),
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    pub system_program: Program<'info, System>,
}
