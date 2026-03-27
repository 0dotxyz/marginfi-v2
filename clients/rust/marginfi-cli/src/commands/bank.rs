use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use fixed::types::I80F48;
use solana_sdk::pubkey::Pubkey;

use marginfi_type_crate::types::{
    make_points, BankOperationalState, InterestRateConfigOpt, RatePoint, CURVE_POINTS,
};

use super::group::{RatePointArg, RiskTierArg};
use crate::config::GlobalOptions;
use crate::configs;
use crate::processor;

#[derive(Clone, Copy, Debug, Parser, ValueEnum)]
pub enum BankOperationalStateArg {
    Paused,
    Operational,
    ReduceOnly,
}

impl From<BankOperationalStateArg> for BankOperationalState {
    fn from(val: BankOperationalStateArg) -> Self {
        match val {
            BankOperationalStateArg::Paused => BankOperationalState::Paused,
            BankOperationalStateArg::Operational => BankOperationalState::Operational,
            BankOperationalStateArg::ReduceOnly => BankOperationalState::ReduceOnly,
        }
    }
}

#[allow(clippy::large_enum_variant)]
/// Bank management commands.
#[derive(Debug, Parser)]
#[clap(
    after_help = "Common subcommands:\n  mfi bank add --config ./configs/bank/add/config.json.example\n  mfi bank add-staked --config ./configs/bank/add-staked/config.json.example\n  mfi bank get <BANK_PUBKEY>\n  mfi bank get-all\n  mfi bank update <BANK_PUBKEY> --config ./configs/bank/update/config.json.example\n  mfi bank sync-metadata --group <GROUP_PUBKEY>",
    after_long_help = "Common subcommands:\n  mfi bank add --config ./configs/bank/add/config.json.example\n  mfi bank add-staked --config ./configs/bank/add-staked/config.json.example\n  mfi bank get <BANK_PUBKEY>\n  mfi bank get-all\n  mfi bank update <BANK_PUBKEY> --config ./configs/bank/update/config.json.example\n  mfi bank sync-metadata --group <GROUP_PUBKEY>"
)]
pub enum BankCommand {
    /// Add a new bank to a marginfi group
    ///
    /// Example: `mfi bank add --config ./configs/bank/add/config.json.example`
    #[clap(
        visible_alias = "create",
        after_help = "Example:\n  mfi bank add --config ./configs/bank/add/config.json.example",
        after_long_help = "Example:\n  mfi bank add --config ./configs/bank/add/config.json.example"
    )]
    Add {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long, help = "Marginfi group pubkey (defaults to profile group)")]
        group: Option<Pubkey>,
        #[clap(long)]
        mint: Option<Pubkey>,
        #[clap(long)]
        oracle: Option<Pubkey>,
        #[clap(
            long,
            help = "Oracle type: 3=PythPushOracle, 4=SwitchboardPull, 5=StakedWithPythPush"
        )]
        oracle_type: Option<u8>,
        #[clap(long)]
        asset_weight_init: Option<f64>,
        #[clap(long)]
        asset_weight_maint: Option<f64>,
        #[clap(long)]
        liability_weight_init: Option<f64>,
        #[clap(long)]
        liability_weight_maint: Option<f64>,
        #[clap(long)]
        deposit_limit_ui: Option<u64>,
        #[clap(long)]
        borrow_limit_ui: Option<u64>,
        #[clap(long)]
        zero_util_rate: Option<u32>,
        #[clap(long)]
        hundred_util_rate: Option<u32>,
        #[clap(long)]
        points: Vec<RatePointArg>,
        #[clap(long)]
        insurance_fee_fixed_apr: Option<f64>,
        #[clap(long)]
        insurance_ir_fee: Option<f64>,
        #[clap(long)]
        protocol_fixed_fee_apr: Option<f64>,
        #[clap(long)]
        protocol_ir_fee: Option<f64>,
        #[clap(long)]
        protocol_origination_fee: Option<f64>,
        #[clap(long, value_enum)]
        risk_tier: Option<RiskTierArg>,
        #[clap(long, default_value = "70")]
        oracle_max_age: u16,
        #[clap(long)]
        oracle_max_confidence: Option<u32>,
        #[clap(long, help = "0=Default, 1=SOL, 2=Staked")]
        asset_tag: Option<u8>,
        #[clap(
            long,
            help = "Override the live fee-state wallet (normally auto-derived)"
        )]
        global_fee_wallet: Option<Pubkey>,
        #[clap(
            long,
            help = "Bank seed number (auto-finds next free if not specified)"
        )]
        seed: Option<u64>,
    },
    /// Add a staked collateral bank to a marginfi group
    ///
    /// Example: `mfi bank add-staked --config ./configs/bank/add-staked/config.json.example`
    #[clap(
        visible_alias = "create-staked",
        after_help = "Example:\n  mfi bank add-staked --config ./configs/bank/add-staked/config.json.example",
        after_long_help = "Example:\n  mfi bank add-staked --config ./configs/bank/add-staked/config.json.example"
    )]
    AddStaked {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long, help = "Marginfi group pubkey (defaults to profile group)")]
        group: Option<Pubkey>,
        #[clap(long, help = "SPL single-pool stake pool address")]
        stake_pool: Option<Pubkey>,
        #[clap(
            long,
            help = "Bank seed number (auto-finds next free if not specified)"
        )]
        seed: Option<u64>,
    },
    /// Clone a mainnet bank into staging/localnet using a deterministic seed
    ///
    /// Example: `mfi bank clone --source-bank <BANK_PUBKEY> --mint <MINT_PUBKEY> --bank-seed 42`
    #[clap(
        after_help = "Example:\n  mfi bank clone --source-bank <BANK_PUBKEY> --mint <MINT_PUBKEY> --bank-seed 42",
        after_long_help = "Example:\n  mfi bank clone --source-bank <BANK_PUBKEY> --mint <MINT_PUBKEY> --bank-seed 42"
    )]
    Clone {
        #[clap(long, help = "Marginfi group pubkey (defaults to profile group)")]
        group: Option<Pubkey>,
        #[clap(long)]
        source_bank: Pubkey,
        #[clap(long)]
        mint: Pubkey,
        #[clap(long)]
        bank_seed: u64,
    },
    /// Display details for a specific bank (or the profile default)
    ///
    /// Example: `mfi bank get <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank get <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank get <BANK_PUBKEY>"
    )]
    Get { bank: Option<String> },
    /// List all banks in a group
    ///
    /// Example: `mfi bank get-all`
    #[clap(
        after_help = "Example:\n  mfi bank get-all",
        after_long_help = "Example:\n  mfi bank get-all"
    )]
    GetAll { marginfi_group: Option<Pubkey> },
    /// Update bank configuration parameters
    ///
    /// Example: `mfi bank update <BANK_PUBKEY> --config ./configs/bank/update/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi bank update <BANK_PUBKEY> --config ./configs/bank/update/config.json.example",
        after_long_help = "Example:\n  mfi bank update <BANK_PUBKEY> --config ./configs/bank/update/config.json.example"
    )]
    Update {
        bank_pk: Option<String>,
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long)]
        asset_weight_init: Option<f32>,
        #[clap(long)]
        asset_weight_maint: Option<f32>,

        #[clap(long)]
        liability_weight_init: Option<f32>,
        #[clap(long)]
        liability_weight_maint: Option<f32>,

        #[clap(long)]
        deposit_limit_ui: Option<f64>,

        #[clap(long)]
        borrow_limit_ui: Option<f64>,

        #[clap(long, value_enum)]
        operational_state: Option<BankOperationalStateArg>,

        #[clap(long, help = "Insurance fee fixed APR")]
        if_fa: Option<f64>,
        #[clap(long, help = "Insurance IR fee")]
        if_ir: Option<f64>,
        #[clap(long, help = "Protocol fixed fee APR")]
        pf_fa: Option<f64>,
        #[clap(long, help = "Protocol IR fee")]
        pf_ir: Option<f64>,
        #[clap(long, help = "Protocol origination fee")]
        pf_or: Option<f64>,

        #[clap(
            long,
            help = "Base rate at utilization=0; a % as u32 out of 1000% (100% = 0.1 * u32::MAX)"
        )]
        zero_util_rate: Option<u32>,

        #[clap(
            long,
            help = "Base rate at utilization=100; a % as u32 out of 1000% (100% = 0.1 * u32::MAX)"
        )]
        hundred_util_rate: Option<u32>,

        #[clap(
            long = "point",
            value_parser = RatePointArg::from_str,
            help = "Kink point as 'util,rate'. util: u32 out of 100%; rate: u32 out of 1000%. Repeat up to 5 times in ascending util order."
        )]
        points: Vec<RatePointArg>,

        #[clap(long, value_enum, help = "Bank risk tier")]
        risk_tier: Option<RiskTierArg>,
        #[clap(long, help = "0 = default, 1 = SOL, 2 = Staked SOL LST")]
        asset_tag: Option<u8>,
        #[clap(long, help = "Soft USD init limit")]
        usd_init_limit: Option<u64>,
        #[clap(
            long,
            help = "Oracle max confidence, a % as u32, e.g. 50% = u32::MAX/2"
        )]
        oracle_max_confidence: Option<u32>,
        #[clap(long, help = "Oracle max age in seconds, 0 to use default value (60s)")]
        oracle_max_age: Option<u16>,
        #[clap(
            long,
            help = "Permissionless bad debt settlement, if true the group admin is not required to settle bad debt"
        )]
        permissionless_bad_debt_settlement: Option<bool>,
        #[clap(
            long,
            help = "If enabled, will prevent this Update ix from ever running against after this invocation"
        )]
        freeze_settings: Option<bool>,
        #[clap(
            long,
            help = "If enabled, allows risk admin to \"repay\" debts in this bank with nothing"
        )]
        tokenless_repayments_allowed: Option<bool>,
    },
    /// Update only the interest rate config
    ///
    /// Example: `mfi bank configure-interest-only <BANK_PUBKEY> --zero-util-rate 0 --hundred-util-rate 1000000000`
    #[clap(
        after_help = "Example:\n  mfi bank configure-interest-only <BANK_PUBKEY> --zero-util-rate 0 --hundred-util-rate 1000000000",
        after_long_help = "Example:\n  mfi bank configure-interest-only <BANK_PUBKEY> --zero-util-rate 0 --hundred-util-rate 1000000000"
    )]
    ConfigureInterestOnly {
        bank_pk: String,
        #[clap(long, help = "Insurance fee fixed APR")]
        if_fa: Option<f64>,
        #[clap(long, help = "Insurance IR fee")]
        if_ir: Option<f64>,
        #[clap(long, help = "Protocol fixed fee APR")]
        pf_fa: Option<f64>,
        #[clap(long, help = "Protocol IR fee")]
        pf_ir: Option<f64>,
        #[clap(long, help = "Protocol origination fee")]
        pf_or: Option<f64>,
        #[clap(long)]
        zero_util_rate: Option<u32>,
        #[clap(long)]
        hundred_util_rate: Option<u32>,
        #[clap(long = "point", value_parser = RatePointArg::from_str)]
        points: Vec<RatePointArg>,
    },
    /// Update only deposit/borrow/init limits
    ///
    /// Example: `mfi bank configure-limits-only <BANK_PUBKEY> --deposit-limit-ui 1000000 --borrow-limit-ui 500000`
    #[clap(
        after_help = "Example:\n  mfi bank configure-limits-only <BANK_PUBKEY> --deposit-limit-ui 1000000 --borrow-limit-ui 500000",
        after_long_help = "Example:\n  mfi bank configure-limits-only <BANK_PUBKEY> --deposit-limit-ui 1000000 --borrow-limit-ui 500000"
    )]
    ConfigureLimitsOnly {
        bank_pk: String,
        #[clap(long)]
        deposit_limit_ui: Option<f64>,
        #[clap(long)]
        borrow_limit_ui: Option<f64>,
        #[clap(long, help = "Soft USD init limit")]
        usd_init_limit: Option<u64>,
    },
    /// Change oracle type and key for a bank
    ///
    /// Example: `mfi bank update-oracle <BANK_PUBKEY> --oracle-type 3 --oracle-key <ORACLE_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank update-oracle <BANK_PUBKEY> --oracle-type 3 --oracle-key <ORACLE_PUBKEY>",
        after_long_help = "Example:\n  mfi bank update-oracle <BANK_PUBKEY> --oracle-type 3 --oracle-key <ORACLE_PUBKEY>"
    )]
    UpdateOracle {
        bank_pk: String,
        #[clap(
            long,
            help = "Bank oracle type (3 = Pyth Push, 4 = Switchboard Pull, 5 = Staked Pyth Push)"
        )]
        oracle_type: u8,
        #[clap(long, help = "Bank oracle account (or feed if using Pyth Push)")]
        oracle_key: Pubkey,
    },
    /// Mark tokenless repayment workflow complete for a deleveraging bank
    ///
    /// Example: `mfi bank force-tokenless-repay-complete <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank force-tokenless-repay-complete <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank force-tokenless-repay-complete <BANK_PUBKEY>"
    )]
    ForceTokenlessRepayComplete { bank_pk: String },
    /// Show current oracle price and metadata for a bank
    ///
    /// Example: `mfi bank inspect-price-oracle <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank inspect-price-oracle <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank inspect-price-oracle <BANK_PUBKEY>"
    )]
    InspectPriceOracle { bank_pk: String },
    /// Collect accrued protocol fees from a bank
    ///
    /// Example: `mfi bank collect-fees <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank collect-fees <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank collect-fees <BANK_PUBKEY>"
    )]
    CollectFees { bank: String },
    /// Withdraw collected fees from a bank's fee vault
    ///
    /// Example: `mfi bank withdraw-fees <BANK_PUBKEY> 10`
    #[clap(
        after_help = "Example:\n  mfi bank withdraw-fees <BANK_PUBKEY> 10",
        after_long_help = "Example:\n  mfi bank withdraw-fees <BANK_PUBKEY> 10"
    )]
    WithdrawFees {
        bank: String,
        amount: f64,
        #[clap(help = "Destination address, defaults to the profile authority")]
        destination_address: Option<Pubkey>,
    },
    /// Withdraw funds from a bank's insurance vault
    ///
    /// Example: `mfi bank withdraw-insurance <BANK_PUBKEY> 10`
    #[clap(
        after_help = "Example:\n  mfi bank withdraw-insurance <BANK_PUBKEY> 10",
        after_long_help = "Example:\n  mfi bank withdraw-insurance <BANK_PUBKEY> 10"
    )]
    WithdrawInsurance {
        bank: String,
        amount: f64,
        #[clap(help = "Destination address, defaults to the profile authority")]
        destination_address: Option<Pubkey>,
    },
    /// Close a bank (must be empty)
    ///
    /// Example: `mfi bank close <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank close <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank close <BANK_PUBKEY>"
    )]
    Close { bank_pk: String },
    /// Manually trigger interest accrual on a bank
    ///
    /// Example: `mfi bank accrue-interest <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank accrue-interest <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank accrue-interest <BANK_PUBKEY>"
    )]
    AccrueInterest { bank_pk: String },
    /// Override oracle with a fixed price
    ///
    /// Example: `mfi bank set-fixed-price <BANK_PUBKEY> --price 1.0`
    #[clap(
        after_help = "Example:\n  mfi bank set-fixed-price <BANK_PUBKEY> --price 1.0",
        after_long_help = "Example:\n  mfi bank set-fixed-price <BANK_PUBKEY> --price 1.0"
    )]
    SetFixedPrice {
        bank_pk: String,
        #[clap(long)]
        price: f64,
    },
    /// Set the e-mode tag for a bank
    ///
    /// Example: `mfi bank configure-emode <BANK_PUBKEY> --emode-tag 1`
    #[clap(
        after_help = "Example:\n  mfi bank configure-emode <BANK_PUBKEY> --emode-tag 1",
        after_long_help = "Example:\n  mfi bank configure-emode <BANK_PUBKEY> --emode-tag 1"
    )]
    ConfigureEmode {
        bank_pk: String,
        #[clap(long)]
        emode_tag: u16,
    },
    /// Copy e-mode settings from one bank to another in the same group
    ///
    /// Example: `mfi bank clone-emode --copy-from-bank <SOURCE_BANK_PUBKEY> --copy-to-bank <TARGET_BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank clone-emode --copy-from-bank <SOURCE_BANK_PUBKEY> --copy-to-bank <TARGET_BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank clone-emode --copy-from-bank <SOURCE_BANK_PUBKEY> --copy-to-bank <TARGET_BANK_PUBKEY>"
    )]
    CloneEmode {
        #[clap(long)]
        copy_from_bank: String,
        #[clap(long)]
        copy_to_bank: String,
    },
    /// Migrate legacy curve encoding to seven-point format
    ///
    /// Example: `mfi bank migrate-curve <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank migrate-curve <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank migrate-curve <BANK_PUBKEY>"
    )]
    MigrateCurve { bank_pk: String },
    /// Refresh the cached oracle price for a bank
    ///
    /// Example: `mfi bank pulse-price-cache <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank pulse-price-cache <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank pulse-price-cache <BANK_PUBKEY>"
    )]
    PulsePriceCache { bank_pk: String },
    /// Set hourly/daily outflow rate limits for a bank
    ///
    /// Example: `mfi bank configure-rate-limits <BANK_PUBKEY> --hourly-max-outflow 100000 --daily-max-outflow 500000`
    #[clap(
        after_help = "Example:\n  mfi bank configure-rate-limits <BANK_PUBKEY> --hourly-max-outflow 100000 --daily-max-outflow 500000",
        after_long_help = "Example:\n  mfi bank configure-rate-limits <BANK_PUBKEY> --hourly-max-outflow 100000 --daily-max-outflow 500000"
    )]
    ConfigureRateLimits {
        bank_pk: String,
        #[clap(long)]
        hourly_max_outflow: Option<u64>,
        #[clap(long)]
        daily_max_outflow: Option<u64>,
    },
    /// Withdraw fees without admin
    ///
    /// Example: `mfi bank withdraw-fees-permissionless <BANK_PUBKEY> --amount 1000`
    #[clap(
        after_help = "Example:\n  mfi bank withdraw-fees-permissionless <BANK_PUBKEY> --amount 1000",
        after_long_help = "Example:\n  mfi bank withdraw-fees-permissionless <BANK_PUBKEY> --amount 1000"
    )]
    WithdrawFeesPermissionless {
        bank_pk: String,
        #[clap(long)]
        amount: u64,
    },
    /// Change the fee destination address for a bank
    ///
    /// Example: `mfi bank update-fees-destination <BANK_PUBKEY> --destination <DESTINATION_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank update-fees-destination <BANK_PUBKEY> --destination <DESTINATION_PUBKEY>",
        after_long_help = "Example:\n  mfi bank update-fees-destination <BANK_PUBKEY> --destination <DESTINATION_PUBKEY>"
    )]
    UpdateFeesDestination {
        bank_pk: String,
        #[clap(long)]
        destination: Pubkey,
    },
    /// Initialize on-chain metadata account for a bank
    ///
    /// Example: `mfi bank init-metadata <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank init-metadata <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi bank init-metadata <BANK_PUBKEY>"
    )]
    InitMetadata {
        bank_pk: String,
        #[clap(
            long,
            help = "Required for pre-init banks if the bank account does not exist yet"
        )]
        group: Option<Pubkey>,
        #[clap(
            long,
            help = "Required for pre-init banks if the bank account does not exist yet"
        )]
        mint: Option<Pubkey>,
        #[clap(long, help = "Canonical bank seed", required = true)]
        seed: u64,
    },
    /// Initialize if needed, then write ticker and description to one or more bank metadata accounts
    ///
    /// Example: `mfi bank write-metadata <BANK_PUBKEY> --symbol USDC --name "USD Coin" --wait-for-bank`
    #[clap(
        after_help = "Examples:\n  mfi bank write-metadata <BANK_PUBKEY> --symbol USDC --name \"USD Coin\" --wait-for-bank\n  mfi bank write-metadata <BANK_PUBKEY> --ticker \"USDC | USD Coin\" --description \"USD Coin | stablecoins | USDC | P0 | -\"\n  mfi bank write-metadata --config ./configs/bank/write-metadata/config.json.example --wait-for-bank",
        after_long_help = "Examples:\n  mfi bank write-metadata <BANK_PUBKEY> --symbol USDC --name \"USD Coin\" --wait-for-bank\n  mfi bank write-metadata <BANK_PUBKEY> --ticker \"USDC | USD Coin\" --description \"USD Coin | stablecoins | USDC | P0 | -\"\n  mfi bank write-metadata --config ./configs/bank/write-metadata/config.json.example --wait-for-bank"
    )]
    WriteMetadata {
        bank_pk: Option<String>,
        #[clap(
            long,
            help = "Path to JSON config file (API row shape; single object or array)"
        )]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long)]
        group: Option<Pubkey>,
        #[clap(long)]
        ticker: Option<String>,
        #[clap(long)]
        description: Option<String>,
        #[clap(long, help = "Canonical bank seed")]
        seed: Option<u64>,
        #[clap(long)]
        mint: Option<Pubkey>,
        #[clap(long)]
        symbol: Option<String>,
        #[clap(long)]
        name: Option<String>,
        #[clap(long = "asset-group")]
        asset_group: Option<String>,
        #[clap(long)]
        venue: Option<String>,
        #[clap(long = "venue-identifier")]
        venue_identifier: Option<String>,
        #[clap(long = "risk-tier-name")]
        risk_tier_name: Option<String>,
        #[clap(
            long,
            help = "Wait for each bank account to appear on-chain before initializing and writing metadata",
            action
        )]
        wait_for_bank: bool,
        #[clap(
            long,
            default_value_t = 300,
            help = "Maximum time to wait for a missing bank account in seconds when --wait-for-bank is set"
        )]
        wait_for_bank_timeout_secs: u64,
    },
    /// Sync bank metadata from a metadata source and write it on-chain
    ///
    /// Example: `mfi bank sync-metadata --group <GROUP_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi bank sync-metadata --group <GROUP_PUBKEY>",
        after_long_help = "Example:\n  mfi bank sync-metadata --group <GROUP_PUBKEY>"
    )]
    SyncMetadata {
        #[clap(long, help = "Target group (defaults to profile group)")]
        group: Option<Pubkey>,
        #[clap(
            long,
            help = "Metadata source URL",
            default_value = "https://app.0.xyz/api/banks/db"
        )]
        url: String,
        #[clap(long, help = "Optional max banks to process after filtering")]
        limit: Option<usize>,
        #[clap(
            long,
            default_value_t = 1000,
            help = "Delay between banks in milliseconds"
        )]
        delay_ms: u64,
    },
    /// Dump bank metadata PDAs and decoded on-chain metadata to a local JSON file
    ///
    /// Example: `mfi bank dump-metadata`
    #[clap(
        after_help = "Example:\n  mfi bank dump-metadata",
        after_long_help = "Example:\n  mfi bank dump-metadata"
    )]
    DumpMetadata {
        #[clap(long, help = "Optional group to filter source banks by")]
        group: Option<Pubkey>,
        #[clap(
            long,
            help = "Metadata source URL",
            default_value = "https://app.0.xyz/api/banks/db"
        )]
        url: String,
        #[clap(
            long,
            help = "Output JSON path",
            default_value = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/mainnet_metadata_dump.json"
            )
        )]
        out: PathBuf,
        #[clap(long, help = "Optional max banks to dump after filtering")]
        limit: Option<usize>,
    },
}

