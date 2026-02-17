use anchor_lang::prelude::*;

use library::structs::AddressBool;

#[event]
pub struct LogUpdateRates {
    pub token_exchange_price: u64,
    pub liquidity_exchange_price: u64,
}

#[event]
pub struct LogDeposit {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub assets: u64,
    pub shares_minted: u64,
}

#[event]
pub struct LogWithdraw {
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub owner: Pubkey,
    pub assets: u64,
    pub shares_burned: u64,
}

#[event]
pub struct LogUpdateRewards {
    pub rewards_rate_model: Pubkey,
}

#[event]
pub struct LogRebalance {
    pub assets: u64,
}

#[event]
pub struct LogUpdateRebalancer {
    pub new_rebalancer: Pubkey,
}

#[event]
pub struct LogUpdateAuths {
    pub auth_status: Vec<AddressBool>,
}

#[event]
pub struct LogUpdateAuthority {
    pub new_authority: Pubkey,
}
