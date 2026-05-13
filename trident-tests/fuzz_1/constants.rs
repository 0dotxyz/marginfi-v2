use trident_fuzz::fuzzing::*;

pub const FEE_STATE_SEED: &str = "feestate";
/// PDA seed for on-chain `LiquidationRecord` (matches `marginfi_type_crate`).
pub const LIQUIDATION_RECORD_SEED: &str = "liq_record";
pub const LIQUIDITY_VAULT_AUTHORITY_SEED: &str = "liquidity_vault_auth";
pub const LIQUIDITY_VAULT_SEED: &str = "liquidity_vault";
pub const INSURANCE_VAULT_AUTHORITY_SEED: &str = "insurance_vault_auth";
pub const INSURANCE_VAULT_SEED: &str = "insurance_vault";
pub const FEE_VAULT_AUTHORITY_SEED: &str = "fee_vault_auth";
pub const FEE_VAULT_SEED: &str = "fee_vault";

pub const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

// Placeholder oracle pubkeys per bank/mint.
// Replace these with real oracle accounts when you switch oracle setups away from `Fixed`.
pub const USDC_PYTH_PUSH: Pubkey = pubkey!("Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX");
pub const WETH_PYTH_PUSH: Pubkey = pubkey!("42amVS4KgzR9rA28tkVYqVXjq9Qa8dcZQMbH5EYFX6XC");
pub const BTC_PYTH_PUSH: Pubkey = pubkey!("4cSM2e6rvbGQUFiJbqytoVMi5GgghSMr8LwVrT9VPSPo");
// Bank asset tags (copied from `marginfi_type_crate::constants`).
// These drive the required number of risk/oracle accounts per bank in `remaining_accounts`.
pub const KAMINO_LENDING_MARKET: Pubkey = pubkey!("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF");
pub const KAMINO_LENDING_MARKET_AUTHORITY: Pubkey =
    pubkey!("24LjDBukaUSHgPowcF2wY1XscnhChcBUDETN2UhBZMMT");
pub const KAMINO_USDC_RESERVE: Pubkey = pubkey!("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59");
pub const USDC_SCOPE_PRICES: Pubkey = pubkey!("3t4JZcueEzTbVP6kLxXrL3VpWx45jDer4eqysweBchNH");
pub const USDC_RESERVE_LIQUIDITY_SUPPLY: Pubkey =
    pubkey!("Bgq7trRgVMeq33yt235zM2onQ4bRDBsY5EWiTetF4qw6");
pub const USDC_RESERVE_COLLATERAL_MINT: Pubkey =
    pubkey!("B8V6WVjPxW1UGwVDfxH2d2r8SyT4cqn7dQRK6XneVa7D");
pub const USDC_RESERVE_COLLATERAL_SUPPLY_VAULT: Pubkey =
    pubkey!("3DzjXRfxRm6iejfyyMynR4tScddaanrePJ1NJU2XnPPL");

pub const KLEND_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");

pub const PYTH_PULL_MIGRATED_CONFIG_FLAGS: u8 = 1;
pub const FARMS_PROGRAM_KEY: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");

pub const FARMS_OBLIGATION_FARM_USER_STATE_KEY: Pubkey =
    pubkey!("GqwDJdk7FtyHZ7GjaoUSFseXxwpYvSqThBGDLMjdmnnh");
pub const FARMS_RESERVE_FARM_STATE_KEY: Pubkey =
    pubkey!("JAvnB9AKtgPsTEoKmn24Bq64UMoYcrtWtq42HHBdsPkh");

pub const KAMINO_BANK_SEED: u64 = 9_001;
pub const KAMINO_PYTH_ORACLE: Pubkey = USDC_PYTH_PUSH;
pub const KLEND_LENDING_MARKET_AUTH: &[u8] = b"lma";
pub const KFARMS_BASE_SEED_USER_STATE: &[u8; 4] = b"user";