pub fn parse_risk_tier_config(value: &str) -> Result<RiskTierArg> {
    match value.to_lowercase().as_str() {
        "collateral" => Ok(RiskTierArg::Collateral),
        "isolated" => Ok(RiskTierArg::Isolated),
        other => anyhow::bail!("Unknown risk_tier in config: {other}"),
    }
}

fn parse_operational_state_config(value: &str) -> Result<BankOperationalStateArg> {
    match value.to_lowercase().as_str() {
        "paused" => Ok(BankOperationalStateArg::Paused),
        "operational" => Ok(BankOperationalStateArg::Operational),
        "reduce_only" | "reduceonly" => Ok(BankOperationalStateArg::ReduceOnly),
        other => anyhow::bail!("Unknown operational_state in config: {other}"),
    }
}

fn build_add_bank_request(
    profile: &crate::profile::Profile,
    config_path: Option<PathBuf>,
    group_pk: Option<Pubkey>,
    mint: Option<Pubkey>,
    oracle: Option<Pubkey>,
    oracle_type: Option<u8>,
    asset_weight_init: Option<f64>,
    asset_weight_maint: Option<f64>,
    liability_weight_init: Option<f64>,
    liability_weight_maint: Option<f64>,
    deposit_limit_ui: Option<u64>,
    borrow_limit_ui: Option<u64>,
    zero_util_rate: Option<u32>,
    hundred_util_rate: Option<u32>,
    points: Vec<RatePointArg>,
    insurance_fee_fixed_apr: Option<f64>,
    insurance_ir_fee: Option<f64>,
    protocol_fixed_fee_apr: Option<f64>,
    protocol_ir_fee: Option<f64>,
    protocol_origination_fee: Option<f64>,
    risk_tier: Option<RiskTierArg>,
    oracle_max_age: u16,
    oracle_max_confidence: Option<u32>,
    asset_tag: Option<u8>,
    global_fee_wallet: Option<Pubkey>,
    seed: Option<u64>,
) -> Result<processor::StandardBankCreateRequest> {
    if let Some(path) = config_path {
        let cfg: configs::AddBankConfig = configs::load_config(&path)?;
        let points = cfg
            .points
            .iter()
            .map(|p| RatePointArg {
                util: p.util,
                rate: p.rate,
            })
            .collect();

        return Ok(processor::StandardBankCreateRequest {
            group: cfg
                .group
                .as_deref()
                .map(configs::parse_pubkey)
                .transpose()?
                .or(profile.marginfi_group)
                .context("group required: set in config, --group, or profile")?,
            bank_mint: configs::parse_pubkey(&cfg.mint)?,
            seed: cfg.seed,
            asset_weight_init: cfg.asset_weight_init,
            asset_weight_maint: cfg.asset_weight_maint,
            liability_weight_init: cfg.liability_weight_init,
            liability_weight_maint: cfg.liability_weight_maint,
            deposit_limit_ui: cfg.deposit_limit_ui,
            borrow_limit_ui: cfg.borrow_limit_ui,
            zero_util_rate: cfg.zero_util_rate,
            hundred_util_rate: cfg.hundred_util_rate,
            points,
            insurance_fee_fixed_apr: cfg.insurance_fee_fixed_apr,
            insurance_ir_fee: cfg.insurance_ir_fee,
            protocol_fixed_fee_apr: cfg.protocol_fixed_fee_apr,
            protocol_ir_fee: cfg.protocol_ir_fee,
            protocol_origination_fee: cfg.protocol_origination_fee.unwrap_or(0.0),
            risk_tier: parse_risk_tier_config(&cfg.risk_tier)?,
            oracle_max_age: cfg.oracle_max_age,
            oracle_max_confidence: cfg.oracle_max_confidence.unwrap_or(0),
            asset_tag: cfg.asset_tag.unwrap_or(0),
            global_fee_wallet: configs::parse_optional_pubkey(&cfg.global_fee_wallet)?,
            oracle: configs::parse_pubkey(&cfg.oracle)?,
            oracle_type: cfg.oracle_type.context("oracle_type required in config")?,
        });
    }

    Ok(processor::StandardBankCreateRequest {
        group: group_pk
            .or(profile.marginfi_group)
            .context("--group required or set in profile")?,
        bank_mint: mint.context("--mint required (or use --config)")?,
        seed,
        asset_weight_init: asset_weight_init.context("--asset-weight-init required")?,
        asset_weight_maint: asset_weight_maint.context("--asset-weight-maint required")?,
        liability_weight_init: liability_weight_init.context("--liability-weight-init required")?,
        liability_weight_maint: liability_weight_maint
            .context("--liability-weight-maint required")?,
        deposit_limit_ui: deposit_limit_ui.context("--deposit-limit-ui required")?,
        borrow_limit_ui: borrow_limit_ui.context("--borrow-limit-ui required")?,
        zero_util_rate: zero_util_rate.context("--zero-util-rate required")?,
        hundred_util_rate: hundred_util_rate.context("--hundred-util-rate required")?,
        points,
        insurance_fee_fixed_apr: insurance_fee_fixed_apr
            .context("--insurance-fee-fixed-apr required")?,
        insurance_ir_fee: insurance_ir_fee.context("--insurance-ir-fee required")?,
        protocol_fixed_fee_apr: protocol_fixed_fee_apr
            .context("--protocol-fixed-fee-apr required")?,
        protocol_ir_fee: protocol_ir_fee.context("--protocol-ir-fee required")?,
        protocol_origination_fee: protocol_origination_fee.unwrap_or(0.0),
        risk_tier: risk_tier.context("--risk-tier required")?,
        oracle_max_age,
        oracle_max_confidence: oracle_max_confidence.unwrap_or(0),
        asset_tag: asset_tag.unwrap_or(0),
        global_fee_wallet,
        oracle: oracle.context("--oracle required")?,
        oracle_type: oracle_type.context("--oracle-type required")?,
    })
}

