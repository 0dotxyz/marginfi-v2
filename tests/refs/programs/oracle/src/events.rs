use anchor_lang::prelude::*;

use library::structs::AddressBool;

#[event]
pub struct LogUpdateAuths {
    pub auth_status: Vec<AddressBool>,
}

#[event]
pub struct LogUpdateAuthority {
    pub new_authority: Pubkey,
}

#[event]
pub struct LogStakePoolHighFeeDetected {
    pub stake_pool: Pubkey,
    pub epoch: u64,
}
