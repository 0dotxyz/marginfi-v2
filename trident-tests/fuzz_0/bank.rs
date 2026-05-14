use trident_fuzz::fuzzing::*;

use crate::types::marginfi::OracleSetup;

#[derive(Clone, Copy)]
pub struct Currency {
    pub mint: Pubkey,
    pub mint_authority: Pubkey,
}

impl Currency {
    pub fn new(mint: Pubkey, mint_authority: Pubkey) -> Self {
        Self {
            mint,
            mint_authority,
        }
    }
}

#[derive(Clone, Copy)]
pub struct FuzzTestBank {
    pub currency: Currency,
    pub address: Pubkey,
    pub oracle_setup: (OracleSetup, Pubkey),
}
