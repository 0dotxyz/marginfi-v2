use crate::check;
use crate::events::EditStakedSettingsEvent;
use crate::ix_utils;
use crate::state::staked_settings::StakedSettingsImpl;
// Used by the slow governance admin to edit the default features of staked collateral banks. Remember
// to propagate afterwards.
use crate::set_if_some;
use crate::MarginfiError;
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{MarginfiGroup, RiskTier, StakedSettings, WrappedI80F48};

pub fn edit_staked_settings(
    ctx: Context<EditStakedSettings>,
    settings: StakedSettingsEditConfig,
) -> Result<()> {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;
    let mut staked_settings = ctx.accounts.staked_settings.load_mut()?;
    let feed_before = (staked_settings.oracle, staked_settings.scope_base_feed());

    set_if_some!(staked_settings.oracle, settings.oracle);
    match settings.base_feed {
        Some(StakedBaseFeed::Pyth) => staked_settings.set_scope_base_feed(None),
        Some(StakedBaseFeed::Scope { entry_index }) => {
            staked_settings.set_scope_base_feed(Some(entry_index))
        }
        None => {}
    }
    let feed_changed = feed_before != (staked_settings.oracle, staked_settings.scope_base_feed());
    apply_smoothed_entry(&mut staked_settings, settings.smoothed_entry, feed_changed)?;

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

/// Sets or clears the smoothed entry. Setting one requires a Scope base feed; which entry is the EMA
/// rests with the admin, like the base entry (banks check it is readable when the settings are
/// propagated), so it only has to be a nonzero entry other than the base entry. Switching the
/// base feed to Pyth clears the entry; any other feed change (the Scope oracle account or its
/// base entry) without an explicit `smoothed_entry` is rejected while one is set, so a stale entry
/// never outlives the feed it was chosen for.
fn apply_smoothed_entry(
    staked_settings: &mut StakedSettings,
    edit: Option<StakedSmoothedEntry>,
    feed_changed: bool,
) -> Result<()> {
    match edit {
        Some(StakedSmoothedEntry::Disabled) => staked_settings.set_scope_smoothed_entry(None),
        Some(StakedSmoothedEntry::Scope { entry_index }) => {
            let Some(base_entry) = staked_settings.scope_base_feed() else {
                msg!("a smoothed entry needs a Scope base feed");
                return err!(MarginfiError::InvalidOracleSetup);
            };
            check!(
                entry_index != 0 && entry_index != base_entry,
                MarginfiError::ScopeInvalidSmoothedEntry,
                "the smoothed entry must be a nonzero entry other than the base entry"
            );
            staked_settings.set_scope_smoothed_entry(Some(entry_index));
        }
        None => check!(
            !feed_changed || staked_settings.scope_smoothed_entry().is_none(),
            MarginfiError::InvalidOracleSetup,
            "set or disable `smoothed_entry` when changing the Scope oracle or base entry"
        ),
    }
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

/// Where staked banks read SOL/USD from: `Pyth` propagates them as `StakedWithPythPush`, `Scope`
/// as `StakedWithScope`. Pair a switch with the matching `oracle` (Pyth push account or Scope
/// `OraclePrices` account).
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StakedBaseFeed {
    Pyth,
    Scope { entry_index: u16 },
}

/// The time-weighted SOL/USD price staked banks use for initial margin. `Disabled` uses the spot
/// price for both price types.
#[derive(AnchorDeserialize, AnchorSerialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum StakedSmoothedEntry {
    Disabled,
    Scope { entry_index: u16 },
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
    /// Switch the SOL/USD base feed between Pyth (`StakedWithPythPush`) and a Scope entry
    /// (`StakedWithScope`). `None` leaves it unchanged.
    pub base_feed: Option<StakedBaseFeed>,
    /// Set or disable the Scope smoothed entry (a PythLazerEMA/ScopeSmoothed of the base feed
    /// entry). `None` leaves it unchanged.
    pub smoothed_entry: Option<StakedSmoothedEntry>,
    // Note: we may want to make `oracleMaxConfidence` editable at some point, so it doesn't use the
    // default max. Since staked collateral banks only trade SOL, which only uses the ever-popular
    // SOL oracle, this is unlikely to ever come up. If SOL confidence is poor, we are in dire
    // straights!
}
