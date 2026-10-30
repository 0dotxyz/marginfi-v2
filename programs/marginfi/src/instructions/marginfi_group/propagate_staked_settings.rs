use crate::state::bank::BankImpl;
// Permissionless ix to propagate a group's staked collateral settings to any bank in that group.
use crate::check;
use crate::state::bank_config::BankConfigImpl;
use crate::MarginfiError;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{ASSET_TAG_STAKED, BANK_SAME_ASSET_EMODE_ELIGIBLE, STAKED_ORACLE_FLAGS},
    types::{Bank, MarginfiGroup, ScopePriceSelection, StakedSettings},
};

pub fn propagate_staked_settings(ctx: Context<PropagateStakedSettings>) -> Result<()> {
    let settings = ctx.accounts.staked_settings.load()?;
    let mut bank = ctx.accounts.bank.load_mut()?;

    // `StakedWithPythPush` or `StakedWithScope`; the Scope entries are zero / unset for Pyth.
    let oracle_setup = settings.staked_oracle_setup();
    let selection = ScopePriceSelection {
        entry_index: settings.scope_base_feed().unwrap_or(0),
        smoothed_entry: settings.scope_smoothed_entry(),
    };
    let pricing_changed = bank.config.oracle_setup != oracle_setup
        || bank.config.oracle_keys[0] != settings.oracle
        || bank.config.effective_scope_selection() != selection;

    // The setup and both Scope entries are part of the same-asset price identity, like the feed.
    check!(
        !pricing_changed || !bank.get_flag(BANK_SAME_ASSET_EMODE_ELIGIBLE),
        MarginfiError::BadEmodeConfig,
        "disable same-asset e-mode eligibility before changing the oracle setup, oracle_keys[0], \
         or the Scope entries"
    );

    bank.config.oracle_setup = oracle_setup;
    bank.config.oracle_keys[0] = settings.oracle;
    bank.config.scope_entry_index = selection.entry_index;
    bank.config
        .set_scope_smoothed_entry(selection.smoothed_entry);
    bank.config.asset_weight_init = settings.asset_weight_init;
    bank.config.asset_weight_maint = settings.asset_weight_maint;
    bank.config.deposit_limit = settings.deposit_limit;
    bank.config.total_asset_value_init_limit = settings.total_asset_value_init_limit;
    bank.config.oracle_max_age = settings.oracle_max_age;
    bank.config.risk_tier = settings.risk_tier;
    bank.flags &= !STAKED_ORACLE_FLAGS;
    bank.flags |= settings.flags & STAKED_ORACLE_FLAGS;

    // Revalidate whenever the setup, the feed, or either Scope entry changes.
    if pricing_changed {
        bank.config
            .validate_oracle_setup(bank.mint, ctx.remaining_accounts, None, None, None)?;
    }

    bank.config.validate()?;

    Ok(())
}

#[derive(Accounts)]
pub struct PropagateStakedSettings<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        has_one = marginfi_group @ MarginfiError::InvalidGroup
    )]
    pub staked_settings: AccountLoader<'info, StakedSettings>,

    #[account(
        mut,
        constraint = {
            let bank = bank.load()?;
            bank.group == marginfi_group.key() &&
                bank.config.asset_tag == ASSET_TAG_STAKED
        }
    )]
    pub bank: AccountLoader<'info, Bank>,
}
