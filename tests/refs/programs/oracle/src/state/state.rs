use anchor_lang::prelude::*;

use crate::constants::{MAX_AUTH_COUNT, MAX_SOURCES};
use crate::state::structs::Sources;

#[account]
#[derive(InitSpace)]
pub struct Oracle {
    pub nonce: u16,
    #[max_len(MAX_SOURCES)]
    pub sources: Vec<Sources>,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct OracleAdmin {
    pub authority: Pubkey,

    #[max_len(MAX_AUTH_COUNT)]
    pub auths: Vec<Pubkey>,
}
