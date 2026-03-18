use anyhow::{Context, Result};
use serde::Deserialize;
use solana_sdk::pubkey::Pubkey;
use std::path::Path;

/// JSON config for `bank add --config <path>`.
#[derive(Debug, Deserialize)]
pub struct AddBankConfig {
    pub group: Option<String>,
    pub mint: String,
    #[serde(default)]
    pub seed: Option<u64>,
    pub oracle: String,
    pub oracle_type: Option<u8>,
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
    pub protocol_fixed_fee_apr: f64,
    pub protocol_ir_fee: f64,
    pub protocol_origination_fee: Option<f64>,
    pub risk_tier: String,
    #[serde(default = "default_oracle_max_age")]
    pub oracle_max_age: u16,
    pub oracle_max_confidence: Option<u32>,
    pub asset_tag: Option<u8>,
    pub global_fee_wallet: Option<String>,
}

/// JSON config for `bank add-staked --config <path>`.
#[derive(Debug, Deserialize)]
pub struct AddStakedBankConfig {
    pub group: Option<String>,
    pub stake_pool: String,
    #[serde(default)]
    pub seed: Option<u64>,
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

/// JSON config entry for `bank write-metadata --config <path>`.
#[derive(Debug, Clone, Deserialize)]
pub struct WriteBankMetadataConfigEntry {
    #[serde(alias = "bank")]
    pub bank_address: String,
    #[serde(alias = "mint", alias = "tokenAddress")]
    pub token_address: Option<String>,
    #[serde(alias = "symbol", alias = "tokenSymbol")]
    pub token_symbol: Option<String>,
    #[serde(alias = "name", alias = "tokenName")]
    pub token_name: Option<String>,
    #[serde(alias = "assetGroup")]
    pub asset_group: Option<String>,
    pub venue: Option<String>,
    #[serde(alias = "venueIdentifier")]
    pub venue_identifier: Option<String>,
    #[serde(alias = "riskTierName")]
    pub risk_tier_name: Option<String>,
}

/// JSON config for `bank write-metadata --config <path>`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum WriteBankMetadataConfig {
    Single(WriteBankMetadataConfigEntry),
    Multiple(Vec<WriteBankMetadataConfigEntry>),
}

/// JSON config for `group update --config <path>`.
#[derive(Debug, Deserialize)]
pub struct GroupUpdateConfig {
    pub new_admin: Option<String>,
    pub new_emode_admin: Option<String>,
    pub new_curve_admin: Option<String>,
    pub new_limit_admin: Option<String>,
    pub new_flow_admin: Option<String>,
    pub new_emissions_admin: Option<String>,
    pub new_metadata_admin: Option<String>,
    pub new_risk_admin: Option<String>,
    pub emode_max_init_leverage: Option<f64>,
    pub emode_max_maint_leverage: Option<f64>,
}

/// JSON config for `group create --config <path>`.
#[derive(Debug, Deserialize)]
pub struct GroupCreateConfig {
    pub admin: Option<String>,
    pub emode_admin: Option<String>,
    pub curve_admin: Option<String>,
    pub limit_admin: Option<String>,
    pub flow_admin: Option<String>,
    pub emissions_admin: Option<String>,
    pub metadata_admin: Option<String>,
    pub risk_admin: Option<String>,
    pub emode_max_init_leverage: Option<f64>,
    pub emode_max_maint_leverage: Option<f64>,
}

/// JSON config for `group init-staked-settings --config <path>`.
#[derive(Debug, Deserialize)]
pub struct StakedSettingsConfig {
    pub oracle: String,
    pub asset_weight_init: f64,
    pub asset_weight_maint: f64,
    pub deposit_limit: u64,
    pub total_asset_value_init_limit: u64,
    #[serde(default = "default_oracle_max_age")]
    pub oracle_max_age: u16,
    pub risk_tier: String,
}

/// JSON config for `group edit-staked-settings --config <path>`.
#[derive(Debug, Deserialize)]
pub struct EditStakedSettingsConfig {
    pub oracle: Option<String>,
    pub asset_weight_init: Option<f64>,
    pub asset_weight_maint: Option<f64>,
    pub deposit_limit: Option<u64>,
    pub total_asset_value_init_limit: Option<u64>,
    pub oracle_max_age: Option<u16>,
    pub risk_tier: Option<String>,
}

/// JSON config for `kamino init-obligation --config <path>`.
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

/// JSON config for `kamino add-bank --config <path>`.
#[derive(Debug, Deserialize)]
pub struct AddBankKaminoConfig {
    pub group: Option<String>,
    pub mint: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    pub oracle: Option<String>,
    pub reserve_oracle: Option<String>,
    /// "kaminoPythPush" or "kaminoSwitchboardPull"
    pub oracle_setup: String,
    pub kamino_reserve: String,
    pub kamino_market: Option<String>,
    pub asset_weight_init: Option<f64>,
    pub asset_weight_maint: Option<f64>,
    pub deposit_limit: Option<u64>,
    pub total_asset_value_init_limit: Option<u64>,
    #[serde(default = "default_oracle_max_age")]
    pub oracle_max_age: u16,
    pub oracle_max_confidence: Option<u32>,
    pub risk_tier: Option<String>,
    pub config_flags: Option<u8>,
    pub init_deposit_amount: Option<u64>,
}

