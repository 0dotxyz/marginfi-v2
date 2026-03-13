use anyhow::Result;
use clap::Parser;
use solana_sdk::pubkey::Pubkey;

use crate::config::GlobalOptions;
use crate::processor;

/// DeFi protocol integration commands (Kamino, Drift, JupLend).
#[derive(Debug, Parser)]
pub enum IntegrationCommand {
    // ---- Kamino ----
    /// Initialize a Kamino obligation for a bank's reserve
    KaminoInitObligation {
        bank_pk: Pubkey,
        #[clap(long, help = "Native amount for seed deposit (minimum 10)")]
        amount: u64,
        #[clap(long)]
        lending_market: Pubkey,
        #[clap(long)]
        lending_market_authority: Pubkey,
        #[clap(long)]
        reserve_liquidity_supply: Pubkey,
        #[clap(long)]
        reserve_collateral_mint: Pubkey,
        #[clap(long)]
        reserve_destination_deposit_collateral: Pubkey,
        #[clap(long)]
        user_metadata: Pubkey,
        #[clap(long)]
        pyth_oracle: Option<Pubkey>,
        #[clap(long)]
        switchboard_price_oracle: Option<Pubkey>,
        #[clap(long)]
        switchboard_twap_oracle: Option<Pubkey>,
        #[clap(long)]
        scope_prices: Option<Pubkey>,
        #[clap(long)]
        obligation_farm_user_state: Option<Pubkey>,
        #[clap(long)]
        reserve_farm_state: Option<Pubkey>,
    },
    /// Deposit into a Kamino reserve via marginfi
    KaminoDeposit {
        bank_pk: Pubkey,
        ui_amount: f64,
        #[clap(long)]
        lending_market: Pubkey,
        #[clap(long)]
        lending_market_authority: Pubkey,
        #[clap(long)]
        reserve_liquidity_supply: Pubkey,
        #[clap(long)]
        reserve_collateral_mint: Pubkey,
        #[clap(long)]
        reserve_destination_deposit_collateral: Pubkey,
        #[clap(long)]
        obligation_farm_user_state: Option<Pubkey>,
        #[clap(long)]
        reserve_farm_state: Option<Pubkey>,
    },
    /// Withdraw from a Kamino reserve via marginfi
    KaminoWithdraw {
        bank_pk: Pubkey,
        ui_amount: f64,
        #[clap(short = 'a', long = "all")]
        withdraw_all: bool,
        #[clap(long)]
        lending_market: Pubkey,
        #[clap(long)]
        lending_market_authority: Pubkey,
        #[clap(long)]
        reserve_liquidity_supply: Pubkey,
        #[clap(long)]
        reserve_collateral_mint: Pubkey,
        #[clap(long)]
        reserve_source_collateral: Pubkey,
        #[clap(long)]
        obligation_farm_user_state: Option<Pubkey>,
        #[clap(long)]
        reserve_farm_state: Option<Pubkey>,
    },
    /// Harvest Kamino farm rewards
    KaminoHarvestReward {
        bank_pk: Pubkey,
        #[clap(long)]
        reward_index: u64,
        #[clap(long)]
        user_state: Pubkey,
        #[clap(long)]
        farm_state: Pubkey,
        #[clap(long)]
        global_config: Pubkey,
        #[clap(long)]
        reward_mint: Pubkey,
        #[clap(long)]
        user_reward_ata: Pubkey,
        #[clap(long)]
        rewards_vault: Pubkey,
        #[clap(long)]
        rewards_treasury_vault: Pubkey,
        #[clap(long)]
        farm_vaults_authority: Pubkey,
        #[clap(long)]
        scope_prices: Option<Pubkey>,
    },

