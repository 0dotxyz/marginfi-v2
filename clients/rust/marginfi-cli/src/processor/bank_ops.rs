use {
    super::{group_get_all, load_all_banks},
    crate::output,
    crate::{
        config::Config,
        utils::{
            find_bank_emssions_token_account_pda, find_bank_vault_authority_pda,
            find_bank_vault_pda, send_tx,
        },
    },
    anchor_client::anchor_lang::{AnchorDeserialize, InstructionData, ToAccountMetas},
    anyhow::{bail, Context, Result},
    fixed::types::I80F48,
    marginfi::state::{
        bank::{BankImpl, BankVaultType},
        price::{
            parse_swb_ignore_alignment, LitePullFeedAccountData, OraclePriceFeedAdapter,
            PriceAdapter,
        },
    },
    marginfi_type_crate::{
        constants::METADATA_SEED,
        types::{
            Bank, BankConfigOpt, BankMetadata, InterestRateConfigOpt, MarginfiGroup, OracleSetup,
            RatePoint, RiskTier, WrappedI80F48,
        },
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
        collections::{HashMap, HashSet},
        fs,
        mem::size_of,
        path::PathBuf,
        sync::OnceLock,
        thread::sleep,
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
    switchboard_on_demand::PullFeedAccountData,
};

const DEFAULT_METADATA_DB_URL: &str = "https://app.0.xyz/api/banks/db";

pub struct BankUpdateInterestRateRequest {
    pub insurance_fee_fixed_apr: Option<f64>,
    pub insurance_ir_fee: Option<f64>,
    pub protocol_fixed_fee_apr: Option<f64>,
    pub protocol_ir_fee: Option<f64>,
    pub protocol_origination_fee: Option<f64>,
    pub zero_util_rate: Option<u32>,
    pub hundred_util_rate: Option<u32>,
    pub points: Vec<RatePoint>,
}

impl BankUpdateInterestRateRequest {
    fn into_interest_rate_config_opt(self) -> InterestRateConfigOpt {
        InterestRateConfigOpt {
            insurance_fee_fixed_apr: self
                .insurance_fee_fixed_apr
                .map(|value| I80F48::from_num(value).into()),
            insurance_ir_fee: self
                .insurance_ir_fee
                .map(|value| I80F48::from_num(value).into()),
            protocol_fixed_fee_apr: self
                .protocol_fixed_fee_apr
                .map(|value| I80F48::from_num(value).into()),
            protocol_ir_fee: self
                .protocol_ir_fee
                .map(|value| I80F48::from_num(value).into()),
            protocol_origination_fee: self
                .protocol_origination_fee
                .map(|value| I80F48::from_num(value).into()),
            zero_util_rate: self.zero_util_rate,
            hundred_util_rate: self.hundred_util_rate,
            points: if self.points.is_empty() {
                None
            } else {
                Some(marginfi_type_crate::types::make_points(&self.points))
            },
        }
    }
}

pub struct BankUpdateRequest {
    pub bank_pk: Pubkey,
    pub asset_weight_init: Option<f32>,
    pub asset_weight_maint: Option<f32>,
    pub liability_weight_init: Option<f32>,
    pub liability_weight_maint: Option<f32>,
    pub deposit_limit_ui: Option<f64>,
    pub borrow_limit_ui: Option<f64>,
    pub operational_state: Option<marginfi_type_crate::types::BankOperationalState>,
    pub interest_rate_config: Option<BankUpdateInterestRateRequest>,
    pub risk_tier: Option<RiskTier>,
    pub asset_tag: Option<u8>,
    pub total_asset_value_init_limit: Option<u64>,
    pub oracle_max_confidence: Option<u32>,
    pub oracle_max_age: Option<u16>,
    pub permissionless_bad_debt_settlement: Option<bool>,
    pub freeze_settings: Option<bool>,
    pub tokenless_repayments_allowed: Option<bool>,
}

impl BankUpdateRequest {
    fn into_bank_config_opt(self, bank: &Bank) -> BankConfigOpt {
        BankConfigOpt {
            asset_weight_init: self
                .asset_weight_init
                .map(|value| I80F48::from_num(value).into()),
            asset_weight_maint: self
                .asset_weight_maint
                .map(|value| I80F48::from_num(value).into()),
            liability_weight_init: self
                .liability_weight_init
                .map(|value| I80F48::from_num(value).into()),
            liability_weight_maint: self
                .liability_weight_maint
                .map(|value| I80F48::from_num(value).into()),
            deposit_limit: self
                .deposit_limit_ui
                .map(|ui_amount| spl_token::ui_amount_to_amount(ui_amount, bank.mint_decimals)),
            borrow_limit: self
                .borrow_limit_ui
                .map(|ui_amount| spl_token::ui_amount_to_amount(ui_amount, bank.mint_decimals)),
            operational_state: self.operational_state,
            interest_rate_config: self
                .interest_rate_config
                .map(BankUpdateInterestRateRequest::into_interest_rate_config_opt),
            risk_tier: self.risk_tier,
            asset_tag: self.asset_tag,
            total_asset_value_init_limit: self.total_asset_value_init_limit,
            oracle_max_confidence: self.oracle_max_confidence,
            oracle_max_age: self.oracle_max_age,
            permissionless_bad_debt_settlement: self.permissionless_bad_debt_settlement,
            freeze_settings: self.freeze_settings,
            tokenless_repayments_allowed: self.tokenless_repayments_allowed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BankMetadataEntry {
    pub bank: Pubkey,
    pub ticker: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct BankMetadataInput {
    pub bank: Pubkey,
    pub ticker: Option<String>,
    pub description: Option<String>,
    pub mint: Option<Pubkey>,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub asset_group: Option<String>,
    pub venue: Option<String>,
    pub venue_identifier: Option<String>,
    pub risk_tier_name: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct BankMetadataWriteOptions {
    pub wait_for_bank: bool,
    pub wait_for_bank_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BankMetadataSnapshot {
    ticker: String,
    description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetadataWriteStatus {
    UpToDate,
    Updated,
    CreatedAndUpdated,
    UpdatePrepared,
    CreateAndUpdatePrepared,
}

#[derive(Debug, Clone)]
struct BankMetadataWriteResult {
    bank: Pubkey,
    metadata: Pubkey,
    status: MetadataWriteStatus,
    resulting_metadata: BankMetadataSnapshot,
}

#[derive(Debug, Clone, Deserialize)]
struct MetadataRow {
    #[serde(alias = "bankAddress")]
    bank_address: String,
    #[serde(alias = "tokenAddress")]
    mint: String,
    #[serde(alias = "tokenSymbol")]
    symbol: String,
    #[serde(alias = "tokenName")]
    name: String,
    #[serde(default, alias = "assetGroup")]
    asset_group: Option<String>,
    #[serde(default)]
    venue: Option<String>,
    #[serde(default, alias = "venueIdentifier")]
    venue_identifier: Option<String>,
    #[serde(default, alias = "riskTierName")]
    risk_tier_name: Option<String>,
}

const STAGING_METADATA_URL_FRAGMENT: &str = "mrgn-bank-metadata-cache-stage";
const ADDITIONAL_STAGING_BANKS: &[(&str, &str, &str)] = &[(
    "GFMZQWGdfvcXQd6PM3ZTtMjYhEFh9gBEogfKsZKBsKjs",
    "ptBulkSOL | PT-bulkSOL-26FEB26",
    "PT-bulkSOL-26FEB26 | rate-products | ptBulkSOL | P0 | -",
)];

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
    use marginfi::state::price::{OraclePriceType, PriceBias};

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

pub fn update_bank(config: Config, request: BankUpdateRequest) -> Result<()> {
    let bank: Bank = config.mfi_program.account(request.bank_pk)?;
    let bank_pk = request.bank_pk;
    let bank_config_opt = request.into_bank_config_opt(&bank);
    let configure_bank_ixs_builder = config.mfi_program.request();
    let signing_keypairs = config.get_signers(false);

    let configure_bank_ixs = configure_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolConfigureBank {
            group: bank.group,
            admin: config.authority(),
            bank: bank_pk,
        })
        .args(marginfi::instruction::LendingPoolConfigureBank {
            bank_config_opt: bank_config_opt.clone(),
        })
        .instructions()?;

    let sig = send_tx(&config, configure_bank_ixs, &signing_keypairs)?;

    println!("Transaction signature: {}", sig);

    Ok(())
}

type AssetGroups = HashMap<String, HashMap<String, String>>;

static ASSET_GROUPS: OnceLock<AssetGroups> = OnceLock::new();
static MINT_TO_GROUP: OnceLock<HashMap<String, String>> = OnceLock::new();
const ASSET_GROUP_PRIORITY: &[&str] = &[
    "blue-chip",
    "stablecoins",
    "sol-lst",
    "bitcoin",
    "rate-products",
    "ecosystem",
    "governance",
    "memes",
];

fn asset_groups_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("assetGroups.json")
}

fn asset_groups() -> &'static AssetGroups {
    ASSET_GROUPS.get_or_init(|| {
        let content = fs::read_to_string(asset_groups_path())
            .unwrap_or_else(|_| include_str!("../../assets/assetGroups.json").to_string());
        serde_json::from_str(&content).expect("asset group json must be valid")
    })
}

fn update_asset_groups_file(asset_group: &str, symbol: &str, mint: &Pubkey) -> Result<()> {
    let path = asset_groups_path();
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| include_str!("../../assets/assetGroups.json").to_string());
    let mut value: serde_json::Value =
        serde_json::from_str(&content).context("asset group json must be valid")?;
    let root = value
        .as_object_mut()
        .context("asset group json root must be an object")?;

    if !root.contains_key(asset_group) {
        root.insert(asset_group.to_string(), serde_json::json!({}));
    }

    let group = root
        .get_mut(asset_group)
        .and_then(serde_json::Value::as_object_mut)
        .context("asset group entry must be an object")?;

    match group.get(symbol).and_then(serde_json::Value::as_str) {
        Some(existing_mint) if existing_mint == mint.to_string() => return Ok(()),
        Some(existing_mint) => {
            println!(
                "assetGroups.json not updated for {} in {}: existing mint {} differs from {}",
                symbol, asset_group, existing_mint, mint
            );
            return Ok(());
        }
        None => {}
    }

    group.insert(
        symbol.to_string(),
        serde_json::Value::String(mint.to_string()),
    );
    fs::write(&path, serde_json::to_vec_pretty(&value)?)?;
    println!(
        "Updated assetGroups.json: added {} -> {} to {}",
        symbol, mint, asset_group
    );

    Ok(())
}

fn mint_to_group() -> &'static HashMap<String, String> {
    MINT_TO_GROUP.get_or_init(|| {
        let asset_groups = asset_groups();
        let mut mint_to_group = HashMap::new();

        for group_name in ASSET_GROUP_PRIORITY {
            if let Some(tokens) = asset_groups.get(*group_name) {
                for mint in tokens.values() {
                    mint_to_group
                        .entry(mint.clone())
                        .or_insert_with(|| (*group_name).to_string());
                }
            }
        }

        let mut remaining_groups: Vec<&String> = asset_groups
            .keys()
            .filter(|group_name| !ASSET_GROUP_PRIORITY.contains(&group_name.as_str()))
            .collect();
        remaining_groups.sort();

        for group_name in remaining_groups {
            if let Some(tokens) = asset_groups.get(group_name) {
                for mint in tokens.values() {
                    mint_to_group
                        .entry(mint.clone())
                        .or_insert_with(|| group_name.clone());
                }
            }
        }

        mint_to_group
    })
}

fn asset_group_for_mint(mint: &str, risk_tier_name: Option<&str>) -> &'static str {
    if risk_tier_name
        .map(|value| value.eq_ignore_ascii_case("isolated"))
        .unwrap_or(false)
    {
        return "W/E";
    }

    mint_to_group()
        .get(mint)
        .map(String::as_str)
        .unwrap_or("W/E")
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

fn print_bank_metadata_snapshot(metadata: &BankMetadataSnapshot) {
    println!("  ticker: {}", metadata.ticker);
    println!("  description: {}", metadata.description);
}

fn target_bank_metadata_snapshot(entry: &BankMetadataEntry) -> BankMetadataSnapshot {
    BankMetadataSnapshot {
        ticker: entry.ticker.clone(),
        description: entry.description.clone(),
    }
}

fn metadata_write_status_label(status: MetadataWriteStatus) -> &'static str {
    match status {
        MetadataWriteStatus::UpToDate => "up to date",
        MetadataWriteStatus::Updated => "updated",
        MetadataWriteStatus::CreatedAndUpdated => "initialized + updated",
        MetadataWriteStatus::UpdatePrepared => "update prepared",
        MetadataWriteStatus::CreateAndUpdatePrepared => "init + update prepared",
    }
}

fn apply_bank_metadata_entry(
    config: &Config,
    entry: &BankMetadataEntry,
    options: &BankMetadataWriteOptions,
) -> Result<BankMetadataWriteResult> {
    let rpc_client = config.mfi_program.rpc();
    let metadata = derive_bank_metadata_address(&config.program_id, &entry.bank);
    println!("  ensuring bank account {} exists", entry.bank);
    let bank = wait_for_bank_account(config, entry.bank, options)?;
    println!("  bank account found");
    println!("  metadata PDA: {}", metadata);
    let target_metadata = target_bank_metadata_snapshot(entry);
    let current_metadata = read_current_bank_metadata(config, entry.bank)?;

    if current_metadata.as_ref() == Some(&target_metadata) {
        println!("  on-chain metadata already matches target values");
        return Ok(BankMetadataWriteResult {
            bank: entry.bank,
            metadata,
            status: MetadataWriteStatus::UpToDate,
            resulting_metadata: target_metadata,
        });
    }

    let mut ixs = Vec::new();
    let needs_init = rpc_client.get_account(&metadata).is_err();

    if needs_init {
        println!("  metadata account not found; init will be included");
        ixs.push(Instruction {
            program_id: config.program_id,
            accounts: marginfi::accounts::InitBankMetadata {
                bank: entry.bank,
                fee_payer: config.authority(),
                metadata,
                system_program: system_program::id(),
            }
            .to_account_metas(Some(true)),
            data: marginfi::instruction::InitBankMetadata {}.data(),
        });
    } else {
        println!("  metadata account already exists");
    }

    println!("  scheduling metadata write");
    ixs.push(Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::WriteBankMetadata {
            group: bank.group,
            bank: entry.bank,
            metadata_admin: config.authority(),
            metadata,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::WriteBankMetadata {
            ticker: Some(entry.ticker.clone().into_bytes()),
            description: Some(entry.description.clone().into_bytes()),
        }
        .data(),
    });

    let signing_keypairs = config.get_signers(false);
    println!(
        "  {} {} instruction{}",
        if config.send_tx {
            "sending"
        } else {
            "preparing"
        },
        ixs.len(),
        if ixs.len() == 1 { "" } else { "s" }
    );
    send_tx(config, ixs, &signing_keypairs)?;

    let status = match (config.send_tx, needs_init) {
        (true, true) => MetadataWriteStatus::CreatedAndUpdated,
        (true, false) => MetadataWriteStatus::Updated,
        (false, true) => MetadataWriteStatus::CreateAndUpdatePrepared,
        (false, false) => MetadataWriteStatus::UpdatePrepared,
    };

    let resulting_metadata = if config.send_tx {
        read_current_bank_metadata(config, entry.bank)?.unwrap_or_else(|| target_metadata.clone())
    } else {
        target_metadata
    };

    Ok(BankMetadataWriteResult {
        bank: entry.bank,
        metadata,
        status,
        resulting_metadata,
    })
}

fn wait_for_bank_account(
    config: &Config,
    bank: Pubkey,
    options: &BankMetadataWriteOptions,
) -> Result<Bank> {
    let rpc_client = config.mfi_program.rpc();
    let started = std::time::Instant::now();
    let poll_interval = Duration::from_secs(1);
    let mut wait_notice_printed = false;

    loop {
        match rpc_client.get_account(&bank) {
            Ok(_) => return Ok(config.mfi_program.account(bank)?),
            Err(err) if options.wait_for_bank => {
                if started.elapsed() >= options.wait_for_bank_timeout {
                    return Err(err).with_context(|| {
                        format!(
                            "bank {bank} did not appear on-chain within {}s",
                            options.wait_for_bank_timeout.as_secs()
                        )
                    });
                }

                if !wait_notice_printed {
                    println!(
                        "  bank account not found yet; waiting up to {}s for it to appear on-chain",
                        options.wait_for_bank_timeout.as_secs()
                    );
                    wait_notice_printed = true;
                }

                println!(
                    "  still waiting for bank account {} (elapsed: {}s)",
                    bank,
                    started.elapsed().as_secs()
                );
                sleep(poll_interval);
            }
            Err(err) => return Err(err.into()),
        }
    }
}

fn build_metadata_entry(row: MetadataRow) -> Result<BankMetadataEntry> {
    let asset_group = row.asset_group.unwrap_or_else(|| {
        asset_group_for_mint(&row.mint, row.risk_tier_name.as_deref()).to_string()
    });
    let venue = row.venue.unwrap_or_else(|| "P0".to_string());
    let market_type = row
        .venue_identifier
        .as_deref()
        .and_then(|value| value.split(" - ").nth(1))
        .and_then(|value| {
            if value == venue {
                None
            } else if let Some(stripped) = value.strip_prefix(&venue) {
                Some(stripped.trim().to_string())
            } else {
                Some(value.trim().to_string())
            }
        });
    let market_suffix = market_type
        .filter(|value| !value.is_empty())
        .map(|value| format!(" | {value}"))
        .unwrap_or_else(|| " | -".to_string());

    Ok(BankMetadataEntry {
        bank: row.bank_address.parse()?,
        ticker: format!("{} | {}", row.symbol, row.name),
        description: format!(
            "{} | {} | {} | {}{}",
            row.name, asset_group, row.symbol, venue, market_suffix
        ),
    })
}

fn resolve_bank_metadata_input(
    config: &Config,
    input: BankMetadataInput,
    options: &BankMetadataWriteOptions,
) -> Result<BankMetadataEntry> {
    let BankMetadataInput {
        bank,
        ticker,
        description,
        mint,
        symbol,
        name,
        asset_group,
        venue,
        venue_identifier,
        risk_tier_name,
    } = input;

    if let (Some(ticker), Some(description)) = (&ticker, &description) {
        return Ok(BankMetadataEntry {
            bank,
            ticker: ticker.clone(),
            description: description.clone(),
        });
    }

    let effective_mint = if let Some(mint) = mint {
        mint
    } else {
        wait_for_bank_account(config, bank, options)?.mint
    };
    if let (Some(asset_group), Some(symbol)) = (asset_group.as_deref(), symbol.as_deref()) {
        update_asset_groups_file(asset_group, symbol, &effective_mint)?;
    }
    let row = MetadataRow {
        bank_address: bank.to_string(),
        mint: effective_mint.to_string(),
        symbol: symbol.context("symbol required when --ticker or --description is omitted")?,
        name: name.context("name required when --ticker or --description is omitted")?,
        asset_group,
        venue,
        venue_identifier,
        risk_tier_name,
    };
    let derived = build_metadata_entry(row)?;

    Ok(BankMetadataEntry {
        bank,
        ticker: ticker.unwrap_or(derived.ticker),
        description: description.unwrap_or(derived.description),
    })
}

pub fn resolve_bank_metadata_inputs(
    config: &Config,
    inputs: Vec<BankMetadataInput>,
    options: &BankMetadataWriteOptions,
) -> Result<Vec<BankMetadataEntry>> {
    inputs
        .into_iter()
        .map(|input| resolve_bank_metadata_input(config, input, options))
        .collect()
}

fn parse_metadata_source_rows(body: &str, url: &str) -> Result<Vec<BankMetadataEntry>> {
    let rows = parse_metadata_source_db(body)?;
    let mut entries: Vec<BankMetadataEntry> = rows
        .into_iter()
        .map(build_metadata_entry)
        .collect::<Result<Vec<_>>>()?;

    if url.contains(STAGING_METADATA_URL_FRAGMENT) {
        for (bank, ticker, description) in ADDITIONAL_STAGING_BANKS {
            entries.push(BankMetadataEntry {
                bank: bank.parse()?,
                ticker: (*ticker).to_string(),
                description: (*description).to_string(),
            });
        }
    }

    Ok(entries)
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

pub fn sync_bank_metadata_from_url(
    config: Config,
    group: Pubkey,
    url: Option<String>,
    limit: Option<usize>,
    delay_ms: u64,
) -> Result<()> {
    let url = url.unwrap_or_else(|| DEFAULT_METADATA_DB_URL.to_string());
    let response = reqwest::blocking::get(&url)
        .with_context(|| format!("failed to fetch metadata source {}", url))?;
    let response = response
        .error_for_status()
        .with_context(|| format!("metadata source {} returned an error", url))?;
    let body = response.text()?;
    let source_entries = parse_metadata_source_rows(&body, &url)?;

    let banks = load_all_banks(&config, Some(group))?;
    let group_bank_set: HashSet<Pubkey> = banks.iter().map(|(pk, _)| *pk).collect();

    let mut entries: Vec<BankMetadataEntry> = source_entries
        .into_iter()
        .filter(|entry| group_bank_set.contains(&entry.bank))
        .collect();

    if let Some(limit) = limit {
        entries.truncate(limit);
    }

    println!("Metadata source: {}", url);
    println!("Target group: {}", group);
    println!("Banks selected: {}", entries.len());
    let mut synced_results = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let result = apply_bank_metadata_entry(&config, entry)?;
        println!(
            "[{}/{}] {} - {}",
            index + 1,
            entries.len(),
            result.bank,
            metadata_write_status_label(result.status)
        );
        print_bank_metadata_snapshot(&result.resulting_metadata);
        if result.status != MetadataWriteStatus::UpToDate {
            synced_results.push(result);
        }

        if index + 1 < entries.len() && delay_ms > 0 {
            sleep(Duration::from_millis(delay_ms));
        }
    }

    println!("Banks synced this iteration: {}", synced_results.len());
    if !synced_results.is_empty() {
        println!("Updated accounts:");
        for result in synced_results {
            println!(
                "- bank: {} | metadata: {} | status: {}",
                result.bank,
                result.metadata,
                metadata_write_status_label(result.status)
            );
        }
    }

    Ok(())
}

pub fn bank_configure_interest_only(
    config: Config,
    bank_pk: Pubkey,
    interest_rate_config: InterestRateConfigOpt,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let group: MarginfiGroup = config.mfi_program.account(bank.group)?;

    if group.delegate_curve_admin != config.authority() {
        bail!(
            "Authority {} does not match the group's delegate_curve_admin {}. \
             Only the delegate curve admin can configure interest rates via this command.",
            config.authority(),
            group.delegate_curve_admin
        );
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolConfigureBankInterestOnly {
            group: bank.group,
            delegate_curve_admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBankInterestOnly {
            interest_rate_config,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank interest config updated (sig: {})", sig);

    Ok(())
}

pub fn bank_configure_limits_only(
    config: Config,
    bank_pk: Pubkey,
    deposit_limit_ui: Option<f64>,
    borrow_limit_ui: Option<f64>,
    total_asset_value_init_limit: Option<u64>,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let group: MarginfiGroup = config.mfi_program.account(bank.group)?;

    if group.delegate_limit_admin != config.authority() {
        bail!(
            "Authority {} does not match the group's delegate_limit_admin {}. \
             Only the delegate limit admin can configure limits via this command.",
            config.authority(),
            group.delegate_limit_admin
        );
    }

    let deposit_limit = deposit_limit_ui
        .map(|ui_amount| spl_token::ui_amount_to_amount(ui_amount, bank.mint_decimals));
    let borrow_limit = borrow_limit_ui
        .map(|ui_amount| spl_token::ui_amount_to_amount(ui_amount, bank.mint_decimals));

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolConfigureBankLimitsOnly {
            group: bank.group,
            delegate_limit_admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBankLimitsOnly {
            deposit_limit,
            borrow_limit,
            total_asset_value_init_limit,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank limits updated (sig: {})", sig);

    Ok(())
}

pub fn bank_configure_oracle(
    config: Config,
    bank_pk: Pubkey,
    setup: u8,
    oracle: Pubkey,
) -> Result<()> {
    let configure_bank_ixs_builder = config.mfi_program.request();
    let signing_keypairs = config.get_signers(false);
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let extra_accounts = vec![AccountMeta::new_readonly(oracle, false)];

    let mut configure_bank_ixs = configure_bank_ixs_builder
        .accounts(marginfi::accounts::LendingPoolConfigureBankOracle {
            group: bank.group,
            admin: config.authority(),
            bank: bank_pk,
        })
        .args(marginfi::instruction::LendingPoolConfigureBankOracle { setup, oracle })
        .instructions()?;

    configure_bank_ixs[0].accounts.extend(extra_accounts);

    let sig = send_tx(&config, configure_bank_ixs, &signing_keypairs)?;

    println!("Transaction signature: {}", sig);

    Ok(())
}

pub fn bank_force_tokenless_repay_complete(config: Config, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let group: MarginfiGroup = config.mfi_program.account(bank.group)?;

    if group.risk_admin != config.authority() {
        bail!(
            "Authority {} does not match the group's risk_admin {}. \
             Only the risk admin can force tokenless repay complete.",
            config.authority(),
            group.risk_admin
        );
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolForceTokenlessRepayComplete {
            group: bank.group,
            risk_admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolForceTokenlessRepayComplete {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Tokenless repay complete set (sig: {})", sig);

    Ok(())
}

// --------------------------------------------------------------------------
// New bank commands
// --------------------------------------------------------------------------

pub fn bank_close(config: Config, bank_pk: Pubkey) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolCloseBank {
            group: bank.group,
            bank: bank_pk,
            admin: config.authority(),
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolCloseBank {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Bank closed (sig: {})", sig);

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

pub fn bank_set_fixed_price(config: Config, bank_pk: Pubkey, price: f64) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let price_wrapped: WrappedI80F48 = I80F48::from_num(price).into();

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolSetFixedOraclePrice {
            group: bank.group,
            admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolSetFixedOraclePrice {
            price: price_wrapped,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Fixed price set (sig: {})", sig);

    Ok(())
}

pub fn bank_configure_emode(config: Config, bank_pk: Pubkey, emode_tag: u16) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;
    let group: MarginfiGroup = config.mfi_program.account(bank.group)?;
    let entries = bank.emode.emode_config.entries;

    if group.emode_admin != config.authority() {
        bail!(
            "Authority {} does not match the group's emode_admin {}. \
             Only the emode admin can configure emode.",
            config.authority(),
            group.emode_admin
        );
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolConfigureBankEmode {
            group: bank.group,
            emode_admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolConfigureBankEmode { emode_tag, entries }.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Emode configured (sig: {})", sig);

    Ok(())
}

pub fn bank_clone_emode(
    config: Config,
    copy_from_bank: Pubkey,
    copy_to_bank: Pubkey,
) -> Result<()> {
    let source: Bank = config.mfi_program.account(copy_from_bank)?;
    let destination: Bank = config.mfi_program.account(copy_to_bank)?;

    if source.group != destination.group {
        bail!("source and destination banks belong to different groups");
    }

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolCloneEmode {
            group: source.group,
            signer: config.authority(),
            copy_from_bank,
            copy_to_bank,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolCloneEmode {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("emode cloned (sig: {})", sig);

    Ok(())
}

pub fn bank_migrate_curve(config: Config, bank_pk: Pubkey) -> Result<()> {
    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::MigrateCurve { bank: bank_pk }.to_account_metas(Some(true)),
        data: marginfi::instruction::MigrateCurve {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("curve migrated (sig: {})", sig);

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

pub fn bank_configure_rate_limits(
    config: Config,
    bank_pk: Pubkey,
    hourly_max_outflow: Option<u64>,
    daily_max_outflow: Option<u64>,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::ConfigureBankRateLimits {
            group: bank.group,
            admin: config.authority(),
            bank: bank_pk,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::ConfigureBankRateLimits {
            hourly_max_outflow,
            daily_max_outflow,
        }
        .data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Rate limits configured (sig: {})", sig);

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

pub fn bank_update_fees_destination(
    config: Config,
    bank_pk: Pubkey,
    destination: Pubkey,
) -> Result<()> {
    let bank: Bank = config.mfi_program.account(bank_pk)?;

    let ix = Instruction {
        program_id: config.program_id,
        accounts: marginfi::accounts::LendingPoolUpdateFeesDestinationAccount {
            group: bank.group,
            bank: bank_pk,
            admin: config.authority(),
            destination_account: destination,
        }
        .to_account_metas(Some(true)),
        data: marginfi::instruction::LendingPoolUpdateFeesDestinationAccount {}.data(),
    };

    let signing_keypairs = config.get_signers(false);
    let sig = send_tx(&config, vec![ix], &signing_keypairs)?;
    println!("Fees destination updated (sig: {})", sig);

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

pub fn bank_write_metadata_entries(
    config: &Config,
    entries: Vec<BankMetadataEntry>,
    options: &BankMetadataWriteOptions,
) -> Result<()> {
    for (index, entry) in entries.iter().enumerate() {
        println!(
            "[{}/{}] processing bank {}",
            index + 1,
            entries.len(),
            entry.bank
        );
        let result = apply_bank_metadata_entry(config, entry, options)?;
        println!(
            "[{}/{}] {} - {} (metadata: {})",
            index + 1,
            entries.len(),
            result.bank,
            metadata_write_status_label(result.status),
            result.metadata
        );
        print_bank_metadata_snapshot(&result.resulting_metadata);
    }

    Ok(())
}
