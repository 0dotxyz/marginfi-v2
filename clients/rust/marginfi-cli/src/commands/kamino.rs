use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use kamino_mocks::state::MinimalReserve;
use marginfi::state::bank::BankVaultType;
use marginfi_type_crate::{
    constants::{ASSET_TAG_KAMINO, PYTH_PUSH_MIGRATED_DEPRECATED},
    pdas::{
        derive_kamino_lending_market_authority, derive_kamino_user_state, FARMS_PROGRAM_ID,
        KAMINO_PROGRAM_ID,
    },
    types::{Bank, OracleSetup},
};
use solana_sdk::pubkey::Pubkey;

use crate::config::{Config, GlobalOptions};
use crate::configs;
use crate::processor;
use crate::utils::find_bank_vault_authority_pda;

/// Kamino integration commands.
#[derive(Debug, Parser)]
#[clap(
    after_help = "Common subcommands:\n  mfi kamino add-bank --config ./configs/kamino/add-bank/config.json.example\n  mfi kamino init-obligation --config ./configs/kamino/init-obligation/config.json.example\n  mfi kamino deposit --config ./configs/kamino/deposit/config.json.example\n  mfi kamino withdraw --config ./configs/kamino/withdraw/config.json.example\n  mfi kamino harvest-reward --config ./configs/kamino/harvest-reward/config.json.example",
    after_long_help = "Common subcommands:\n  mfi kamino add-bank --config ./configs/kamino/add-bank/config.json.example\n  mfi kamino init-obligation --config ./configs/kamino/init-obligation/config.json.example\n  mfi kamino deposit --config ./configs/kamino/deposit/config.json.example\n  mfi kamino withdraw --config ./configs/kamino/withdraw/config.json.example\n  mfi kamino harvest-reward --config ./configs/kamino/harvest-reward/config.json.example"
)]
pub enum KaminoCommand {
    /// Add a new Kamino bank to a marginfi group
    ///
    /// Example: `mfi kamino add-bank --config ./configs/kamino/add-bank/config.json.example`
    #[clap(
        visible_alias = "create-bank",
        after_help = "Example:\n  mfi kamino add-bank --config ./configs/kamino/add-bank/config.json.example",
        after_long_help = "Example:\n  mfi kamino add-bank --config ./configs/kamino/add-bank/config.json.example"
    )]
    AddBank {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
    },
    /// Initialize a Kamino obligation for a bank's reserve
    ///
    /// Example: `mfi kamino init-obligation --config ./configs/kamino/init-obligation/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi kamino init-obligation --config ./configs/kamino/init-obligation/config.json.example",
        after_long_help = "Example:\n  mfi kamino init-obligation --config ./configs/kamino/init-obligation/config.json.example"
    )]
    InitObligation {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        #[clap(long, help = "Native amount for seed deposit (minimum 10)")]
        amount: Option<u64>,
        #[clap(long, help = "Override the reserve oracle used for derivation")]
        reserve_oracle: Option<Pubkey>,
    },
    /// Deposit into a Kamino reserve via marginfi
    ///
    /// Example: `mfi kamino deposit --config ./configs/kamino/deposit/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi kamino deposit --config ./configs/kamino/deposit/config.json.example",
        after_long_help = "Example:\n  mfi kamino deposit --config ./configs/kamino/deposit/config.json.example"
    )]
    Deposit {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        ui_amount: Option<f64>,
    },
    /// Withdraw from a Kamino reserve via marginfi
    ///
    /// Example: `mfi kamino withdraw --config ./configs/kamino/withdraw/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi kamino withdraw --config ./configs/kamino/withdraw/config.json.example",
        after_long_help = "Example:\n  mfi kamino withdraw --config ./configs/kamino/withdraw/config.json.example"
    )]
    Withdraw {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        #[clap(
            help = "Kamino collateral-token UI amount to withdraw; use --all to close the full position"
        )]
        ui_amount: Option<f64>,
        #[clap(short = 'a', long = "all")]
        withdraw_all: bool,
    },
    /// Harvest Kamino farm rewards
    ///
    /// Example: `mfi kamino harvest-reward --config ./configs/kamino/harvest-reward/config.json.example`
    #[clap(
        after_help = "Example:\n  mfi kamino harvest-reward --config ./configs/kamino/harvest-reward/config.json.example",
        after_long_help = "Example:\n  mfi kamino harvest-reward --config ./configs/kamino/harvest-reward/config.json.example"
    )]
    HarvestReward {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        #[clap(long)]
        reward_index: Option<u64>,
        #[clap(long)]
        global_config: Option<Pubkey>,
        #[clap(long)]
        reward_mint: Option<Pubkey>,
        #[clap(long)]
        scope_prices: Option<Pubkey>,
    },
}