fn build_add_staked_bank_request(
    profile: &crate::profile::Profile,
    config_path: Option<PathBuf>,
    group_pk: Option<Pubkey>,
    stake_pool: Option<Pubkey>,
    seed: Option<u64>,
) -> Result<processor::StakedBankCreateRequest> {
    if let Some(path) = config_path {
        let cfg: configs::AddStakedBankConfig = configs::load_config(&path)?;
        return Ok(processor::StakedBankCreateRequest {
            group: cfg
                .group
                .as_deref()
                .map(configs::parse_pubkey)
                .transpose()?
                .or(profile.marginfi_group)
                .context("group required: set in config, --group, or profile")?,
            stake_pool: configs::parse_pubkey(&cfg.stake_pool)?,
            seed: cfg.seed,
        });
    }

    Ok(processor::StakedBankCreateRequest {
        group: group_pk
            .or(profile.marginfi_group)
            .context("--group required or set in profile")?,
        stake_pool: stake_pool.context("--stake-pool required (or use --config)")?,
        seed,
    })
}

fn build_bank_update_interest_request(
    insurance_fee_fixed_apr: Option<f64>,
    insurance_ir_fee: Option<f64>,
    protocol_fixed_fee_apr: Option<f64>,
    protocol_ir_fee: Option<f64>,
    protocol_origination_fee: Option<f64>,
    zero_util_rate: Option<u32>,
    hundred_util_rate: Option<u32>,
    points: Vec<RatePointArg>,
) -> Option<processor::BankUpdateInterestRateRequest> {
    if insurance_fee_fixed_apr.is_none()
        && insurance_ir_fee.is_none()
        && protocol_fixed_fee_apr.is_none()
        && protocol_ir_fee.is_none()
        && protocol_origination_fee.is_none()
        && zero_util_rate.is_none()
        && hundred_util_rate.is_none()
        && points.is_empty()
    {
        return None;
    }

    Some(processor::BankUpdateInterestRateRequest {
        insurance_fee_fixed_apr,
        insurance_ir_fee,
        protocol_fixed_fee_apr,
        protocol_ir_fee,
        protocol_origination_fee,
        zero_util_rate,
        hundred_util_rate,
        points: points.into_iter().map(Into::into).collect(),
    })
}

