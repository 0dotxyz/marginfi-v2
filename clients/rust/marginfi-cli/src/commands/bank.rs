use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use fixed::types::I80F48;
use solana_sdk::pubkey::Pubkey;

use marginfi_type_crate::types::{
    make_points, Bank, BankConfigOpt, BankOperationalState, InterestRateConfigOpt, RatePoint,
    CURVE_POINTS,
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
/// Bank management commands (view, configure, fees, metadata).
#[derive(Debug, Parser)]
pub enum BankCommand {
    /// Display details for a specific bank (or the profile default)
    Get { bank: Option<String> },
    /// List all banks in a group
    GetAll { marginfi_group: Option<Pubkey> },
    /// Update bank configuration parameters (weights, limits, fees, oracle, risk tier, etc.)
    ///
    /// Accepts either CLI flags or --config <path> with a JSON file.
    /// Example JSON: `mfi bank update --config-example`
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
    /// Update only the interest rate config (curve-admin instruction)
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
    /// Update only deposit/borrow/init limits (limit-admin instruction)
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
    UpdateOracle {
        bank_pk: String,
        #[clap(
            long,
            help = "Bank oracle type (3 = Pyth Pull, 4 = Switchboard Pull, 5 = Staked Pyth Pull)"
        )]
        oracle_type: u8,
        #[clap(long, help = "Bank oracle account (or feed if using Pyth Pull")]
        oracle_key: Pubkey,
    },
    /// Mark tokenless repayment workflow complete for a deleveraging bank
    ForceTokenlessRepayComplete { bank_pk: String },
    /// Show current oracle price and metadata for a bank
    InspectPriceOracle { bank_pk: String },
    /// Collect accrued protocol fees from a bank
    CollectFees {
        bank: String,
        #[clap(help = "The ATA for fee_state.global_fee_wallet and the bank's mint")]
        fee_ata: Pubkey,
    },
    /// Withdraw collected fees from a bank's fee vault
    WithdrawFees {
        bank: String,
        amount: f64,
        #[clap(help = "Destination address, defaults to the profile authority")]
        destination_address: Option<Pubkey>,
    },
    /// Withdraw funds from a bank's insurance vault
    WithdrawInsurance {
        bank: String,
        amount: f64,
        #[clap(help = "Destination address, defaults to the profile authority")]
        destination_address: Option<Pubkey>,
    },
    /// Close a bank (must be empty)
    Close { bank_pk: String },
    /// Manually trigger interest accrual on a bank
    AccrueInterest { bank_pk: String },
    /// Override oracle with a fixed price (admin only)
    SetFixedPrice {
        bank_pk: String,
        #[clap(long)]
        price: f64,
    },
    /// Set the e-mode tag for a bank
    ConfigureEmode {
        bank_pk: String,
        #[clap(long)]
        emode_tag: u16,
    },
    /// Copy e-mode settings from one bank to another in the same group
    CloneEmode {
        #[clap(long)]
        copy_from_bank: String,
        #[clap(long)]
        copy_to_bank: String,
    },
    /// Migrate legacy curve encoding to seven-point format
    MigrateCurve { bank_pk: String },
    /// Refresh the cached oracle price for a bank
    PulsePriceCache { bank_pk: String },
    /// Set hourly/daily outflow rate limits for a bank
    ConfigureRateLimits {
        bank_pk: String,
        #[clap(long)]
        hourly_max_outflow: Option<u64>,
        #[clap(long)]
        daily_max_outflow: Option<u64>,
    },
    /// Withdraw fees without admin (permissionless, if enabled)
    WithdrawFeesPermissionless {
        bank_pk: String,
        #[clap(long)]
        amount: u64,
    },
    /// Change the fee destination address for a bank
    UpdateFeesDestination {
        bank_pk: String,
        #[clap(long)]
        destination: Pubkey,
    },
    /// Initialize on-chain metadata account for a bank
    InitMetadata { bank_pk: String },
    /// Write ticker and description to a bank's metadata account
    WriteMetadata {
        bank_pk: String,
        #[clap(long)]
        ticker: String,
        #[clap(long)]
        description: String,
    },
}