/// JSON config for `drift add-bank --config <path>`.
#[derive(Debug, Deserialize)]
pub struct AddBankDriftConfig {
    pub group: Option<String>,
    pub mint: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    pub oracle: Option<String>,
    /// "driftPythPull" or "driftSwitchboardPull"
    pub oracle_setup: String,
    pub drift_market_index: u16,
    pub drift_oracle: Option<String>,
    pub asset_weight_init: Option<f64>,
    pub asset_weight_maint: Option<f64>,
    pub deposit_limit: Option<u64>,
    pub total_asset_value_init_limit: Option<u64>,
    #[serde(default = "default_oracle_max_age")]
    pub oracle_max_age: u16,
    pub oracle_max_confidence: Option<u32>,
    pub risk_tier: Option<String>,
    pub config_flags: Option<u8>,
    pub init_deposit_amount: Option<u64>,
}

/// JSON config for `juplend add-bank --config <path>`.
#[derive(Debug, Deserialize)]
pub struct AddBankJuplendConfig {
    pub group: Option<String>,
    pub mint: Option<String>,
    #[serde(default)]
    pub seed: Option<u64>,
    pub oracle: String,
    /// "juplendPythPull" or "juplendSwitchboardPull"
    pub oracle_setup: String,
    pub juplend_lending: Option<String>,
    pub asset_weight_init: Option<f64>,
    pub asset_weight_maint: Option<f64>,
    pub deposit_limit: Option<u64>,
    pub total_asset_value_init_limit: Option<u64>,
    #[serde(default = "default_oracle_max_age")]
    pub oracle_max_age: u16,
    pub oracle_max_confidence: Option<u32>,
    pub risk_tier: Option<String>,
    pub config_flags: Option<u8>,
    pub init_deposit_amount: Option<u64>,
}

/// Helper to parse an optional pubkey string from config.
pub fn parse_optional_pubkey(s: &Option<String>) -> Result<Option<Pubkey>> {
    match s {
        Some(v) => Ok(Some(parse_pubkey(v)?)),
        None => Ok(None),
    }
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
  "group": null,
  "mint": "So11111111111111111111111111111111111111112",
  "seed": 0,
  "oracle": "<SWITCHBOARD_PULL_FEED_PUBKEY>",
  "oracle_type": 4,
  "asset_weight_init": 0.8,
  "asset_weight_maint": 0.9,
  "liability_weight_init": 1.14,
  "liability_weight_maint": 1.1,
  "deposit_limit_ui": 2000000,
  "borrow_limit_ui": 500000,
  "zero_util_rate": 0,
  "hundred_util_rate": 858993459,
  "points": [
    { "util": 2147483647, "rate": 214748364 },
    { "util": 3865470566, "rate": 429496729 }
  ],
  "insurance_fee_fixed_apr": 0.0,
  "insurance_ir_fee": 0.0,
  "protocol_fixed_fee_apr": 0.0001,
  "protocol_ir_fee": 0.06,
  "protocol_origination_fee": 0.0,
  "risk_tier": "collateral",
  "oracle_max_age": 70,
  "oracle_max_confidence": 0,
  "asset_tag": 0,
  "global_fee_wallet": null
}"#
    }
}

impl AddStakedBankConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "group": null,
  "stake_pool": "<STAKE_POOL_PUBKEY>",
  "seed": 0
}"#
    }
}

impl ConfigureBankConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "bank": "<BANK_PUBKEY>",
  "asset_weight_init": 0.8,
  "asset_weight_maint": 0.9,
  "liability_weight_init": 1.14,
  "liability_weight_maint": 1.1,
  "deposit_limit_ui": 2000000.0,
  "borrow_limit_ui": 500000.0,
  "operational_state": "operational",
  "insurance_fee_fixed_apr": 0.0,
  "insurance_ir_fee": 0.0,
  "protocol_fixed_fee_apr": 0.0001,
  "protocol_ir_fee": 0.06,
  "protocol_origination_fee": 0.0,
  "zero_util_rate": 0,
  "hundred_util_rate": 858993459,
  "points": [
    { "util": 2147483647, "rate": 214748364 },
    { "util": 3865470566, "rate": 429496729 }
  ],
  "risk_tier": "collateral",
  "asset_tag": 0,
  "total_asset_value_init_limit": 50000000,
  "oracle_max_confidence": 0,
  "oracle_max_age": 70,
  "permissionless_bad_debt_settlement": false,
  "freeze_settings": false,
  "tokenless_repayments_allowed": false
}"#
    }
}

