use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use drift_mocks::state::MinimalSpotMarket;
use marginfi_type_crate::constants::{ASSET_TAG_DRIFT, PYTH_PUSH_MIGRATED_DEPRECATED};
use marginfi_type_crate::types::Bank;
use solana_sdk::pubkey::Pubkey;

use crate::config::{Config, GlobalOptions};
use crate::configs;
use crate::processor;

const DRIFT_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH");

/// Drift integration commands.
#[derive(Debug, Parser)]
#[clap(
    after_help = "Common subcommands:\n  mfi drift add-bank --config ./configs/drift/add-bank/config.json.example\n  mfi drift init-user --config ./configs/drift/init-user/config.json.example\n  mfi drift deposit --config ./configs/drift/deposit/config.json.example\n  mfi drift withdraw --config ./configs/drift/withdraw/config.json.example\n  mfi drift harvest-reward --config ./configs/drift/harvest-reward/config.json.example",
    after_long_help = "Common subcommands:\n  mfi drift add-bank --config ./configs/drift/add-bank/config.json.example\n  mfi drift init-user --config ./configs/drift/init-user/config.json.example\n  mfi drift deposit --config ./configs/drift/deposit/config.json.example\n  mfi drift withdraw --config ./configs/drift/withdraw/config.json.example\n  mfi drift harvest-reward --config ./configs/drift/harvest-reward/config.json.example"
)]
pub enum DriftCommand {
    /// Add a new Drift bank to a marginfi group
    ///
    /// Example: `mfi drift add-bank --config ./configs/drift/add-bank/config.json.example`
    #[clap(
        visible_alias = "create-bank",
        after_help = "Example:\n  mfi drift add-bank --config ./configs/drift/add-bank/config.json.example",
        after_long_help = "Example:\n  mfi drift add-bank --config ./configs/drift/add-bank/config.json.example"
    )]
    AddBank {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
    },
    /// Initialize a Drift user account for a bank
    ///
    /// Example: `mfi drift init-user --config ./configs/drift/init-user/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi drift init-user --config ./configs/drift/init-user/config.json.example",
        after_long_help = "Example:\n  mfi drift init-user --config ./configs/drift/init-user/config.json.example"
    )]
    InitUser {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        #[clap(long, help = "Native amount for seed deposit (minimum 10)")]
        amount: Option<u64>,
    },
    /// Deposit into Drift via marginfi
    ///
    /// Example: `mfi drift deposit --config ./configs/drift/deposit/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi drift deposit --config ./configs/drift/deposit/config.json.example",
        after_long_help = "Example:\n  mfi drift deposit --config ./configs/drift/deposit/config.json.example"
    )]
    Deposit {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        ui_amount: Option<f64>,
    },
    /// Withdraw from Drift via marginfi
    ///
    /// Example: `mfi drift withdraw --config ./configs/drift/withdraw/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi drift withdraw --config ./configs/drift/withdraw/config.json.example",
        after_long_help = "Example:\n  mfi drift withdraw --config ./configs/drift/withdraw/config.json.example"
    )]
    Withdraw {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        ui_amount: Option<f64>,
        #[clap(short = 'a', long = "all")]
        withdraw_all: bool,
        #[clap(long)]
        drift_reward_spot_market: Option<Pubkey>,
        #[clap(long)]
        drift_reward_spot_market_2: Option<Pubkey>,
    },
    /// Harvest Drift spot market rewards
    ///
    /// Example: `mfi drift harvest-reward --config ./configs/drift/harvest-reward/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi drift harvest-reward --config ./configs/drift/harvest-reward/config.json.example",
        after_long_help = "Example:\n  mfi drift harvest-reward --config ./configs/drift/harvest-reward/config.json.example"
    )]
    HarvestReward {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        #[clap(long)]
        harvest_drift_spot_market: Option<Pubkey>,
    },
}

use super::require_field;

fn load_drift_spot_market_roots(
    rpc: &solana_client::rpc_client::RpcClient,
    spot_market: Pubkey,
) -> Result<(Pubkey, Pubkey)> {
    let spot_market_data = rpc.get_account_data(&spot_market)?;
    let spot_market_size = std::mem::size_of::<MinimalSpotMarket>();
    if spot_market_data.len() < 8 + spot_market_size {
        anyhow::bail!(
            "Drift spot market account {} data too small ({} bytes)",
            spot_market,
            spot_market_data.len()
        );
    }
    let spot_market_state: &MinimalSpotMarket =
        bytemuck::from_bytes(&spot_market_data[8..8 + spot_market_size]);
    Ok((spot_market_state.mint, spot_market_state.oracle))
}

