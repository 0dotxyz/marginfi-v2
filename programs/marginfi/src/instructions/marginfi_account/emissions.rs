use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::EMISSION_FLAGS,
    types::{Bank, MarginfiAccount},
};

use crate::{
    check,
    prelude::{MarginfiError, MarginfiResult},
};

/// (account authority) Set the wallet whose canonical ATA will receive
/// off-chain emissions distributions.
pub fn marginfi_account_update_emissions_destination_account(
    ctx: Context<MarginfiAccountUpdateEmissionsDestinationAccount>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    marginfi_account.emissions_destination_account =
        ctx.accounts.destination_account.key();
    Ok(())
}

#[derive(Accounts)]
pub struct MarginfiAccountUpdateEmissionsDestinationAccount<'info> {
    #[account(mut)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        address = marginfi_account.load()?.authority,
    )]
    pub authority: Signer<'info>,

    /// CHECK: Any valid public key. Off-chain systems use this to derive
    /// the canonical ATA for each emissions mint.
    pub destination_account: AccountInfo<'info>,
}

/// Permissionlessly zero out `emissions_outstanding` on a balance after
/// emissions have been disabled on the bank.
pub fn lending_account_clear_emissions(
    ctx: Context<LendingAccountClearEmissions>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let bank = ctx.accounts.bank.load()?;

    check!(
        bank.emissions_rate == 0,
        MarginfiError::InvalidConfig,
        "Emissions rate must be zero"
    );
    check!(
        bank.flags & EMISSION_FLAGS == 0,
        MarginfiError::InvalidConfig,
        "Emission flags must be cleared"
    );

    let balance = marginfi_account
        .lending_account
        .balances
        .iter_mut()
        .find(|b| b.is_active() && b.bank_pk == ctx.accounts.bank.key())
        .ok_or(MarginfiError::BankAccountNotFound)?;

    balance.emissions_outstanding = fixed::types::I80F48::ZERO.into();

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountClearEmissions<'info> {
    #[account(
        mut,
        constraint = marginfi_account.load()?.group == bank.load()?.group,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub bank: AccountLoader<'info, Bank>,
}
