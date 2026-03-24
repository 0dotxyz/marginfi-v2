use anchor_client::Cluster;
use anyhow::Result;
use clap::Parser;
use solana_sdk::{commitment_config::CommitmentLevel, pubkey::Pubkey};

use crate::processor;

/// CLI profile management.
#[derive(Debug, Parser)]
#[clap(
    after_help = "Common subcommands:\n  mfi profile create --name mainnet --cluster mainnet --keypair-path ~/.config/solana/id.json --rpc-url https://api.mainnet-beta.solana.com\n  mfi profile show\n  mfi profile list\n  mfi profile set mainnet\n  mfi profile update mainnet --group <GROUP_PUBKEY> --account <ACCOUNT_PUBKEY>",
    after_long_help = "Common subcommands:\n  mfi profile create --name mainnet --cluster mainnet --keypair-path ~/.config/solana/id.json --rpc-url https://api.mainnet-beta.solana.com\n  mfi profile show\n  mfi profile list\n  mfi profile set mainnet\n  mfi profile update mainnet --group <GROUP_PUBKEY> --account <ACCOUNT_PUBKEY>"
)]
pub enum ProfileCommand {
    /// Create a new CLI profile
    ///
    /// Example: `mfi profile create --name mainnet --cluster mainnet --keypair-path ~/.config/solana/id.json --rpc-url https://api.mainnet-beta.solana.com`
    #[clap(
        after_help = "Example:\n  mfi profile create --name mainnet --cluster mainnet --keypair-path ~/.config/solana/id.json --rpc-url https://api.mainnet-beta.solana.com",
        after_long_help = "Example:\n  mfi profile create --name mainnet --cluster mainnet --keypair-path ~/.config/solana/id.json --rpc-url https://api.mainnet-beta.solana.com"
    )]
    Create {
        #[clap(long)]
        name: String,
        #[clap(long)]
        cluster: Cluster,
        #[clap(long)]
        keypair_path: String,
        #[clap(long)]
        multisig: Option<Pubkey>,
        #[clap(long)]
        rpc_url: String,
        #[clap(long)]
        program_id: Option<Pubkey>,
        #[clap(long)]
        commitment: Option<CommitmentLevel>,
        #[clap(long)]
        group: Option<Pubkey>,
        #[clap(long)]
        account: Option<Pubkey>,
    },
    /// Show the active profile, or a named profile if provided
    ///
    /// Example: `mfi profile show`
    #[clap(
        after_help = "Example:\n  mfi profile show",
        after_long_help = "Example:\n  mfi profile show"
    )]
    Show { name: Option<String> },
    /// List all profiles
    ///
    /// Example: `mfi profile list`
    #[clap(
        after_help = "Example:\n  mfi profile list",
        after_long_help = "Example:\n  mfi profile list"
    )]
    List,
    /// Switch to a different profile
    ///
    /// Example: `mfi profile set mainnet`
    #[clap(
        after_help = "Example:\n  mfi profile set mainnet",
        after_long_help = "Example:\n  mfi profile set mainnet"
    )]
    Set { name: String },
    /// Update an existing profile's settings
    ///
    /// Example: `mfi profile update mainnet --group <GROUP_PUBKEY> --account <ACCOUNT_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi profile update mainnet --group <GROUP_PUBKEY> --account <ACCOUNT_PUBKEY>",
        after_long_help = "Example:\n  mfi profile update mainnet --group <GROUP_PUBKEY> --account <ACCOUNT_PUBKEY>"
    )]
    Update {
        name: String,
        #[clap(long)]
        new_name: Option<String>,
        #[clap(long)]
        cluster: Option<Cluster>,
        #[clap(long)]
        keypair_path: Option<String>,
        #[clap(long)]
        multisig: Option<Pubkey>,
        #[clap(long)]
        rpc_url: Option<String>,
        #[clap(long)]
        program_id: Option<Pubkey>,
        #[clap(long)]
        commitment: Option<CommitmentLevel>,
        #[clap(long)]
        group: Option<Pubkey>,
        #[clap(long)]
        account: Option<Pubkey>,
    },
    /// Delete a profile
    ///
    /// Example: `mfi profile delete old-profile`
    #[clap(
        after_help = "Example:\n  mfi profile delete old-profile",
        after_long_help = "Example:\n  mfi profile delete old-profile"
    )]
    Delete { name: String },
}

pub fn dispatch(subcmd: ProfileCommand) -> Result<()> {
    match subcmd {
        ProfileCommand::Create {
            name,
            cluster,
            keypair_path,
            multisig,
            rpc_url,
            program_id,
            commitment,
            group,
            account,
        } => processor::create_profile(
            name,
            cluster,
            keypair_path,
            multisig,
            rpc_url,
            program_id,
            commitment,
            group,
            account,
        ),
        ProfileCommand::Show { name } => processor::show_profile(name),
        ProfileCommand::List => processor::list_profiles(),
        ProfileCommand::Set { name } => processor::set_profile(name),
        ProfileCommand::Update {
            cluster,
            keypair_path,
            multisig,
            rpc_url,
            program_id,
            commitment,
            group,
            name,
            new_name,
            account,
        } => processor::configure_profile(
            name,
            new_name,
            cluster,
            keypair_path,
            multisig,
            rpc_url,
            program_id,
            commitment,
            group,
            account,
        ),
        ProfileCommand::Delete { name } => processor::delete_profile(name),
    }
}