use super::require_field;

fn load_kamino_reserve_roots(
    rpc: &solana_client::rpc_client::RpcClient,
    reserve: Pubkey,
) -> Result<(Pubkey, Pubkey, Pubkey)> {
    let reserve_data = rpc.get_account_data(&reserve)?;
    let reserve_size = std::mem::size_of::<MinimalReserve>();
    if reserve_data.len() < 8 + reserve_size {
        anyhow::bail!(
            "Kamino reserve account {} data too small ({} bytes)",
            reserve,
            reserve_data.len()
        );
    }
    let reserve_state: &MinimalReserve = bytemuck::from_bytes(&reserve_data[8..8 + reserve_size]);
    Ok((
        reserve_state.mint_pubkey,
        reserve_state.lending_market,
        reserve_state.token_program,
    ))
}

struct KaminoDerivedAccounts {
    lending_market: Pubkey,
    lending_market_authority: Pubkey,
    reserve_liquidity_supply: Pubkey,
    reserve_collateral_mint: Pubkey,
    reserve_destination_deposit_collateral: Pubkey,
    reserve_source_collateral: Pubkey,
    user_metadata: Pubkey,
    pyth_oracle: Option<Pubkey>,
    switchboard_price_oracle: Option<Pubkey>,
    switchboard_twap_oracle: Option<Pubkey>,
    scope_prices: Option<Pubkey>,
    obligation_farm_user_state: Option<Pubkey>,
    reserve_farm_state: Option<Pubkey>,
}

struct KaminoHarvestDerivedAccounts {
    user_state: Pubkey,
    farm_state: Pubkey,
    user_reward_ata: Pubkey,
    rewards_vault: Pubkey,
    rewards_treasury_vault: Pubkey,
    farm_vaults_authority: Pubkey,
    scope_prices: Option<Pubkey>,
}

