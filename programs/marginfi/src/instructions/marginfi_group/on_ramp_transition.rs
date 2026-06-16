use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{STAKED_ORACLE_DISABLED, STAKED_ORACLE_PRICE_USES_ONRAMP},
    types::MarginfiGroup,
};

// To be removed once SVSP update is rolled out (likely in 1.10)
pub fn disable_staked_oracles(ctx: Context<DisableStakedOracles>) -> MarginfiResult {
    let mut group = ctx.accounts.group.load_mut()?;

    group.group_flags |= STAKED_ORACLE_DISABLED;

    Ok(())
}

#[derive(Accounts)]
pub struct DisableStakedOracles<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,
}

// To be removed once SVSP update is rolled out (likely in 1.10)
pub fn enable_staked_oracle_onramp(ctx: Context<EnableStakedOracleOnramp>) -> MarginfiResult {
    let mut group = ctx.accounts.group.load_mut()?;

    group.group_flags &= !STAKED_ORACLE_DISABLED;
    group.group_flags |= STAKED_ORACLE_PRICE_USES_ONRAMP;

    Ok(())
}

#[derive(Accounts)]
pub struct EnableStakedOracleOnramp<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,
}
