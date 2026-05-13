pub mod account;
pub mod bank;
pub mod drift;
pub mod group;
pub mod juplend;
pub mod kamino;
pub mod profile;
pub mod util;

use std::str::FromStr;

use anyhow::Result;
use clap::Parser;
use solana_sdk::pubkey::Pubkey;

use crate::config::{Config, GlobalOptions};
use crate::profile::{load_profile, load_profile_by_name, Profile};

/// Resolve a required CLI field or return a uniform "missing flag" error.
macro_rules! require_field {
    ($val:expr, $name:expr) => {
        $val.ok_or_else(|| anyhow::anyhow!("--{} required (or use --config)", $name))?
    };
}
pub(crate) use require_field;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Parser)]
#[clap(
    version = VERSION,
    about = "P0 CLI",
    after_help = "Main commands:\n  p0 group -h\n  p0 bank -h\n  p0 profile -h\n  p0 account -h\n  p0 kamino -h\n  p0 drift -h\n  p0 juplend -h\n  p0 util -h"
)]
pub struct Opts {
    #[clap(flatten)]
    pub cfg_override: GlobalOptions,
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Debug, Parser)]
pub enum Command {
    /// Read marginfi group state and run permissionless group ops
    Group {
        #[clap(subcommand)]
        subcmd: group::GroupCommand,
    },
    /// Read bank state and run permissionless bank ops
    Bank {
        #[clap(subcommand)]
        subcmd: bank::BankCommand,
    },
    /// Manage CLI profiles (create, switch, update)
    Profile {
        #[clap(subcommand)]
        subcmd: profile::ProfileCommand,
    },
    /// Manage marginfi accounts (deposit, withdraw, borrow, repay, liquidate, orders)
    Account {
        #[clap(subcommand)]
        subcmd: account::AccountCommand,
    },
    /// Kamino integration (init-obligation, deposit, withdraw, harvest)
    Kamino {
        #[clap(subcommand)]
        subcmd: kamino::KaminoCommand,
    },
    /// Drift integration (init-user, deposit, withdraw, harvest)
    Drift {
        #[clap(subcommand)]
        subcmd: drift::DriftCommand,
    },
    /// JupLend integration (init-position, deposit, withdraw)
    Juplend {
        #[clap(subcommand)]
        subcmd: juplend::JuplendCommand,
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
        Command::Kamino { subcmd } => kamino::dispatch(subcmd, &opts.cfg_override),
        Command::Drift { subcmd } => drift::dispatch(subcmd, &opts.cfg_override),
        Command::Juplend { subcmd } => juplend::dispatch(subcmd, &opts.cfg_override),
        Command::Util { subcmd } => util::dispatch(subcmd, &opts.cfg_override),
    }
}

/// Prompt for an interactive confirmation. The user must type the active profile name to proceed.
pub fn get_consent<T: std::fmt::Debug>(cmd: T, profile: &Profile) -> Result<()> {
    let mut input = String::new();
    println!("Command: {cmd:#?}");
    println!("{profile:#?}");
    println!(
        "Type the name of the profile [{}] to continue",
        profile.name
    );
    std::io::stdin().read_line(&mut input)?;
    if input.trim() != profile.name {
        println!("Aborting");
        std::process::exit(1);
    }
    Ok(())
}

/// Parse a bank pubkey from a CLI-supplied string. Symbol lookup is intentionally not supported.
pub fn resolve_bank(input: &str) -> Result<Pubkey> {
    Pubkey::from_str(input).map_err(|_| anyhow::anyhow!("Invalid bank pubkey: {input}"))
}

/// Resolve a bank pubkey from a CLI string. The `_group` parameter is kept for future
/// symbol-resolution support; for now every bank must be supplied as a pubkey.
pub fn resolve_bank_for_group(input: &str, _group: Option<Pubkey>) -> Result<Pubkey> {
    resolve_bank(input)
}

pub fn load_profile_for_command(global_options: &GlobalOptions) -> Result<Profile> {
    match global_options.profile_name.as_deref() {
        Some(name) => load_profile_by_name(name),
        None => load_profile(),
    }
}

pub fn load_profile_and_config(global_options: &GlobalOptions) -> Result<(Profile, Config)> {
    let profile = load_profile_for_command(global_options)?;
    let config = profile.get_config(Some(global_options))?;
    Ok((profile, config))
}