fn resolve_drift_reward_accounts(
    rpc: &solana_client::rpc_client::RpcClient,
    reward_spot_market: Option<Pubkey>,
    reward_oracle: Option<Pubkey>,
    reward_mint: Option<Pubkey>,
    label: &str,
) -> Result<(Option<Pubkey>, Option<Pubkey>, Option<Pubkey>)> {
    let Some(reward_spot_market) = reward_spot_market else {
        if reward_oracle.is_some() || reward_mint.is_some() {
            anyhow::bail!(
                "{label}_spot_market is required when setting {label}_oracle or {label}_mint"
            );
        }
        return Ok((None, None, None));
    };

    let (derived_reward_mint, derived_reward_oracle) =
        load_drift_spot_market_roots(rpc, reward_spot_market)?;

    if let Some(reward_mint) = reward_mint {
        if reward_mint != derived_reward_mint {
            anyhow::bail!(
                "Configured {label}_mint {} does not match Drift reward spot market {} mint {}",
                reward_mint,
                reward_spot_market,
                derived_reward_mint
            );
        }
    }

    if let Some(reward_oracle) = reward_oracle {
        if derived_reward_oracle != Pubkey::default() && reward_oracle != derived_reward_oracle {
            anyhow::bail!(
                "Configured {label}_oracle {} does not match Drift reward spot market {} oracle {}",
                reward_oracle,
                reward_spot_market,
                derived_reward_oracle
            );
        }
    }

    Ok((
        reward_oracle
            .or((derived_reward_oracle != Pubkey::default()).then_some(derived_reward_oracle)),
        Some(reward_spot_market),
        Some(derived_reward_mint),
    ))
}

struct DriftDerivedAccounts {
    drift_state: Pubkey,
    drift_spot_market_vault: Pubkey,
    drift_signer: Pubkey,
    drift_oracle: Option<Pubkey>,
}

fn derive_drift_bank_accounts(config: &Config, bank_pk: Pubkey) -> Result<DriftDerivedAccounts> {
    let rpc = config.mfi_program.rpc();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let spot_market = bank.integration_acc_1;
    let spot_market_data = rpc.get_account_data(&spot_market)?;
    let spot_market_size = std::mem::size_of::<MinimalSpotMarket>();
    if spot_market_data.len() < 8 + spot_market_size {
        anyhow::bail!(
            "Drift spot market account {} data too small ({} bytes)",
            spot_market,
            spot_market_data.len()
        );
    }
    let spot_market_state: &MinimalSpotMarket =
        bytemuck::from_bytes(&spot_market_data[8..8 + spot_market_size]);

    let (drift_state, _) = Pubkey::find_program_address(&[b"drift_state"], &DRIFT_PROGRAM_ID);
    let (drift_spot_market_vault, _) = Pubkey::find_program_address(
        &[
            b"spot_market_vault",
            &spot_market_state.market_index.to_le_bytes(),
        ],
        &DRIFT_PROGRAM_ID,
    );
    let (drift_signer, _) = Pubkey::find_program_address(&[b"drift_signer"], &DRIFT_PROGRAM_ID);

    Ok(DriftDerivedAccounts {
        drift_state,
        drift_spot_market_vault,
        drift_signer,
        drift_oracle: (spot_market_state.oracle != Pubkey::default())
            .then_some(spot_market_state.oracle),
    })
}

fn derive_drift_reward_market_accounts(
    rpc: &solana_client::rpc_client::RpcClient,
    spot_market: Pubkey,
) -> Result<(Pubkey, Pubkey)> {
    let spot_market_data = rpc.get_account_data(&spot_market)?;
    let spot_market_size = std::mem::size_of::<MinimalSpotMarket>();
    if spot_market_data.len() < 8 + spot_market_size {
        anyhow::bail!(
            "Drift spot market account {} data too small ({} bytes)",
            spot_market,
            spot_market_data.len()
        );
    }
    let spot_market_state: &MinimalSpotMarket =
        bytemuck::from_bytes(&spot_market_data[8..8 + spot_market_size]);
    let (spot_market_vault, _) = Pubkey::find_program_address(
        &[
            b"spot_market_vault",
            &spot_market_state.market_index.to_le_bytes(),
        ],
        &DRIFT_PROGRAM_ID,
    );
    Ok((spot_market_vault, spot_market_state.mint))
}

