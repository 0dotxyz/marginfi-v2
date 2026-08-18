use fixed::types::I80F48;
use fixed_macro::types::I80F48;

pub const LIQUIDITY_VAULT_AUTHORITY_SEED: &str = "liquidity_vault_auth";
pub const INSURANCE_VAULT_AUTHORITY_SEED: &str = "insurance_vault_auth";
pub const FEE_VAULT_AUTHORITY_SEED: &str = "fee_vault_auth";

pub const LIQUIDITY_VAULT_SEED: &str = "liquidity_vault";
pub const INSURANCE_VAULT_SEED: &str = "insurance_vault";
pub const FEE_VAULT_SEED: &str = "fee_vault";
pub const DRIFT_USER_SEED: &str = "user";
pub const DRIFT_USER_STATS_SEED: &str = "user_stats";
pub const SOLEND_OBLIGATION_SEED: &str = "solend_obligation";
pub const JUPLEND_F_TOKEN_VAULT_SEED: &str = "f_token_vault";

pub const FEE_STATE_SEED: &str = "feestate";
pub const STAKED_SETTINGS_SEED: &str = "staked_settings";
pub const SAME_ASSET_EMODE_REGISTRY_SEED: &str = "same_asset_emode_registry";

pub const EMISSIONS_TOKEN_ACCOUNT_SEED: &str = "emissions_token_account_seed";

pub const LIQUIDATION_RECORD_SEED: &str = "liq_record";
pub const MARGINFI_ACCOUNT_SEED: &str = "marginfi_account";
pub const ORDER_SEED: &str = "order";
pub const EXECUTE_ORDER_SEED: &str = "execute_order";

pub const METADATA_SEED: &str = "metadata";

/// Default liquidation fee as an I80F48 fraction (2.5%), used when a bank's
/// `liquidation_liquidator_fee` / `liquidation_insurance_fee` is 0. Matches the historical
/// hardcoded values.
pub const DEFAULT_LIQUIDATION_FEE: I80F48 = I80F48!(0.025);

/// Maximum per-fee liquidation fee an admin may configure, encoded like the fields it caps
/// (`u32_to_centi`, `u32::MAX` = 100%). Caps each of the two fees at ~50% so their sum stays below
/// 100% — otherwise the liquidatee's collateral credit (`final_discount`) would go negative.
pub const MAX_LIQUIDATION_FEE_U32: u32 = u32::MAX / 2;

pub const SECONDS_PER_YEAR: I80F48 = I80F48!(31_536_000);
pub const DAILY_RESET_INTERVAL: i64 = 24 * 60 * 60; // 24 hours
pub const HOURLY_RESET_DURATION: u64 = 60 * 60; // 1 hour in seconds

/// Due to real-world constraints, oracles using an age less than this value are typically too
/// unreliable, and we want to restrict pools from picking an oracle that is effectively unusable
/// Switchboard oracles are cranked on demand, so we can use a lower value (10 seconds)
pub const ORACLE_MIN_AGE: u16 = 10;
pub const MAX_PYTH_ORACLE_AGE: u64 = 60;
/// Number of active tags currently supported for orders.
pub const ORDER_ACTIVE_TAGS: usize = 2;
/// Compile-time guard to ensure ORDER_ACTIVE_TAGS stays 2 as assumed
/// in several places in the code for simplicity.
/// It can be removed when orders are extended to allow more balances.
pub const _: () = assert!(ORDER_ACTIVE_TAGS == 2);
/// Padding length (in bytes) to preserve `Order` layout when more balances are added.
pub const ORDER_TAG_PADDING: usize = 32;

/// Range that contains 95% price data distribution
///
/// https://docs.pyth.network/price-feeds/best-practices#confidence-intervals
pub const CONF_INTERVAL_MULTIPLE: I80F48 = I80F48!(2.12);
/// Maximum confidence interval allowed
pub const MAX_CONF_INTERVAL: I80F48 = I80F48!(0.05);

pub const U32_MAX: I80F48 = I80F48!(4_294_967_295);
pub const U32_MAX_DIV_10: I80F48 = I80F48!(429_496_730);

