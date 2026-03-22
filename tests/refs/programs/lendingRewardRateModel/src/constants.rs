use anchor_lang::prelude::*;

/// @dev precision decimals for rewards rate
const RATE_PRECISION: u128 = 1_000_000_000_000;

pub const SECONDS_PER_YEAR: u128 = 365 * 24 * 60 * 60;

/// @dev maximum rewards rate is 50%. no config higher than this should be possible.
pub const MAX_RATE: u128 = 50 * RATE_PRECISION; // 1e12 = 1%, this is 50%.

pub const RETURN_PERCENT_PRECISION: u128 = RATE_PRECISION * 100;

/// @dev max allowed auth count
pub const MAX_AUTH_COUNT: usize = 10;

/// temporary hardcoded solution to give dev team MS non-problematic rights to speed up setting up new protocols. to be improved later
pub const PROTOCOL_INIT_AUTH: Pubkey = pubkey!("3H8C6yYTXUcN9RRRDmcLDt3e4aZLYRRX4x2HbEjTqQAA");

pub const GOVERNANCE_MS: Pubkey = pubkey!("HqPrpa4ESBDnRHRWaiYtjv4xe93wvCS9NNZtDwR89cVa");