use anchor_lang::prelude::*;

/// @dev precision used for exchange prices
pub const EXCHANGE_PRICES_PRECISION: u128 = liquidity::constants::EXCHANGE_PRICES_PRECISION; // 1e12

pub const RETURN_PERCENT_PRECISION: u128 = EXCHANGE_PRICES_PRECISION * 100;

/// @dev Ignoring leap years
pub const SECONDS_PER_YEAR: u128 = 365 * 24 * 60 * 60;

/// @dev max allowed reward rate is 50%
pub const MAX_REWARDS_RATE: u64 = 50 * (EXCHANGE_PRICES_PRECISION as u64); // 50%;

/// @dev max allowed auth count
pub const MAX_AUTH_COUNT: usize = 10;

/// temporary hardcoded solution to give dev team MS non-problematic rights to speed up setting up new protocols. to be improved later
pub const PROTOCOL_INIT_AUTH: Pubkey = pubkey!("3H8C6yYTXUcN9RRRDmcLDt3e4aZLYRRX4x2HbEjTqQAA");

pub const GOVERNANCE_MS: Pubkey = pubkey!("HqPrpa4ESBDnRHRWaiYtjv4xe93wvCS9NNZtDwR89cVa");