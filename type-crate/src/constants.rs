use fixed::types::I80F48;
use fixed_macro::types::I80F48;

pub const LIQUIDITY_VAULT_AUTHORITY_SEED: &str = "liquidity_vault_auth";
pub const INSURANCE_VAULT_AUTHORITY_SEED: &str = "insurance_vault_auth";
pub const FEE_VAULT_AUTHORITY_SEED: &str = "fee_vault_auth";

pub const LIQUIDITY_VAULT_SEED: &str = "liquidity_vault";
pub const INSURANCE_VAULT_SEED: &str = "insurance_vault";
pub const FEE_VAULT_SEED: &str = "fee_vault";

pub const FEE_STATE_SEED: &str = "feestate";
pub const STAKED_SETTINGS_SEED: &str = "staked_settings";

pub const EMISSIONS_AUTH_SEED: &str = "emissions_auth_seed";
pub const EMISSIONS_TOKEN_ACCOUNT_SEED: &str = "emissions_token_account_seed";

pub const LIQUIDATION_RECORD_SEED: &str = "liq_record";
pub const MARGINFI_ACCOUNT_SEED: &str = "marginfi_account";

pub const METADATA_SEED: &str = "metadata";

/// TODO: Make these variable per bank
pub const LIQUIDATION_LIQUIDATOR_FEE: I80F48 = I80F48!(0.025);
pub const LIQUIDATION_INSURANCE_FEE: I80F48 = I80F48!(0.025);

pub const SECONDS_PER_YEAR: I80F48 = I80F48!(31_536_000);
pub const DAILY_RESET_INTERVAL: i64 = 24 * 60 * 60; // 24 hours
pub const HOURLY_WINDOW_DURATION: u64 = 60 * 60; // 1 hour in seconds
pub const DAILY_WINDOW_DURATION: u64 = 24 * 60 * 60; // 24 hours in seconds

/// Due to real-world constraints, oracles using an age less than this value are typically too
/// unreliable, and we want to restrict pools from picking an oracle that is effectively unusable
/// Switchboard oracles are cranked on demand, so we can use a lower value (10 seconds)
pub const ORACLE_MIN_AGE: u16 = 10;
pub const MAX_PYTH_ORACLE_AGE: u64 = 60;

/// Range that contains 95% price data distribution
///
/// https://docs.pyth.network/price-feeds/best-practices#confidence-intervals
pub const CONF_INTERVAL_MULTIPLE: I80F48 = I80F48!(2.12);
/// Range that contains 95% price data distribution in a normal distribution
pub const STD_DEV_MULTIPLE: I80F48 = I80F48!(1.96);
/// Maximum confidence interval allowed
pub const MAX_CONF_INTERVAL: I80F48 = I80F48!(0.05);

pub const U32_MAX: I80F48 = I80F48!(4_294_967_295);
pub const U32_MAX_DIV_10: I80F48 = I80F48!(429_496_730);

pub const USDC_EXPONENT: i32 = 6;

pub const MAX_ORACLE_KEYS: usize = 5;

/// Any balance below 1 SPL token amount is treated as none,
/// this is to account for any artifacts resulting from binary fraction arithemtic.
pub const EMPTY_BALANCE_THRESHOLD: I80F48 = I80F48!(1);

/// Any account with assets below this threshold is considered bankrupt.
/// The account also needs to have more liabilities than assets.
///
/// This is USD denominated, so 0.001 = $0.1
pub const BANKRUPT_THRESHOLD: I80F48 = I80F48!(0.1);

/// Comparios threshold used to account for arithmetic artifacts on balances
pub const ZERO_AMOUNT_THRESHOLD: I80F48 = I80F48!(0.0001);

pub const EMISSIONS_FLAG_BORROW_ACTIVE: u64 = 1 << 0;
pub const EMISSIONS_FLAG_LENDING_ACTIVE: u64 = 1 << 1;
pub const PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG: u64 = 1 << 2;
pub const FREEZE_SETTINGS: u64 = 1 << 3;
pub const CLOSE_ENABLED_FLAG: u64 = 1 << 4;
pub const TOKENLESS_REPAYMENTS_ALLOWED: u64 = 1 << 5;
pub const TOKENLESS_REPAYMENTS_COMPLETE: u64 = 1 << 6;

