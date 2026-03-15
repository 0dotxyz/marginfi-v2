use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use solana_sdk::pubkey::Pubkey;

use marginfi_type_crate::types::{RatePoint, RiskTier};

use crate::config::GlobalOptions;
use crate::configs;
use crate::processor;

/// Marginfi group management commands.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Parser)]
#[clap(
    after_help = "Common subcommands:\n  mfi group get <GROUP_PUBKEY>\n  mfi group get-all\n  mfi group create --config ./configs/group/create/config.json.example\n  mfi group update --config ./configs/group/update/config.json.example\n  mfi group init-fee-state --config ./configs/group/fee-state/config.json.example\n  mfi group init-staked-settings --config ./configs/group/staked-settings/config.json.example",
    after_long_help = "Common subcommands:\n  mfi group get <GROUP_PUBKEY>\n  mfi group get-all\n  mfi group create --config ./configs/group/create/config.json.example\n  mfi group update --config ./configs/group/update/config.json.example\n  mfi group init-fee-state --config ./configs/group/fee-state/config.json.example\n  mfi group init-staked-settings --config ./configs/group/staked-settings/config.json.example"
)]
pub enum GroupCommand {
    /// Display group details and its banks
    ///
    /// Example: `mfi group get <GROUP_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi group get <GROUP_PUBKEY>",
        after_long_help = "Example:\n  mfi group get <GROUP_PUBKEY>"
    )]
    Get { marginfi_group: Option<Pubkey> },
    /// List all marginfi groups
    ///
    /// Example: `mfi group get-all`
    #[clap(
        after_help = "Example:\n  mfi group get-all",
        after_long_help = "Example:\n  mfi group get-all"
    )]
    GetAll {},
    /// Create a new marginfi group
    ///
    /// Example: `mfi group create --config ./configs/group/create/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi group create --config ./configs/group/create/config.json.example",
        after_long_help = "Example:\n  mfi group create --config ./configs/group/create/config.json.example"
    )]
    Create {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        admin: Option<Pubkey>,
        #[clap(long)]
        emode_admin: Option<Pubkey>,
        #[clap(long)]
        curve_admin: Option<Pubkey>,
        #[clap(long)]
        limit_admin: Option<Pubkey>,
        #[clap(long)]
        flow_admin: Option<Pubkey>,
        #[clap(long)]
        emissions_admin: Option<Pubkey>,
        #[clap(long)]
        metadata_admin: Option<Pubkey>,
        #[clap(long)]
        risk_admin: Option<Pubkey>,
        #[clap(long)]
        emode_max_init_leverage: Option<f64>,
        #[clap(long)]
        emode_max_maint_leverage: Option<f64>,
        #[clap(short = 'f', long = "override")]
        override_existing_profile_group: bool,
    },
    /// Update group admin roles
    ///
    /// Example: `mfi group update --config ./configs/group/update/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi group update --config ./configs/group/update/config.json.example",
        after_long_help = "Example:\n  mfi group update --config ./configs/group/update/config.json.example"
    )]
    Update {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long)]
        new_admin: Option<Pubkey>,
        #[clap(long)]
        new_emode_admin: Option<Pubkey>,
        #[clap(long)]
        new_curve_admin: Option<Pubkey>,
        #[clap(long)]
        new_limit_admin: Option<Pubkey>,
        #[clap(long)]
        new_flow_admin: Option<Pubkey>,
        #[clap(long)]
        new_emissions_admin: Option<Pubkey>,
        #[clap(long)]
        new_metadata_admin: Option<Pubkey>,
        #[clap(long)]
        new_risk_admin: Option<Pubkey>,
        #[clap(long)]
        emode_max_init_leverage: Option<f64>,
        #[clap(long)]
        emode_max_maint_leverage: Option<f64>,
    },
    /// Handle bankruptcy for specified accounts
    ///
    /// Example: `mfi group handle-bankruptcy <ACCOUNT_PUBKEY_1> <ACCOUNT_PUBKEY_2>`
    #[clap(
        after_help = "Example:\n  mfi group handle-bankruptcy <ACCOUNT_PUBKEY_1> <ACCOUNT_PUBKEY_2>",
        after_long_help = "Example:\n  mfi group handle-bankruptcy <ACCOUNT_PUBKEY_1> <ACCOUNT_PUBKEY_2>"
    )]
    HandleBankruptcy { accounts: Vec<Pubkey> },
    /// Update address lookup table for the group
    ///
    /// Example: `mfi group update-lookup-table -t <TOKEN_ALT_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi group update-lookup-table -t <TOKEN_ALT_PUBKEY>",
        after_long_help = "Example:\n  mfi group update-lookup-table -t <TOKEN_ALT_PUBKEY>"
    )]
    UpdateLookupTable {
        #[clap(short = 't', long)]
        existing_token_lookup_tables: Vec<Pubkey>,
    },
    /// Check address lookup table status
    ///
    /// Example: `mfi group check-lookup-table -t <TOKEN_ALT_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi group check-lookup-table -t <TOKEN_ALT_PUBKEY>",
        after_long_help = "Example:\n  mfi group check-lookup-table -t <TOKEN_ALT_PUBKEY>"
    )]
    CheckLookupTable {
        #[clap(short = 't', long)]
        existing_token_lookup_tables: Vec<Pubkey>,
    },
    /// Initialize global fee state
    ///
    /// Example: `mfi group init-fee-state --config ./configs/group/fee-state/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi group init-fee-state --config ./configs/group/fee-state/config.json.example",
        after_long_help = "Example:\n  mfi group init-fee-state --config ./configs/group/fee-state/config.json.example"
    )]
    InitFeeState {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long)]
        admin: Option<Pubkey>,
        #[clap(long)]
        fee_wallet: Option<Pubkey>,
        #[clap(long)]
        bank_init_flat_sol_fee: Option<u32>,
        #[clap(long)]
        liquidation_flat_sol_fee: Option<u32>,
        #[clap(long)]
        program_fee_fixed: Option<f64>,
        #[clap(long)]
        program_fee_rate: Option<f64>,
        #[clap(long)]
        liquidation_max_fee: Option<f64>,
        #[clap(long)]
        order_init_flat_sol_fee: Option<u32>,
        #[clap(long)]
        order_execution_max_fee: Option<f64>,
    },
    /// Edit global fee state parameters
    ///
    /// Example: `mfi group edit-fee-state --config ./configs/group/fee-state/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi group edit-fee-state --config ./configs/group/fee-state/config.json.example",
        after_long_help = "Example:\n  mfi group edit-fee-state --config ./configs/group/fee-state/config.json.example"
    )]
    EditFeeState {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long)]
        new_admin: Option<Pubkey>,
        #[clap(long)]
        fee_wallet: Option<Pubkey>,
        #[clap(long)]
        bank_init_flat_sol_fee: Option<u32>,
        #[clap(long)]
        liquidation_flat_sol_fee: Option<u32>,
        #[clap(long)]
        program_fee_fixed: Option<f64>,
        #[clap(long)]
        program_fee_rate: Option<f64>,
        #[clap(long)]
        liquidation_max_fee: Option<f64>,
        #[clap(long, help = "Flat SOL fee (lamports) when creating an order")]
        order_init_flat_sol_fee: Option<u32>,
        #[clap(
            long,
            help = "Max order execution fee (as a decimal, e.g. 0.05 for 5%)"
        )]
        order_execution_max_fee: Option<f64>,
    },
    /// Configure group-level fee collection
    ///
    /// Example: `mfi group config-group-fee --enable-program-fee true`
    #[clap(
        after_help = "Example:\n  mfi group config-group-fee --enable-program-fee true",
        after_long_help = "Example:\n  mfi group config-group-fee --enable-program-fee true"
    )]
    ConfigGroupFee {
        #[clap(
            long,
            help = "True to enable collecting program fees for all banks in this group"
        )]
        enable_program_fee: bool,
    },
    /// Propagate fee state to a group
    ///
    /// Example: `mfi group propagate-fee`
    #[clap(
        after_help = "Examples:\n  mfi group propagate-fee\n  mfi group propagate-fee --marginfi-group <GROUP_PUBKEY>",
        after_long_help = "Examples:\n  mfi group propagate-fee\n  mfi group propagate-fee --marginfi-group <GROUP_PUBKEY>"
    )]
    PropagateFee {
        #[clap(long)]
        marginfi_group: Option<Pubkey>,
    },
    /// Emergency pause all group operations
    ///
    /// Example: `mfi group panic-pause`
    #[clap(
        after_help = "Example:\n  mfi group panic-pause",
        after_long_help = "Example:\n  mfi group panic-pause"
    )]
    PanicPause {},
    /// Unpause group operations (admin only)
    ///
    /// Example: `mfi group panic-unpause`
    #[clap(
        after_help = "Example:\n  mfi group panic-unpause",
        after_long_help = "Example:\n  mfi group panic-unpause"
    )]
    PanicUnpause {},
    /// Permissionless unpause after timeout
    ///
    /// Example: `mfi group panic-unpause-permissionless`
    #[clap(
        after_help = "Example:\n  mfi group panic-unpause-permissionless",
        after_long_help = "Example:\n  mfi group panic-unpause-permissionless"
    )]
    PanicUnpausePermissionless {},
    /// Initialize staked collateral settings
    ///
    /// Example: `mfi group init-staked-settings --config ./configs/group/staked-settings/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi group init-staked-settings --config ./configs/group/staked-settings/config.json.example",
        after_long_help = "Example:\n  mfi group init-staked-settings --config ./configs/group/staked-settings/config.json.example"
    )]
    InitStakedSettings {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long)]
        oracle: Option<Pubkey>,
        #[clap(long)]
        asset_weight_init: Option<f64>,
        #[clap(long)]
        asset_weight_maint: Option<f64>,
        #[clap(long)]
        deposit_limit: Option<u64>,
        #[clap(long)]
        total_asset_value_init_limit: Option<u64>,
        #[clap(long)]
        oracle_max_age: Option<u16>,
        #[clap(long, value_enum)]
        risk_tier: Option<RiskTierArg>,
    },
    /// Edit staked collateral settings
    ///
    /// Example: `mfi group edit-staked-settings --config ./configs/group/edit-staked-settings/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi group edit-staked-settings --config ./configs/group/edit-staked-settings/config.json.example",
        after_long_help = "Example:\n  mfi group edit-staked-settings --config ./configs/group/edit-staked-settings/config.json.example"
    )]
    EditStakedSettings {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long)]
        oracle: Option<Pubkey>,
        #[clap(long)]
        asset_weight_init: Option<f64>,
        #[clap(long)]
        asset_weight_maint: Option<f64>,
        #[clap(long)]
        deposit_limit: Option<u64>,
        #[clap(long)]
        total_asset_value_init_limit: Option<u64>,
        #[clap(long)]
        oracle_max_age: Option<u16>,
        #[clap(long, value_enum)]
        risk_tier: Option<RiskTierArg>,
    },
    /// Propagate staked settings to a specific bank
    ///
    /// Example: `mfi group propagate-staked-settings <BANK_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi group propagate-staked-settings <BANK_PUBKEY>",
        after_long_help = "Example:\n  mfi group propagate-staked-settings <BANK_PUBKEY>"
    )]
    PropagateStakedSettings { bank_pk: Pubkey },
    /// Configure group-level outflow rate limits
    ///
    /// Example: `mfi group configure-rate-limits --hourly-max-outflow-usd 1000000 --daily-max-outflow-usd 5000000`
    #[clap(
        after_help = "Example:\n  mfi group configure-rate-limits --hourly-max-outflow-usd 1000000 --daily-max-outflow-usd 5000000",
        after_long_help = "Example:\n  mfi group configure-rate-limits --hourly-max-outflow-usd 1000000 --daily-max-outflow-usd 5000000"
    )]
    ConfigureRateLimits {
        #[clap(long)]
        hourly_max_outflow_usd: Option<u64>,
        #[clap(long)]
        daily_max_outflow_usd: Option<u64>,
    },
    /// Configure daily deleverage withdrawal limit
    ///
    /// Example: `mfi group configure-deleverage-limit --daily-limit 250000`
    #[clap(
        after_help = "Example:\n  mfi group configure-deleverage-limit --daily-limit 250000",
        after_long_help = "Example:\n  mfi group configure-deleverage-limit --daily-limit 250000"
    )]
    ConfigureDeleverageLimit {
        #[clap(long)]
        daily_limit: u32,
    },
}

