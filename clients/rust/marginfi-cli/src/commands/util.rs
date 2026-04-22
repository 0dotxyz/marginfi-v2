use anyhow::{anyhow, Result};
use clap::Parser;
use fixed::types::I80F48;
use rand::Rng;
use solana_sdk::pubkey::Pubkey;
use std::path::PathBuf;

use marginfi_type_crate::types::{
    Balance, Bank, BankConfig, BankConfigOpt, InterestRateConfig, LendingAccount, MarginfiAccount,
    MarginfiGroup, WrappedI80F48,
};
use pyth_solana_receiver_sdk::price_update::get_feed_id_from_hex;

use crate::config::GlobalOptions;
use crate::configs;
use crate::processor;
use crate::processor::oracle::find_pyth_push_oracles_for_feed_id;

/// Debug and utility commands.
#[derive(Debug, Parser)]
#[clap(
    after_help = "Common subcommands:\n  mfi util inspect-size\n  mfi util make-test-i80f48\n  mfi util show-oracle-ages --group <GROUP_PUBKEY> --only-stale\n  mfi util inspect-pyth-push-oracle-feed <PYTH_FEED_PUBKEY>\n  mfi util find-pyth-push <PYTH_FEED_ID_HEX>\n  mfi util inspect-swb-pull-feed <SWITCHBOARD_FEED_PUBKEY>\n  mfi util derive-bank-pda --group <GROUP_PUBKEY> --mint <MINT_PUBKEY> --program <PROGRAM_ID> --next-available",
    after_long_help = "Common subcommands:\n  mfi util inspect-size\n  mfi util make-test-i80f48\n  mfi util show-oracle-ages --group <GROUP_PUBKEY> --only-stale\n  mfi util inspect-pyth-push-oracle-feed <PYTH_FEED_PUBKEY>\n  mfi util find-pyth-push <PYTH_FEED_ID_HEX>\n  mfi util inspect-swb-pull-feed <SWITCHBOARD_FEED_PUBKEY>\n  mfi util derive-bank-pda --group <GROUP_PUBKEY> --mint <MINT_PUBKEY> --program <PROGRAM_ID> --next-available"
)]
pub enum UtilCommand {
    /// Print the byte size of core on-chain types
    ///
    /// Example: `mfi util inspect-size`
    #[clap(
        after_help = "Example:\n  mfi util inspect-size",
        after_long_help = "Example:\n  mfi util inspect-size"
    )]
    InspectSize {},
    /// Generate random I80F48 test vectors
    ///
    /// Example: `mfi util make-test-i80f48`
    #[clap(
        after_help = "Example:\n  mfi util make-test-i80f48",
        after_long_help = "Example:\n  mfi util make-test-i80f48"
    )]
    MakeTestI80F48,
    /// Show oracle ages for all banks
    ///
    /// Example: `mfi util show-oracle-ages --group <GROUP_PUBKEY> --only-stale`
    #[clap(
        after_help = "Example:\n  mfi util show-oracle-ages --group <GROUP_PUBKEY> --only-stale",
        after_long_help = "Example:\n  mfi util show-oracle-ages --group <GROUP_PUBKEY> --only-stale"
    )]
    ShowOracleAges {
        #[clap(
            long = "group",
            help = "Group address to inspect. Defaults to the active profile group, then falls back to mainnet group 4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8"
        )]
        marginfi_group: Option<Pubkey>,
        #[clap(long, action)]
        only_stale: bool,
    },
    /// Inspect a Pyth push oracle feed account
    ///
    /// Example: `mfi util inspect-pyth-push-oracle-feed <PYTH_FEED_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi util inspect-pyth-push-oracle-feed <PYTH_FEED_PUBKEY>",
        after_long_help = "Example:\n  mfi util inspect-pyth-push-oracle-feed <PYTH_FEED_PUBKEY>"
    )]
    InspectPythPushOracleFeed { pyth_feed: Pubkey },
    /// Find Pyth push oracle accounts by feed ID hex
    ///
    /// Example: `mfi util find-pyth-push <PYTH_FEED_ID_HEX>`
    #[clap(
        name = "find-pyth-push",
        visible_alias = "find-pyth-pull",
        after_help = "Example:\n  mfi util find-pyth-push <PYTH_FEED_ID_HEX>",
        after_long_help = "Example:\n  mfi util find-pyth-push <PYTH_FEED_ID_HEX>"
    )]
    FindPythPush { feed_id: String },
    /// Inspect a Switchboard pull feed account
    ///
    /// Example: `mfi util inspect-swb-pull-feed <SWITCHBOARD_FEED_PUBKEY>`
    #[clap(
        after_help = "Example:\n  mfi util inspect-swb-pull-feed <SWITCHBOARD_FEED_PUBKEY>",
        after_long_help = "Example:\n  mfi util inspect-swb-pull-feed <SWITCHBOARD_FEED_PUBKEY>"
    )]
    InspectSwbPullFeed { address: Pubkey },
    /// Derive a bank PDA by scanning seeds for a group + mint + program
    ///
    /// Example: `mfi util derive-bank-pda --config ./configs/util/derive-bank-pda/config.json.example`
    #[clap(
        name = "derive-bank-pda",
        after_help = "Examples:\n  mfi util derive-bank-pda --group <GROUP_PUBKEY> --mint <MINT_PUBKEY> --program <PROGRAM_ID> --next-available\n  mfi util derive-bank-pda --group <GROUP_PUBKEY> --mint <MINT_PUBKEY> --program <PROGRAM_ID> --last-used\n  mfi util derive-bank-pda --config ./configs/util/derive-bank-pda/config.json.example",
        after_long_help = "Examples:\n  mfi util derive-bank-pda --group <GROUP_PUBKEY> --mint <MINT_PUBKEY> --program <PROGRAM_ID> --next-available\n  mfi util derive-bank-pda --group <GROUP_PUBKEY> --mint <MINT_PUBKEY> --program <PROGRAM_ID> --last-used\n  mfi util derive-bank-pda --config ./configs/util/derive-bank-pda/config.json.example"
    )]
    DeriveBankPda {
        #[clap(long, help = "Path to JSON config file (see --config-example)")]
        config: Option<PathBuf>,
        #[clap(long, help = "Print an example JSON config and exit", action)]
        config_example: bool,
        #[clap(long, help = "Defaults to the active profile group when omitted")]
        group: Option<Pubkey>,
        #[clap(long)]
        mint: Option<Pubkey>,
        #[clap(long, help = "Defaults to the active profile program when omitted")]
        program: Option<Pubkey>,
        #[clap(long, action)]
        next_available: bool,
        #[clap(long, action)]
        last_used: bool,
    },
}

