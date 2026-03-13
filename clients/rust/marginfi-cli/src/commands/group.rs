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
pub enum GroupCommand {
    /// Display group details and its banks
    Get { marginfi_group: Option<Pubkey> },
    /// List all marginfi groups
    GetAll {},
    /// Create a new marginfi group
    Create {
        admin: Option<Pubkey>,
        #[clap(short = 'f', long = "override")]
        override_existing_profile_group: bool,
    },
    /// Update group admin roles.
    ///
    /// Accepts either CLI flags or --config <path> with a JSON file.
    /// Example JSON: `mfi group update --config-example`
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
    /// Add a new lending bank to the group.
    ///
    /// Accepts either CLI flags or --config <path> with a JSON file.
    /// Example JSON: `mfi group add-bank --config-example`
    AddBank {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long)]
        mint: Option<Pubkey>,
        /// Generates a PDA for the bank key
        #[clap(long, action)]
        seed: bool,
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
        group_fixed_fee_apr: Option<f64>,
        #[clap(long)]
        group_ir_fee: Option<f64>,
        #[clap(long, value_enum)]
        risk_tier: Option<RiskTierArg>,
        #[clap(
            long,
            help = "Max oracle age in seconds, 0 for default (60s)",
            default_value = "60"
        )]
        oracle_max_age: u16,
        #[clap(long)]
        global_fee_wallet: Option<Pubkey>,
    },
    /// Clone a mainnet bank into staging/localnet using a deterministic seed
    CloneBank {
        #[clap(long)]
        source_bank: Pubkey,
        #[clap(long)]
        mint: Pubkey,
        #[clap(long)]
        bank_seed: u64,
    },
    /// Handle bankruptcy for specified accounts
    HandleBankruptcy { accounts: Vec<Pubkey> },
    /// Update address lookup table for the group
    UpdateLookupTable {
        #[clap(short = 't', long)]
        existing_token_lookup_tables: Vec<Pubkey>,
    },
    /// Check address lookup table status
    CheckLookupTable {
        #[clap(short = 't', long)]
        existing_token_lookup_tables: Vec<Pubkey>,
    },
    /// Initialize global fee state.
    ///
    /// Accepts either CLI flags or --config <path> with a JSON file.
    /// Example JSON: `mfi group init-fee-state --config-example`
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
    /// Edit global fee state parameters.
    ///
    /// Accepts either CLI flags or --config <path> with a JSON file.
    /// Example JSON: `mfi group edit-fee-state --config-example`
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
    ConfigGroupFee {
        #[clap(
            long,
            help = "True to enable collecting program fees for all banks in this group"
        )]
        enable_program_fee: bool,
    },
    /// Propagate fee state to a group
    PropagateFee {
        #[clap(long)]
        marginfi_group: Pubkey,
    },
    /// Emergency pause all group operations
    PanicPause {},
    /// Unpause group operations (admin only)
    PanicUnpause {},
    /// Permissionless unpause after timeout
    PanicUnpausePermissionless {},
    /// Initialize staked collateral settings
    InitStakedSettings {
        #[clap(long)]
        oracle: Pubkey,
        #[clap(long)]
        asset_weight_init: f64,
        #[clap(long)]
        asset_weight_maint: f64,
        #[clap(long)]
        deposit_limit: u64,
        #[clap(long)]
        total_asset_value_init_limit: u64,
        #[clap(long)]
        oracle_max_age: u16,
        #[clap(long, value_enum)]
        risk_tier: RiskTierArg,
    },
    /// Edit staked collateral settings
    EditStakedSettings {
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
    PropagateStakedSettings { bank_pk: Pubkey },
    /// Configure group-level outflow rate limits
    ConfigureRateLimits {
        #[clap(long)]
        hourly_max_outflow_usd: Option<u64>,
        #[clap(long)]
        daily_max_outflow_usd: Option<u64>,
    },
    /// Configure daily deleverage withdrawal limit
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
            admin,
            override_existing_profile_group,
        } => processor::group_create(config, profile, admin, override_existing_profile_group),

        GroupCommand::Update {
            config: config_path,
            config_example,
            new_admin,
            new_emode_admin,
            new_curve_admin,
            new_limit_admin,
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
                    configs::parse_pubkey(&cfg.new_admin)?,
                    configs::parse_pubkey(&cfg.new_emode_admin)?,
                    configs::parse_pubkey(&cfg.new_curve_admin)?,
                    configs::parse_pubkey(&cfg.new_limit_admin)?,
                    configs::parse_pubkey(&cfg.new_emissions_admin)?,
                    configs::parse_pubkey(&cfg.new_metadata_admin)?,
                    configs::parse_pubkey(&cfg.new_risk_admin)?,
                    cfg.emode_max_init_leverage,
                    cfg.emode_max_maint_leverage,
                )
            } else {
                processor::group_configure(
                    config,
                    profile,
                    new_admin.context("--new-admin required")?,
                    new_emode_admin.context("--new-emode-admin required")?,
                    new_curve_admin.context("--new-curve-admin required")?,
                    new_limit_admin.context("--new-limit-admin required")?,
                    new_emissions_admin.context("--new-emissions-admin required")?,
                    new_metadata_admin.context("--new-metadata-admin required")?,
                    new_risk_admin.context("--new-risk-admin required")?,
                    emode_max_init_leverage,
                    emode_max_maint_leverage,
                )
            }
        }

        GroupCommand::AddBank {
            config: config_path,
            config_example,
            mint: bank_mint,
            seed,
            asset_weight_init,
            asset_weight_maint,
            liability_weight_init,
            liability_weight_maint,
            zero_util_rate,
            hundred_util_rate,
            points,
            insurance_fee_fixed_apr,
            insurance_ir_fee,
            group_fixed_fee_apr,
            group_ir_fee,
            deposit_limit_ui,
            borrow_limit_ui,
            risk_tier,
            oracle_max_age,
            global_fee_wallet,
        } => {
            if config_example {
                println!("{}", configs::AddBankConfig::example_json());
                return Ok(());
            }
            if let Some(path) = config_path {
                let cfg: configs::AddBankConfig = configs::load_config(&path)?;
                let risk_tier_arg = match cfg.risk_tier.to_lowercase().as_str() {
                    "collateral" => RiskTierArg::Collateral,
                    "isolated" => RiskTierArg::Isolated,
                    other => anyhow::bail!("Unknown risk_tier in config: {other}"),
                };
                let pts: Vec<RatePointArg> = cfg
                    .points
                    .iter()
                    .map(|p| RatePointArg {
                        util: p.util,
                        rate: p.rate,
                    })
                    .collect();
                processor::group_add_bank(
                    config,
                    profile,
                    configs::parse_pubkey(&cfg.mint)?,
                    cfg.seed,
                    cfg.asset_weight_init,
                    cfg.asset_weight_maint,
                    cfg.liability_weight_init,
                    cfg.liability_weight_maint,
                    cfg.deposit_limit_ui,
                    cfg.borrow_limit_ui,
                    cfg.zero_util_rate,
                    cfg.hundred_util_rate,
                    pts,
                    cfg.insurance_fee_fixed_apr,
                    cfg.insurance_ir_fee,
                    cfg.group_fixed_fee_apr,
                    cfg.group_ir_fee,
                    risk_tier_arg,
                    cfg.oracle_max_age,
                    global_options.compute_unit_price,
                    configs::parse_pubkey(&cfg.global_fee_wallet)?,
                )
            } else {
                processor::group_add_bank(
                    config,
                    profile,
                    bank_mint.context("--mint required (or use --config)")?,
                    seed,
                    asset_weight_init.context("--asset-weight-init required")?,
                    asset_weight_maint.context("--asset-weight-maint required")?,
                    liability_weight_init.context("--liability-weight-init required")?,
                    liability_weight_maint.context("--liability-weight-maint required")?,
                    deposit_limit_ui.context("--deposit-limit-ui required")?,
                    borrow_limit_ui.context("--borrow-limit-ui required")?,
                    zero_util_rate.context("--zero-util-rate required")?,
                    hundred_util_rate.context("--hundred-util-rate required")?,
                    points,
                    insurance_fee_fixed_apr.context("--insurance-fee-fixed-apr required")?,
                    insurance_ir_fee.context("--insurance-ir-fee required")?,
                    group_fixed_fee_apr.context("--group-fixed-fee-apr required")?,
                    group_ir_fee.context("--group-ir-fee required")?,
                    risk_tier.context("--risk-tier required")?,
                    oracle_max_age,
                    global_options.compute_unit_price,
                    global_fee_wallet.context("--global-fee-wallet required")?,
                )
            }
        }
        GroupCommand::CloneBank {
            source_bank,
            mint,
            bank_seed,
        } => processor::group_clone_bank(config, profile, source_bank, mint, bank_seed),

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
        GroupCommand::PropagateFee { marginfi_group } => {
            processor::propagate_fee(config, marginfi_group)
        }
        GroupCommand::PanicPause {} => processor::panic_pause(config),
        GroupCommand::PanicUnpause {} => processor::panic_unpause(config),
        GroupCommand::PanicUnpausePermissionless {} => {
            processor::panic_unpause_permissionless(config)
        }
        GroupCommand::InitStakedSettings {
            oracle,
            asset_weight_init,
            asset_weight_maint,
            deposit_limit,
            total_asset_value_init_limit,
            oracle_max_age,
            risk_tier,
        } => processor::init_staked_settings(
            config,
            profile,
            oracle,
            asset_weight_init,
            asset_weight_maint,
            deposit_limit,
            total_asset_value_init_limit,
            oracle_max_age,
            risk_tier.into(),
        ),
        GroupCommand::EditStakedSettings {
            oracle,
            asset_weight_init,
            asset_weight_maint,
            deposit_limit,
            total_asset_value_init_limit,
            oracle_max_age,
            risk_tier,
        } => processor::edit_staked_settings(
            config,
            profile,
            oracle,
            asset_weight_init,
            asset_weight_maint,
            deposit_limit,
            total_asset_value_init_limit,
            oracle_max_age,
            risk_tier.map(Into::into),
        ),
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
