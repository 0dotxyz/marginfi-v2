use anchor_lang::prelude::*;

use crate::{errors::ErrorCodes, state::*};

pub fn get_exchange_prices(ctx: Context<GetExchangePrices>) -> Result<(u128, u128, u128, u128)> {
    let vault_state = &ctx.accounts.vault_state.load()?;
    let vault_config = &ctx.accounts.vault_config.load()?;
    let supply_token_reserves = &ctx.accounts.supply_token_reserves;
    let borrow_token_reserves = &ctx.accounts.borrow_token_reserves;

    if vault_config.vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    if supply_token_reserves.load()?.mint != vault_config.supply_token {
        return Err(error!(ErrorCodes::VaultInvalidSupplyMint));
    }

    if borrow_token_reserves.load()?.mint != vault_config.borrow_token {
        return Err(error!(ErrorCodes::VaultInvalidBorrowMint));
    }

    Ok(vault_state.load_exchange_prices(
        vault_config,
        supply_token_reserves,
        borrow_token_reserves,
    )?)
}
