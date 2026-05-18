use {
    super::{group_get_all, load_all_banks},
    crate::{
        config::Config,
        output,
        utils::{
            find_bank_emssions_token_account_pda, find_bank_vault_authority_pda,
            find_bank_vault_pda, send_tx,
        },
    },
    anchor_client::anchor_lang::{AnchorDeserialize, InstructionData, ToAccountMetas},
    anyhow::{bail, Context, Result},
    marginfi::state::{
        bank::{BankImpl, BankVaultType},
        price::{
            parse_swb_ignore_alignment, LitePullFeedAccountData, OraclePriceFeedAdapter,
            PriceAdapter,
        },
    },
    marginfi_type_crate::{
        constants::METADATA_SEED,
        types::{Bank, BankMetadata, MarginfiGroup, OraclePriceType, OracleSetup, PriceBias},
    },
    pyth_solana_receiver_sdk::price_update::PriceUpdateV2,
    serde::{Deserialize, Serialize},
    solana_client::rpc_filter::{Memcmp, RpcFilterType},
    solana_sdk::{
        account::{ReadableAccount, WritableAccount},
        account_info::IntoAccountInfo,
        clock::Clock,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        system_program,
    },
    std::{
        cell::RefCell,
        collections::HashSet,
        fs,
        mem::size_of,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    },
    switchboard_on_demand::PullFeedAccountData,
};

const DEFAULT_METADATA_DB_URL: &str = "https://app.0.xyz/api/banks/db";

struct BankMetadataSnapshot {
    ticker: String,
    description: String,
}

/// Subset of the metadata-source row that `dump_bank_metadata` reads. Extra fields in
/// the source JSON are ignored.
#[derive(Debug, Clone, Deserialize)]
struct MetadataRow {
    #[serde(alias = "bankAddress")]
    bank_address: String,
    #[serde(alias = "tokenName")]
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BankMetadataDumpRow {
    bank_name: String,
    bank_address: String,
    bank_metadata_address: String,
    ticker: Option<String>,
    description: Option<String>,
}

pub fn bank_get(config: Config, bank_pk: Option<Pubkey>) -> Result<()> {
    let rpc_client = config.mfi_program.rpc();
    let json = config.json_output;

    if let Some(address) = bank_pk {
        let mut bank: Bank = config.mfi_program.account(address)?;
        let group: MarginfiGroup = config.mfi_program.account(bank.group)?;

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let current_timestamp = current_timestamp.as_secs() as i64;

        bank.accrue_interest(current_timestamp, &group)?;
        bank.update_bank_cache(&group)?;

        output::print_bank_detail(&address, &bank, json);

        // Vault balances (table mode only for now)
        if !json {
            let liquidity_vault_balance =
                rpc_client.get_token_account_balance(&bank.liquidity_vault)?;
            let fee_vault_balance = rpc_client.get_token_account_balance(&bank.fee_vault)?;
            let insurance_vault_balance =
                rpc_client.get_token_account_balance(&bank.insurance_vault)?;

            println!("Token balances:");
            println!(
                "\tliquidity vault: {} (native: {})",
                liquidity_vault_balance.ui_amount.unwrap_or(0.0),
                liquidity_vault_balance.amount
            );
            println!(
                "\tfee vault: {} (native: {})",
                fee_vault_balance.ui_amount.unwrap_or(0.0),
                fee_vault_balance.amount
            );
            println!(
                "\tinsurance vault: {} (native: {})",
                insurance_vault_balance.ui_amount.unwrap_or(0.0),
                insurance_vault_balance.amount
            );
            if bank.emissions_mint != Pubkey::default() {
                let emissions_token_account = find_bank_emssions_token_account_pda(
                    address,
                    bank.emissions_mint,
                    config.program_id,
                )
                .0;
                let emissions_vault_balance =
                    rpc_client.get_token_account_balance(&emissions_token_account)?;
                println!(
                    "\temissions vault: {} (native: {} - TA: {})",
                    emissions_vault_balance.ui_amount.unwrap_or(0.0),
                    emissions_vault_balance.amount,
                    emissions_token_account
                );
            }
        }
    } else {
        group_get_all(config)?;
    }
    Ok(())
}

pub fn bank_get_all(config: Config, marginfi_group: Option<Pubkey>) -> Result<()> {
    let json = config.json_output;
    let accounts = load_all_banks(&config, marginfi_group)?;
    output::print_banks_table(&accounts, json);
    Ok(())
}

pub fn bank_inspect_price_oracle(config: Config, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let opfa = match bank.config.oracle_setup {
        OracleSetup::Fixed => OraclePriceFeedAdapter::try_from_bank_with_max_age(
            &bank,
            &[],
            &Clock::default(),
            u64::MAX,
        )
        .map_err(|e| anyhow::anyhow!("failed to create oracle price feed adapter: {:?}", e))?,
        _ => {
            let oracle_keys = crate::utils::bank_observation_keys(&bank);
            let rpc = config.mfi_program.rpc();
            let mut oracle_accounts: Vec<_> = oracle_keys
                .iter()
                .map(|pk| rpc.get_account(pk))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let oracle_ais: Vec<_> = oracle_keys
                .iter()
                .zip(oracle_accounts.iter_mut())
                .map(|(pk, acc)| (pk, acc).into_account_info())
                .collect();

            OraclePriceFeedAdapter::try_from_bank_with_max_age(
                &bank,
                &oracle_ais,
                &Clock::default(),
                u64::MAX,
            )
            .map_err(|e| anyhow::anyhow!("failed to create oracle price feed adapter: {:?}", e))?
        }
    };

    let (real_price, maint_asset_price, maint_liab_price, init_asset_price, init_liab_price) = (
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, None)?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, Some(PriceBias::Low))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::RealTime, Some(PriceBias::High))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::TimeWeighted, Some(PriceBias::Low))?,
        opfa.get_price_of_type_ignore_conf(OraclePriceType::TimeWeighted, Some(PriceBias::High))?,
    );

    let keys = bank
        .config
        .oracle_keys
        .iter()
        .filter(|k| k != &&Pubkey::default())
        .collect::<Vec<_>>();

    println!(
        r##"
Oracle Setup: {setup:?}
Oracle Keys: {keys:#?}
Price:
    Realtime: {real_price}
    Maint: {maint_asset_price} (asset) {maint_liab_price} (liab)
    Init: {init_asset_price} (asset) {init_liab_price} (liab)
    "##,
        setup = bank.config.oracle_setup,
        keys = keys,
        real_price = real_price,
        maint_asset_price = maint_asset_price,
        maint_liab_price = maint_liab_price,
        init_asset_price = init_asset_price,
        init_liab_price = init_liab_price,
    );

    Ok(())
}