#[derive(Clone, Copy, Debug, Parser, ValueEnum)]
pub enum RiskTierArg {
    Collateral,
    Isolated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RatePointArg {
    pub util: u32,
    pub rate: u32,
}

impl FromStr for RatePointArg {
    type Err = String;

    /// Parse "util,rate" -> (u32, u32)
    /// util: a %, as u32, out of 100%     (e.g., 50% = 0.5 * u32::MAX)
    /// rate: a %, as u32, out of 1000%    (e.g., 100% = 0.1 * u32::MAX)
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (lhs, rhs) = s
            .split_once(',')
            .ok_or_else(|| "expected format: util,rate".to_string())?;

        let util = lhs
            .trim()
            .parse::<u32>()
            .map_err(|e| format!("invalid util u32: {e}"))?;
        let rate = rhs
            .trim()
            .parse::<u32>()
            .map_err(|e| format!("invalid rate u32: {e}"))?;

        Ok(RatePointArg { util, rate })
    }
}

impl From<RatePointArg> for RatePoint {
    fn from(p: RatePointArg) -> Self {
        RatePoint {
            util: p.util,
            rate: p.rate,
        }
    }
}

impl From<RiskTierArg> for RiskTier {
    fn from(value: RiskTierArg) -> Self {
        match value {
            RiskTierArg::Collateral => RiskTier::Collateral,
            RiskTierArg::Isolated => RiskTier::Isolated,
        }
    }
}

