use crate::events::{EditStakedSettingsEvent, EditStakedSettingsFastEvent};
use crate::ix_utils;
use crate::state::staked_settings::StakedSettingsImpl;
// Used by the slow governance admin to edit the default features of staked collateral banks. Remember
// to propagate afterwards.
use crate::set_if_some;
use crate::MarginfiError;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::CIRCUIT_BREAKER_ENABLED,
    types::{MarginfiGroup, RiskTier, StakedSettings, WrappedI80F48},
};

pub fn edit_staked_settings(
    ctx: Context<EditStakedSettings>,
    settings: StakedSettingsEditConfig,
) -> Result<()> {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;
    let mut staked_settings = ctx.accounts.staked_settings.load_mut()?;

    set_if_some!(staked_settings.oracle, settings.oracle);

    set_if_some!(
        staked_settings.asset_weight_init,
        settings.asset_weight_init
    );
    set_if_some!(
        staked_settings.asset_weight_maint,
        settings.asset_weight_maint
    );
    set_if_some!(staked_settings.deposit_limit, settings.deposit_limit);
    set_if_some!(
        staked_settings.total_asset_value_init_limit,
        settings.total_asset_value_init_limit
    );
    set_if_some!(staked_settings.oracle_max_age, settings.oracle_max_age);
    set_if_some!(staked_settings.risk_tier, settings.risk_tier);

    staked_settings.validate()?;

    emit!(EditStakedSettingsEvent {
        group: ctx.accounts.marginfi_group.key(),
        settings
    });

    Ok(())
}

#[derive(Accounts)]
pub struct EditStakedSettings<'info> {
    #[account(has_one = governance_admin @ MarginfiError::Unauthorized)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub governance_admin: Signer<'info>,

    #[account(
        mut,
        has_one = marginfi_group @ MarginfiError::InvalidGroup
    )]
    pub staked_settings: AccountLoader<'info, StakedSettings>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}

#[derive(AnchorDeserialize, AnchorSerialize, Default)]
pub struct StakedSettingsEditConfig {
    pub oracle: Option<Pubkey>,

    pub asset_weight_init: Option<WrappedI80F48>,
    pub asset_weight_maint: Option<WrappedI80F48>,

    pub deposit_limit: Option<u64>,
    pub total_asset_value_init_limit: Option<u64>,

    pub oracle_max_age: Option<u16>,
    /// WARN: You almost certainly want "Collateral", using Isolated risk tier makes the asset
    /// worthless as collateral, making all outstanding accounts eligible to be liquidated, and is
    /// generally useful only when creating a staked collateral pool for rewards purposes only.
    pub risk_tier: Option<RiskTier>,
    // Note: we may want to make `oracleMaxConfidence` editable at some point, so it doesn't use the
    // default max. Since staked collateral banks only trade SOL, which only uses the ever-popular
    // SOL oracle, this is unlikely to ever come up. If SOL confidence is poor, we are in dire
    // straights!
}

/// Used by the fast admin to edit the circuit breaker that staked collateral banks inherit.
/// Staked banks pick this up on the next propagation.
pub fn edit_staked_settings_fast(
    ctx: Context<EditStakedSettingsFast>,
    settings: StakedSettingsEditConfigFast,
) -> Result<()> {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;
    let mut staked_settings = ctx.accounts.staked_settings.load_mut()?;

    if let Some(enabled) = settings.circuit_breaker_enabled {
        msg!("setting circuit breaker enabled: {:?}", enabled);
        if enabled {
            staked_settings.flags |= CIRCUIT_BREAKER_ENABLED;
        } else {
            staked_settings.flags &= !CIRCUIT_BREAKER_ENABLED;
        }
    }
    set_if_some!(
        staked_settings.cb_deviation_bps_tiers,
        settings.cb_deviation_bps_tiers
    );
    set_if_some!(
        staked_settings.cb_tier_durations_seconds,
        settings.cb_tier_durations_seconds
    );
    set_if_some!(
        staked_settings.cb_escalation_window_mult,
        settings.cb_escalation_window_mult
    );
    set_if_some!(staked_settings.cb_ema_alpha_bps, settings.cb_ema_alpha_bps);
    set_if_some!(
        staked_settings.cb_window_seconds,
        settings.cb_window_seconds
    );
    set_if_some!(
        staked_settings.cb_window_max_up_bps,
        settings.cb_window_max_up_bps
    );
    set_if_some!(
        staked_settings.cb_window_max_down_bps,
        settings.cb_window_max_down_bps
    );

    staked_settings.validate()?;

    emit!(EditStakedSettingsFastEvent {
        group: ctx.accounts.marginfi_group.key(),
        settings
    });

    Ok(())
}

#[derive(Accounts)]
pub struct EditStakedSettingsFast<'info> {
    #[account(has_one = admin @ MarginfiError::Unauthorized)]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = marginfi_group @ MarginfiError::InvalidGroup
    )]
    pub staked_settings: AccountLoader<'info, StakedSettings>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}

#[derive(AnchorDeserialize, AnchorSerialize, Default)]
pub struct StakedSettingsEditConfigFast {
    pub circuit_breaker_enabled: Option<bool>,
    pub cb_deviation_bps_tiers: Option<[u16; 3]>,
    pub cb_tier_durations_seconds: Option<[u16; 3]>,
    pub cb_escalation_window_mult: Option<u8>,
    pub cb_ema_alpha_bps: Option<u16>,
    pub cb_window_seconds: Option<u32>,
    pub cb_window_max_up_bps: Option<u16>,
    pub cb_window_max_down_bps: Option<u16>,
}