pub fn show_oracle_ages(
    config: Config,
    marginfi_group: Option<Pubkey>,
    only_stale: bool,
) -> Result<()> {
    let default_group = solana_sdk::pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8");
    let group = marginfi_group.unwrap_or(default_group);

    let banks = config
        .mfi_program
        .accounts::<Bank>(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            8 + size_of::<Pubkey>() + size_of::<u8>(),
            group.to_bytes().to_vec(),
        ))])?;

    if banks.is_empty() {
        println!("No banks found for group {}", group);
        return Ok(());
    }

    let mut pyth_feeds: Vec<(u16, Pubkey, Pubkey)> = Vec::new();
    let mut swb_feeds: Vec<(u16, Pubkey, Pubkey)> = Vec::new();

    for (_, bank) in banks {
        let Some(first_oracle) = bank
            .config
            .oracle_keys
            .iter()
            .copied()
            .find(|key| *key != Pubkey::default())
        else {
            continue;
        };

        match bank.config.oracle_setup {
            OracleSetup::PythPushOracle
            | OracleSetup::KaminoPythPush
            | OracleSetup::StakedWithPythPush
            | OracleSetup::DriftPythPull
            | OracleSetup::SolendPythPull
            | OracleSetup::JuplendPythPull => {
                pyth_feeds.push((bank.config.oracle_max_age, bank.mint, first_oracle));
            }
            OracleSetup::SwitchboardPull
            | OracleSetup::KaminoSwitchboardPull
            | OracleSetup::DriftSwitchboardPull
            | OracleSetup::SolendSwitchboardPull
            | OracleSetup::JuplendSwitchboardPull => {
                swb_feeds.push((bank.config.oracle_max_age, bank.mint, first_oracle));
            }
            _ => {}
        }
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    let mut pyth_rows: Vec<(f64, f64, Pubkey)> = Vec::new();
    if !pyth_feeds.is_empty() {
        let keys = pyth_feeds
            .iter()
            .map(|(_, _, key)| *key)
            .collect::<Vec<_>>();
        let accounts = config
            .mfi_program
            .rpc()
            .get_multiple_accounts(keys.as_slice())?;

        for (maybe_account, (max_age, mint, _)) in accounts.into_iter().zip(pyth_feeds.iter()) {
            let Some(account) = maybe_account else {
                continue;
            };

            let Ok(price_update) = PriceUpdateV2::deserialize(&mut &account.data()[8..]) else {
                continue;
            };

            let age_min = (now - price_update.price_message.publish_time) as f64 / 60.0;
            let allowed_min = if *max_age == 0 {
                1.0
            } else {
                *max_age as f64 / 60.0
            };
            pyth_rows.push((age_min, allowed_min, *mint));
        }
    }

    let mut swb_rows: Vec<(f64, f64, Pubkey)> = Vec::new();
    if !swb_feeds.is_empty() {
        let keys = swb_feeds.iter().map(|(_, _, key)| *key).collect::<Vec<_>>();
        let mut accounts = config
            .mfi_program
            .rpc()
            .get_multiple_accounts(keys.as_slice())?;

        for (maybe_account, (max_age, mint, _)) in accounts.iter_mut().zip(swb_feeds.iter()) {
            let Some(account) = maybe_account else {
                continue;
            };

            let data = account.data_as_mut_slice();
            let cell = RefCell::new(data);
            let Ok(feed): Result<PullFeedAccountData, _> =
                parse_swb_ignore_alignment(cell.borrow())
            else {
                continue;
            };
            let lite_feed = LitePullFeedAccountData::from(&feed);

            let age_min = (now - lite_feed.last_update_timestamp) as f64 / 60.0;
            let allowed_min = if *max_age == 0 {
                1.0
            } else {
                *max_age as f64 / 60.0
            };
            swb_rows.push((age_min, allowed_min, *mint));
        }
    }

    pyth_rows.sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    swb_rows.sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    println!("Group: {}", group);
    println!("Pyth");
    for (age, allowed, mint) in pyth_rows {
        if only_stale && age < allowed {
            continue;
        }
        println!(
            "- {:?}: {:.2}min (allowed: {:.2}min){}",
            mint,
            age,
            allowed,
            if age >= allowed { " [STALE]" } else { "" }
        );
    }

    println!("Switchboard");
    for (age, allowed, mint) in swb_rows {
        if only_stale && age < allowed {
            continue;
        }
        println!(
            "- {:?}: {:.2}min (allowed: {:.2}min){}",
            mint,
            age,
            allowed,
            if age >= allowed { " [STALE]" } else { "" }
        );
    }

    Ok(())
}

