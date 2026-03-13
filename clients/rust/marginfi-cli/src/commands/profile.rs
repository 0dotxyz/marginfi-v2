use anchor_client::Cluster;
use anyhow::Result;
use clap::Parser;
use solana_sdk::{commitment_config::CommitmentLevel, pubkey::Pubkey};

use crate::processor;

/// CLI profile management (RPC endpoints, keypairs, default accounts).
#[derive(Debug, Parser)]
pub enum ProfileCommand {
    /// Create a new CLI profile
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
    /// Show the active profile
    Show,
    /// List all profiles
    List,
    /// Switch to a different profile
    Set { name: String },
    /// Update an existing profile's settings
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
        ProfileCommand::Show => processor::show_profile(),
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