fn derive_kamino_accounts(
    config: &Config,
    bank_pk: Pubkey,
    reserve_oracle_override: Option<Pubkey>,
) -> Result<KaminoDerivedAccounts> {
    let rpc = config.mfi_program.rpc();
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let reserve = bank.integration_acc_1;
    let reserve_data = rpc.get_account_data(&reserve)?;
    let reserve_size = std::mem::size_of::<MinimalReserve>();
    if reserve_data.len() < 8 + reserve_size {
        anyhow::bail!(
            "Kamino reserve account {} data too small ({} bytes)",
            reserve,
            reserve_data.len()
        );
    }
    let reserve_state: &MinimalReserve = bytemuck::from_bytes(&reserve_data[8..8 + reserve_size]);

    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);
    let (lending_market_authority, _) =
        derive_kamino_lending_market_authority(&reserve_state.lending_market);
    let (reserve_liquidity_supply, _) = Pubkey::find_program_address(
        &[b"reserve_liq_supply", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    );
    let (reserve_collateral_mint, _) = Pubkey::find_program_address(
        &[b"reserve_coll_mint", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    );
    let (reserve_destination_deposit_collateral, _) = Pubkey::find_program_address(
        &[b"reserve_coll_supply", reserve.as_ref()],
        &KAMINO_PROGRAM_ID,
    );
    let (user_metadata, _) = Pubkey::find_program_address(
        &[b"user_meta", liquidity_vault_authority.as_ref()],
        &KAMINO_PROGRAM_ID,
    );

    let reserve_farm_state = (reserve_state.farm_collateral != Pubkey::default())
        .then_some(reserve_state.farm_collateral);
    let obligation_farm_user_state = reserve_farm_state
        .map(|farm_state| derive_kamino_user_state(&farm_state, &bank.integration_acc_2).0);

    let reserve_oracle =
        reserve_oracle_override
            .or((bank.config.oracle_keys[0] != Pubkey::default())
                .then_some(bank.config.oracle_keys[0]));
    let (pyth_oracle, scope_prices) = match bank.config.oracle_setup {
        OracleSetup::KaminoPythPush => (reserve_oracle, None),
        OracleSetup::KaminoSwitchboardPull => (None, reserve_oracle),
        _ => (None, None),
    };

    Ok(KaminoDerivedAccounts {
        lending_market: reserve_state.lending_market,
        lending_market_authority,
        reserve_liquidity_supply,
        reserve_collateral_mint,
        reserve_destination_deposit_collateral,
        reserve_source_collateral: reserve_state.collateral_supply_vault,
        user_metadata,
        pyth_oracle,
        switchboard_price_oracle: None,
        switchboard_twap_oracle: None,
        scope_prices,
        obligation_farm_user_state,
        reserve_farm_state,
    })
}

fn derive_kamino_harvest_reward_accounts(
    config: &Config,
    bank_pk: Pubkey,
    global_config: Pubkey,
    reward_mint: Pubkey,
) -> Result<KaminoHarvestDerivedAccounts> {
    let bank = config.mfi_program.account::<Bank>(bank_pk)?;
    let derived = derive_kamino_accounts(config, bank_pk, None)?;
    let farm_state = derived
        .reserve_farm_state
        .context("Kamino reserve has no farm state; rewards are not initialized for this bank")?;
    let (farm_vaults_authority, _) =
        Pubkey::find_program_address(&[b"authority", farm_state.as_ref()], &FARMS_PROGRAM_ID);
    let (rewards_vault, _) = Pubkey::find_program_address(
        &[b"rvault", farm_state.as_ref(), reward_mint.as_ref()],
        &FARMS_PROGRAM_ID,
    );
    let (rewards_treasury_vault, _) = Pubkey::find_program_address(
        &[b"tvault", global_config.as_ref(), reward_mint.as_ref()],
        &FARMS_PROGRAM_ID,
    );
    let (liquidity_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Liquidity, &config.program_id);
    let reward_mint_account = config.mfi_program.rpc().get_account(&reward_mint)?;
    let reward_token_program = reward_mint_account.owner;
    let user_reward_ata =
        anchor_spl::associated_token::get_associated_token_address_with_program_id(
            &liquidity_vault_authority,
            &reward_mint,
            &reward_token_program,
        );

    Ok(KaminoHarvestDerivedAccounts {
        user_state: bank.integration_acc_2,
        farm_state,
        user_reward_ata,
        rewards_vault,
        rewards_treasury_vault,
        farm_vaults_authority,
        scope_prices: derived.scope_prices,
    })
}

pub fn dispatch(subcmd: KaminoCommand, global_options: &GlobalOptions) -> Result<()> {
    match &subcmd {
        KaminoCommand::AddBank {
            config_example: true,
            ..
        } => {
            println!("{}", configs::AddBankKaminoConfig::example_json());
            return Ok(());
        }
        KaminoCommand::InitObligation {
            config_example: true,
            ..
        } => {
            println!("{}", configs::KaminoInitObligationConfig::example_json());
            return Ok(());
        }
        KaminoCommand::Deposit {
            config_example: true,
            ..
        } => {
            println!("{}", configs::KaminoDepositConfig::example_json());
            return Ok(());
        }
        KaminoCommand::Withdraw {
            config_example: true,
            ..
        } => {
            println!("{}", configs::KaminoWithdrawConfig::example_json());
            return Ok(());
        }
        KaminoCommand::HarvestReward {
            config_example: true,
            ..
        } => {
            println!("{}", configs::KaminoHarvestRewardConfig::example_json());
            return Ok(());
        }
        _ => {}
    }

    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        super::get_consent(&subcmd, &profile)?;
    }

    match subcmd {
        KaminoCommand::AddBank {
            config: config_path,
            config_example,
        } => {
            if config_example {
                println!("{}", configs::AddBankKaminoConfig::example_json());
                return Ok(());
            }
            let path = config_path.context("--config <path> required for add-bank")?;
            let c: configs::AddBankKaminoConfig = configs::load_config(&path)?;
            let group = c
                .group
                .as_deref()
                .map(configs::parse_pubkey)
                .transpose()?
                .or(profile.marginfi_group)
                .context("group required: set in config or profile")?;
            let rpc = config.mfi_program.rpc();
            let kamino_reserve = configs::parse_pubkey(&c.kamino_reserve)?;
            let (derived_mint, derived_market, token_program) =
                load_kamino_reserve_roots(&rpc, kamino_reserve)?;
            let mint = configs::parse_optional_pubkey(&c.mint)?.unwrap_or(derived_mint);
            if mint != derived_mint {
                anyhow::bail!(
                    "Configured mint {} does not match Kamino reserve {} mint {}",
                    mint,
                    kamino_reserve,
                    derived_mint
                );
            }
            let kamino_market =
                configs::parse_optional_pubkey(&c.kamino_market)?.unwrap_or(derived_market);
            if kamino_market != derived_market {
                anyhow::bail!(
                    "Configured Kamino market {} does not match reserve {} lending market {}",
                    kamino_market,
                    kamino_reserve,
                    derived_market
                );
            }
            let reserve_oracle = configs::parse_optional_pubkey(&c.reserve_oracle)?;
            let oracle = configs::parse_optional_pubkey(&c.oracle)?
                .or(reserve_oracle)
                .context("oracle required: set oracle or reserve_oracle in config")?;
            if let Some(existing_bank) = processor::integrations::find_existing_integration_bank(
                &config,
                group,
                mint,
                ASSET_TAG_KAMINO,
                kamino_reserve,
            )? {
                anyhow::bail!(
                    "Kamino reserve {} already exists as bank {} in group {}",
                    kamino_reserve,
                    existing_bank,
                    group
                );
            }
            let seed = processor::integrations::resolve_integration_bank_seed(
                &config, group, mint, c.seed,
            )?;

            // Parse oracle setup
            let oracle_setup = match c.oracle_setup.as_str() {
                "kaminoPythPush" => 11u8,
                "kaminoSwitchboardPull" => 12u8,
                other => anyhow::bail!("Unknown oracle_setup: {other}. Use 'kaminoPythPush' or 'kaminoSwitchboardPull'"),
            };
            let risk_tier = match c.risk_tier.as_deref().unwrap_or("collateral") {
                "isolated" => marginfi_type_crate::types::RiskTier::Isolated,
                _ => marginfi_type_crate::types::RiskTier::Collateral,
            };

            processor::integrations::kamino_add_bank(
                &config,
                processor::integrations::KaminoBankCreateRequest {
                    group,
                    bank_mint: mint,
                    seed,
                    oracle,
                    reserve_oracle: reserve_oracle.unwrap_or(oracle),
                    oracle_setup,
                    kamino_reserve,
                    kamino_market,
                    asset_weight_init: c.asset_weight_init.unwrap_or(0.85),
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
        KaminoCommand::InitObligation {
            config: config_path,
            config_example,
            bank_pk,
            amount,
            reserve_oracle,
        } => {
            if config_example {
                println!("{}", configs::KaminoInitObligationConfig::example_json());
                return Ok(());
            }
            let (bank_pk, amount, reserve_oracle) = if let Some(path) = config_path {
                let c: configs::KaminoInitObligationConfig = configs::load_config(&path)?;
                (
                    configs::parse_pubkey(&c.bank_pk)?,
                    c.amount,
                    configs::parse_optional_pubkey(&c.reserve_oracle)?,
                )
            } else {
                (
                    require_field!(bank_pk, "bank-pk"),
                    require_field!(amount, "amount"),
                    reserve_oracle,
                )
            };
            let derived = derive_kamino_accounts(&config, bank_pk, reserve_oracle)?;
            processor::integrations::kamino_init_obligation(
                &profile,
                &config,
                bank_pk,
                amount,
                derived.lending_market,
                derived.lending_market_authority,
                derived.reserve_liquidity_supply,
                derived.reserve_collateral_mint,
                derived.reserve_destination_deposit_collateral,
                derived.user_metadata,
                derived.pyth_oracle,
                derived.switchboard_price_oracle,
                derived.switchboard_twap_oracle,
                derived.scope_prices,
                derived.obligation_farm_user_state,
                derived.reserve_farm_state,
            )
        }
        KaminoCommand::Deposit {
            config: config_path,
            config_example,
            bank_pk,
            ui_amount,
        } => {
            if config_example {
                println!("{}", configs::KaminoDepositConfig::example_json());
                return Ok(());
            }
            let (bank_pk, ui_amount) = if let Some(path) = config_path {
                let c: configs::KaminoDepositConfig = configs::load_config(&path)?;
                (configs::parse_pubkey(&c.bank_pk)?, c.ui_amount)
            } else {
                (
                    require_field!(bank_pk, "bank-pk"),
                    require_field!(ui_amount, "ui-amount"),
                )
            };
            let derived = derive_kamino_accounts(&config, bank_pk, None)?;
            processor::integrations::kamino_deposit(
                &profile,
                &config,
                bank_pk,
                ui_amount,
                derived.lending_market,
                derived.lending_market_authority,
                derived.reserve_liquidity_supply,
                derived.reserve_collateral_mint,
                derived.reserve_destination_deposit_collateral,
                derived.obligation_farm_user_state,
                derived.reserve_farm_state,
            )
        }
        KaminoCommand::Withdraw {
            config: config_path,
            config_example,
            bank_pk,
            ui_amount,
            withdraw_all,
        } => {
            if config_example {
                println!("{}", configs::KaminoWithdrawConfig::example_json());
                return Ok(());
            }
            let (bank_pk, ui_amount, withdraw_all) = if let Some(path) = config_path {
                let c: configs::KaminoWithdrawConfig = configs::load_config(&path)?;
                (
                    configs::parse_pubkey(&c.bank_pk)?,
                    c.ui_amount,
                    c.withdraw_all,
                )
            } else {
                (
                    require_field!(bank_pk, "bank-pk"),
                    ui_amount.unwrap_or(0.0),
                    withdraw_all,
                )
            };
            let derived = derive_kamino_accounts(&config, bank_pk, None)?;
            processor::integrations::kamino_withdraw(
                &profile,
                &config,
                bank_pk,
                ui_amount,
                withdraw_all,
                derived.lending_market,
                derived.lending_market_authority,
                derived.reserve_liquidity_supply,
                derived.reserve_collateral_mint,
                derived.reserve_source_collateral,
                derived.obligation_farm_user_state,
                derived.reserve_farm_state,
            )
        }
        KaminoCommand::HarvestReward {
            config: config_path,
            config_example,
            bank_pk,
            reward_index,
            global_config,
            reward_mint,
            scope_prices,
        } => {
            if config_example {
                println!("{}", configs::KaminoHarvestRewardConfig::example_json());
                return Ok(());
            }
            let (bank_pk, reward_index, global_config, reward_mint, scope_prices) =
                if let Some(path) = config_path {
                    let c: configs::KaminoHarvestRewardConfig = configs::load_config(&path)?;
                    (
                        configs::parse_pubkey(&c.bank_pk)?,
                        c.reward_index,
                        configs::parse_pubkey(&c.global_config)?,
                        configs::parse_pubkey(&c.reward_mint)?,
                        configs::parse_optional_pubkey(&c.scope_prices)?,
                    )
                } else {
                    (
                        require_field!(bank_pk, "bank-pk"),
                        require_field!(reward_index, "reward-index"),
                        require_field!(global_config, "global-config"),
                        require_field!(reward_mint, "reward-mint"),
                        scope_prices,
                    )
                };
            let derived = derive_kamino_harvest_reward_accounts(
                &config,
                bank_pk,
                global_config,
                reward_mint,
            )?;
            processor::integrations::kamino_harvest_reward(
                &config,
                bank_pk,
                reward_index,
                derived.user_state,
                derived.farm_state,
                global_config,
                reward_mint,
                derived.user_reward_ata,
                derived.rewards_vault,
                derived.rewards_treasury_vault,
                derived.farm_vaults_authority,
                scope_prices.or(derived.scope_prices),
            )
        }
    }
}
