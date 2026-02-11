use crate::state::emode::EmodeSettingsImpl;
use crate::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, EmodeEntry, MarginfiGroup, MAX_EMODE_ENTRIES};

pub fn lending_pool_configure_bank_emode(
    ctx: Context<LendingPoolConfigureBankEmode>,
    emode_tag: u16,
    entries: [EmodeEntry; MAX_EMODE_ENTRIES],
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;
    let group = ctx.accounts.group.load()?;

    let mut sorted_entries = entries;
    sorted_entries.sort_by_key(|e| e.collateral_bank_emode_tag);

    // Prevent footguns from passing data in padding, which could interfere with future values in
    // that assumed-empty space. Yes, we could simply take a struct without padding as input, but
    // having a separate config type has proved to be more of a pain than dealing with padding.
    for entry in sorted_entries.iter_mut() {
        entry.pad0 = [0; 5];
    }

    bank.emode.emode_tag = emode_tag;
    bank.emode.emode_config.entries = sorted_entries;
    bank.emode.timestamp = Clock::get()?.unix_timestamp;

    bank.emode.validate_entries_with_liability_weights(
        &bank.config,
        group.emode_max_init_leverage,
        group.emode_max_maint_leverage,
    )?;

    if bank.emode.emode_config.has_entries() {
        msg!("emode entries detected and activated");
        bank.emode.update_emode_enabled();
    } else {
        msg!("no emode entries detected");
        bank.emode.update_emode_enabled();
    }

    msg!(
        "emode tag set to {:?} entries set to: {:?}",
        emode_tag,
        sorted_entries
    );

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBankEmode<'info> {
    #[account(
        has_one = emode_admin @ MarginfiError::Unauthorized
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub emode_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}
