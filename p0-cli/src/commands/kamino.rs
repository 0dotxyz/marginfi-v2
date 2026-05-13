use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use kamino_mocks::state::MinimalReserve;
use marginfi::state::bank::BankVaultType;
use marginfi_type_crate::{
    pdas::{
        derive_kamino_farm_vaults_authority, derive_kamino_lending_market_authority,
        derive_kamino_reserve_collateral_mint, derive_kamino_reserve_collateral_supply,
        derive_kamino_reserve_liquidity_supply, derive_kamino_rewards_treasury_vault,
        derive_kamino_rewards_vault, derive_kamino_user_metadata, derive_kamino_user_state,
    },
    types::{Bank, OracleSetup},
};
use solana_sdk::pubkey::Pubkey;

use super::require_field;
use crate::config::{Config, GlobalOptions};
use crate::configs;
use crate::processor;
use crate::utils::find_bank_vault_authority_pda;

/// Kamino integration commands (user / permissionless).
#[derive(Debug, Parser)]
pub enum KaminoCommand {
    /// Initialize a Kamino obligation for a bank's reserve (permissionless)
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
    Deposit {
        #[clap(long, help = "Path to JSON config file")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        bank_pk: Option<Pubkey>,
        ui_amount: Option<f64>,
    },
    /// Withdraw from a Kamino reserve via marginfi
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
    /// Harvest Kamino farm rewards to the protocol fee wallet (permissionless)
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
    let (reserve_liquidity_supply, _) = derive_kamino_reserve_liquidity_supply(&reserve);
    let (reserve_collateral_mint, _) = derive_kamino_reserve_collateral_mint(&reserve);
    let (reserve_destination_deposit_collateral, _) =
        derive_kamino_reserve_collateral_supply(&reserve);
    let (user_metadata, _) = derive_kamino_user_metadata(&liquidity_vault_authority);

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
    let (farm_vaults_authority, _) = derive_kamino_farm_vaults_authority(&farm_state);
    let (rewards_vault, _) = derive_kamino_rewards_vault(&farm_state, &reward_mint);
    let (rewards_treasury_vault, _) =
        derive_kamino_rewards_treasury_vault(&global_config, &reward_mint);
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
        KaminoCommand::InitObligation {
            config: config_path,
            bank_pk,
            amount,
            reserve_oracle,
            ..
        } => {
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
            bank_pk,
            ui_amount,
            ..
        } => {
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
            bank_pk,
            ui_amount,
            withdraw_all,
            ..
        } => {
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
            bank_pk,
            reward_index,
            global_config,
            reward_mint,
            scope_prices,
            ..
        } => {
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