fn build_bank_metadata_entries(
    profile_group: Option<Pubkey>,
    bank_pk: Option<String>,
    config_path: Option<PathBuf>,
    group: Option<Pubkey>,
    ticker: Option<String>,
    description: Option<String>,
    bank_seed: Option<u64>,
    mint: Option<Pubkey>,
    symbol: Option<String>,
    name: Option<String>,
    asset_group: Option<String>,
    venue: Option<String>,
    venue_identifier: Option<String>,
    risk_tier_name: Option<String>,
) -> Result<Vec<processor::BankMetadataInput>> {
    if let Some(path) = config_path {
        let cfg: configs::WriteBankMetadataConfig = configs::load_config(&path)?;
        return cfg
            .into_entries()
            .into_iter()
            .map(|entry| {
                Ok(processor::BankMetadataInput {
                    bank: super::resolve_bank_for_group(&entry.bank_address, profile_group)?,
                    group: entry
                        .group
                        .as_deref()
                        .map(configs::parse_pubkey)
                        .transpose()?,
                    bank_seed: entry.bank_seed,
                    ticker: None,
                    description: None,
                    mint: entry
                        .token_address
                        .as_deref()
                        .map(configs::parse_pubkey)
                        .transpose()?,
                    symbol: entry.token_symbol,
                    name: entry.token_name,
                    asset_group: entry.asset_group,
                    venue: entry.venue,
                    venue_identifier: entry.venue_identifier,
                    risk_tier_name: entry.risk_tier_name,
                })
            })
            .collect();
    }

    Ok(vec![processor::BankMetadataInput {
        bank: super::resolve_bank_for_group(
            &bank_pk.context("bank_pk required unless --config is provided")?,
            profile_group,
        )?,
        group,
        bank_seed: bank_seed.context("--seed required unless --config is provided")?,
        ticker,
        description,
        mint,
        symbol,
        name,
        asset_group,
        venue,
        venue_identifier,
        risk_tier_name,
    }])
}