pub fn dispatch(subcmd: UtilCommand, global_options: &GlobalOptions) -> Result<()> {
    match subcmd {
        UtilCommand::InspectSize {} => inspect_size(),

        UtilCommand::MakeTestI80F48 => {
            process_make_test_i80f48();
            Ok(())
        }

        UtilCommand::ShowOracleAges {
            marginfi_group,
            only_stale,
        } => {
            let (profile, config) = super::load_profile_and_config(global_options)?;

            processor::show_oracle_ages(
                config,
                marginfi_group.or(profile.marginfi_group),
                only_stale,
            )?;

            Ok(())
        }

        UtilCommand::InspectPythPushOracleFeed { pyth_feed } => {
            let (_, config) = super::load_profile_and_config(global_options)?;

            processor::oracle::inspect_pyth_push_feed(&config, pyth_feed)?;

            Ok(())
        }
        UtilCommand::FindPythPush { feed_id } => {
            let (_, config) = super::load_profile_and_config(global_options)?;
            let feed_id = get_feed_id_from_hex(&feed_id)
                .map_err(|err| anyhow!("invalid feed id '{}': {}", feed_id, err))?;

            let rpc = config.mfi_program.rpc();

            find_pyth_push_oracles_for_feed_id(&rpc, feed_id)?;

            Ok(())
        }
        UtilCommand::InspectSwbPullFeed { address } => {
            let (_, config) = super::load_profile_and_config(global_options)?;

            processor::oracle::inspect_swb_pull_feed(&config, address)?;

            Ok(())
        }
        UtilCommand::DeriveBankPda {
            config: config_path,
            config_example,
            group,
            mint,
            program,
            next_available,
            last_used,
        } => {
            if config_example {
                println!("{}", configs::DeriveBankPdaConfig::example_json());
                return Ok(());
            }

            let (profile, config) = super::load_profile_and_config(global_options)?;
            let (group, mint, program, mode) = if let Some(path) = config_path {
                let cfg: configs::DeriveBankPdaConfig = configs::load_config(&path)?;
                (
                    cfg.group
                        .as_deref()
                        .map(configs::parse_pubkey)
                        .transpose()?
                        .or(profile.marginfi_group)
                        .ok_or_else(|| anyhow!("group required in config or active profile"))?,
                    configs::parse_pubkey(&cfg.mint)?,
                    cfg.program
                        .as_deref()
                        .map(configs::parse_pubkey)
                        .transpose()?
                        .unwrap_or(config.program_id),
                    cfg.mode,
                )
            } else {
                let group = group
                    .or(profile.marginfi_group)
                    .ok_or_else(|| anyhow!("--group required (or set a profile group)"))?;
                let mint = mint.ok_or_else(|| anyhow!("--mint required (or use --config)"))?;
                let program = program.unwrap_or(config.program_id);
                let mode = match (next_available, last_used) {
                    (true, false) => "nextAvailable".to_string(),
                    (false, true) => "lastUsed".to_string(),
                    _ => {
                        return Err(anyhow!(
                            "exactly one of --next-available or --last-used is required"
                        ))
                    }
                };
                (group, mint, program, mode)
            };

            match mode.as_str() {
                "nextAvailable" | "next_available" | "next-available" => processor::find_bank_pda(
                    &config,
                    group,
                    mint,
                    program,
                    processor::BankPdaLookupMode::NextAvailable,
                ),
                "lastUsed" | "last_used" | "last-used" => processor::find_bank_pda(
                    &config,
                    group,
                    mint,
                    program,
                    processor::BankPdaLookupMode::LastUsed,
                ),
                other => Err(anyhow!("unknown mode '{}'", other)),
            }
        }
    }
}