pub const USDC_EXPONENT: i32 = 6;

pub const MAX_ORACLE_KEYS: usize = 5;

/// Any balance below 1 SPL token unit is treated as empty.
/// This is to account for any artifacts resulting from binary fraction arithmetic.
pub const EMPTY_BALANCE_THRESHOLD: I80F48 = I80F48!(1);

/// Any account with assets below this threshold is considered bankrupt.
/// The account also needs to have more liabilities than assets.
///
/// This is USD denominated, so 0.001 = $0.1
pub const BANKRUPT_THRESHOLD: I80F48 = I80F48!(0.1);

/// Comparison threshold used to account for arithmetic artifacts on balances
pub const ZERO_AMOUNT_THRESHOLD: I80F48 = I80F48!(0.0001);

pub const EMISSIONS_FLAG_BORROW_ACTIVE: u64 = 1 << 0;
pub const EMISSIONS_FLAG_LENDING_ACTIVE: u64 = 1 << 1;
pub const PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG: u64 = 1 << 2;
pub const FREEZE_SETTINGS: u64 = 1 << 3;
pub const CLOSE_ENABLED_FLAG: u64 = 1 << 4;
pub const TOKENLESS_REPAYMENTS_ALLOWED: u64 = 1 << 5;
pub const TOKENLESS_REPAYMENTS_COMPLETE: u64 = 1 << 6;
pub const IS_T22: u64 = 1 << 7;
/// Bank provenance bit: set when the bank is known to be seed-derived (PDA).
pub const BANK_SEED_KNOWN: u64 = 1 << 8;
/// True if bank created in 0.1.4 or later, or if migrated to the new oracle setup from a prior
/// version. False otherwise.
pub const PYTH_PUSH_MIGRATED_DEPRECATED: u8 = 1 << 0;

/// Staked-collateral oracle transition flags stored on `Bank.flags` and copied from
/// `StakedSettings.flags` during staked-settings propagation.
/// To be removed once SVSP update is rolled out (likely in 1.10)
pub const STAKED_ORACLE_DISABLED: u64 = 1 << 9;
pub const STAKED_ORACLE_PRICE_USES_ONRAMP: u64 = 1 << 10;
pub const STAKED_ORACLE_FLAGS: u64 = STAKED_ORACLE_DISABLED | STAKED_ORACLE_PRICE_USES_ONRAMP;
/// Enables the per-bank oracle circuit breaker.
pub const CIRCUIT_BREAKER_ENABLED: u64 = 1 << 11;
/// Bank opt-in bit: set when same-asset e-mode may use this bank.
pub const BANK_SAME_ASSET_EMODE_ELIGIBLE: u64 = 1 << 12;

pub const GROUP_FLAGS: u64 = PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG
    | FREEZE_SETTINGS
    | TOKENLESS_REPAYMENTS_ALLOWED
    | TOKENLESS_REPAYMENTS_COMPLETE
    | CIRCUIT_BREAKER_ENABLED
    | BANK_SAME_ASSET_EMODE_ELIGIBLE;

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
///   `ASSET_TAG_STAKED` positions, but not both
pub const ASSET_TAG_SOL: u8 = 1;
/// Staked SOL assets. Accounts with a STAKED position can only deposit other STAKED assets or SOL
///   (`ASSET_TAG_SOL`) and can only borrow SOL (`ASSET_TAG_SOL`)
pub const ASSET_TAG_STAKED: u8 = 2;
/// Kamino assets. Accounts with a KAMINO position can only deposit other KAMINO assets or regular
///   assets (`ASSET_TAG_DEFAULT`).
pub const ASSET_TAG_KAMINO: u8 = 3;
/// Drift assets. Accounts with a DRIFT position can only deposit other DRIFT assets or regular
///   assets (`ASSET_TAG_DEFAULT`).
pub const ASSET_TAG_DRIFT: u8 = 4;
/// Solend assets. Accounts with a SOLEND position can only deposit other SOLEND assets or regular
///   assets (`ASSET_TAG_DEFAULT`).
pub const ASSET_TAG_SOLEND: u8 = 5;
/// JupLend assets. Accounts with a JUPLEND position can only deposit other JUPLEND assets or regular
///   assets (`ASSET_TAG_DEFAULT`).
pub const ASSET_TAG_JUPLEND: u8 = 6;

