use anyhow::{Context, Result};
use clap::Parser;
use solana_sdk::pubkey::Pubkey;

use crate::config::GlobalOptions;
use crate::processor;

/// User-facing group commands. Read state, propagate cached values, recover from a pause.
#[derive(Debug, Parser)]
pub enum GroupCommand {
    /// Display a group and its banks
    Get { marginfi_group: Option<Pubkey> },
    /// List every marginfi group on the cluster
    GetAll {},
    /// Propagate the latest fee-state cache into a group (permissionless)
    PropagateFee {
        #[clap(long)]
        marginfi_group: Option<Pubkey>,
    },
    /// Propagate updated staked settings to a specific staked bank (permissionless)
    PropagateStakedSettings { bank_pk: Pubkey },
    /// Unpause the protocol after the pause window expires (permissionless)
    PanicUnpausePermissionless {},
}

pub fn dispatch(subcmd: GroupCommand, global_options: &GlobalOptions) -> Result<()> {
    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        match subcmd {
            GroupCommand::Get { .. } | GroupCommand::GetAll {} => (),
            _ => super::get_consent(&subcmd, &profile)?,
        }
    }

    match subcmd {
        GroupCommand::Get { marginfi_group } => {
            processor::group_get(config, marginfi_group.or(profile.marginfi_group))
        }
        GroupCommand::GetAll {} => processor::group_get_all(config),
        GroupCommand::PropagateFee { marginfi_group } => processor::propagate_fee(
            config,
            marginfi_group
                .or(profile.marginfi_group)
                .context("--marginfi-group required or set in profile")?,
        ),
        GroupCommand::PropagateStakedSettings { bank_pk } => {
            processor::propagate_staked_settings(config, profile, bank_pk)
        }
        GroupCommand::PanicUnpausePermissionless {} => {
            processor::panic_unpause_permissionless(config)
        }
    }
}
