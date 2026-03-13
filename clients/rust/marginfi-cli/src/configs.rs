use anyhow::{Context, Result};
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::path::Path;

/// JSON config for `group add-bank --config <path>`.
#[derive(Debug, Deserialize)]
pub struct AddBankConfig {
    pub mint: String,
    #[serde(default)]
    pub seed: bool,
    pub asset_weight_init: f64,
    pub asset_weight_maint: f64,
    pub liability_weight_init: f64,
    pub liability_weight_maint: f64,
    pub deposit_limit_ui: u64,
    pub borrow_limit_ui: u64,
    pub zero_util_rate: u32,
    pub hundred_util_rate: u32,
    #[serde(default)]
    pub points: Vec<RatePointConfig>,
    pub insurance_fee_fixed_apr: f64,
    pub insurance_ir_fee: f64,
    pub group_fixed_fee_apr: f64,
    pub group_ir_fee: f64,
    pub risk_tier: String,
    #[serde(default = "default_oracle_max_age")]
    pub oracle_max_age: u16,
    pub global_fee_wallet: String,
}

/// Rate curve kink point in JSON config.
#[derive(Debug, Deserialize)]
pub struct RatePointConfig {
    pub util: u32,
    pub rate: u32,
}

/// JSON config for `bank update --config <path>`.
#[derive(Debug, Deserialize)]
pub struct ConfigureBankConfig {
    pub bank: String,
    pub asset_weight_init: Option<f32>,
    pub asset_weight_maint: Option<f32>,
    pub liability_weight_init: Option<f32>,
    pub liability_weight_maint: Option<f32>,
    pub deposit_limit_ui: Option<f64>,
    pub borrow_limit_ui: Option<f64>,
    pub operational_state: Option<String>,
    pub insurance_fee_fixed_apr: Option<f64>,
    pub insurance_ir_fee: Option<f64>,
    pub protocol_fixed_fee_apr: Option<f64>,
    pub protocol_ir_fee: Option<f64>,
    pub protocol_origination_fee: Option<f64>,
    pub zero_util_rate: Option<u32>,
    pub hundred_util_rate: Option<u32>,
    #[serde(default)]
    pub points: Vec<RatePointConfig>,
    pub risk_tier: Option<String>,
    pub asset_tag: Option<u8>,
    pub total_asset_value_init_limit: Option<u64>,
    pub oracle_max_confidence: Option<u32>,
    pub oracle_max_age: Option<u16>,
    pub permissionless_bad_debt_settlement: Option<bool>,
    pub freeze_settings: Option<bool>,
    pub tokenless_repayments_allowed: Option<bool>,
}

/// JSON config for `group init-fee-state --config <path>` and `group edit-fee-state --config <path>`.
#[derive(Debug, Deserialize)]
pub struct FeeStateConfig {
    pub admin: String,
    pub fee_wallet: String,
    pub bank_init_flat_sol_fee: u32,
    pub liquidation_flat_sol_fee: u32,
    pub program_fee_fixed: f64,
    pub program_fee_rate: f64,
    pub liquidation_max_fee: f64,
    #[serde(default)]
    pub order_init_flat_sol_fee: u32,
    #[serde(default)]
    pub order_execution_max_fee: f64,
}

/// JSON config for `group update --config <path>`.
#[derive(Debug, Deserialize)]
pub struct GroupUpdateConfig {
    pub new_admin: String,
    pub new_emode_admin: String,
    pub new_curve_admin: String,
    pub new_limit_admin: String,
    pub new_emissions_admin: String,
    pub new_metadata_admin: String,
    pub new_risk_admin: String,
    pub emode_max_init_leverage: Option<f64>,
    pub emode_max_maint_leverage: Option<f64>,
}

fn default_oracle_max_age() -> u16 {
    60
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

impl AddBankConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "mint": "<PUBKEY>",
  "seed": true,
  "asset_weight_init": 0.8,
  "asset_weight_maint": 0.9,
  "liability_weight_init": 1.2,
  "liability_weight_maint": 1.1,
  "deposit_limit_ui": 1000000,
  "borrow_limit_ui": 500000,
  "zero_util_rate": 0,
  "hundred_util_rate": 429496729,
  "points": [
    { "util": 2147483647, "rate": 214748364 },
    { "util": 3865470566, "rate": 858993459 }
  ],
  "insurance_fee_fixed_apr": 0.0,
  "insurance_ir_fee": 0.0,
  "group_fixed_fee_apr": 0.0,
  "group_ir_fee": 0.0,
  "risk_tier": "collateral",
  "oracle_max_age": 60,
  "global_fee_wallet": "<PUBKEY>"
}"#
    }
}

impl ConfigureBankConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank": "<PUBKEY>",
  "asset_weight_init": 0.8,
  "asset_weight_maint": 0.9,
  "liability_weight_init": 1.2,
  "liability_weight_maint": 1.1,
  "deposit_limit_ui": 1000000.0,
  "borrow_limit_ui": 500000.0,
  "operational_state": "operational",
  "insurance_fee_fixed_apr": 0.01,
  "insurance_ir_fee": 0.05,
  "protocol_fixed_fee_apr": 0.0,
  "protocol_ir_fee": 0.0,
  "protocol_origination_fee": 0.0,
  "zero_util_rate": 0,
  "hundred_util_rate": 429496729,
  "points": [],
  "risk_tier": "collateral",
  "asset_tag": null,
  "total_asset_value_init_limit": null,
  "oracle_max_confidence": null,
  "oracle_max_age": 60,
  "permissionless_bad_debt_settlement": null,
  "freeze_settings": null,
  "tokenless_repayments_allowed": null
}"#
    }
}

impl FeeStateConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "admin": "<PUBKEY>",
  "fee_wallet": "<PUBKEY>",
  "bank_init_flat_sol_fee": 0,
  "liquidation_flat_sol_fee": 0,
  "program_fee_fixed": 0.0,
  "program_fee_rate": 0.0,
  "liquidation_max_fee": 0.05,
  "order_init_flat_sol_fee": 0,
  "order_execution_max_fee": 0.0
}"#
    }
}

impl GroupUpdateConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "new_admin": "<PUBKEY>",
  "new_emode_admin": "<PUBKEY>",
  "new_curve_admin": "<PUBKEY>",
  "new_limit_admin": "<PUBKEY>",
  "new_emissions_admin": "<PUBKEY>",
  "new_metadata_admin": "<PUBKEY>",
  "new_risk_admin": "<PUBKEY>",
  "emode_max_init_leverage": null,
  "emode_max_maint_leverage": null
}"#
    }
}
