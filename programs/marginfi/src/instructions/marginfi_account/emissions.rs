use anchor_lang::prelude::*;
use marginfi_type_crate::types::{MarginfiAccount, ACCOUNT_FROZEN};

use crate::{
    check,
    prelude::{MarginfiError, MarginfiResult},
    state::marginfi_account::MarginfiAccountImpl,
};

/// (account authority) Set the wallet whose canonical ATA will receive
/// off-chain emissions distributions.
pub fn marginfi_account_update_emissions_destination_account(
    ctx: Context<MarginfiAccountUpdateEmissionsDestinationAccount>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_FROZEN),
        MarginfiError::AccountFrozen
    );

    marginfi_account.emissions_destination_account = ctx.accounts.destination_account.key();
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