#[allow(clippy::too_many_arguments)]
fn build_bank_update_request(
    profile_group: Option<Pubkey>,
    bank_pk: Option<String>,
    config_path: Option<PathBuf>,
    asset_weight_init: Option<f32>,
    asset_weight_maint: Option<f32>,
    liability_weight_init: Option<f32>,
    liability_weight_maint: Option<f32>,
    deposit_limit_ui: Option<f64>,
    borrow_limit_ui: Option<f64>,
    operational_state: Option<BankOperationalStateArg>,
    if_fa: Option<f64>,
    if_ir: Option<f64>,
    pf_fa: Option<f64>,
    pf_ir: Option<f64>,
    pf_or: Option<f64>,
    zero_util_rate: Option<u32>,
    hundred_util_rate: Option<u32>,
    points: Vec<RatePointArg>,
    risk_tier: Option<RiskTierArg>,
    asset_tag: Option<u8>,
    usd_init_limit: Option<u64>,
    oracle_max_confidence: Option<u32>,
    oracle_max_age: Option<u16>,
    permissionless_bad_debt_settlement: Option<bool>,
    freeze_settings: Option<bool>,
    tokenless_repayments_allowed: Option<bool>,
) -> Result<processor::BankUpdateRequest> {
    if let Some(path) = config_path {
        let cfg: configs::ConfigureBankConfig = configs::load_config(&path)?;
        let bank_pk = super::resolve_bank_for_group(&cfg.bank, profile_group)?;
        return Ok(processor::BankUpdateRequest {
            bank_pk,
            asset_weight_init: cfg.asset_weight_init,
            asset_weight_maint: cfg.asset_weight_maint,
            liability_weight_init: cfg.liability_weight_init,
            liability_weight_maint: cfg.liability_weight_maint,
            deposit_limit_ui: cfg.deposit_limit_ui,
            borrow_limit_ui: cfg.borrow_limit_ui,
            operational_state: cfg
                .operational_state
                .as_deref()
                .map(parse_operational_state_config)
                .transpose()?
                .map(Into::into),
            interest_rate_config: build_bank_update_interest_request(
                cfg.insurance_fee_fixed_apr,
                cfg.insurance_ir_fee,
                cfg.protocol_fixed_fee_apr,
                cfg.protocol_ir_fee,
                cfg.protocol_origination_fee,
                cfg.zero_util_rate,
                cfg.hundred_util_rate,
                cfg.points
                    .into_iter()
                    .map(|point| RatePointArg {
                        util: point.util,
                        rate: point.rate,
                    })
                    .collect(),
            ),
            risk_tier: cfg
                .risk_tier
                .as_deref()
                .map(parse_risk_tier_config)
                .transpose()?
                .map(Into::into),
            asset_tag: cfg.asset_tag,
            total_asset_value_init_limit: cfg.total_asset_value_init_limit,
            oracle_max_confidence: cfg.oracle_max_confidence,
            oracle_max_age: cfg.oracle_max_age,
            permissionless_bad_debt_settlement: cfg.permissionless_bad_debt_settlement,
            freeze_settings: cfg.freeze_settings,
            tokenless_repayments_allowed: cfg.tokenless_repayments_allowed,
        });
    }

    Ok(processor::BankUpdateRequest {
        bank_pk: super::resolve_bank_for_group(
            &bank_pk.context("bank_pk required (or use --config)")?,
            profile_group,
        )?,
        asset_weight_init,
        asset_weight_maint,
        liability_weight_init,
        liability_weight_maint,
        deposit_limit_ui,
        borrow_limit_ui,
        operational_state: operational_state.map(Into::into),
        interest_rate_config: build_bank_update_interest_request(
            if_fa,
            if_ir,
            pf_fa,
            pf_ir,
            pf_or,
            zero_util_rate,
            hundred_util_rate,
            points,
        ),
        risk_tier: risk_tier.map(Into::into),
        asset_tag,
        total_asset_value_init_limit: usd_init_limit,
        oracle_max_confidence,
        oracle_max_age,
        permissionless_bad_debt_settlement,
        freeze_settings,
        tokenless_repayments_allowed,
    })
}

