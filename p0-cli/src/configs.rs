use anyhow::{Context, Result};
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::path::Path;

/// JSON config for `bank add --config <path>`.
#[derive(Debug, Deserialize)]
pub struct KaminoInitObligationConfig {
    pub bank_pk: String,
    pub amount: u64,
    pub reserve_oracle: Option<String>,
}

/// JSON config for `kamino deposit --config <path>`.
#[derive(Debug, Deserialize)]
pub struct KaminoDepositConfig {
    pub bank_pk: String,
    pub ui_amount: f64,
}

/// JSON config for `kamino withdraw --config <path>`.
#[derive(Debug, Deserialize)]
pub struct KaminoWithdrawConfig {
    pub bank_pk: String,
    pub ui_amount: f64,
    #[serde(default)]
    pub withdraw_all: bool,
}

/// JSON config for `kamino harvest-reward --config <path>`.
#[derive(Debug, Deserialize)]
pub struct KaminoHarvestRewardConfig {
    pub bank_pk: String,
    pub reward_index: u64,
    pub global_config: String,
    pub reward_mint: String,
    pub scope_prices: Option<String>,
}

/// JSON config for `drift init-user --config <path>`.
#[derive(Debug, Deserialize)]
pub struct DriftInitUserConfig {
    pub bank_pk: String,
    pub amount: u64,
}

/// JSON config for `drift deposit --config <path>`.
#[derive(Debug, Deserialize)]
pub struct DriftDepositConfig {
    pub bank_pk: String,
    pub ui_amount: f64,
}

/// JSON config for `drift withdraw --config <path>`.
#[derive(Debug, Deserialize)]
pub struct DriftWithdrawConfig {
    pub bank_pk: String,
    pub ui_amount: f64,
    #[serde(default)]
    pub withdraw_all: bool,
    pub drift_reward_spot_market: Option<String>,
    pub drift_reward_spot_market_2: Option<String>,
}

/// JSON config for `drift harvest-reward --config <path>`.
#[derive(Debug, Deserialize)]
pub struct DriftHarvestRewardConfig {
    pub bank_pk: String,
    pub harvest_drift_spot_market: String,
}

pub fn parse_optional_pubkey(s: &Option<String>) -> Result<Option<Pubkey>> {
    match s {
        Some(v) => Ok(Some(parse_pubkey(v)?)),
        None => Ok(None),
    }
}

/// Load and parse a JSON config file.
pub fn load_config<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))
}

/// Parse a pubkey string from config.
pub fn parse_pubkey(s: &str) -> Result<Pubkey> {
    s.parse::<Pubkey>()
        .with_context(|| format!("Invalid pubkey: {s}"))
}

// ── Example JSON generators ──

impl KaminoInitObligationConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank_pk": "<BANK_PUBKEY>",
  "amount": 10,
  "reserve_oracle": null
}"#
    }
}

impl KaminoDepositConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank_pk": "<BANK_PUBKEY>",
  "ui_amount": 100.0
}"#
    }
}

impl KaminoWithdrawConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank_pk": "<BANK_PUBKEY>",
  "ui_amount": 50.0,
  "withdraw_all": false
}"#
    }
}

impl KaminoHarvestRewardConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank_pk": "<BANK_PUBKEY>",
  "reward_index": 0,
  "global_config": "<KAMINO_GLOBAL_CONFIG_PUBKEY>",
  "reward_mint": "<KAMINO_REWARD_MINT_PUBKEY>",
  "scope_prices": null
}"#
    }
}

impl DriftInitUserConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank_pk": "<BANK_PUBKEY>",
  "amount": 10
}"#
    }
}

impl DriftDepositConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank_pk": "<BANK_PUBKEY>",
  "ui_amount": 100.0
}"#
    }
}

impl DriftWithdrawConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank_pk": "<BANK_PUBKEY>",
  "ui_amount": 50.0,
  "withdraw_all": false,
  "drift_reward_spot_market": null,
  "drift_reward_spot_market_2": null
}"#
    }
}

impl DriftHarvestRewardConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank_pk": "<BANK_PUBKEY>",
  "harvest_drift_spot_market": "<DRIFT_REWARD_SPOT_MARKET_PUBKEY>"
}"#
    }
}
