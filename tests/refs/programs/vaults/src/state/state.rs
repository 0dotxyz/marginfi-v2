use anchor_lang::prelude::*;

use crate::constants::MAX_AUTH_COUNT;

#[account]
#[derive(InitSpace)]
pub struct VaultAdmin {
    pub authority: Pubkey,
    pub liquidity_program: Pubkey,
    pub next_vault_id: u16, // Next vault ID

    #[max_len(MAX_AUTH_COUNT)]
    pub auths: Vec<Pubkey>,

    pub bump: u8,
}

// @dev This account is used to store the metadata for the vault
// Currently, it is used to store the lookup table for the vault
// More fields can be added here in the future
#[account]
#[derive(InitSpace)]
pub struct VaultMetadata {
    pub vault_id: u16,
    pub lookup_table: Pubkey, // Address of lookup table for this vault
    pub supply_mint_decimals: u8,
    pub borrow_mint_decimals: u8,
}
