use crate::{config::Config, profile::Profile, utils};
use anyhow::{Context, Result};
use log::{debug, info, warn};
use marginfi::state::bank::BankVaultType;
use marginfi_type_crate::types::Bank;
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_sdk::address_lookup_table::instruction::{create_lookup_table, extend_lookup_table};
use solana_sdk::address_lookup_table::state::AddressLookupTable;
use solana_sdk::{
    account::Account, pubkey::Pubkey, signer::Signer, system_program, transaction::Transaction,
};
use std::mem::size_of;

const CHUNK_SIZE: usize = 22;
const KEY_BATCH_SIZE: usize = 20;

pub fn process_check_lookup_tables(
    config: &Config,
    profile: &Profile,
    existing_lookup_tables: Vec<Pubkey>,
) -> Result<()> {
    let rpc = config.mfi_program.rpc();
    let marginfi_group = profile
        .marginfi_group
        .context("marginfi group not set in profile")?;

    let mut accounts: Vec<Account> = vec![];

    for chunk in existing_lookup_tables.chunks(CHUNK_SIZE) {
        let accounts_2: Vec<Account> = rpc
            .get_multiple_accounts(chunk)?
            .into_iter()
            .flatten()
            .collect();

        accounts.extend(accounts_2);
    }

    let lookup_tables: Vec<AddressLookupTable> = accounts
        .iter_mut()
        .zip(existing_lookup_tables.iter())
        .map(|(account, address)| {
            let lookup_table = AddressLookupTable::deserialize(&account.data).map_err(|e| {
                anyhow::anyhow!("failed to deserialize lookup table {}: {}", address, e)
            })?;
            println!(
                "Loaded table {} with {} addresses",
                address,
                lookup_table.addresses.len()
            );

            if lookup_table.meta.authority != Some(config.authority()) {
                println!(
                    "Lookup table {} has wrong authority {:?}",
                    address, lookup_table.meta.authority,
                );
            }

            Ok(lookup_table)
        })
        .collect::<Result<Vec<_>>>()?;

    let banks = config
        .mfi_program
        .accounts::<Bank>(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            marginfi_group.to_bytes().to_vec(),
        ))])?;

    let _bank_pks = banks.iter().map(|(pk, _)| *pk).collect::<Vec<Pubkey>>();

    let oracle_pks = banks
        .iter()
        .flat_map(|(_, bank)| bank.config.oracle_keys)
        .filter(|pk| pk != &Pubkey::default())
        .collect::<Vec<Pubkey>>();

    // Dedup the oracle pks.
    let _oracle_pks = oracle_pks
        .into_iter()
        .fold(vec![], |mut acc, pk| {
            if !acc.contains(&pk) {
                acc.push(pk);
            }
            acc
        })
        .into_iter()
        .collect::<Vec<Pubkey>>();

    // Join keys
    let mut keys = vec![
        config.mfi_program.id(),
        marginfi_group,
        spl_token::id(),
        system_program::id(),
    ];

    for (bank_pk, bank) in banks.iter() {
        keys.push(*bank_pk);
        keys.push(bank.liquidity_vault);
        let (vault_auth, _) = utils::find_bank_vault_authority_pda(
            bank_pk,
            BankVaultType::Liquidity,
            &config.program_id,
        );

        keys.push(vault_auth);

        keys.extend_from_slice(
            &bank
                .config
                .oracle_keys
                .iter()
                .filter(|pk| **pk != Pubkey::default())
                .cloned()
                .collect::<Vec<_>>(),
        );
    }

    keys.sort();
    keys.dedup();

    // Find missing keys in lookup tables
    let missing_keys = keys
        .iter()
        .filter(|pk| {
            let missing = !lookup_tables
                .iter()
                .any(|lookup_table| lookup_table.addresses.iter().any(|address| &address == pk));

            println!("Key {} missing: {}", pk, missing);

            missing
        })
        .cloned()
        .collect::<Vec<Pubkey>>();

    println!("Missing {} keys", missing_keys.len());

    Ok(())
}