/// True if bank created in 0.1.4 or later, or if migrated to the new oracle setup from a prior
/// version. False otherwise.
pub const PYTH_PUSH_MIGRATED_DEPRECATED: u8 = 1 << 0;

pub const EMISSION_FLAGS: u64 = EMISSIONS_FLAG_BORROW_ACTIVE | EMISSIONS_FLAG_LENDING_ACTIVE;
pub const GROUP_FLAGS: u64 = PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG
    | FREEZE_SETTINGS
    | TOKENLESS_REPAYMENTS_ALLOWED
    | TOKENLESS_REPAYMENTS_COMPLETE;

/// Cutoff timestamp for balance last_update used in accounting collected emissions.
/// Any balance updates before this timestamp are ignored, and current_timestamp is used instead.
pub const MIN_EMISSIONS_START_TIME: u64 = 1681989983;

pub const MAX_EXP_10_I80F48: usize = 24;
pub const EXP_10_I80F48: [I80F48; MAX_EXP_10_I80F48] = [
    I80F48!(1),                        // 10^0
    I80F48!(10),                       // 10^1
    I80F48!(100),                      // 10^2
    I80F48!(1000),                     // 10^3
    I80F48!(10000),                    // 10^4
    I80F48!(100000),                   // 10^5
    I80F48!(1000000),                  // 10^6
    I80F48!(10000000),                 // 10^7
    I80F48!(100000000),                // 10^8
    I80F48!(1000000000),               // 10^9
    I80F48!(10000000000),              // 10^10
    I80F48!(100000000000),             // 10^11
    I80F48!(1000000000000),            // 10^12
    I80F48!(10000000000000),           // 10^13
    I80F48!(100000000000000),          // 10^14
    I80F48!(1000000000000000),         // 10^15
    I80F48!(10000000000000000),        // 10^16
    I80F48!(100000000000000000),       // 10^17
    I80F48!(1000000000000000000),      // 10^18
    I80F48!(10000000000000000000),     // 10^19
    I80F48!(100000000000000000000),    // 10^20
    I80F48!(1000000000000000000000),   // 10^21
    I80F48!(10000000000000000000000),  // 10^22
    I80F48!(100000000000000000000000), // 10^23
];

pub const MAX_EXP_10: usize = 21;
pub const EXP_10: [i128; MAX_EXP_10] = [
    1,                     // 10^0
    10,                    // 10^1
    100,                   // 10^2
    1000,                  // 10^3
    10000,                 // 10^4
    100000,                // 10^5
    1000000,               // 10^6
    10000000,              // 10^7
    100000000,             // 10^8
    1000000000,            // 10^9
    10000000000,           // 10^10
    100000000000,          // 10^11
    1000000000000,         // 10^12
    10000000000000,        // 10^13
    100000000000000,       // 10^14
    1000000000000000,      // 10^15
    10000000000000000,     // 10^16
    100000000000000000,    // 10^17
    1000000000000000000,   // 10^18
    10000000000000000000,  // 10^19
    100000000000000000000, // 10^20
];

/// Value where total_asset_value_init_limit is considered inactive
pub const TOTAL_ASSET_VALUE_INIT_LIMIT_INACTIVE: u64 = 0;

/// For testing, this is a typical program fee.
pub const PROTOCOL_FEE_RATE_DEFAULT: I80F48 = I80F48!(0.025);
/// For testing, this is a typical program fee.
pub const PROTOCOL_FEE_FIXED_DEFAULT: I80F48 = I80F48!(0.01);

/// Pyth Pull Oracles sponsored by Pyth use this shard ID.
pub const PYTH_SPONSORED_SHARD_ID: u16 = 0;
/// Pyth Pull Oracles sponsored by Marginfi use this shard ID.
pub const MARGINFI_SPONSORED_SHARD_ID: u16 = 3301;