/// Drift uses a fixed 9 decimal precision for all spot market scaled balances,
///   regardless of the underlying token's decimals
pub const DRIFT_SCALED_BALANCE_DECIMALS: u8 = 9;

/// Maximum number of integration positions (Kamino + Drift + Solend + JupLend) allowed per account. Hardcoded
///   limit to prevent accounts from becoming unliquidatable due to CU/heap memory issues in
///   liquidation. These integrations require 3 accounts per position for health checks (bank + oracle
///   + reserve/spot-market), so they share the same limit.
///
/// Note: it's disabled in local integration tests so that we can measure the performance and
///   eventually get rid of this limit altogether.
pub const MAX_INTEGRATION_POSITIONS: usize = 8;
// WARN: You can set anything here, including a discrim that's technically "wrong" for the struct
//   with that name, and prod will use that hash anyways. Don't change these hashes once a struct is
//   live in prod.
pub mod discriminators {
    pub const GROUP: [u8; 8] = [182, 23, 173, 240, 151, 206, 182, 67];
    pub const BANK: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
    pub const ACCOUNT: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
    pub const FEE_STATE: [u8; 8] = [63, 224, 16, 85, 193, 36, 235, 220];
    pub const STAKED_SETTINGS: [u8; 8] = [157, 140, 6, 77, 89, 173, 173, 125];
    pub const LIQUIDATION_RECORD: [u8; 8] = [95, 116, 23, 132, 89, 210, 245, 162];
    pub const ORDER: [u8; 8] = [134, 173, 223, 185, 77, 86, 28, 51];
    pub const EXECUTE_ORDER_RECORD: [u8; 8] = [6, 100, 107, 60, 164, 226, 56, 97];
    pub const BANK_METADATA: [u8; 8] = [49, 207, 31, 34, 67, 225, 169, 186];
    pub const SAME_ASSET_EMODE_REGISTRY: [u8; 8] = [222, 21, 195, 149, 193, 72, 219, 31];
}