pub fn dispatch(subcmd: BankCommand, global_options: &GlobalOptions) -> Result<()> {
    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        match subcmd {
            BankCommand::Get { .. } | BankCommand::GetAll { .. } => (),

            BankCommand::InspectPriceOracle { .. } => (),
            #[allow(unreachable_patterns)]
            _ => super::get_consent(&subcmd, &profile)?,
        }
    }

    match subcmd {
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
            if let Some(path) = config_path {
                let cfg: configs::ConfigureBankConfig = configs::load_config(&path)?;
                let bank_pk = super::resolve_bank_for_group(&cfg.bank, profile.marginfi_group)?;
                let bank = config.mfi_program.account::<Bank>(bank_pk)?;
                let points_opt: Option<[RatePoint; CURVE_POINTS]> = if cfg.points.is_empty() {
                    None
                } else {
                    let pts: Vec<RatePoint> = cfg
                        .points
                        .iter()
                        .map(|p| RatePoint {
                            util: p.util,
                            rate: p.rate,
                        })
                        .collect();
                    Some(make_points(&pts))
                };
                let op_state = cfg.operational_state.as_deref().map(|s| match s {
                    "paused" => BankOperationalState::Paused,
                    "operational" => BankOperationalState::Operational,
                    "reduce_only" | "reduceonly" => BankOperationalState::ReduceOnly,
                    _ => BankOperationalState::Operational,
                });
                let risk = cfg.risk_tier.as_deref().map(|s| match s {
                    "isolated" => marginfi_type_crate::types::RiskTier::Isolated,
                    _ => marginfi_type_crate::types::RiskTier::Collateral,
                });
                let has_ir = cfg.insurance_fee_fixed_apr.is_some()
                    || cfg.insurance_ir_fee.is_some()
                    || cfg.protocol_fixed_fee_apr.is_some()
                    || cfg.protocol_ir_fee.is_some()
                    || cfg.protocol_origination_fee.is_some()
                    || cfg.zero_util_rate.is_some()
                    || cfg.hundred_util_rate.is_some()
                    || points_opt.is_some();

                processor::bank_configure(
                    config,
                    profile,
                    bank_pk,
                    BankConfigOpt {
                        asset_weight_init: cfg.asset_weight_init.map(|x| I80F48::from_num(x).into()),
                        asset_weight_maint: cfg.asset_weight_maint.map(|x| I80F48::from_num(x).into()),
                        liability_weight_init: cfg.liability_weight_init.map(|x| I80F48::from_num(x).into()),
                        liability_weight_maint: cfg.liability_weight_maint.map(|x| I80F48::from_num(x).into()),
                        deposit_limit: cfg.deposit_limit_ui.map(|ui| spl_token::ui_amount_to_amount(ui, bank.mint_decimals)),
                        borrow_limit: cfg.borrow_limit_ui.map(|ui| spl_token::ui_amount_to_amount(ui, bank.mint_decimals)),
                        operational_state: op_state,
                        interest_rate_config: if has_ir {
                            Some(InterestRateConfigOpt {
                                insurance_fee_fixed_apr: cfg.insurance_fee_fixed_apr.map(|x| I80F48::from_num(x).into()),
                                insurance_ir_fee: cfg.insurance_ir_fee.map(|x| I80F48::from_num(x).into()),
                                protocol_fixed_fee_apr: cfg.protocol_fixed_fee_apr.map(|x| I80F48::from_num(x).into()),
                                protocol_ir_fee: cfg.protocol_ir_fee.map(|x| I80F48::from_num(x).into()),
                                protocol_origination_fee: cfg.protocol_origination_fee.map(|x| I80F48::from_num(x).into()),
                                zero_util_rate: cfg.zero_util_rate,
                                hundred_util_rate: cfg.hundred_util_rate,
                                points: points_opt,
                            })
                        } else {
                            None
                        },
                        risk_tier: risk,
                        asset_tag: cfg.asset_tag,
                        total_asset_value_init_limit: cfg.total_asset_value_init_limit,
                        oracle_max_confidence: cfg.oracle_max_confidence,
                        oracle_max_age: cfg.oracle_max_age,
                        permissionless_bad_debt_settlement: cfg.permissionless_bad_debt_settlement,
                        freeze_settings: cfg.freeze_settings,
                        tokenless_repayments_allowed: cfg.tokenless_repayments_allowed,
                    },
                )
            } else {
                let bank_pk_str = bank_pk.context("bank_pk required (or use --config)")?;
                let bank_pk = super::resolve_bank_for_group(&bank_pk_str, profile.marginfi_group)?;
                let bank = config.mfi_program.account::<Bank>(bank_pk)?;
                let points_opt: Option<[RatePoint; CURVE_POINTS]> = if points.is_empty() {
                    None
                } else {
                    let pts: Vec<RatePoint> = points.iter().map(|p| (*p).into()).collect();
                    Some(make_points(&pts))
                };

                processor::bank_configure(
                    config,
                    profile,
                    bank_pk,
                    BankConfigOpt {
                        asset_weight_init: asset_weight_init.map(|x| I80F48::from_num(x).into()),
                        asset_weight_maint: asset_weight_maint.map(|x| I80F48::from_num(x).into()),
                        liability_weight_init: liability_weight_init
                            .map(|x| I80F48::from_num(x).into()),
                        liability_weight_maint: liability_weight_maint
                            .map(|x| I80F48::from_num(x).into()),
                        deposit_limit: deposit_limit_ui.map(|ui_amount| {
                            spl_token::ui_amount_to_amount(ui_amount, bank.mint_decimals)
                        }),
                        borrow_limit: borrow_limit_ui.map(|ui_amount| {
                            spl_token::ui_amount_to_amount(ui_amount, bank.mint_decimals)
                        }),
                        operational_state: operational_state.map(|x| x.into()),
                        interest_rate_config: if if_fa.is_some()
                            || if_ir.is_some()
                            || pf_fa.is_some()
                            || pf_ir.is_some()
                            || pf_or.is_some()
                            || zero_util_rate.is_some()
                            || hundred_util_rate.is_some()
                            || points_opt.is_some()
                        {
                            Some(InterestRateConfigOpt {
                                insurance_fee_fixed_apr: if_fa.map(|x| I80F48::from_num(x).into()),
                                insurance_ir_fee: if_ir.map(|x| I80F48::from_num(x).into()),
                                protocol_fixed_fee_apr: pf_fa.map(|x| I80F48::from_num(x).into()),
                                protocol_ir_fee: pf_ir.map(|x| I80F48::from_num(x).into()),
                                protocol_origination_fee: pf_or.map(|x| I80F48::from_num(x).into()),
                                zero_util_rate,
                                hundred_util_rate,
                                points: points_opt,
                            })
                        } else {
                            None
                        },
                        risk_tier: risk_tier.map(|x| x.into()),
                        asset_tag,
                        total_asset_value_init_limit: usd_init_limit,
                        oracle_max_confidence,
                        oracle_max_age,
                        permissionless_bad_debt_settlement,
                        freeze_settings,
                        tokenless_repayments_allowed,
                    },
                )
            }
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
            processor::bank_configure_oracle(config, profile, bank_pk, oracle_type, oracle_key)
        }
        BankCommand::ForceTokenlessRepayComplete { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_force_tokenless_repay_complete(config, bank_pk)
        }
        BankCommand::InspectPriceOracle { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_inspect_price_oracle(config, bank_pk)
        }
        BankCommand::CollectFees { bank, fee_ata } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::admin::process_collect_fees(config, bank_pk, fee_ata)
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
            processor::bank_close(config, profile, bank_pk)
        }
        BankCommand::AccrueInterest { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_accrue_interest(config, bank_pk)
        }
        BankCommand::SetFixedPrice { bank_pk, price } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_set_fixed_price(config, profile, bank_pk, price)
        }
        BankCommand::ConfigureEmode { bank_pk, emode_tag } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_configure_emode(config, profile, bank_pk, emode_tag)
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
                profile,
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
            processor::bank_update_fees_destination(config, profile, bank_pk, destination)
        }
        BankCommand::InitMetadata { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_init_metadata(config, bank_pk)
        }
        BankCommand::WriteMetadata {
            bank_pk,
            ticker,
            description,
        } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_write_metadata(config, profile, bank_pk, ticker, description)
        }
    }
}
