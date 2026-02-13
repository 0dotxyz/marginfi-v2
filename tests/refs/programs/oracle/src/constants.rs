use anchor_lang::prelude::*;

pub const RATE_OUTPUT_DECIMALS: u32 = 15;
pub const MAX_SOURCES: usize = 4;
pub const MAX_AUTH_COUNT: usize = 10;

pub const SECONDS_PER_HOUR: u64 = 3600;
pub const MAX_AGE_OPERATE: u64 = 600; // should be in seconds -> 10 minutes

pub const MAX_AGE_LIQUIDATE: u64 = 7200; // should be in seconds, less requirements on liquidate() to keep protocol safe -> 2hrs

pub const CONFIDENCE_SCALE_FACTOR_LIQUIDATE: u64 = 25; // Rejects if confidence < 1/25 = 4% of price
pub const CONFIDENCE_SCALE_FACTOR_OPERATE: u64 = 50; // Rejects if confidence < 1/50 = 2% of price

pub const MAX_DIVISOR: u128 = 10u128.pow(10);
pub const MAX_MULTIPLIER: u128 = 10u128.pow(10);

pub const GOVERNANCE_MS: Pubkey = pubkey!("HqPrpa4ESBDnRHRWaiYtjv4xe93wvCS9NNZtDwR89cVa");

// 0.5% max fee ceiling
pub const MAX_FEE_CEILING: u64 = 5;

pub const AVG_SLOT_TIME_IN_MILLISECONDS: u64 = 400;

pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

pub const FACTOR: u128 = 10u128.pow(RATE_OUTPUT_DECIMALS);

pub const MILLISECONDS_PER_SECOND: u64 = 1000;
pub const DEFAULT_DIVISOR: u128 = 1;
pub const DEFAULT_MULTIPLIER: u128 = 1;

pub const SINGLE_POOL_ACCOUNTS_COUNT: usize = 3;
pub const JUP_LEND_ACCOUNTS_COUNT: usize = 4;