    // ---- Drift ----
    /// Initialize a Drift user account for a bank
    DriftInitUser {
        bank_pk: Pubkey,
        #[clap(long, help = "Native amount for seed deposit (minimum 10)")]
        amount: u64,
        #[clap(long)]
        drift_state: Pubkey,
        #[clap(long)]
        drift_spot_market_vault: Pubkey,
        #[clap(long)]
        drift_oracle: Option<Pubkey>,
    },
    /// Deposit into Drift via marginfi
    DriftDeposit {
        bank_pk: Pubkey,
        ui_amount: f64,
        #[clap(long)]
        drift_state: Pubkey,
        #[clap(long)]
        drift_spot_market_vault: Pubkey,
        #[clap(long)]
        drift_oracle: Option<Pubkey>,
    },
    /// Withdraw from Drift via marginfi
    DriftWithdraw {
        bank_pk: Pubkey,
        ui_amount: f64,
        #[clap(short = 'a', long = "all")]
        withdraw_all: bool,
        #[clap(long)]
        drift_state: Pubkey,
        #[clap(long)]
        drift_spot_market_vault: Pubkey,
        #[clap(long)]
        drift_signer: Pubkey,
        #[clap(long)]
        drift_oracle: Option<Pubkey>,
        #[clap(long)]
        drift_reward_oracle: Option<Pubkey>,
        #[clap(long)]
        drift_reward_spot_market: Option<Pubkey>,
        #[clap(long)]
        drift_reward_mint: Option<Pubkey>,
        #[clap(long)]
        drift_reward_oracle_2: Option<Pubkey>,
        #[clap(long)]
        drift_reward_spot_market_2: Option<Pubkey>,
        #[clap(long)]
        drift_reward_mint_2: Option<Pubkey>,
    },
    /// Harvest Drift spot market rewards
    DriftHarvestReward {
        bank_pk: Pubkey,
        #[clap(long)]
        drift_state: Pubkey,
        #[clap(long)]
        drift_signer: Pubkey,
        #[clap(long)]
        harvest_drift_spot_market: Pubkey,
        #[clap(long)]
        harvest_drift_spot_market_vault: Pubkey,
        #[clap(long)]
        reward_mint: Pubkey,
    },

    // ---- JupLend ----
    /// Initialize a JupLend position for a bank (all CPI accounts auto-derived)
    JuplendInitPosition {
        bank_pk: Pubkey,
        #[clap(long, help = "Native amount for seed deposit (minimum 10)")]
        amount: u64,
    },
    /// Deposit into JupLend via marginfi (all CPI accounts auto-derived)
    JuplendDeposit {
        bank_pk: Pubkey,
        ui_amount: f64,
    },
    /// Withdraw from JupLend via marginfi (all CPI accounts auto-derived)
    JuplendWithdraw {
        bank_pk: Pubkey,
        ui_amount: f64,
        #[clap(short = 'a', long = "all")]
        withdraw_all: bool,
    },
}

