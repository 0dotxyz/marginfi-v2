use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use juplend_mocks::state::Lending;
use marginfi_type_crate::constants::{ASSET_TAG_JUPLEND, PYTH_PUSH_MIGRATED_DEPRECATED};
use solana_sdk::pubkey::Pubkey;

use crate::config::GlobalOptions;
use crate::configs;
use crate::processor;
use crate::utils::derive_juplend_lending_from_mint;

/// JupLend integration commands.
#[derive(Debug, Parser)]
#[clap(
    after_help = "Common subcommands:\n  mfi juplend add-bank --config ./configs/juplend/add-bank/config.json.example\n  mfi juplend init-position <BANK_PUBKEY> --amount 100\n  mfi juplend deposit <BANK_PUBKEY> 10\n  mfi juplend withdraw <BANK_PUBKEY> 5",
    after_long_help = "Common subcommands:\n  mfi juplend add-bank --config ./configs/juplend/add-bank/config.json.example\n  mfi juplend init-position <BANK_PUBKEY> --amount 100\n  mfi juplend deposit <BANK_PUBKEY> 10\n  mfi juplend withdraw <BANK_PUBKEY> 5"
)]
pub enum JuplendCommand {
    /// Add a new JupLend bank to a marginfi group
    ///
    /// Example: `mfi juplend add-bank --config ./configs/juplend/add-bank/config.json.example`
    #[clap(
        visible_alias = "create-bank",
        after_help = "Example:\n  mfi juplend add-bank --config ./configs/juplend/add-bank/config.json.example",
        after_long_help = "Example:\n  mfi juplend add-bank --config ./configs/juplend/add-bank/config.json.example"
    )]
    AddBank {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
    },
    /// Initialize a JupLend position for a bank
    ///
    /// Example: `mfi juplend init-position <BANK_PUBKEY> --amount 100`
    #[clap(
        after_help = "Example:\n  mfi juplend init-position <BANK_PUBKEY> --amount 100",
        after_long_help = "Example:\n  mfi juplend init-position <BANK_PUBKEY> --amount 100"
    )]
    InitPosition {
        bank_pk: Pubkey,
        #[clap(long, help = "Native amount for seed deposit (minimum 10)")]
        amount: u64,
    },
    /// Deposit into JupLend via marginfi
    ///
    /// Example: `mfi juplend deposit <BANK_PUBKEY> 10`
    #[clap(
        after_help = "Example:\n  mfi juplend deposit <BANK_PUBKEY> 10",
        after_long_help = "Example:\n  mfi juplend deposit <BANK_PUBKEY> 10"
    )]
    Deposit { bank_pk: Pubkey, ui_amount: f64 },
    /// Withdraw from JupLend via marginfi
    ///
    /// Example: `mfi juplend withdraw <BANK_PUBKEY> 5`
    #[clap(
        after_help = "Example:\n  mfi juplend withdraw <BANK_PUBKEY> 5",
        after_long_help = "Example:\n  mfi juplend withdraw <BANK_PUBKEY> 5"
    )]
    Withdraw {
        bank_pk: Pubkey,
        ui_amount: f64,
        #[clap(short = 'a', long = "all")]
        withdraw_all: bool,
    },
}

fn load_juplend_lending_roots(
    rpc: &solana_client::rpc_client::RpcClient,
    lending: Pubkey,
) -> Result<(Pubkey, Pubkey)> {
    let lending_data = rpc.get_account_data(&lending)?;
    let lending_size = std::mem::size_of::<Lending>();
    if lending_data.len() < 8 + lending_size {
        anyhow::bail!(
            "JupLend lending account {} data too small ({} bytes)",
            lending,
            lending_data.len()
        );
    }
    let lending_state: &Lending = bytemuck::from_bytes(&lending_data[8..8 + lending_size]);
    Ok((lending_state.mint, lending_state.f_token_mint))
}