pub fn dispatch(subcmd: BankCommand, global_options: &GlobalOptions) -> Result<()> {
    match &subcmd {
        BankCommand::Add {
            config_example: true,
            ..
        } => {
            println!("{}", configs::AddBankConfig::example_json());
            return Ok(());
        }
        BankCommand::AddStaked {
            config_example: true,
            ..
        } => {
            println!("{}", configs::AddStakedBankConfig::example_json());
            return Ok(());
        }
        BankCommand::Update {
            config_example: true,
            ..
        } => {
            println!("{}", configs::ConfigureBankConfig::example_json());
            return Ok(());
        }
        BankCommand::WriteMetadata {
            config_example: true,
            ..
        } => {
            println!("{}", configs::WriteBankMetadataConfig::example_json());
            return Ok(());
        }
        _ => {}
    }

    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        match subcmd {
            BankCommand::Get { .. } | BankCommand::GetAll { .. } => (),

            BankCommand::InspectPriceOracle { .. } => (),
            BankCommand::DumpMetadata { .. } => (),
            #[allow(unreachable_patterns)]
            _ => super::get_consent(&subcmd, &profile)?,
        }
    }

    match subcmd {
        BankCommand::Add {
            config: config_path,
            config_example,
            group: group_pk,
            mint,
            oracle,
            oracle_type,
            asset_weight_init,
            asset_weight_maint,
            liability_weight_init,
            liability_weight_maint,
            deposit_limit_ui,
            borrow_limit_ui,
            zero_util_rate,
            hundred_util_rate,
            points,
            insurance_fee_fixed_apr,
            insurance_ir_fee,
            protocol_fixed_fee_apr,
            protocol_ir_fee,
            protocol_origination_fee,
            risk_tier,
            oracle_max_age,
            oracle_max_confidence,
            asset_tag,
            global_fee_wallet,
            seed,
        } => {
            if config_example {
                println!("{}", configs::AddBankConfig::example_json());
                return Ok(());
            }
            let request = build_add_bank_request(
                &profile,
                config_path,
                group_pk,
                mint,
                oracle,
                oracle_type,
                asset_weight_init,
                asset_weight_maint,
                liability_weight_init,
                liability_weight_maint,
                deposit_limit_ui,
                borrow_limit_ui,
                zero_util_rate,
                hundred_util_rate,
                points,
                insurance_fee_fixed_apr,
                insurance_ir_fee,
                protocol_fixed_fee_apr,
                protocol_ir_fee,
                protocol_origination_fee,
                risk_tier,
                oracle_max_age,
                oracle_max_confidence,
                asset_tag,
                global_fee_wallet,
                seed,
            )?;
            processor::create_standard_bank(config, request)
        }
        BankCommand::AddStaked {
            config: config_path,
            config_example,
            group: group_pk,
            stake_pool,
            seed,
        } => {
            if config_example {
                println!("{}", configs::AddStakedBankConfig::example_json());
                return Ok(());
            }
            let request =
                build_add_staked_bank_request(&profile, config_path, group_pk, stake_pool, seed)?;
            processor::create_staked_bank(config, request)
        }
        BankCommand::Clone {
            group: group_pk,
            source_bank,
            mint,
            bank_seed,
        } => {
            let group = group_pk
                .or(profile.marginfi_group)
                .context("--group required or set in profile")?;
            processor::group_clone_bank(config, group, source_bank, mint, bank_seed)
        }
        BankCommand::Get { bank } => {
            let bank_pk = bank
                .as_deref()
                .map(|value| super::resolve_bank_for_group(value, profile.marginfi_group))
                .transpose()?;
            processor::bank_get(config, bank_pk)
        }
        BankCommand::GetAll { marginfi_group } => processor::bank_get_all(config, marginfi_group),
        BankCommand::Update {
            asset_weight_init,
            asset_weight_maint,
            liability_weight_init,
            liability_weight_maint,
            deposit_limit_ui,
            borrow_limit_ui,
            operational_state,
            bank_pk,
            config: config_path,
            config_example,
            if_fa,
            if_ir,
            pf_fa,
            pf_ir,
            pf_or,
            zero_util_rate,
            hundred_util_rate,
            points,
            risk_tier,
            asset_tag,
            usd_init_limit,
            oracle_max_confidence,
            oracle_max_age,
            permissionless_bad_debt_settlement,
            freeze_settings,
            tokenless_repayments_allowed,
        } => {
            if config_example {
                println!("{}", configs::ConfigureBankConfig::example_json());
                return Ok(());
            }
            let request = build_bank_update_request(
                profile.marginfi_group,
                bank_pk,
                config_path,
                asset_weight_init,
                asset_weight_maint,
                liability_weight_init,
                liability_weight_maint,
                deposit_limit_ui,
                borrow_limit_ui,
                operational_state,
                if_fa,
                if_ir,
                pf_fa,
                pf_ir,
                pf_or,
                zero_util_rate,
                hundred_util_rate,
                points,
                risk_tier,
                asset_tag,
                usd_init_limit,
                oracle_max_confidence,
                oracle_max_age,
                permissionless_bad_debt_settlement,
                freeze_settings,
                tokenless_repayments_allowed,
            )?;
            processor::update_bank(config, request)
        }
        BankCommand::ConfigureInterestOnly {
            bank_pk,
            if_fa,
            if_ir,
            pf_fa,
            pf_ir,
            pf_or,
            zero_util_rate,
            hundred_util_rate,
            points,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            let points_opt: Option<[RatePoint; CURVE_POINTS]> = if points.is_empty() {
                None
            } else {
                let pts: Vec<RatePoint> = points.iter().map(|p| (*p).into()).collect();
                Some(make_points(&pts))
            };

            processor::bank_configure_interest_only(
                config,
                bank_pk,
                InterestRateConfigOpt {
                    insurance_fee_fixed_apr: if_fa.map(|x| I80F48::from_num(x).into()),
                    insurance_ir_fee: if_ir.map(|x| I80F48::from_num(x).into()),
                    protocol_fixed_fee_apr: pf_fa.map(|x| I80F48::from_num(x).into()),
                    protocol_ir_fee: pf_ir.map(|x| I80F48::from_num(x).into()),
                    protocol_origination_fee: pf_or.map(|x| I80F48::from_num(x).into()),
                    zero_util_rate,
                    hundred_util_rate,
                    points: points_opt,
                },
            )
        }
        BankCommand::ConfigureLimitsOnly {
            bank_pk,
            deposit_limit_ui,
            borrow_limit_ui,
            usd_init_limit,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_configure_limits_only(
                config,
                bank_pk,
                deposit_limit_ui,
                borrow_limit_ui,
                usd_init_limit,
            )
        }
        BankCommand::UpdateOracle {
            bank_pk,
            oracle_type,
            oracle_key,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_configure_oracle(config, bank_pk, oracle_type, oracle_key)
        }
        BankCommand::ForceTokenlessRepayComplete { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_force_tokenless_repay_complete(config, bank_pk)
        }
        BankCommand::InspectPriceOracle { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_inspect_price_oracle(config, bank_pk)
        }
        BankCommand::CollectFees { bank } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::admin::process_collect_fees(config, bank_pk)
        }
        BankCommand::WithdrawFees {
            bank,
            amount,
            destination_address,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::admin::process_withdraw_fees(config, bank_pk, amount, destination_address)
        }
        BankCommand::WithdrawInsurance {
            bank,
            amount,
            destination_address,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::admin::process_withdraw_insurance(
                config,
                bank_pk,
                amount,
                destination_address,
            )
        }
        BankCommand::Close { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_close(config, bank_pk)
        }
        BankCommand::AccrueInterest { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_accrue_interest(config, bank_pk)
        }
        BankCommand::SetFixedPrice { bank_pk, price } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_set_fixed_price(config, bank_pk, price)
        }
        BankCommand::ConfigureEmode { bank_pk, emode_tag } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_configure_emode(config, bank_pk, emode_tag)
        }
        BankCommand::CloneEmode {
            copy_from_bank,
            copy_to_bank,
        } => {
            let copy_from_bank =
                super::resolve_bank_for_group(&copy_from_bank, profile.marginfi_group)?;
            let copy_to_bank =
                super::resolve_bank_for_group(&copy_to_bank, profile.marginfi_group)?;
            processor::bank_clone_emode(config, copy_from_bank, copy_to_bank)
        }
        BankCommand::MigrateCurve { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_migrate_curve(config, bank_pk)
        }
        BankCommand::PulsePriceCache { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_pulse_price_cache(config, bank_pk)
        }
        BankCommand::ConfigureRateLimits {
            bank_pk,
            hourly_max_outflow,
            daily_max_outflow,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_configure_rate_limits(
                config,
                bank_pk,
                hourly_max_outflow,
                daily_max_outflow,
            )
        }
        BankCommand::WithdrawFeesPermissionless { bank_pk, amount } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_withdraw_fees_permissionless(config, bank_pk, amount)
        }
        BankCommand::UpdateFeesDestination {
            bank_pk,
            destination,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_update_fees_destination(config, bank_pk, destination)
        }
        BankCommand::InitMetadata {
            bank_pk,
            group,
            mint,
            seed,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_init_metadata(config, bank_pk, group, mint, seed)
        }
        BankCommand::WriteMetadata {
            bank_pk,
            config: config_path,
            config_example,
            group,
            ticker,
            description,
            seed,
            mint,
            symbol,
            name,
            asset_group,
            venue,
            venue_identifier,
            risk_tier_name,
            wait_for_bank,
            wait_for_bank_timeout_secs,
        } => {
            if config_example {
                println!("{}", configs::WriteBankMetadataConfig::example_json());
                return Ok(());
            }
            let inputs = build_bank_metadata_entries(
                profile.marginfi_group,
                bank_pk,
                config_path,
                group,
                ticker,
                description,
                seed,
                mint,
                symbol,
                name,
                asset_group,
                venue,
                venue_identifier,
                risk_tier_name,
            )?;
            let options = processor::BankMetadataWriteOptions {
                wait_for_bank,
                wait_for_bank_timeout: std::time::Duration::from_secs(wait_for_bank_timeout_secs),
            };
            let entries = processor::resolve_bank_metadata_inputs(&config, inputs, &options)?;
            processor::bank_write_metadata_entries(&config, entries, &options)
        }
        BankCommand::SyncMetadata {
            group,
            url,
            limit,
            delay_ms,
        } => {
            let group = group
                .or(profile.marginfi_group)
                .context("--group required or set in profile")?;
            processor::sync_bank_metadata_from_url(config, group, Some(url), limit, delay_ms)
        }
        BankCommand::DumpMetadata {
            group,
            url,
            out,
            limit,
        } => processor::dump_bank_metadata(config, group, Some(url), out, limit),
    }
}
