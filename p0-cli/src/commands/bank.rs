use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use solana_sdk::pubkey::Pubkey;

use crate::config::GlobalOptions;
use crate::processor;

/// User-facing bank commands (read state + permissionless ops).
#[derive(Debug, Parser)]
pub enum BankCommand {
    /// Display details for a bank (defaults to profile bank if set)
    Get { bank: Option<String> },
    /// List every bank in a group (defaults to profile group)
    GetAll { marginfi_group: Option<Pubkey> },
    /// Show current oracle price and metadata for a bank
    InspectPriceOracle { bank_pk: String },
    /// Collect accrued protocol fees from a bank (permissionless)
    CollectFees { bank: String },
    /// Trigger interest accrual on a bank (permissionless)
    AccrueInterest { bank_pk: String },
    /// Refresh the cached oracle price for a bank (permissionless)
    PulsePriceCache { bank_pk: String },
    /// Withdraw collected fees to the bank's pre-configured destination (permissionless)
    WithdrawFeesPermissionless {
        bank_pk: String,
        #[clap(long)]
        amount: u64,
    },
    /// Initialize the on-chain metadata PDA for a bank (permissionless rent payment)
    InitMetadata { bank_pk: String },
    /// Dump bank metadata PDAs and decoded on-chain metadata to a local JSON file
    DumpMetadata {
        #[clap(long, help = "Optional group to filter source banks by")]
        group: Option<Pubkey>,
        #[clap(
            long,
            help = "Metadata source URL",
            default_value = "https://app.0.xyz/api/banks/db"
        )]
        url: String,
        #[clap(
            long,
            help = "Output JSON path",
            default_value = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/mainnet_metadata_dump.json"
            )
        )]
        out: PathBuf,
        #[clap(long, help = "Optional max banks to dump after filtering")]
        limit: Option<usize>,
    },
}

pub fn dispatch(subcmd: BankCommand, global_options: &GlobalOptions) -> Result<()> {
    let (profile, config) = super::load_profile_and_config(global_options)?;

    if !global_options.skip_confirmation {
        match subcmd {
            BankCommand::Get { .. }
            | BankCommand::GetAll { .. }
            | BankCommand::InspectPriceOracle { .. }
            | BankCommand::DumpMetadata { .. } => (),
            _ => super::get_consent(&subcmd, &profile)?,
        }
    }

    match subcmd {
        BankCommand::Get { bank } => {
            let bank_pk = bank
                .as_deref()
                .map(|value| super::resolve_bank_for_group(value, profile.marginfi_group))
                .transpose()?;
            processor::bank_get(config, bank_pk)
        }
        BankCommand::GetAll { marginfi_group } => {
            processor::bank_get_all(config, marginfi_group.or(profile.marginfi_group))
        }
        BankCommand::InspectPriceOracle { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_inspect_price_oracle(config, bank_pk)
        }
        BankCommand::CollectFees { bank } => {
            let bank_pk = super::resolve_bank_for_group(&bank, profile.marginfi_group)?;
            processor::admin::process_collect_fees(config, bank_pk)
        }
        BankCommand::AccrueInterest { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_accrue_interest(config, bank_pk)
        }
        BankCommand::PulsePriceCache { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_pulse_price_cache(config, bank_pk)
        }
        BankCommand::WithdrawFeesPermissionless { bank_pk, amount } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_withdraw_fees_permissionless(config, bank_pk, amount)
        }
        BankCommand::InitMetadata { bank_pk } => {
            let bank_pk = super::resolve_bank_for_group(&bank_pk, profile.marginfi_group)?;
            processor::bank_init_metadata(config, bank_pk)
        }
        BankCommand::DumpMetadata {
            group,
            url,
            out,
            limit,
        } => processor::dump_bank_metadata(config, group, Some(url), out, limit),
    }
}