pub fn dispatch(subcmd: DriftCommand, global_options: &GlobalOptions) -> Result<()> {
    match &subcmd {
        DriftCommand::AddBank {
            config_example: true,
            ..
        } => {
            println!("{}", configs::AddBankDriftConfig::example_json());
            return Ok(());
        }
        DriftCommand::InitUser {
            config_example: true,
            ..
        } => {
            println!("{}", configs::DriftInitUserConfig::example_json());
            return Ok(());
        }
        DriftCommand::Deposit {
            config_example: true,
            ..
        } => {
            println!("{}", configs::DriftDepositConfig::example_json());
            return Ok(());
        }
        DriftCommand::Withdraw {
            config_example: true,
            ..
        } => {
            println!("{}", configs::DriftWithdrawConfig::example_json());
            return Ok(());
        }
        DriftCommand::HarvestReward {
            config_example: true,
            ..
        } => {
            println!("{}", configs::DriftHarvestRewardConfig::example_json());
            return Ok(());
        }
        _ => {}
    }

    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        super::get_consent(&subcmd, &profile)?;
    }

    match subcmd {
        DriftCommand::AddBank {
            config: config_path,
            config_example,
        } => {
            if config_example {
                println!("{}", configs::AddBankDriftConfig::example_json());
                return Ok(());
            }
            let path = config_path.context("--config <path> required for add-bank")?;
            let c: configs::AddBankDriftConfig = configs::load_config(&path)?;
            let group = c
                .group
                .as_deref()
                .map(|s| configs::parse_pubkey(s))
                .transpose()?
                .or(profile.marginfi_group)
                .context("group required: set in config or profile")?;
            let rpc = config.mfi_program.rpc();
            let drift_spot_market = Pubkey::find_program_address(
                &[b"spot_market", &c.drift_market_index.to_le_bytes()],
                &solana_sdk::pubkey!("dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH"),
            )
            .0;
            let (derived_mint, derived_drift_oracle) =
                load_drift_spot_market_roots(&rpc, drift_spot_market)?;
            let mint = configs::parse_optional_pubkey(&c.mint)?.unwrap_or(derived_mint);
            if mint != derived_mint {
                anyhow::bail!(
                    "Configured mint {} does not match Drift spot market {} mint {}",
                    mint,
                    drift_spot_market,
                    derived_mint
                );
            }
            let drift_oracle = configs::parse_optional_pubkey(&c.drift_oracle)?
                .or((derived_drift_oracle != Pubkey::default()).then_some(derived_drift_oracle));
            let oracle = configs::parse_optional_pubkey(&c.oracle)?
                .or(drift_oracle)
                .context("oracle required: set oracle in config or use a Drift spot market with a configured oracle")?;
            if let Some(existing_bank) = processor::integrations::find_existing_integration_bank(
                &config,
                group,
                mint,
                ASSET_TAG_DRIFT,
                drift_spot_market,
            )? {
                anyhow::bail!(
                    "Drift spot market {} already exists as bank {} in group {}",
                    drift_spot_market,
                    existing_bank,
                    group
                );
            }
            let seed = processor::integrations::resolve_integration_bank_seed(
                &config, group, mint, c.seed,
            )?;
            let mint_account = rpc.get_account(&mint)?;
            let token_program = mint_account.owner;

            let oracle_setup = match c.oracle_setup.as_str() {
                "driftPythPull" => 9u8,
                "driftSwitchboardPull" => 10u8,
                other => anyhow::bail!(
                    "Unknown oracle_setup: {other}. Use 'driftPythPull' or 'driftSwitchboardPull'"
                ),
            };
            let risk_tier = match c.risk_tier.as_deref().unwrap_or("collateral") {
                "isolated" => marginfi_type_crate::types::RiskTier::Isolated,
                _ => marginfi_type_crate::types::RiskTier::Collateral,
            };

            processor::integrations::drift_add_bank(
                &config,
                processor::integrations::DriftBankCreateRequest {
                    group,
                    bank_mint: mint,
                    seed,
                    oracle,
                    oracle_setup,
                    drift_market_index: c.drift_market_index,
                    asset_weight_init: c.asset_weight_init.unwrap_or(0.55),
                    asset_weight_maint: c.asset_weight_maint.unwrap_or(0.65),
                    deposit_limit: c.deposit_limit.unwrap_or(10_000_000_000),
                    total_asset_value_init_limit: c
                        .total_asset_value_init_limit
                        .unwrap_or(10_000_000_000),
                    oracle_max_age: c.oracle_max_age,
                    oracle_max_confidence: c.oracle_max_confidence.unwrap_or(0),
                    risk_tier,
                    config_flags: c.config_flags.unwrap_or(PYTH_PUSH_MIGRATED_DEPRECATED),
                    drift_oracle,
                    init_deposit_amount: c.init_deposit_amount.unwrap_or(100),
                    token_program,
                },
            )
        }
        DriftCommand::InitUser {
            config: config_path,
            config_example,
            bank_pk,
            amount,
        } => {
            if config_example {
                println!("{}", configs::DriftInitUserConfig::example_json());
                return Ok(());
            }
            let (bank_pk, amount) = if let Some(path) = config_path {
                let c: configs::DriftInitUserConfig = configs::load_config(&path)?;
                (configs::parse_pubkey(&c.bank_pk)?, c.amount)
            } else {
                (
                    require_field!(bank_pk, "bank-pk"),
                    require_field!(amount, "amount"),
                )
            };
            let derived = derive_drift_bank_accounts(&config, bank_pk)?;
            processor::integrations::drift_init_user(
                &profile,
                &config,
                bank_pk,
                amount,
                derived.drift_state,
                derived.drift_spot_market_vault,
                derived.drift_oracle,
            )
        }
        DriftCommand::Deposit {
            config: config_path,
            config_example,
            bank_pk,
            ui_amount,
        } => {
            if config_example {
                println!("{}", configs::DriftDepositConfig::example_json());
                return Ok(());
            }
            let (bank_pk, ui_amount) = if let Some(path) = config_path {
                let c: configs::DriftDepositConfig = configs::load_config(&path)?;
                (configs::parse_pubkey(&c.bank_pk)?, c.ui_amount)
            } else {
                (
                    require_field!(bank_pk, "bank-pk"),
                    require_field!(ui_amount, "ui-amount"),
                )
            };
            let derived = derive_drift_bank_accounts(&config, bank_pk)?;
            processor::integrations::drift_deposit(
                &profile,
                &config,
                bank_pk,
                ui_amount,
                derived.drift_state,
                derived.drift_spot_market_vault,
                derived.drift_oracle,
            )
        }
        DriftCommand::Withdraw {
            config: config_path,
            config_example,
            bank_pk,
            ui_amount,
            withdraw_all,
            drift_reward_spot_market,
            drift_reward_spot_market_2,
        } => {
            if config_example {
                println!("{}", configs::DriftWithdrawConfig::example_json());
                return Ok(());
            }
            let (bank_pk, ui_amount, withdraw_all, reward_spot_market, reward_spot_market_2) =
                if let Some(path) = config_path {
                    let c: configs::DriftWithdrawConfig = configs::load_config(&path)?;
                    (
                        configs::parse_pubkey(&c.bank_pk)?,
                        c.ui_amount,
                        c.withdraw_all,
                        configs::parse_optional_pubkey(&c.drift_reward_spot_market)?,
                        configs::parse_optional_pubkey(&c.drift_reward_spot_market_2)?,
                    )
                } else {
                    (
                        require_field!(bank_pk, "bank-pk"),
                        ui_amount.unwrap_or(0.0),
                        withdraw_all,
                        drift_reward_spot_market,
                        drift_reward_spot_market_2,
                    )
                };
            let derived = derive_drift_bank_accounts(&config, bank_pk)?;
            let (reward_oracle, reward_spot_market, reward_mint) = resolve_drift_reward_accounts(
                &config.mfi_program.rpc(),
                reward_spot_market,
                None,
                None,
                "drift_reward",
            )?;
            let (reward_oracle_2, reward_spot_market_2, reward_mint_2) =
                resolve_drift_reward_accounts(
                    &config.mfi_program.rpc(),
                    reward_spot_market_2,
                    None,
                    None,
                    "drift_reward_2",
                )?;
            processor::integrations::drift_withdraw(
                &profile,
                &config,
                bank_pk,
                ui_amount,
                withdraw_all,
                derived.drift_state,
                derived.drift_spot_market_vault,
                derived.drift_oracle,
                derived.drift_signer,
                reward_oracle,
                reward_spot_market,
                reward_mint,
                reward_oracle_2,
                reward_spot_market_2,
                reward_mint_2,
            )
        }
        DriftCommand::HarvestReward {
            config: config_path,
            config_example,
            bank_pk,
            harvest_drift_spot_market,
        } => {
            if config_example {
                println!("{}", configs::DriftHarvestRewardConfig::example_json());
                return Ok(());
            }
            let (bank_pk, harvest_drift_spot_market) = if let Some(path) = config_path {
                let c: configs::DriftHarvestRewardConfig = configs::load_config(&path)?;
                (
                    configs::parse_pubkey(&c.bank_pk)?,
                    configs::parse_pubkey(&c.harvest_drift_spot_market)?,
                )
            } else {
                (
                    require_field!(bank_pk, "bank-pk"),
                    require_field!(harvest_drift_spot_market, "harvest-drift-spot-market"),
                )
            };
            let derived = derive_drift_bank_accounts(&config, bank_pk)?;
            let (derived_harvest_vault, derived_reward_mint) = derive_drift_reward_market_accounts(
                &config.mfi_program.rpc(),
                harvest_drift_spot_market,
            )?;
            processor::integrations::drift_harvest_reward(
                &config,
                bank_pk,
                derived.drift_state,
                derived.drift_signer,
                harvest_drift_spot_market,
                derived_harvest_vault,
                derived_reward_mint,
            )
        }
    }
}