fn resolve_juplend_lending_accounts(
    rpc: &solana_client::rpc_client::RpcClient,
    configured_mint: Option<Pubkey>,
    configured_lending: Option<Pubkey>,
) -> Result<(Pubkey, Pubkey, Pubkey)> {
    let (mint, juplend_lending, onchain_f_token_mint) = match configured_lending {
        Some(juplend_lending) => {
            let (derived_mint, derived_f_token_mint) =
                load_juplend_lending_roots(rpc, juplend_lending)?;
            if let Some(mint) = configured_mint {
                if mint != derived_mint {
                    anyhow::bail!(
                        "Configured mint {} does not match JupLend lending {} mint {}",
                        mint,
                        juplend_lending,
                        derived_mint
                    );
                }
            }
            (derived_mint, juplend_lending, derived_f_token_mint)
        }
        None => {
            let mint = configured_mint.context(
                "mint or juplend_lending required: set one in config to derive the JupLend pool",
            )?;
            let (juplend_lending, expected_f_token_mint) = derive_juplend_lending_from_mint(&mint);
            let (derived_mint, derived_f_token_mint) =
                load_juplend_lending_roots(rpc, juplend_lending)?;
            if derived_mint != mint {
                anyhow::bail!(
                    "Derived JupLend lending {} mint {} does not match configured mint {}",
                    juplend_lending,
                    derived_mint,
                    mint
                );
            }
            if derived_f_token_mint != expected_f_token_mint {
                anyhow::bail!(
                    "Derived JupLend lending {} f_token_mint {} does not match expected {} for mint {}",
                    juplend_lending,
                    derived_f_token_mint,
                    expected_f_token_mint,
                    mint
                );
            }
            (mint, juplend_lending, derived_f_token_mint)
        }
    };

    let (expected_lending, expected_f_token_mint) = derive_juplend_lending_from_mint(&mint);
    if juplend_lending != expected_lending {
        anyhow::bail!(
            "Resolved JupLend lending {} does not match expected lending {} for mint {}",
            juplend_lending,
            expected_lending,
            mint
        );
    }
    if onchain_f_token_mint != expected_f_token_mint {
        anyhow::bail!(
            "Resolved JupLend lending {} f_token_mint {} does not match expected {} for mint {}",
            juplend_lending,
            onchain_f_token_mint,
            expected_f_token_mint,
            mint
        );
    }

    Ok((mint, juplend_lending, onchain_f_token_mint))
}

pub fn dispatch(subcmd: JuplendCommand, global_options: &GlobalOptions) -> Result<()> {
    if let JuplendCommand::AddBank {
        config_example: true,
        ..
    } = &subcmd
    {
        println!("{}", configs::AddBankJuplendConfig::example_json());
        return Ok(());
    }

    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        super::get_consent(&subcmd, &profile)?;
    }

    match subcmd {
        JuplendCommand::AddBank {
            config: config_path,
            config_example,
        } => {
            if config_example {
                println!("{}", configs::AddBankJuplendConfig::example_json());
                return Ok(());
            }
            let path = config_path.context("--config <path> required for add-bank")?;
            let c: configs::AddBankJuplendConfig = configs::load_config(&path)?;
            let group = c
                .group
                .as_deref()
                .map(configs::parse_pubkey)
                .transpose()?
                .or(profile.marginfi_group)
                .context("group required: set in config or profile")?;
            let rpc = config.mfi_program.rpc();
            let (mint, juplend_lending, f_token_mint) = resolve_juplend_lending_accounts(
                &rpc,
                configs::parse_optional_pubkey(&c.mint)?,
                configs::parse_optional_pubkey(&c.juplend_lending)?,
            )?;
            let oracle = configs::parse_pubkey(&c.oracle)?;
            if let Some(existing_bank) = processor::integrations::find_existing_integration_bank(
                &config,
                group,
                mint,
                ASSET_TAG_JUPLEND,
                juplend_lending,
            )? {
                anyhow::bail!(
                    "JupLend lending {} already exists as bank {} in group {}",
                    juplend_lending,
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
                "juplendPythPull" => 15u8,
                "juplendSwitchboardPull" => 16u8,
                other => anyhow::bail!("Unknown oracle_setup: {other}. Use 'juplendPythPull' or 'juplendSwitchboardPull'"),
            };
            let risk_tier = match c.risk_tier.as_deref().unwrap_or("collateral") {
                "isolated" => marginfi_type_crate::types::RiskTier::Isolated,
                _ => marginfi_type_crate::types::RiskTier::Collateral,
            };

            processor::integrations::juplend_add_bank(
                &config,
                processor::integrations::JuplendBankCreateRequest {
                    group,
                    bank_mint: mint,
                    seed,
                    oracle,
                    oracle_setup,
                    juplend_lending,
                    f_token_mint,
                    asset_weight_init: c.asset_weight_init.unwrap_or(0.8),
                    asset_weight_maint: c.asset_weight_maint.unwrap_or(0.9),
                    deposit_limit: c.deposit_limit.unwrap_or(1_000_000_000_000),
                    total_asset_value_init_limit: c
                        .total_asset_value_init_limit
                        .unwrap_or(1_000_000_000),
                    oracle_max_age: c.oracle_max_age,
                    oracle_max_confidence: c.oracle_max_confidence.unwrap_or(0),
                    risk_tier,
                    config_flags: c.config_flags.unwrap_or(PYTH_PUSH_MIGRATED_DEPRECATED),
                    init_deposit_amount: c.init_deposit_amount.unwrap_or(100),
                    token_program,
                },
            )
        }
        JuplendCommand::InitPosition { bank_pk, amount } => {
            processor::integrations::juplend_init_position(&profile, &config, bank_pk, amount)
        }
        JuplendCommand::Deposit { bank_pk, ui_amount } => {
            processor::integrations::juplend_deposit(&profile, &config, bank_pk, ui_amount)
        }
        JuplendCommand::Withdraw {
            bank_pk,
            ui_amount,
            withdraw_all,
        } => processor::integrations::juplend_withdraw(
            &profile,
            &config,
            bank_pk,
            ui_amount,
            withdraw_all,
        ),
    }
}
