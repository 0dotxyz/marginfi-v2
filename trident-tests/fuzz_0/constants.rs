use trident_fuzz::fuzzing::*;

// ================================================================================================
// Seeds
// ================================================================================================
pub const FEE_STATE_SEED: &str = "feestate";
pub const FEE_STATE_V2_SEED: &str = "feestate_v2";
pub const LIQUIDATION_RECORD_SEED: &str = "liq_record";
pub const LIQUIDITY_VAULT_AUTHORITY_SEED: &str = "liquidity_vault_auth";
pub const LIQUIDITY_VAULT_SEED: &str = "liquidity_vault";
pub const INSURANCE_VAULT_AUTHORITY_SEED: &str = "insurance_vault_auth";
pub const INSURANCE_VAULT_SEED: &str = "insurance_vault";
pub const FEE_VAULT_AUTHORITY_SEED: &str = "fee_vault_auth";
pub const FEE_VAULT_SEED: &str = "fee_vault";
pub const KLEND_LENDING_MARKET_AUTH: &[u8] = b"lma";
pub const KFARMS_BASE_SEED_USER_STATE: &[u8; 4] = b"user";
/// Bit-0 in the `flags: Option<u8>` arg of `kamino_withdraw` — selects the
/// "withdraw_all" behaviour (program ignores `amount` and pays the full
/// position). Mirrors the layout in `programs/marginfi/src/instructions/
/// kamino/withdraw.rs` (`WITHDRAW_ALL_FLAG = 1 << 0`).
pub const WITHDRAW_ALL_FLAG: u8 = 1 << 0;
// ================================================================================================
// Mints
// ================================================================================================
// USDC
pub const USDC: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const USDC_DECIMALS: u8 = 6;
pub const USDC_MINT_AUTHORITY: Pubkey = pubkey!("BJE5MMbqXjVwjAF7oxwPYXnTXDyspzZyt4vwenNw5ruG");
// WETH
pub const WETH: Pubkey = pubkey!("7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs");
pub const WETH_DECIMALS: u8 = 8;
pub const WETH_MINT_AUTHORITY: Pubkey = pubkey!("BCD75RNBHrJJpW4dXVagL5mPjzRLnVZq4YirJdjEYMV7");
// cbBTC
pub const WBTC: Pubkey = pubkey!("5XZw2LKTyrfvfiskJ78AMpackRjPcyCif1WhUsPDuVqQ");
pub const WBTC_DECIMALS: u8 = 8;
pub const WBTC_MINT_AUTHORITY: Pubkey = pubkey!("8qAJSTfLJH7MWDMDGTNEFCijHXHmd5gxu22erUnQ9zt8");
// ================================================================================================

// ================================================================================================
// Program IDs
// ================================================================================================
pub const KLEND: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const KFARMS: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");
pub const LIQUIDITY_PROGRAM: Pubkey = pubkey!("jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC");
// ================================================================================================
// KAMINO ACCOUNTS
// ================================================================================================
pub const KAMINO_BANK_SEED: u64 = 9_001;
pub const KAMINO_MAIN_LENDING_MARKET: Pubkey =
    pubkey!("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF");
pub const KAMINO_MAIN_MARKET_USDC_RESERVE: Pubkey =
    pubkey!("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59");
pub const USDC_RESERVE_LIQUIDITY_VAULT: Pubkey =
    pubkey!("Bgq7trRgVMeq33yt235zM2onQ4bRDBsY5EWiTetF4qw6");
pub const USDC_RESERVE_COLLATERAL_MINT: Pubkey =
    pubkey!("B8V6WVjPxW1UGwVDfxH2d2r8SyT4cqn7dQRK6XneVa7D");
pub const USDC_RESERVE_COLLATERAL_VAULT: Pubkey =
    pubkey!("3DzjXRfxRm6iejfyyMynR4tScddaanrePJ1NJU2XnPPL");
pub const SCOPE_PRICES: Pubkey = pubkey!("3t4JZcueEzTbVP6kLxXrL3VpWx45jDer4eqysweBchNH");

// ================================================================================================
// Oracles
// ================================================================================================
pub const USDC_PYTH_PUSH: Pubkey = pubkey!("Dpw1EAVrSB1ibxiDQyTAW6Zip3J4Btk2x4SgApQCeFbX");
pub const WETH_PYTH_PUSH: Pubkey = pubkey!("42amVS4KgzR9rA28tkVYqVXjq9Qa8dcZQMbH5EYFX6XC");
pub const BTC_PYTH_PUSH: Pubkey = pubkey!("4cSM2e6rvbGQUFiJbqytoVMi5GgghSMr8LwVrT9VPSPo");
// ================================================================================================

// ================================================================================================
// JUPITER ACCOUNTS
// ================================================================================================
pub const JUPITER_BANK_SEED: u64 = 9_002;
pub const JUPITER_USDC: Pubkey = pubkey!("9BEcn9aPEmhSPbPQeFGjidRiEKki46fVQDyPpSQXPA2D");
pub const JUPITER_USDC_LENDING_STATE: Pubkey =
    pubkey!("2vVYHYM8VYnvZqQWpTJSj8o8DBf1wM8pVs3bsTgYZiqJ");
pub const JUPITER_USDC_LENDING_STATE_ADMIN: Pubkey =
    pubkey!("5nmGjA4s7ATzpBQXC5RNceRpaJ7pYw2wKsNBWyuSAZV6");
pub const JUPITER_USDC_SUPPLY_TOKEN_RESERVES_LIQUIDITY: Pubkey =
    pubkey!("94vK29npVbyRHXH63rRcTiSr26SFhrQTzbpNJuhQEDu");
pub const JUPITER_USDC_LENDING_SUPPLY_POSITION_ON_LIQUIDITY: Pubkey =
    pubkey!("Hf9gtkM4dpVBahVSzEXSVCAPpKzBsBcns3s8As3z77oF");
pub const JUPITER_USDC_RATE_MODEL: Pubkey = pubkey!("5pjzT5dFTsXcwixoab1QDLvZQvpYJxJeBphkyfHGn688");
pub const JUPITER_USDC_VAULT: Pubkey = pubkey!("BmkUoKMFYBxNSzWXyUjyMJjMAaVz4d8ZnxwwmhDCUXFB");
pub const JUPITER_USDC_LIQUIDITY: Pubkey = pubkey!("7s1da8DduuBFqGra5bJBjpnvL5E9mGzCuMk1Qkh4or2Z");
pub const JUPITER_USDC_REWARDS_RATE_MODEL: Pubkey =
    pubkey!("5xSPBiD3TibamAnwHDhZABdB4z4F9dcj5PnbteroBTTd");
pub const JUPITER_CLAIM_ACCOUNT: Pubkey = pubkey!("HN1r4VfkDn53xQQfeGDYrNuDKFdemAhZsHYRwBrFhsW");
// ================================================================================================

pub const PYTH_PULL_MIGRATED_CONFIG_FLAGS: u8 = 1;

pub const FARMS_RESERVE_FARM_STATE_KEY: Pubkey =
    pubkey!("JAvnB9AKtgPsTEoKmn24Bq64UMoYcrtWtq42HHBdsPkh");
