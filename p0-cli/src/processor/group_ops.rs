use anyhow::Result;
use log::info;
use marginfi_type_crate::types::{Bank, MarginfiGroup};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_sdk::pubkey::Pubkey;
use std::mem::size_of;

use crate::{config::Config, output};

pub fn group_get(config: Config, marginfi_group: Option<Pubkey>) -> Result<()> {
    let json = config.json_output;
    if let Some(marginfi_group) = marginfi_group {
        let group: MarginfiGroup = config.mfi_program.account(marginfi_group)?;
        if json {
            let banks = load_all_banks(&config, Some(marginfi_group))?;
            let val = serde_json::json!({
                "group": output::group_detail_json(&marginfi_group, &group),
                "banks": output::banks_table_json(&banks),
            });
            println!("{}", serde_json::to_string_pretty(&val)?);
        } else {
            output::print_group_detail(&marginfi_group, &group, false);
            println!("--------\nBanks:");
            print_group_banks(config, marginfi_group)?;
        }
    } else {
        group_get_all(config)?;
    }
    Ok(())
}

pub fn group_get_all(config: Config) -> Result<()> {
    let json = config.json_output;
    let accounts: Vec<(Pubkey, MarginfiGroup)> = config.mfi_program.accounts(vec![])?;

    if json {
        let vals = accounts
            .iter()
            .map(|(address, group)| output::group_detail_json(address, group))
            .collect::<Vec<_>>();
        println!("{}", serde_json::to_string_pretty(&vals)?);
    } else {
        for (address, group) in &accounts {
            output::print_group_detail(address, group, false);
        }
    }

    Ok(())
}

pub fn print_group_banks(config: Config, marginfi_group: Pubkey) -> Result<()> {
    let json = config.json_output;
    let banks = config
        .mfi_program
        .accounts::<Bank>(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            marginfi_group.to_bytes().to_vec(),
        ))])?;

    output::print_banks_table(&banks, json);

    Ok(())
}

pub fn load_all_banks(
    config: &Config,
    marginfi_group: Option<Pubkey>,
) -> Result<Vec<(Pubkey, Bank)>> {
    info!("Loading banks for group {:?}", marginfi_group);
    let filters = match marginfi_group {
        Some(marginfi_group) => vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            marginfi_group.to_bytes().to_vec(),
        ))],
        None => vec![],
    };

    let banks_with_addresses = config.mfi_program.accounts::<Bank>(filters)?;

    Ok(banks_with_addresses)
}
