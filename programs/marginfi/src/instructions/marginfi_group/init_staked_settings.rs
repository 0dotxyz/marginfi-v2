// Used by the group admin to enable staked collateral banks and configure their default features
use crate::ix_utils;
use crate::state::staked_settings::StakedSettingsImpl;
use crate::utils::wrapped_i80f48_to_f64;
use crate::MarginfiError;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::STAKED_SETTINGS_SEED,
    types::{MarginfiGroup, RiskTier, StakedSettings, WrappedI80F48},
};

pub fn initialize_staked_settings(
    ctx: Context<InitStakedSettings>,
    settings: StakedSettingsConfig,
) -> Result<()> {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let mut staked_settings = ctx.accounts.staked_settings.load_init()?;

    *staked_settings = StakedSettings::new(
        ctx.accounts.staked_settings.key(),
        ctx.accounts.marginfi_group.key(),
        settings.oracle,
        settings.asset_weight_init,
        settings.asset_weight_maint,
        settings.deposit_limit,
        settings.total_asset_value_init_limit,
        settings.oracle_max_age,
        settings.risk_tier,
    );

    msg!(
        "oracle: {:?} max age: {:?}",
        staked_settings.oracle,
        staked_settings.oracle_max_age
    );
    let init_f64: f64 = wrapped_i80f48_to_f64(staked_settings.asset_weight_init);
    let maint_f64: f64 = wrapped_i80f48_to_f64(staked_settings.asset_weight_maint);
    msg!("asset weight init: {:?} maint: {:?}", init_f64, maint_f64);
    msg!(
        "deposit limit: {:?} value limit: {:?} risk tier: {:?}",
        staked_settings.deposit_limit,
        staked_settings.total_asset_value_init_limit,
        staked_settings.risk_tier as u8
    );

    staked_settings.validate()?;

    Ok(())
}

#[derive(Accounts)]
pub struct InitStakedSettings<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    /// Pays the init fee
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        init,
        seeds = [
            STAKED_SETTINGS_SEED.as_bytes(),
            marginfi_group.key().as_ref()
        ],
        bump,
        payer = fee_payer,
        space = 8 + StakedSettings::LEN,
    )]
    pub staked_settings: AccountLoader<'info, StakedSettings>,

    pub system_program: Program<'info, System>,

    #[account(address = pubkey!("Sysvar1nstructions4gQvDKZeTQvzK88j5KqVn5P"))]
    pub instruction_sysvar: UncheckedAccount<'info>,
}

#[derive(AnchorDeserialize, AnchorSerialize, Default)]
pub struct StakedSettingsConfig {
    pub oracle: Pubkey,

    pub asset_weight_init: WrappedI80F48,
    pub asset_weight_maint: WrappedI80F48,

    pub deposit_limit: u64,
    pub total_asset_value_init_limit: u64,

    pub oracle_max_age: u16,
    /// WARN: You almost certainly want "Collateral", using Isolated risk tier makes the asset
    /// worthless as collateral, and is generally useful only when creating a staked collateral pool
    /// for rewards purposes only.
    pub risk_tier: RiskTier,
}