pub fn dispatch(subcmd: IntegrationCommand, global_options: &GlobalOptions) -> Result<()> {
    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        super::get_consent(&subcmd, &profile)?;
    }

    match subcmd {
        // ---- Kamino ----
        IntegrationCommand::KaminoInitObligation {
            bank_pk,
            amount,
            lending_market,
            lending_market_authority,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_destination_deposit_collateral,
            user_metadata,
            pyth_oracle,
            switchboard_price_oracle,
            switchboard_twap_oracle,
            scope_prices,
            obligation_farm_user_state,
            reserve_farm_state,
        } => processor::integrations::kamino_init_obligation(
            &profile,
            &config,
            bank_pk,
            amount,
            lending_market,
            lending_market_authority,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_destination_deposit_collateral,
            user_metadata,
            pyth_oracle,
            switchboard_price_oracle,
            switchboard_twap_oracle,
            scope_prices,
            obligation_farm_user_state,
            reserve_farm_state,
        ),
        IntegrationCommand::KaminoDeposit {
            bank_pk,
            ui_amount,
            lending_market,
            lending_market_authority,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_destination_deposit_collateral,
            obligation_farm_user_state,
            reserve_farm_state,
        } => processor::integrations::kamino_deposit(
            &profile,
            &config,
            bank_pk,
            ui_amount,
            lending_market,
            lending_market_authority,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_destination_deposit_collateral,
            obligation_farm_user_state,
            reserve_farm_state,
        ),
        IntegrationCommand::KaminoWithdraw {
            bank_pk,
            ui_amount,
            withdraw_all,
            lending_market,
            lending_market_authority,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_source_collateral,
            obligation_farm_user_state,
            reserve_farm_state,
        } => processor::integrations::kamino_withdraw(
            &profile,
            &config,
            bank_pk,
            ui_amount,
            withdraw_all,
            lending_market,
            lending_market_authority,
            reserve_liquidity_supply,
            reserve_collateral_mint,
            reserve_source_collateral,
            obligation_farm_user_state,
            reserve_farm_state,
        ),
        IntegrationCommand::KaminoHarvestReward {
            bank_pk,
            reward_index,
            user_state,
            farm_state,
            global_config,
            reward_mint,
            user_reward_ata,
            rewards_vault,
            rewards_treasury_vault,
            farm_vaults_authority,
            scope_prices,
        } => processor::integrations::kamino_harvest_reward(
            &config,
            bank_pk,
            reward_index,
            user_state,
            farm_state,
            global_config,
            reward_mint,
            user_reward_ata,
            rewards_vault,
            rewards_treasury_vault,
            farm_vaults_authority,
            scope_prices,
        ),

        // ---- Drift ----
        IntegrationCommand::DriftInitUser {
            bank_pk,
            amount,
            drift_state,
            drift_spot_market_vault,
            drift_oracle,
        } => processor::integrations::drift_init_user(
            &profile,
            &config,
            bank_pk,
            amount,
            drift_state,
            drift_spot_market_vault,
            drift_oracle,
        ),
        IntegrationCommand::DriftDeposit {
            bank_pk,
            ui_amount,
            drift_state,
            drift_spot_market_vault,
            drift_oracle,
        } => processor::integrations::drift_deposit(
            &profile,
            &config,
            bank_pk,
            ui_amount,
            drift_state,
            drift_spot_market_vault,
            drift_oracle,
        ),
        IntegrationCommand::DriftWithdraw {
            bank_pk,
            ui_amount,
            withdraw_all,
            drift_state,
            drift_spot_market_vault,
            drift_signer,
            drift_oracle,
            drift_reward_oracle,
            drift_reward_spot_market,
            drift_reward_mint,
            drift_reward_oracle_2,
            drift_reward_spot_market_2,
            drift_reward_mint_2,
        } => processor::integrations::drift_withdraw(
            &profile,
            &config,
            bank_pk,
            ui_amount,
            withdraw_all,
            drift_state,
            drift_spot_market_vault,
            drift_oracle,
            drift_signer,
            drift_reward_oracle,
            drift_reward_spot_market,
            drift_reward_mint,
            drift_reward_oracle_2,
            drift_reward_spot_market_2,
            drift_reward_mint_2,
        ),
        IntegrationCommand::DriftHarvestReward {
            bank_pk,
            drift_state,
            drift_signer,
            harvest_drift_spot_market,
            harvest_drift_spot_market_vault,
            reward_mint,
        } => processor::integrations::drift_harvest_reward(
            &config,
            bank_pk,
            drift_state,
            drift_signer,
            harvest_drift_spot_market,
            harvest_drift_spot_market_vault,
            reward_mint,
        ),

        // ---- JupLend (all CPI accounts auto-derived from bank) ----
        IntegrationCommand::JuplendInitPosition { bank_pk, amount } => {
            processor::integrations::juplend_init_position(&profile, &config, bank_pk, amount)
        }
        IntegrationCommand::JuplendDeposit { bank_pk, ui_amount } => {
            processor::integrations::juplend_deposit(&profile, &config, bank_pk, ui_amount)
        }
        IntegrationCommand::JuplendWithdraw {
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