fn inspect_size() -> Result<()> {
    use std::mem::size_of;

    println!("MarginfiGroup: {}", size_of::<MarginfiGroup>());
    println!("InterestRateConfig: {}", size_of::<InterestRateConfig>());
    println!("Bank: {}", size_of::<Bank>());
    println!("BankConfig: {}", size_of::<BankConfig>());
    println!("BankConfigOpt: {}", size_of::<BankConfigOpt>());
    println!("WrappedI80F48: {}", size_of::<WrappedI80F48>());

    println!("MarginfiAccount: {}", size_of::<MarginfiAccount>());
    println!("LendingAccount: {}", size_of::<LendingAccount>());
    println!("Balance: {}", size_of::<Balance>());

    Ok(())
}

pub fn process_make_test_i80f48() {
    let mut rng = rand::thread_rng();

    let i80f48s: Vec<I80F48> = (0..30i128)
        .map(|_| {
            let i = rng.gen_range(-1_000_000_000_000i128..1_000_000_000_000i128);
            I80F48::from_num(i) / I80F48::from_num(1_000_000)
        })
        .collect();

    println!("const testCases = [");
    for i80f48 in i80f48s {
        println!(
            "  {{ number: {:?}, innerValue: {:?} }},",
            i80f48,
            WrappedI80F48::from(i80f48).value
        );
    }

    let explicit = vec![
        0.,
        1.,
        -1.,
        0.328934,
        423947246342.487,
        1783921462347640.,
        0.00000000000232,
    ];
    for f in explicit {
        let i80f48 = I80F48::from_num(f);
        println!(
            "  {{ number: {:?}, innerValue: {:?} }},",
            i80f48,
            WrappedI80F48::from(i80f48).value
        );
    }
    println!("];");
}