fn derive_bank_metadata_address(program_id: &Pubkey, bank_pk: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[METADATA_SEED.as_bytes(), bank_pk.as_ref()], program_id).0
}

fn decode_metadata_field(bytes: &[u8], end_index: usize) -> String {
    if bytes.is_empty() || bytes[0] == 0 {
        return String::new();
    }

    let end = (end_index + 1).min(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

fn read_current_bank_metadata(
    config: &Config,
    bank_pk: Pubkey,
) -> Result<Option<BankMetadataSnapshot>> {
    let rpc_client = config.mfi_program.rpc();
    let metadata = derive_bank_metadata_address(&config.program_id, &bank_pk);

    let account = rpc_client
        .get_account_with_commitment(&metadata, config.commitment)?
        .value;

    let Some(account) = account else {
        return Ok(None);
    };

    let data = account.data();
    let expected_len = 8 + BankMetadata::LEN;
    if data.len() < expected_len {
        bail!(
            "metadata account {} too short: got {} bytes, expected at least {}",
            metadata,
            data.len(),
            expected_len
        );
    }

    let payload = &data[8..expected_len];
    let ticker = &payload[40..104];
    let description = &payload[104..232];
    let end_description = u16::from_le_bytes([payload[488], payload[489]]) as usize;
    let end_ticker = payload[492] as usize;

    Ok(Some(BankMetadataSnapshot {
        ticker: decode_metadata_field(ticker, end_ticker),
        description: decode_metadata_field(description, end_description),
    }))
}

fn parse_metadata_source_db(body: &str) -> Result<Vec<MetadataRow>> {
    serde_json::from_str::<Vec<MetadataRow>>(body).context("unsupported metadata source format")
}

pub fn dump_bank_metadata(
    config: Config,
    group: Option<Pubkey>,
    url: Option<String>,
    out: PathBuf,
    limit: Option<usize>,
) -> Result<()> {
    let url = url.unwrap_or_else(|| DEFAULT_METADATA_DB_URL.to_string());
    let response = reqwest::blocking::get(&url)
        .with_context(|| format!("failed to fetch metadata source {}", url))?;
    let response = response
        .error_for_status()
        .with_context(|| format!("metadata source {} returned an error", url))?;
    let body = response.text()?;
    let source_rows = parse_metadata_source_db(&body)?;

    let group_bank_set = if let Some(group) = group {
        Some(
            load_all_banks(&config, Some(group))?
                .into_iter()
                .map(|(pk, _)| pk)
                .collect::<HashSet<_>>(),
        )
    } else {
        None
    };

    let mut rows: Vec<MetadataRow> = source_rows
        .into_iter()
        .filter(|row| match group_bank_set.as_ref() {
            Some(group_bank_set) => row
                .bank_address
                .parse::<Pubkey>()
                .map(|bank_pk| group_bank_set.contains(&bank_pk))
                .unwrap_or(false),
            None => true,
        })
        .collect();

    if let Some(limit) = limit {
        rows.truncate(limit);
    }

    let dump_rows = rows
        .into_iter()
        .map(|row| {
            let bank_pk: Pubkey = row.bank_address.parse()?;
            let bank_metadata_address = derive_bank_metadata_address(&config.program_id, &bank_pk);
            let metadata = read_current_bank_metadata(&config, bank_pk)?;

            Ok(BankMetadataDumpRow {
                bank_name: row.name,
                bank_address: row.bank_address,
                bank_metadata_address: bank_metadata_address.to_string(),
                ticker: metadata.as_ref().map(|value| value.ticker.clone()),
                description: metadata.as_ref().map(|value| value.description.clone()),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    fs::write(&out, serde_json::to_vec_pretty(&dump_rows)?)?;

    if config.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "outPath": out.display().to_string(),
                "count": dump_rows.len(),
                "group": group.map(|value| value.to_string()),
                "url": url,
            }))?
        );
    } else {
        println!("Metadata source: {}", url);
        if let Some(group) = group {
            println!("Filtered group: {}", group);
        }
        println!(
            "Wrote {} bank metadata rows to {}",
            dump_rows.len(),
            out.display()
        );
    }

    Ok(())
}

pub fn bank_accrue_interest(config: Config, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolAccrueBankInterest {
            group: bank.group,
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolAccrueBankInterest {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Interest accrued (sig: {})", sig);

    Ok(())
}

pub fn bank_pulse_price_cache(config: Config, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let mut accounts = marginfi::accounts::LendingPoolPulseBankPriceCache {
        group: bank.group,
        bank: bank_pk,
    }
    .to_account_metas(Some(true));

    // Append all oracle accounts needed for this bank's oracle setup
    for oracle_pk in crate::utils::bank_observation_keys(&bank) {
        accounts.push(AccountMeta::new_readonly(oracle_pk, false));
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts,
        data: marginfi::instruction::LendingPoolPulseBankPriceCache {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Price cache pulsed (sig: {})", sig);

    Ok(())
}

pub fn bank_withdraw_fees_permissionless(
    config: Config,
    bank_pk: Pubkey,
    amount: u64,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let token_program = config.mfi_program.rpc().get_account(&bank.mint)?.owner;

    let (fee_vault, _) = find_bank_vault_pda(&bank_pk, BankVaultType::Fee, &config.program_id);
    let (fee_vault_authority, _) =
        find_bank_vault_authority_pda(&bank_pk, BankVaultType::Fee, &config.program_id);

    let fees_destination_account = bank.fees_destination_account;

    let mut ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolWithdrawFeesPermissionless {
            group: bank.group,
            bank: bank_pk,
            fee_vault,
            fee_vault_authority,
            fees_destination_account,
            token_program,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolWithdrawFeesPermissionless { amount }.data(),
    };
    if token_program == anchor_spl::token_2022::ID {
        ix.accounts
            .push(AccountMeta::new_readonly(bank.mint, false));
    }

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Fees withdrawn permissionlessly (sig: {})", sig);

    Ok(())
}

pub fn bank_init_metadata(config: Config, bank_pk: Pubkey) -> Result<()> {
    let metadata = derive_bank_metadata_address(&config.program_id, &bank_pk);

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::InitBankMetadata {
            bank: bank_pk,
            fee_payer: config.authority(),
            metadata,
            system_program: system_program::id(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::InitBankMetadata {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank metadata initialized (sig: {})", sig);

    Ok(())
}