pub fn process_update_lookup_tables(
    config: &Config,
    profile: &Profile,
    existing_lookup_tables: Vec<Pubkey>,
) -> Result<()> {
    let rpc = config.mfi_program.rpc();
    let marginfi_group = profile
        .marginfi_group
        .context("marginfi group not set in profile")?;

    let mut accounts: Vec<Account> = vec![];

    for chunk in existing_lookup_tables.chunks(CHUNK_SIZE) {
        let accounts_2: Vec<Account> = rpc
            .get_multiple_accounts(chunk)?
            .into_iter()
            .flatten()
            .collect();

        accounts.extend(accounts_2);
    }

    let lookup_tables: Vec<AddressLookupTable> = accounts
        .iter_mut()
        .zip(existing_lookup_tables.iter())
        .map(|(account, address)| {
            let lookup_table = AddressLookupTable::deserialize(&account.data).map_err(|e| {
                anyhow::anyhow!("failed to deserialize lookup table {}: {}", address, e)
            })?;
            info!(
                "Loaded table {} with {} addresses",
                address,
                lookup_table.addresses.len()
            );

            if lookup_table.meta.authority != Some(config.authority()) {
                warn!(
                    "Lookup table {} has wrong authority {:?}",
                    address, lookup_table.meta.authority,
                );
            }

            Ok(lookup_table)
        })
        .collect::<Result<Vec<_>>>()?;

    let banks = config
        .mfi_program
        .accounts::<Bank>(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            marginfi_group.to_bytes().to_vec(),
        ))])?;

    let _bank_pks = banks.iter().map(|(pk, _)| *pk).collect::<Vec<Pubkey>>();

    let oracle_pks = banks
        .iter()
        .flat_map(|(_, bank)| bank.config.oracle_keys)
        .filter(|pk| pk != &Pubkey::default())
        .collect::<Vec<Pubkey>>();

    // Dedup the oracle pks.
    let _oracle_pks = oracle_pks
        .into_iter()
        .fold(vec![], |mut acc, pk| {
            if !acc.contains(&pk) {
                acc.push(pk);
            }
            acc
        })
        .into_iter()
        .collect::<Vec<Pubkey>>();

    // Join keys
    let mut keys = vec![
        config.mfi_program.id(),
        marginfi_group,
        spl_token::id(),
        system_program::id(),
    ];
    // keys.extend(bank_pks);
    // keys.extend(oracle_pks);

    // Add bank signers
    for (bank_pk, bank) in banks.iter() {
        keys.push(*bank_pk);
        keys.push(bank.liquidity_vault);
        let (vault_auth, _) = utils::find_bank_vault_authority_pda(
            bank_pk,
            BankVaultType::Liquidity,
            &config.program_id,
        );

        keys.push(vault_auth);

        keys.extend_from_slice(
            &bank
                .config
                .oracle_keys
                .iter()
                .filter(|pk| **pk != Pubkey::default())
                .cloned()
                .collect::<Vec<_>>(),
        );
    }

    keys.sort();
    keys.dedup();

    // Find missing keys in lookup tables
    let mut missing_keys = keys
        .iter()
        .filter(|pk| {
            let missing = !lookup_tables
                .iter()
                .any(|lookup_table| lookup_table.addresses.iter().any(|address| &address == pk));

            debug!("Key {} missing: {}", pk, missing);

            missing
        })
        .cloned()
        .collect::<Vec<Pubkey>>();

    info!("Missing {} keys", missing_keys.len());

    // Extend exsiting lookup tables if possible
    for (table, address) in lookup_tables.iter().zip(existing_lookup_tables.iter()) {
        add_to_lut(
            config,
            &rpc,
            &mut missing_keys,
            table.addresses.len(),
            *address,
        )?;
    }

    while !missing_keys.is_empty() {
        let lut_address = create_new_lut(config, &rpc)?;
        add_to_lut(config, &rpc, &mut missing_keys, 0, lut_address)?;
    }

    println!("Done");

    Ok(())
}

fn create_new_lut(config: &Config, rpc: &solana_client::rpc_client::RpcClient) -> Result<Pubkey> {
    let recent_slot = rpc.get_slot()?;
    // Create new lookup tables
    let (ix, address) = create_lookup_table(config.authority(), config.authority(), recent_slot);
    let recent_blockhash = rpc.get_latest_blockhash()?;
    let mut tx = Transaction::new_with_payer(&[ix], Some(&config.fee_payer.pubkey()));

    tx.sign(&[&config.fee_payer], recent_blockhash);

    let sig = rpc.send_and_confirm_transaction_with_spinner(&tx)?;

    info!("Created new lookup table {} {}", address, sig);
    println!("Created new lookup table {} {}", address, sig);

    Ok(address)
}

fn add_to_lut(
    config: &Config,
    rpc: &solana_client::rpc_client::RpcClient,
    missing_keys: &mut Vec<Pubkey>,
    table_current_size: usize,
    address: Pubkey,
) -> Result<()> {
    let remaining_room = 256 - table_current_size;
    let n_keys_to_add_to_table = missing_keys.len().min(remaining_room);

    let keys = missing_keys
        .drain(..n_keys_to_add_to_table)
        .collect::<Vec<_>>();

    for chunk in keys.chunks(KEY_BATCH_SIZE) {
        let ix = extend_lookup_table(
            address,
            config.authority(),
            Some(config.authority()),
            chunk.to_vec(),
        );

        let recent_blockhash = rpc.get_latest_blockhash()?;

        let mut tx = Transaction::new_with_payer(&[ix], Some(&config.fee_payer.pubkey()));
        tx.sign(&[&config.fee_payer], recent_blockhash);

        let sig = rpc.send_and_confirm_transaction_with_spinner(&tx)?;
        info!("Added {} keys to table {} {}", chunk.len(), address, sig);
    }

    Ok(())
}
