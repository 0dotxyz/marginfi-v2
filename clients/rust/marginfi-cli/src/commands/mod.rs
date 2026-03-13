pub mod account;
pub mod bank;
pub mod group;
pub mod integration;
pub mod profile;
pub mod util;

pub use group::RatePointArg;

use std::str::FromStr;

use anyhow::Result;
use clap::Parser;
use solana_sdk::pubkey::Pubkey;

use crate::config::Config;
use crate::config::GlobalOptions;
use crate::profile::{load_profile, Profile};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Top-level CLI options for the marginfi CLI.
#[derive(Debug, Parser)]
#[clap(version = VERSION, about = "marginfi protocol CLI")]
pub struct Opts {
    #[clap(flatten)]
    pub cfg_override: GlobalOptions,
    #[clap(subcommand)]
    pub command: Command,
}

/// Top-level command groups.
#[derive(Debug, Parser)]
pub enum Command {
    /// Manage marginfi groups (create, configure, add banks, fees)
    Group {
        #[clap(subcommand)]
        subcmd: group::GroupCommand,
    },
    /// Manage banks (get info, configure, oracle, fees)
    Bank {
        #[clap(subcommand)]
        subcmd: bank::BankCommand,
    },
    /// Manage CLI profiles (create, switch, update)
    Profile {
        #[clap(subcommand)]
        subcmd: profile::ProfileCommand,
    },
    /// Manage marginfi accounts (deposit, withdraw, borrow, repay, orders)
    Account {
        #[clap(subcommand)]
        subcmd: account::AccountCommand,
    },
    /// DeFi integration commands (Kamino, Drift, JupLend)
    Integration {
        #[clap(subcommand)]
        subcmd: integration::IntegrationCommand,
    },
    /// Debug and utility commands
    Util {
        #[clap(subcommand)]
        subcmd: util::UtilCommand,
    },
}

pub fn entry(opts: Opts) -> Result<()> {
    env_logger::init();
    match opts.command {
        Command::Group { subcmd } => group::dispatch(subcmd, &opts.cfg_override),
        Command::Bank { subcmd } => bank::dispatch(subcmd, &opts.cfg_override),
        Command::Profile { subcmd } => profile::dispatch(subcmd),
        Command::Account { subcmd } => account::dispatch(subcmd, &opts.cfg_override),
        Command::Integration { subcmd } => integration::dispatch(subcmd, &opts.cfg_override),
        Command::Util { subcmd } => util::dispatch(subcmd, &opts.cfg_override),
    }
}

pub fn get_consent<T: std::fmt::Debug>(cmd: T, profile: &Profile) -> Result<()> {
    let mut input = String::new();
    println!("Command: {cmd:#?}");
    println!("{profile:#?}");
    println!(
        "Type the name of the profile [{}] to continue",
        profile.name.clone()
    );
    std::io::stdin().read_line(&mut input)?;
    if input.trim() != profile.name {
        println!("Aborting");
        std::process::exit(1);
    }

    Ok(())
}

pub fn resolve_bank(input: &str) -> Result<Pubkey> {
    Pubkey::from_str(input)
        .map_err(|_| anyhow::anyhow!("Invalid bank pubkey: {input}"))
}

pub fn resolve_bank_for_group(input: &str, _group: Option<Pubkey>) -> Result<Pubkey> {
    resolve_bank(input)
}

pub fn load_profile_and_config(global_options: &GlobalOptions) -> Result<(Profile, Config)> {
    let profile = load_profile()?;
    let config = profile.get_config(Some(global_options))?;
    Ok((profile, config))
}
