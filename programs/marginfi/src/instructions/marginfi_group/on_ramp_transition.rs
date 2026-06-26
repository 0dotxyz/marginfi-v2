use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{STAKED_ORACLE_DISABLED, STAKED_ORACLE_PRICE_USES_ONRAMP, STAKED_SETTINGS_SEED},
    types::{MarginfiGroup, StakedSettings},
};

// To be removed once SVSP update is rolled out (likely in 1.10)
pub fn disable_staked_oracles(ctx: Context<DisableStakedOracles>) -> MarginfiResult {
    let mut staked_settings = ctx.accounts.staked_settings.load_mut()?;

    staked_settings.flags &= !STAKED_ORACLE_PRICE_USES_ONRAMP;
    staked_settings.flags |= STAKED_ORACLE_DISABLED;

    Ok(())
}

#[derive(Accounts)]
pub struct DisableStakedOracles<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [
            STAKED_SETTINGS_SEED.as_bytes(),
            group.key().as_ref()
        ],
        bump,
        constraint = staked_settings.load()?.marginfi_group == group.key()
            @ MarginfiError::InvalidGroup
    )]
    pub staked_settings: AccountLoader<'info, StakedSettings>,
}

// To be removed once SVSP update is rolled out (likely in 1.10)
pub fn enable_staked_oracle_onramp(ctx: Context<EnableStakedOracleOnramp>) -> MarginfiResult {
    let mut staked_settings = ctx.accounts.staked_settings.load_mut()?;

    staked_settings.flags &= !STAKED_ORACLE_DISABLED;
    staked_settings.flags |= STAKED_ORACLE_PRICE_USES_ONRAMP;

    Ok(())
}

#[derive(Accounts)]
pub struct EnableStakedOracleOnramp<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [
            STAKED_SETTINGS_SEED.as_bytes(),
            group.key().as_ref()
        ],
        bump,
        constraint = staked_settings.load()?.marginfi_group == group.key()
            @ MarginfiError::InvalidGroup
    )]
    pub staked_settings: AccountLoader<'info, StakedSettings>,
}
