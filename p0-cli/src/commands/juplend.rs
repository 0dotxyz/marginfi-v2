use anyhow::Result;
use clap::Parser;
use solana_sdk::pubkey::Pubkey;

use crate::config::GlobalOptions;
use crate::processor;

/// JupLend integration commands (user / permissionless).
#[derive(Debug, Parser)]
pub enum JuplendCommand {
    /// Initialize a JupLend position for a bank (permissionless)
    InitPosition {
        bank_pk: Pubkey,
        #[clap(long, help = "Native amount for seed deposit (minimum 10)")]
        amount: u64,
    },
    /// Deposit into JupLend via marginfi
    Deposit { bank_pk: Pubkey, ui_amount: f64 },
    /// Withdraw from JupLend via marginfi
    Withdraw {
        bank_pk: Pubkey,
        ui_amount: f64,
        #[clap(short = 'a', long = "all")]
        withdraw_all: bool,
    },
}

pub fn dispatch(subcmd: JuplendCommand, global_options: &GlobalOptions) -> Result<()> {
    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        super::get_consent(&subcmd, &profile)?;
    }

    match subcmd {
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