/// A regular asset that can be comingled with any other regular asset or with `ASSET_TAG_SOL`
pub const ASSET_TAG_DEFAULT: u8 = 0;
/// Accounts with a SOL position can comingle with **either** `ASSET_TAG_DEFAULT` or
/// `ASSET_TAG_STAKED` positions, but not both
pub const ASSET_TAG_SOL: u8 = 1;
/// Staked SOL assets. Accounts with a STAKED position can only deposit other STAKED assets or SOL
/// (`ASSET_TAG_SOL`) and can only borrow SOL (`ASSET_TAG_SOL`)
pub const ASSET_TAG_STAKED: u8 = 2;
/// Kamino assets. Accounts with a KAMINO position can only deposit other KAMINO assets or regular
/// assets (`ASSET_TAG_DEFAULT`).
pub const ASSET_TAG_KAMINO: u8 = 3;
/// Drift assets. Accounts with a DRIFT position can only deposit other DRIFT assets or regular
/// assets (`ASSET_TAG_DEFAULT`).
pub const ASSET_TAG_DRIFT: u8 = 4;
/// Solend assets. Accounts with a SOLEND position can only deposit other SOLEND assets or regular
/// assets (`ASSET_TAG_DEFAULT`).
pub const ASSET_TAG_SOLEND: u8 = 5;

/// Maximum number of integration positions (Kamino + Drift + Solend) allowed per account. Hardcoded
/// limit to prevent accounts from becoming unliquidatable due to CU/heap memory issues in
/// liquidation. These integrations require 3 accounts per position for health checks (bank + oracle
/// + reserve/spot-market), so they share the same limit.
pub const MAX_INTEGRATION_POSITIONS: usize = 8;

// WARN: You can set anything here, including a discrim that's technically "wrong" for the struct
// with that name, and prod will use that hash anyways. Don't change these hashes once a struct is
// live in prod.
pub mod discriminators {
    pub const GROUP: [u8; 8] = [182, 23, 173, 240, 151, 206, 182, 67];
    pub const BANK: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
    pub const ACCOUNT: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
    pub const FEE_STATE: [u8; 8] = [63, 224, 16, 85, 193, 36, 235, 220];
    pub const STAKED_SETTINGS: [u8; 8] = [157, 140, 6, 77, 89, 173, 173, 125];
    pub const LIQUIDATION_RECORD: [u8; 8] = [95, 116, 23, 132, 89, 210, 245, 162];
}

pub mod ix_discriminators {
    pub const INIT_LIQUIDATION_RECORD: [u8; 8] = [236, 213, 238, 126, 147, 251, 164, 8];
    pub const START_LIQUIDATION: [u8; 8] = [244, 93, 90, 214, 192, 166, 191, 21];
    pub const END_LIQUIDATION: [u8; 8] = [110, 11, 244, 54, 229, 181, 22, 184];
    pub const LENDING_ACCOUNT_WITHDRAW: [u8; 8] = [36, 72, 74, 19, 210, 210, 192, 192];
    pub const LENDING_ACCOUNT_REPAY: [u8; 8] = [79, 209, 172, 177, 222, 51, 173, 151];
    pub const LENDING_SETTLE_EMISSIONS: [u8; 8] = [234, 22, 84, 214, 118, 176, 140, 170];
    pub const LENDING_WITHDRAW_EMISSIONS: [u8; 8] = [161, 58, 136, 174, 242, 223, 156, 176];
    pub const KAMINO_WITHDRAW: [u8; 8] = [199, 101, 41, 45, 213, 98, 224, 200];
    pub const DRIFT_WITHDRAW: [u8; 8] = [86, 59, 186, 123, 183, 181, 234, 137];
    pub const START_FLASHLOAN: [u8; 8] = [14, 131, 33, 220, 81, 186, 180, 107];
    pub const END_FLASHLOAN: [u8; 8] = [105, 124, 201, 106, 153, 2, 8, 156];
    pub const START_DELEVERAGE: [u8; 8] = [10, 138, 10, 57, 40, 232, 182, 193];
    pub const END_DELEVERAGE: [u8; 8] = [114, 14, 250, 143, 252, 104, 214, 209];
}
