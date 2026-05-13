use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use drift_mocks::state::MinimalSpotMarket;
use marginfi_type_crate::pdas::{
    derive_drift_signer, derive_drift_spot_market_vault, derive_drift_state,
};
use marginfi_type_crate::types::Bank;
use solana_sdk::pubkey::Pubkey;

use super::require_field;
use crate::config::{Config, GlobalOptions};
use crate::configs;
use crate::processor;

/// Drift integration commands (user / permissionless).
#[derive(Debug, Parser)]
pub enum DriftCommand {
    /// Initialize a Drift user account for a bank (permissionless)
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
    Deposit {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        ui_amount: Option<f64>,
    },
    /// Withdraw from Drift via marginfi
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
    /// Harvest Drift spot market rewards to the protocol fee wallet (permissionless)
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

struct DriftDerivedAccounts {
    drift_state: Pubkey,
    drift_spot_market_vault: Pubkey,
    drift_signer: Pubkey,
    drift_oracle: Option<Pubkey>,
}

fn derive_drift_bank_accounts(config: &Config, bank_pk: Pubkey) -> Result<DriftDerivedAccounts> {
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let state = load_drift_spot_market(&config.mfi_program.rpc(), bank.integration_acc_1)?;
    let (drift_state, _) = derive_drift_state();
    let (drift_spot_market_vault, _) = derive_drift_spot_market_vault(state.market_index);
    let (drift_signer, _) = derive_drift_signer();
    Ok(DriftDerivedAccounts {
        drift_state,
        drift_spot_market_vault,
        drift_signer,
        drift_oracle: (state.oracle != Pubkey::default()).then_some(state.oracle),
    })
}

/// Read a Drift spot market into its `MinimalSpotMarket` representation.
fn load_drift_spot_market(
    rpc: &solana_client::rpc_client::RpcClient,
    spot_market: Pubkey,
) -> Result<MinimalSpotMarket> {
    let data = rpc.get_account_data(&spot_market)?;
    let size = std::mem::size_of::<MinimalSpotMarket>();
    if data.len() < 8 + size {
        anyhow::bail!(
            "Drift spot market account {} data too small ({} bytes)",
            spot_market,
            data.len()
        );
    }
    Ok(*bytemuck::from_bytes::<MinimalSpotMarket>(
        &data[8..8 + size],
    ))
}

/// Derive a Drift reward spot market's vault and underlying mint, used when withdrawing.
fn derive_drift_reward_market_accounts(
    rpc: &solana_client::rpc_client::RpcClient,
    spot_market: Pubkey,
) -> Result<(Pubkey, Pubkey)> {
    let state = load_drift_spot_market(rpc, spot_market)?;
    let (spot_market_vault, _) = derive_drift_spot_market_vault(state.market_index);
    Ok((spot_market_vault, state.mint))
}

/// Resolve `(reward_oracle, reward_spot_market, reward_mint)` for one optional reward spot market.
fn resolve_drift_reward_accounts(
    rpc: &solana_client::rpc_client::RpcClient,
    reward_spot_market: Option<Pubkey>,
) -> Result<(Option<Pubkey>, Option<Pubkey>, Option<Pubkey>)> {
    let Some(reward_spot_market) = reward_spot_market else {
        return Ok((None, None, None));
    };
    let state = load_drift_spot_market(rpc, reward_spot_market)?;
    let oracle = (state.oracle != Pubkey::default()).then_some(state.oracle);
    Ok((oracle, Some(reward_spot_market), Some(state.mint)))
}

pub fn dispatch(subcmd: DriftCommand, global_options: &GlobalOptions) -> Result<()> {
    match &subcmd {
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
        DriftCommand::InitUser {
            config: config_path,
            bank_pk,
            amount,
            ..
        } => {
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
            bank_pk,
            ui_amount,
            ..
        } => {
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
            bank_pk,
            ui_amount,
            withdraw_all,
            drift_reward_spot_market,
            drift_reward_spot_market_2,
            ..
        } => {
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
            let rpc = config.mfi_program.rpc();
            let (reward_oracle, reward_spot_market, reward_mint) =
                resolve_drift_reward_accounts(&rpc, reward_spot_market)?;
            let (reward_oracle_2, reward_spot_market_2, reward_mint_2) =
                resolve_drift_reward_accounts(&rpc, reward_spot_market_2)?;
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
            bank_pk,
            harvest_drift_spot_market,
            ..
        } => {
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
