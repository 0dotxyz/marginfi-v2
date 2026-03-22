use anchor_lang::prelude::*;

use library::structs::AddressBool;

#[event]
pub struct LogStopRewards {
    pub mint: Pubkey,
}

#[event]
pub struct LogCancelQueuedRewards {
    pub mint: Pubkey,
}

#[event]
pub struct LogTransitionedToNextRewards {
    pub start_time: u64,
    pub end_time: u64,
    pub mint: Pubkey,
}

#[event]
pub struct LogStartRewards {
    pub reward_amount: u64,
    pub duration: u64,
    pub start_time: u64,
    pub mint: Pubkey,
}

#[event]
pub struct LogQueueNextRewards {
    pub reward_amount: u64,
    pub duration: u64,
    pub mint: Pubkey,
}

#[event]
pub struct LogUpdateAuths {
    pub auth_status: Vec<AddressBool>,
}

#[event]
pub struct LogUpdateAuthority {
    pub new_authority: Pubkey,
}
