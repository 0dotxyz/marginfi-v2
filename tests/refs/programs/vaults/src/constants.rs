use anchor_lang::prelude::*;

/// Precision used for exchange prices
pub const EXCHANGE_PRICES_PRECISION: u128 = liquidity::constants::EXCHANGE_PRICES_PRECISION; // 1e12

/// Scale factor for exchange prices during calculations
pub const EXCHANGE_PRICE_SCALE_FACTOR: u128 = 10u128.pow(18); // 1e18

/// Maximum token decimals allowed
pub const MAX_TOKEN_DECIMALS: u8 = 9;

pub const SECONDS_PER_YEAR: u128 = 31_536_000; // 365 * 24 * 60 * 60

pub const BILLION: u128 = 10u128.pow(9); // 1e9

pub const FOUR_DECIMALS: u128 = 10_000; // 1e4

pub const THREE_DECIMALS: u128 = 1000; // 1e3

/// Address of dead account
pub const ADDRESS_DEAD: Pubkey = Pubkey::new_from_array([0; 32]);

/// Initializing branch debt factor. 35 | 15 bit number. Where full 35 bits and 15th bit is occupied.
/// Making the total number as (2**35 - 1) << 2**14.
pub const INITIAL_BRANCH_DEBT_FACTOR: u128 = (X35 << 15) | (1 << 14);

pub const X10: u128 = 0x3ff;
pub const X16: u128 = 0xffff;
pub const X30: u128 = 0x3FFFFFFF;
pub const X35: u128 = 0x7ffffffff;

// Minimum and max acceptable operate amounts
pub const MIN_OPERATE: u128 = 1_000; // 1e3
pub const MAX_OPERATE: u128 = i64::MAX as u128;

pub const LOWER_DECIMALS_OPERATE: u8 = 4;
pub const MIN_OPERATE_LOWER_DECIMALS_AMOUNT: u128 = 10;

// Minimum acceptable debt amount
pub const MIN_DEBT: u128 = 1_000; // 1e3 for a 9 decimals token, becomes 1e6 for 6 decimals token after scaling
pub const MIN_DEBT_LOWER_DECIMALS: u128 = 10; // for decimals < 4, minimum debt is 1, for 2 decimals after scaling it becomes 1e7

// Minimum branch debt
pub const MINIMUM_BRANCH_DEBT: u128 = 100; // 1e2 for a 9 decimals token, becomes 1e5 for 6 decimals token after scaling
pub const MINIMUM_BRANCH_DEBT_LOWER_DECIMALS: u128 = 5; // for decimals < 4, minimum branch debt is 5, for 2 decimals after scaling it becomes 5e7

// Minimum tick debt
// @dev always make sure that minimum tick debt is smaller than minimum debt and minimum operate amount
pub const MINIMUM_TICK_DEBT: u128 = 100; // 1e2 for a 9 decimals token, becomes 1e5 for 6 decimals token after scaling
pub const MINIMUM_TICK_DEBT_LOWER_DECIMALS: u128 = 5; // for decimals < 4, minimum tick debt is 5, for 2 decimals after scaling it becomes 5e7

pub const MAX_LIQUIDATION_PENALTY: u16 = 9970; // 99.7%

// Max allowed liquidation rounding difference between actual_debt_amt and debt_amount
pub const MAX_LIQUIDATION_ROUNDING_DIFF: u128 = 100; // 1e2

/// @dev max allowed auth count
pub const MAX_AUTH_COUNT: usize = 10;

/// temporary hardcoded solution to give dev team MS non-problematic rights to speed up setting up new protocols. to be improved later
pub const PROTOCOL_INIT_AUTH: Pubkey = pubkey!("3H8C6yYTXUcN9RRRDmcLDt3e4aZLYRRX4x2HbEjTqQAA");

pub const GOVERNANCE_MS: Pubkey = pubkey!("HqPrpa4ESBDnRHRWaiYtjv4xe93wvCS9NNZtDwR89cVa");