pub fn dispatch(subcmd: GroupCommand, global_options: &GlobalOptions) -> Result<()> {
    match &subcmd {
        GroupCommand::Create {
            config_example: true,
            ..
        } => {
            println!("{}", configs::GroupCreateConfig::example_json());
            return Ok(());
        }
        GroupCommand::Update {
            config_example: true,
            ..
        } => {
            println!("{}", configs::GroupUpdateConfig::example_json());
            return Ok(());
        }
        GroupCommand::InitFeeState {
            config_example: true,
            ..
        }
        | GroupCommand::EditFeeState {
            config_example: true,
            ..
        } => {
            println!("{}", configs::FeeStateConfig::example_json());
            return Ok(());
        }
        GroupCommand::InitStakedSettings {
            config_example: true,
            ..
        } => {
            println!("{}", configs::StakedSettingsConfig::example_json());
            return Ok(());
        }
        GroupCommand::EditStakedSettings {
            config_example: true,
            ..
        } => {
            println!("{}", configs::EditStakedSettingsConfig::example_json());
            return Ok(());
        }
        _ => {}
    }

    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        match subcmd {
            GroupCommand::Get { marginfi_group: _ } => (),
            GroupCommand::GetAll {} => (),

            _ => super::get_consent(&subcmd, &profile)?,
        }
    }

    match subcmd {
        GroupCommand::Get { marginfi_group } => {
            processor::group_get(config, marginfi_group.or(profile.marginfi_group))
        }
        GroupCommand::GetAll {} => processor::group_get_all(config),

        GroupCommand::Create {
            config: config_path,
            config_example,
            admin,
            emode_admin,
            curve_admin,
            limit_admin,
            flow_admin,
            emissions_admin,
            metadata_admin,
            risk_admin,
            emode_max_init_leverage,
            emode_max_maint_leverage,
            override_existing_profile_group,
        } => {
            if config_example {
                println!("{}", configs::GroupCreateConfig::example_json());
                return Ok(());
            }

            let cfg = if let Some(path) = config_path {
                Some(configs::load_config::<configs::GroupCreateConfig>(&path)?)
            } else {
                None
            };

            let create_config = if let Some(cfg) = cfg.as_ref() {
                processor::GroupCreateConfigRequest {
                    emode_admin: configs::parse_optional_pubkey(&cfg.emode_admin)?,
                    curve_admin: configs::parse_optional_pubkey(&cfg.curve_admin)?,
                    limit_admin: configs::parse_optional_pubkey(&cfg.limit_admin)?,
                    flow_admin: configs::parse_optional_pubkey(&cfg.flow_admin)?,
                    emissions_admin: configs::parse_optional_pubkey(&cfg.emissions_admin)?,
                    metadata_admin: configs::parse_optional_pubkey(&cfg.metadata_admin)?,
                    risk_admin: configs::parse_optional_pubkey(&cfg.risk_admin)?,
                    emode_max_init_leverage: cfg.emode_max_init_leverage,
                    emode_max_maint_leverage: cfg.emode_max_maint_leverage,
                }
            } else {
                processor::GroupCreateConfigRequest {
                    emode_admin,
                    curve_admin,
                    limit_admin,
                    flow_admin,
                    emissions_admin,
                    metadata_admin,
                    risk_admin,
                    emode_max_init_leverage,
                    emode_max_maint_leverage,
                }
            };

            let create_admin = if let Some(cfg) = cfg.as_ref() {
                configs::parse_optional_pubkey(&cfg.admin)?
            } else {
                admin
            };

            processor::group_create(
                config,
                profile,
                create_admin,
                override_existing_profile_group,
                create_config,
            )
        }

        GroupCommand::Update {
            config: config_path,
            config_example,
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
            new_flow_admin,
            new_emissions_admin,
            new_metadata_admin,
            new_risk_admin,
            emode_max_init_leverage,
            emode_max_maint_leverage,
        } => {
            if config_example {
                println!("{}", configs::GroupUpdateConfig::example_json());
                return Ok(());
            }
            if let Some(path) = config_path {
                let cfg: configs::GroupUpdateConfig = configs::load_config(&path)?;
                processor::group_configure(
                    config,
                    profile,
                    configs::parse_optional_pubkey(&cfg.new_admin)?,
                    configs::parse_optional_pubkey(&cfg.new_emode_admin)?,
                    configs::parse_optional_pubkey(&cfg.new_curve_admin)?,
                    configs::parse_optional_pubkey(&cfg.new_limit_admin)?,
                    configs::parse_optional_pubkey(&cfg.new_flow_admin)?,
                    configs::parse_optional_pubkey(&cfg.new_emissions_admin)?,
                    configs::parse_optional_pubkey(&cfg.new_metadata_admin)?,
                    configs::parse_optional_pubkey(&cfg.new_risk_admin)?,
                    cfg.emode_max_init_leverage,
                    cfg.emode_max_maint_leverage,
                )
            } else {
                processor::group_configure(
                    config,
                    profile,
                    new_admin,
                    new_emode_admin,
                    new_curve_admin,
                    new_limit_admin,
                    new_flow_admin,
                    new_emissions_admin,
                    new_metadata_admin,
                    new_risk_admin,
                    emode_max_init_leverage,
                    emode_max_maint_leverage,
                )
            }
        }

        GroupCommand::HandleBankruptcy { accounts } => {
            processor::handle_bankruptcy_for_accounts(&config, &profile, accounts)
        }

        GroupCommand::CheckLookupTable {
            existing_token_lookup_tables,
        } => processor::group::process_check_lookup_tables(
            &config,
            &profile,
            existing_token_lookup_tables,
        ),

        GroupCommand::UpdateLookupTable {
            existing_token_lookup_tables,
        } => processor::group::process_update_lookup_tables(
            &config,
            &profile,
            existing_token_lookup_tables,
        ),
        GroupCommand::InitFeeState {
            config: config_path,
            config_example,
            admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            liquidation_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
            liquidation_max_fee,
            order_init_flat_sol_fee,
            order_execution_max_fee,
        } => {
            if config_example {
                println!("{}", configs::FeeStateConfig::example_json());
                return Ok(());
            }
            if let Some(path) = config_path {
                let cfg: configs::FeeStateConfig = configs::load_config(&path)?;
                processor::initialize_fee_state(
                    config,
                    configs::parse_pubkey(&cfg.admin)?,
                    configs::parse_pubkey(&cfg.fee_wallet)?,
                    cfg.bank_init_flat_sol_fee,
                    cfg.liquidation_flat_sol_fee,
                    cfg.program_fee_fixed,
                    cfg.program_fee_rate,
                    cfg.liquidation_max_fee,
                    cfg.order_init_flat_sol_fee,
                    cfg.order_execution_max_fee,
                )
            } else {
                processor::initialize_fee_state(
                    config,
                    admin.context("--admin required (or use --config)")?,
                    fee_wallet.context("--fee-wallet required")?,
                    bank_init_flat_sol_fee.context("--bank-init-flat-sol-fee required")?,
                    liquidation_flat_sol_fee.context("--liquidation-flat-sol-fee required")?,
                    program_fee_fixed.context("--program-fee-fixed required")?,
                    program_fee_rate.context("--program-fee-rate required")?,
                    liquidation_max_fee.context("--liquidation-max-fee required")?,
                    order_init_flat_sol_fee.unwrap_or(0),
                    order_execution_max_fee.unwrap_or(0.0),
                )
            }
        }
        GroupCommand::EditFeeState {
            config: config_path,
            config_example,
            new_admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            liquidation_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
            liquidation_max_fee,
            order_init_flat_sol_fee,
            order_execution_max_fee,
        } => {
            if config_example {
                println!("{}", configs::FeeStateConfig::example_json());
                return Ok(());
            }
            if let Some(path) = config_path {
                let cfg: configs::FeeStateConfig = configs::load_config(&path)?;
                processor::edit_fee_state(
                    config,
                    configs::parse_pubkey(&cfg.admin)?,
                    configs::parse_pubkey(&cfg.fee_wallet)?,
                    cfg.bank_init_flat_sol_fee,
                    cfg.liquidation_flat_sol_fee,
                    cfg.program_fee_fixed,
                    cfg.program_fee_rate,
                    cfg.liquidation_max_fee,
                    cfg.order_init_flat_sol_fee,
                    cfg.order_execution_max_fee,
                )
            } else {
                processor::edit_fee_state(
                    config,
                    new_admin.context("--new-admin required (or use --config)")?,
                    fee_wallet.context("--fee-wallet required")?,
                    bank_init_flat_sol_fee.context("--bank-init-flat-sol-fee required")?,
                    liquidation_flat_sol_fee.context("--liquidation-flat-sol-fee required")?,
                    program_fee_fixed.context("--program-fee-fixed required")?,
                    program_fee_rate.context("--program-fee-rate required")?,
                    liquidation_max_fee.context("--liquidation-max-fee required")?,
                    order_init_flat_sol_fee.unwrap_or(0),
                    order_execution_max_fee.unwrap_or(0.0),
                )
            }
        }
        GroupCommand::ConfigGroupFee { enable_program_fee } => {
            processor::config_group_fee(config, profile, enable_program_fee)
        }
        GroupCommand::PropagateFee { marginfi_group } => processor::propagate_fee(
            config,
            marginfi_group
                .or(profile.marginfi_group)
                .context("--marginfi-group required or set in profile")?,
        ),
        GroupCommand::PanicPause {} => processor::panic_pause(config),
        GroupCommand::PanicUnpause {} => processor::panic_unpause(config),
        GroupCommand::PanicUnpausePermissionless {} => {
            processor::panic_unpause_permissionless(config)
        }
        GroupCommand::InitStakedSettings {
            config: config_path,
            config_example,
            oracle,
            asset_weight_init,
            asset_weight_maint,
            deposit_limit,
            total_asset_value_init_limit,
            oracle_max_age,
            risk_tier,
        } => {
            if config_example {
                println!("{}", configs::StakedSettingsConfig::example_json());
                return Ok(());
            }
            if let Some(path) = config_path {
                let cfg: configs::StakedSettingsConfig = configs::load_config(&path)?;
                let rt = super::bank::parse_risk_tier_config(&cfg.risk_tier)?;
                processor::init_staked_settings(
                    config,
                    profile,
                    configs::parse_pubkey(&cfg.oracle)?,
                    cfg.asset_weight_init,
                    cfg.asset_weight_maint,
                    cfg.deposit_limit,
                    cfg.total_asset_value_init_limit,
                    cfg.oracle_max_age,
                    rt.into(),
                )
            } else {
                processor::init_staked_settings(
                    config,
                    profile,
                    oracle.context("--oracle required (or use --config)")?,
                    asset_weight_init.context("--asset-weight-init required")?,
                    asset_weight_maint.context("--asset-weight-maint required")?,
                    deposit_limit.context("--deposit-limit required")?,
                    total_asset_value_init_limit
                        .context("--total-asset-value-init-limit required")?,
                    oracle_max_age.context("--oracle-max-age required")?,
                    risk_tier.context("--risk-tier required")?.into(),
                )
            }
        }
        GroupCommand::EditStakedSettings {
            config: config_path,
            config_example,
            oracle,
            asset_weight_init,
            asset_weight_maint,
            deposit_limit,
            total_asset_value_init_limit,
            oracle_max_age,
            risk_tier,
        } => {
            if config_example {
                println!("{}", configs::EditStakedSettingsConfig::example_json());
                return Ok(());
            }
            if let Some(path) = config_path {
                let cfg: configs::EditStakedSettingsConfig = configs::load_config(&path)?;
                let rt = cfg
                    .risk_tier
                    .as_deref()
                    .map(super::bank::parse_risk_tier_config)
                    .transpose()?;
                processor::edit_staked_settings(
                    config,
                    profile,
                    configs::parse_optional_pubkey(&cfg.oracle)?,
                    cfg.asset_weight_init,
                    cfg.asset_weight_maint,
                    cfg.deposit_limit,
                    cfg.total_asset_value_init_limit,
                    cfg.oracle_max_age,
                    rt.map(Into::into),
                )
            } else {
                processor::edit_staked_settings(
                    config,
                    profile,
                    oracle,
                    asset_weight_init,
                    asset_weight_maint,
                    deposit_limit,
                    total_asset_value_init_limit,
                    oracle_max_age,
                    risk_tier.map(Into::into),
                )
            }
        }
        GroupCommand::PropagateStakedSettings { bank_pk } => {
            processor::propagate_staked_settings(config, profile, bank_pk)
        }
        GroupCommand::ConfigureRateLimits {
            hourly_max_outflow_usd,
            daily_max_outflow_usd,
        } => processor::configure_group_rate_limits(
            config,
            profile,
            hourly_max_outflow_usd,
            daily_max_outflow_usd,
        ),
        GroupCommand::ConfigureDeleverageLimit { daily_limit } => {
            processor::configure_deleverage_withdrawal_limit(config, profile, daily_limit)
        }
    }
}