pub mod ix_discriminators {
    pub const INIT_LIQUIDATION_RECORD: [u8; 8] = [236, 213, 238, 126, 147, 251, 164, 8];
    pub const START_LIQUIDATION: [u8; 8] = [244, 93, 90, 214, 192, 166, 191, 21];
    pub const END_LIQUIDATION: [u8; 8] = [110, 11, 244, 54, 229, 181, 22, 184];
    pub const START_EXECUTE_ORDER: [u8; 8] = [1, 70, 140, 134, 183, 29, 208, 224];
    pub const END_EXECUTE_ORDER: [u8; 8] = [115, 42, 20, 93, 121, 84, 178, 83];
    pub const LENDING_ACCOUNT_WITHDRAW: [u8; 8] = [36, 72, 74, 19, 210, 210, 192, 192];
    pub const LENDING_ACCOUNT_REPAY: [u8; 8] = [79, 209, 172, 177, 222, 51, 173, 151];
    pub const KAMINO_WITHDRAW: [u8; 8] = [199, 101, 41, 45, 213, 98, 224, 200];
    pub const DRIFT_WITHDRAW: [u8; 8] = [86, 59, 186, 123, 183, 181, 234, 137];
    pub const JUPLEND_WITHDRAW: [u8; 8] = [245, 164, 253, 202, 53, 77, 251, 221];
    pub const START_FLASHLOAN: [u8; 8] = [14, 131, 33, 220, 81, 186, 180, 107];
    pub const END_FLASHLOAN: [u8; 8] = [105, 124, 201, 106, 153, 2, 8, 156];
    pub const START_DELEVERAGE: [u8; 8] = [10, 138, 10, 57, 40, 232, 182, 193];
    pub const END_DELEVERAGE: [u8; 8] = [114, 14, 250, 143, 252, 104, 214, 209];
    pub const LENDING_ACCOUNT_DEPOSIT: [u8; 8] = [171, 94, 235, 103, 82, 64, 212, 140];
    pub const LENDING_ACCOUNT_BORROW: [u8; 8] = [4, 126, 116, 53, 48, 5, 212, 31];
    pub const LENDING_ACCOUNT_CLOSE_BALANCE: [u8; 8] = [245, 54, 41, 4, 243, 202, 31, 17];
    pub const LENDING_ACCOUNT_LIQUIDATE: [u8; 8] = [214, 169, 151, 213, 251, 167, 86, 219];
    pub const LENDING_ACCOUNT_PULSE_HEALTH: [u8; 8] = [186, 52, 117, 97, 34, 74, 39, 253];
    pub const MARGINFI_ACCOUNT_INITIALIZE: [u8; 8] = [43, 78, 61, 255, 148, 52, 249, 154];
    pub const MARGINFI_ACCOUNT_INITIALIZE_PDA: [u8; 8] = [87, 177, 91, 80, 218, 119, 245, 31];
    pub const MARGINFI_ACCOUNT_CLOSE: [u8; 8] = [186, 221, 93, 34, 50, 97, 194, 241];
    pub const MARGINFI_ACCOUNT_SET_FREEZE: [u8; 8] = [199, 179, 231, 30, 138, 247, 110, 227];
    pub const MARGINFI_ACCOUNT_UPDATE_EMISSIONS_DESTINATION_ACCOUNT: [u8; 8] =
        [73, 185, 162, 201, 111, 24, 116, 185];
    pub const TRANSFER_TO_NEW_ACCOUNT: [u8; 8] = [28, 79, 129, 231, 169, 69, 69, 65];
    pub const TRANSFER_TO_NEW_ACCOUNT_PDA: [u8; 8] = [172, 210, 224, 220, 146, 212, 253, 49];
    pub const MARGINFI_ACCOUNT_CLOSE_LIQ_RECORD: [u8; 8] = [187, 222, 41, 134, 102, 10, 112, 147];
    pub const MARGINFI_ACCOUNT_PLACE_ORDER: [u8; 8] = [244, 112, 75, 138, 143, 108, 7, 186];
    pub const MARGINFI_ACCOUNT_CLOSE_ORDER: [u8; 8] = [212, 223, 79, 182, 172, 183, 205, 237];
    pub const MARGINFI_ACCOUNT_KEEPER_CLOSE_ORDER: [u8; 8] = [128, 114, 71, 46, 194, 71, 186, 106];
    pub const MARGINFI_ACCOUNT_SET_KEEPER_CLOSE_FLAGS: [u8; 8] =
        [82, 163, 165, 222, 212, 255, 33, 210];
    pub const KAMINO_INIT_OBLIGATION: [u8; 8] = [253, 177, 160, 225, 70, 156, 217, 109];
    pub const KAMINO_DEPOSIT: [u8; 8] = [237, 8, 188, 187, 115, 99, 49, 85];
    pub const KAMINO_HARVEST_REWARD: [u8; 8] = [163, 202, 248, 141, 106, 20, 116, 5];
    pub const DRIFT_INIT_USER: [u8; 8] = [29, 18, 236, 190, 29, 254, 114, 169];
    pub const DRIFT_DEPOSIT: [u8; 8] = [252, 63, 250, 201, 98, 55, 130, 12];
    pub const DRIFT_HARVEST_REWARD: [u8; 8] = [167, 161, 240, 194, 138, 54, 87, 189];
    pub const DRIFT_CLAIM_BAD_DEBT: [u8; 8] = [163, 67, 144, 231, 119, 20, 220, 33];
    pub const JUPLEND_INIT_POSITION: [u8; 8] = [176, 255, 151, 106, 5, 207, 74, 215];
    pub const JUPLEND_DEPOSIT: [u8; 8] = [114, 11, 218, 81, 183, 165, 143, 255];
    pub const SOLEND_INIT_OBLIGATION: [u8; 8] = [81, 96, 123, 149, 218, 116, 235, 196];
    pub const SOLEND_DEPOSIT: [u8; 8] = [56, 127, 176, 148, 12, 25, 3, 24];
    pub const SOLEND_WITHDRAW: [u8; 8] = [238, 144, 170, 199, 21, 72, 155, 36];
    pub const MARGINFI_GROUP_INITIALIZE: [u8; 8] = [255, 67, 67, 26, 94, 31, 34, 20];
    pub const LENDING_POOL_ACCRUE_BANK_INTEREST: [u8; 8] = [108, 201, 30, 87, 47, 65, 97, 188];
    pub const LENDING_POOL_PULSE_BANK_PRICE_CACHE: [u8; 8] = [192, 19, 201, 135, 105, 203, 32, 222];
    pub const LENDING_POOL_COLLECT_BANK_FEES: [u8; 8] = [201, 5, 215, 116, 230, 92, 75, 150];
    pub const LENDING_POOL_WITHDRAW_FEES: [u8; 8] = [92, 140, 215, 254, 170, 0, 83, 174];
    pub const LENDING_POOL_WITHDRAW_FEES_PERMISSIONLESS: [u8; 8] =
        [57, 245, 1, 208, 130, 18, 145, 113];
    pub const LENDING_POOL_WITHDRAW_INSURANCE: [u8; 8] = [108, 60, 60, 246, 104, 79, 159, 243];
    pub const LENDING_POOL_UPDATE_FEES_DESTINATION_ACCOUNT: [u8; 8] =
        [102, 4, 121, 243, 237, 110, 95, 13];
    pub const LENDING_POOL_EMISSIONS_DEPOSIT: [u8; 8] = [121, 118, 123, 58, 59, 192, 74, 138];
    pub const LENDING_POOL_CONFIGURE_BANK: [u8; 8] = [121, 173, 156, 40, 93, 148, 56, 237];
    pub const LENDING_POOL_CONFIGURE_BANK_ORACLE: [u8; 8] = [209, 82, 255, 171, 124, 21, 71, 81];
    pub const LENDING_POOL_CLEAR_CIRCUIT_BREAKER: [u8; 8] = [64, 73, 106, 46, 213, 86, 31, 48];
    pub const LENDING_POOL_HANDLE_BANKRUPTCY: [u8; 8] = [162, 11, 56, 139, 90, 128, 70, 173];
    pub const SYNC_INDEXER_FLAGS: [u8; 8] = [171, 146, 145, 43, 190, 175, 9, 32];
    pub const ADMIN_CLOSE_ACCOUNT: [u8; 8] = [131, 60, 75, 215, 109, 34, 157, 26];
    pub const CONFIGURE_DELEVERAGE_WITHDRAWAL_LIMIT: [u8; 8] = [28, 132, 205, 158, 67, 77, 177, 63];
    pub const DISABLE_STAKED_ORACLES: [u8; 8] = [43, 90, 152, 55, 66, 101, 232, 200];
    pub const EDIT_GLOBAL_FEE_STATE: [u8; 8] = [52, 62, 35, 129, 93, 69, 165, 202];
    pub const ENABLE_STAKED_ORACLE_ONRAMP: [u8; 8] = [114, 248, 244, 6, 74, 212, 222, 230];
    pub const INIT_BANK_METADATA: [u8; 8] = [94, 239, 50, 136, 137, 204, 254, 213];
    pub const INIT_GLOBAL_FEE_STATE: [u8; 8] = [82, 48, 247, 59, 220, 109, 231, 44];
    pub const INIT_STAKED_SETTINGS: [u8; 8] = [52, 35, 149, 44, 69, 86, 69, 80];
    pub const LENDING_POOL_ADD_BANK: [u8; 8] = [215, 68, 72, 78, 208, 218, 103, 182];
    pub const LENDING_POOL_ADD_BANK_DRIFT: [u8; 8] = [62, 63, 49, 48, 76, 55, 108, 155];
    pub const LENDING_POOL_ADD_BANK_JUPLEND: [u8; 8] = [18, 208, 117, 90, 53, 111, 195, 41];
    pub const LENDING_POOL_ADD_BANK_KAMINO: [u8; 8] = [118, 53, 16, 243, 255, 245, 149, 241];
    pub const LENDING_POOL_ADD_BANK_WITH_SEED: [u8; 8] = [76, 211, 213, 171, 117, 78, 158, 76];
    pub const LENDING_POOL_BACKFILL_BANK_IS_T22_FLAG: [u8; 8] =
        [189, 14, 205, 160, 172, 46, 157, 52];
    pub const LENDING_POOL_CLONE_EMODE: [u8; 8] = [146, 167, 94, 106, 184, 202, 15, 10];
    pub const LENDING_POOL_CONFIGURE_BANK_EMODE: [u8; 8] = [17, 175, 91, 57, 239, 86, 49, 71];
    pub const LENDING_POOL_CONFIGURE_BANK_INTEREST_ONLY: [u8; 8] =
        [245, 107, 83, 38, 103, 219, 163, 241];
    pub const LENDING_POOL_CONFIGURE_BANK_LIMITS_ONLY: [u8; 8] =
        [157, 196, 221, 200, 202, 62, 84, 21];
    pub const LENDING_POOL_INIT_SAME_ASSET_EMODE_REGISTRY: [u8; 8] =
        [217, 78, 227, 223, 147, 231, 213, 108];
    pub const LENDING_POOL_RESIZE_GROUP_ACCOUNT: [u8; 8] = [97, 221, 69, 96, 204, 162, 174, 250];
    pub const LENDING_POOL_SET_BANK_SAME_ASSET_EMODE_ELIGIBILITY: [u8; 8] =
        [149, 50, 162, 236, 150, 119, 9, 47];
    pub const LENDING_POOL_SET_FIXED_ORACLE_PRICE: [u8; 8] = [28, 126, 127, 127, 60, 37, 211, 125];
    pub const MARGINFI_GROUP_CONFIGURE: [u8; 8] = [62, 199, 81, 78, 33, 13, 236, 61];
    pub const PANIC_PAUSE: [u8; 8] = [76, 164, 123, 25, 4, 43, 79, 165];
    pub const PANIC_UNPAUSE: [u8; 8] = [236, 107, 194, 242, 99, 51, 121, 128];
    pub const PANIC_UNPAUSE_PERMISSIONLESS: [u8; 8] = [245, 139, 50, 159, 213, 62, 91, 248];
    pub const PROPAGATE_FEE_STATE: [u8; 8] = [64, 3, 166, 194, 129, 21, 101, 155];
    pub const PROPAGATE_STAKED_SETTINGS: [u8; 8] = [210, 30, 152, 69, 130, 99, 222, 170];
    pub const RESIZE_GLOBAL_FEE_STATE: [u8; 8] = [141, 111, 97, 79, 111, 143, 77, 159];
    pub const SUPER_ADMIN_DEPOSIT: [u8; 8] = [241, 189, 199, 17, 207, 225, 64, 75];
    pub const SUPER_ADMIN_WITHDRAW: [u8; 8] = [202, 67, 85, 126, 104, 138, 79, 197];
    pub const UPDATE_DELEVERAGE_WITHDRAWALS: [u8; 8] = [56, 3, 181, 118, 27, 247, 207, 227];
    pub const EDIT_STAKED_SETTINGS: [u8; 8] = [11, 108, 215, 87, 240, 9, 66, 241];
    pub const CONFIGURE_BANK_RATE_LIMITS: [u8; 8] = [175, 84, 85, 221, 206, 220, 110, 174];
    pub const CONFIGURE_GROUP_RATE_LIMITS: [u8; 8] = [111, 47, 213, 142, 158, 51, 226, 102];
    pub const UPDATE_GROUP_RATE_LIMITER: [u8; 8] = [23, 78, 60, 139, 187, 44, 129, 37];
    pub const WRITE_BANK_METADATA: [u8; 8] = [147, 78, 81, 133, 129, 138, 233, 59];
    pub const WRITE_BANK_METADATA_PRE_INIT: [u8; 8] = [224, 124, 22, 73, 60, 209, 80, 170];
    pub const LENDING_POOL_ADD_BANK_SOLEND: [u8; 8] = [81, 233, 203, 199, 47, 226, 0, 68];
}
