use anchor_lang::prelude::*;

use crate::state::*;
use library::structs::AddressBool;

#[event]
pub struct LogOperate {
    pub user: Pubkey,
    pub token: Pubkey,
    pub supply_amount: i128,
    pub borrow_amount: i128,
    pub withdraw_to: Pubkey,
    pub borrow_to: Pubkey,
    pub supply_exchange_price: u64,
    pub borrow_exchange_price: u64,
}

#[event]
pub struct LogUpdateAuths {
    pub auth_status: Vec<AddressBool>,
}

#[event]
pub struct LogUpdateGuardians {
    pub guardian_status: Vec<AddressBool>,
}

#[event]
pub struct LogUpdateRevenueCollector {
    pub revenue_collector: Pubkey,
}

#[event]
pub struct LogCollectRevenue {
    pub token: Pubkey,
    pub revenue_amount: u128,
}

#[event]
pub struct LogChangeStatus {
    pub new_status: bool,
}

#[event]
pub struct LogUpdateExchangePrices {
    pub token: Pubkey,
    pub supply_exchange_price: u128,
    pub borrow_exchange_price: u128,
    pub borrow_rate: u16,
    pub utilization: u16,
}

#[event]
pub struct LogUpdateRateDataV1 {
    pub token: Pubkey,
    pub rate_data: RateDataV1Params,
}

#[event]
pub struct LogUpdateRateDataV2 {
    pub token: Pubkey,
    pub rate_data: RateDataV2Params,
}

#[event]
pub struct LogUpdateTokenConfigs {
    pub token_config: TokenConfig,
}

#[event]
pub struct LogUpdateUserClass {
    pub user_class: Vec<AddressU8>,
}

#[event]
pub struct LogUpdateUserWithdrawalLimit {
    pub user: Pubkey,
    pub token: Pubkey,
    pub new_limit: u128,
}

#[event]
pub struct LogUpdateUserSupplyConfigs {
    pub user: Pubkey,
    pub token: Pubkey,
    pub user_supply_config: UserSupplyConfig,
}

#[event]
pub struct LogUpdateUserBorrowConfigs {
    pub user: Pubkey,
    pub token: Pubkey,
    pub user_borrow_config: UserBorrowConfig,
}

#[event]
pub struct LogPauseUser {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub status: u8,
}

#[event]
pub struct LogUnpauseUser {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub status: u8,
}

#[event]
pub struct LogBorrowRateCap {
    pub token: Pubkey,
}

#[event]
pub struct LogClaim {
    pub user: Pubkey,
    pub token: Pubkey,
    pub recipient: Pubkey,
    pub amount: u64,
}

#[event]
pub struct LogUpdateAuthority {
    pub new_authority: Pubkey,
}