impl FeeStateConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "admin": "<ADMIN_PUBKEY>",
  "fee_wallet": "<FEE_WALLET_PUBKEY>",
  "bank_init_flat_sol_fee": 100000,
  "liquidation_flat_sol_fee": 50000,
  "program_fee_fixed": 0.0001,
  "program_fee_rate": 0.05,
  "liquidation_max_fee": 0.05,
  "order_init_flat_sol_fee": 50000,
  "order_execution_max_fee": 0.05
}"#
    }
}

impl WriteBankMetadataConfig {
    pub fn into_entries(self) -> Vec<WriteBankMetadataConfigEntry> {
        match self {
            Self::Single(entry) => vec![entry],
            Self::Multiple(entries) => entries,
        }
    }

    pub fn example_json() -> &'static str {
        r#"[
  {
    "bankAddress": "<BANK_PUBKEY>",
    "tokenAddress": null,
    "tokenSymbol": "USDC",
    "tokenName": "USD Coin",
    "assetGroup": null,
    "venue": null,
    "venueIdentifier": null,
    "riskTierName": null
  },
  {
    "bankAddress": "<BANK_PUBKEY>",
    "tokenAddress": null,
    "tokenSymbol": "SOL",
    "tokenName": "Wrapped SOL",
    "assetGroup": "blue-chip",
    "venue": null,
    "venueIdentifier": null,
    "riskTierName": null
  }
]"#
    }
}

impl GroupUpdateConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "new_admin": null,
  "new_emode_admin": null,
  "new_curve_admin": null,
  "new_limit_admin": null,
  "new_flow_admin": null,
  "new_emissions_admin": null,
  "new_metadata_admin": null,
  "new_risk_admin": null,
  "emode_max_init_leverage": 4.0,
  "emode_max_maint_leverage": 5.0
}"#
    }
}

impl GroupCreateConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "admin": null,
  "emode_admin": null,
  "curve_admin": null,
  "limit_admin": null,
  "flow_admin": null,
  "emissions_admin": null,
  "metadata_admin": null,
  "risk_admin": null,
  "emode_max_init_leverage": 4.0,
  "emode_max_maint_leverage": 5.0
}"#
    }
}

impl StakedSettingsConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "oracle": "<ORACLE_PUBKEY>",
  "asset_weight_init": 0.75,
  "asset_weight_maint": 0.85,
  "deposit_limit": 2000000,
  "total_asset_value_init_limit": 50000000,
  "oracle_max_age": 70,
  "risk_tier": "collateral"
}"#
    }
}

impl EditStakedSettingsConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "oracle": null,
  "asset_weight_init": 0.78,
  "asset_weight_maint": 0.88,
  "deposit_limit": 2500000,
  "total_asset_value_init_limit": 60000000,
  "oracle_max_age": 70,
  "risk_tier": "collateral"
}"#
    }
}

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

impl AddBankKaminoConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "group": null,
  "mint": null,
  "seed": 0,
  "oracle": "<MARGINFI_ORACLE_PUBKEY>",
  "reserve_oracle": null,
  "oracle_setup": "kaminoSwitchboardPull",
  "kamino_reserve": "<KAMINO_RESERVE_PUBKEY>",
  "kamino_market": null,
  "asset_weight_init": 0.85,
  "asset_weight_maint": 0.9,
  "deposit_limit": 2500000000000,
  "total_asset_value_init_limit": 2500000,
  "oracle_max_age": 70,
  "oracle_max_confidence": 0,
  "risk_tier": "collateral",
  "config_flags": 1,
  "init_deposit_amount": 100
}"#
    }
}

impl AddBankDriftConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "group": null,
  "mint": null,
  "seed": 0,
  "oracle": null,
  "oracle_setup": "driftPythPull",
  "drift_market_index": 0,
  "drift_oracle": null,
  "asset_weight_init": 0.55,
  "asset_weight_maint": 0.65,
  "deposit_limit": 10000000000,
  "total_asset_value_init_limit": 10000000000,
  "oracle_max_age": 70,
  "oracle_max_confidence": 0,
  "risk_tier": "collateral",
  "config_flags": 1,
  "init_deposit_amount": 100
}"#
    }
}

impl AddBankJuplendConfig {
    pub fn example_json() -> &'static str {
        r#"{
  "group": null,
  "mint": "<UNDERLYING_MINT_PUBKEY>",
  "seed": 0,
  "oracle": "<MARGINFI_ORACLE_PUBKEY>",
  "oracle_setup": "juplendPythPull",
  "asset_weight_init": 0.8,
  "asset_weight_maint": 0.9,
  "deposit_limit": 1000000000000,
  "total_asset_value_init_limit": 1000000000,
  "oracle_max_age": 70,
  "oracle_max_confidence": 0,
  "risk_tier": "collateral",
  "config_flags": 1,
  "init_deposit_amount": 100
}"#
    }
}
