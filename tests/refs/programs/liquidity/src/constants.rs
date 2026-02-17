use anchor_lang::prelude::*;

/// Precision used for exchange prices
pub const EXCHANGE_PRICES_PRECISION: u128 = 10u128.pow(12); // 1e12

/// Minimum token decimals allowed
pub const MIN_TOKEN_DECIMALS: u8 = 2;

/// Maximum token decimals allowed
pub const MAX_TOKEN_DECIMALS: u8 = 9;

/// Minimum and maximum amount to acceptable for operate function
pub const MIN_OPERATE_AMOUNT: u128 = 10;
pub const MAX_OPERATE: u128 = i64::MAX as u128;

/// Seconds per year (ignoring leap years)
pub const SECONDS_PER_YEAR: u128 = 365 * 24 * 60 * 60;

/// Maximum token amount cap:
/// as max total supply of any mint is u64 in spl token mint, we can use u60 size to be safe for all realistic cases
pub const MAX_TOKEN_AMOUNT_CAP: u128 = (1u128 << 60) - 1;

/// Maximum input amount excess (1% = 100)
pub const MAX_INPUT_AMOUNT_EXCESS: u128 = 100;

pub const FOUR_DECIMALS: u128 = 10u128.pow(4); // 1e4

pub const TWELVE_DECIMALS: u128 = 10u128.pow(12); // 1e12

pub const X14: u128 = 0x3fff;
pub const X24: u128 = 0xffffff;

pub const WSOL: Pubkey = pubkey!("So11111111111111111111111111111111111111112");

/// @dev max allowed auth count
pub const MAX_AUTH_COUNT: usize = 10;

/// @dev max allowed user classes
pub const MAX_USER_CLASSES: usize = 100;

/// temporary hardcoded solution to give dev team MS non-problematic rights to speed up setting up new protocols. to be improved later
pub const PROTOCOL_INIT_AUTH: Pubkey = pubkey!("3H8C6yYTXUcN9RRRDmcLDt3e4aZLYRRX4x2HbEjTqQAA");

pub const GOVERNANCE_MS: Pubkey = pubkey!("HqPrpa4ESBDnRHRWaiYtjv4xe93wvCS9NNZtDwR89cVa");
