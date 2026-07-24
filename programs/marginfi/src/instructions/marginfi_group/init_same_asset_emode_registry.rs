use crate::{check, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::SAME_ASSET_EMODE_REGISTRY_SEED,
    types::{MarginfiGroup, SameAssetEmodeRegistry},
};

/// Initialize the per-group same-asset e-mode registry.
///
/// The registry is informational only. Health computation continues to use bank flags and bank
/// oracle configuration directly.
pub fn lending_pool_init_same_asset_emode_registry(
    ctx: Context<LendingPoolInitSameAssetEmodeRegistry>,
) -> MarginfiResult {
    let group = ctx.accounts.group.load()?;

    check!(
        ctx.accounts.signer.key() == group.admin || ctx.accounts.signer.key() == group.emode_admin,
        MarginfiError::Unauthorized
    );

    let mut registry = ctx.accounts.same_asset_emode_registry.load_init()?;
    registry.key = ctx.accounts.same_asset_emode_registry.key();
    registry.group = ctx.accounts.group.key();
    registry.group_count = 0;
    registry.bank_count = 0;
    registry.bump = ctx.bumps.same_asset_emode_registry;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolInitSameAssetEmodeRegistry<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        seeds = [
            SAME_ASSET_EMODE_REGISTRY_SEED.as_bytes(),
            group.key().as_ref()
        ],
        bump,
        payer = signer,
        space = 8 + SameAssetEmodeRegistry::LEN,
    )]
    pub same_asset_emode_registry: AccountLoader<'info, SameAssetEmodeRegistry>,

    pub system_program: Program<'info, System>,
}
