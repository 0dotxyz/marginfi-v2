use std::{fs::File, io::Read, path::PathBuf, str::FromStr};

use anchor_lang::solana_program::pubkey;
use anchor_lang::AccountDeserialize;
use anyhow::bail;
use base64::{prelude::BASE64_STANDARD, Engine};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::ASSET_TAG_DEFAULT,
    types::{
        Bank, BankCache, BankOperationalState, HealthCache, MarginfiAccount, OracleSetup, RiskTier,
    },
};
use solana_account_decoder::UiAccountData;
use solana_cli_output::CliAccount;
use solana_program_test::tokio;
use solana_sdk::pubkey::Pubkey;

#[tokio::test]
async fn account_field_values_reg() -> anyhow::Result<()> {
    let account_fixtures_path = "tests/fixtures/marginfi_account";

    // Sample 1

    let mut path = PathBuf::from_str(account_fixtures_path).unwrap();
    path.push("marginfi_account_sample_1.json");
    let mut file = File::open(&path).unwrap();
    let mut account_info_raw = String::new();
    file.read_to_string(&mut account_info_raw).unwrap();

    let account: CliAccount = serde_json::from_str(&account_info_raw).unwrap();
    let UiAccountData::Binary(data, _) = account.keyed_account.account.data else {
        bail!("Expecting Binary format for fixtures")
    };
    let account = MarginfiAccount::try_deserialize(&mut BASE64_STANDARD.decode(data)?.as_slice())?;

    assert_eq!(
        account.group,
        pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8")
    );
    assert_eq!(
        account.authority,
        pubkey!("Dq7wypbedtaqQK9QqEFvfrxc4ppfRGXCeTVd7ee7n2jw")
    );
    assert_eq!(account.account_flags, 0);
    // health cache doesn't exist on these old accounts, but it also doesn't matter since it's read-only
    assert_eq!(account.health_cache, HealthCache::zeroed());
    assert_eq!(account.migrated_from, Pubkey::default());
    assert_eq!(account.migrated_to, Pubkey::default());
    assert_eq!(account.last_update, 0);
    assert_eq!(account.account_index, 0);
    assert_eq!(account.third_party_index, 0);
    assert_eq!(account.bump, 0);
    assert_eq!(account._pad0, [0; 3]);
    assert_eq!(account.liquidation_record, Pubkey::default());
    assert_eq!(account._padding0, [0; 7]);

    let balance_1 = account.lending_account.balances[0];
    assert!(balance_1.is_active());
    assert_eq!(
        balance_1.bank_pk,
        pubkey!("2s37akK2eyBbp8DZgCm7RtsaEz8eJP3Nxd4urLHQv7yB")
    );
    assert_eq!(balance_1.bank_asset_tag, ASSET_TAG_DEFAULT);
    assert_eq!(balance_1._pad0, [0; 6]);
    assert_eq!(
        I80F48::from(balance_1.asset_shares),
        I80F48::from_str("1650216221.466876226897366").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.liability_shares),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.emissions_outstanding),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.last_update),
        I80F48::from_str("1711158766").unwrap()
    );
    assert_eq!(balance_1._padding, [0; 1]);

    let balance_2 = account.lending_account.balances[1];
    assert!(balance_2.is_active());
    assert_eq!(
        balance_2.bank_pk,
        pubkey!("CCKtUs6Cgwo4aaQUmBPmyoApH2gUDErxNZCAntD6LYGh")
    );
    assert_eq!(balance_2.bank_asset_tag, ASSET_TAG_DEFAULT);
    assert_eq!(balance_2._pad0, [0; 6]);
    assert_eq!(
        I80F48::from(balance_2.asset_shares),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_2.liability_shares),
        I80F48::from_str("3806372611.588862122556122").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_2.emissions_outstanding),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_2.last_update),
        I80F48::from_str("1711158793").unwrap()
    );
    assert_eq!(balance_2._padding, [0; 1]);

    // Sample 2

    let mut path = PathBuf::from_str(account_fixtures_path).unwrap();
    path.push("marginfi_account_sample_2.json");
    let mut file = File::open(&path).unwrap();
    let mut account_info_raw = String::new();
    file.read_to_string(&mut account_info_raw).unwrap();

    let account: CliAccount = serde_json::from_str(&account_info_raw).unwrap();
    let UiAccountData::Binary(data, _) = account.keyed_account.account.data else {
        bail!("Expecting Binary format for fixtures")
    };
    let account = MarginfiAccount::try_deserialize(&mut BASE64_STANDARD.decode(data)?.as_slice())?;

    assert_eq!(
        account.group,
        pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8")
    );
    assert_eq!(
        account.authority,
        pubkey!("3T1kGHp7CrdeW9Qj1t8NMc2Ks233RyvzVhoaUPWoBEFK")
    );
    assert_eq!(account.account_flags, 0);
    assert_eq!(account.last_update, 0);
    assert_eq!(account._padding0, [0; 7]);

    let balance_1 = account.lending_account.balances[0];
    assert!(balance_1.is_active());
    assert_eq!(
        balance_1.bank_pk,
        pubkey!("6hS9i46WyTq1KXcoa2Chas2Txh9TJAVr6n1t3tnrE23K")
    );
    assert_eq!(balance_1.bank_asset_tag, ASSET_TAG_DEFAULT);
    assert_eq!(balance_1._pad0, [0; 6]);
    assert_eq!(
        I80F48::from(balance_1.asset_shares),
        I80F48::from_str("470.952530958931234").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.liability_shares),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.emissions_outstanding),
        I80F48::from_str("26891413.388324654086347").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.last_update),
        I80F48::from_str("1705760628").unwrap()
    );
    assert_eq!(balance_1._padding, [0; 1]);

    let balance_2 = account.lending_account.balances[1];
    assert!(!balance_2.is_active());
    assert_eq!(
        balance_2.bank_pk,
        pubkey!("11111111111111111111111111111111")
    );
    assert_eq!(balance_2.bank_asset_tag, ASSET_TAG_DEFAULT);
    assert_eq!(balance_2._pad0, [0; 6]);
    assert_eq!(
        I80F48::from(balance_2.asset_shares),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_2.liability_shares),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_2.emissions_outstanding),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_2.last_update),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(balance_2._padding, [0; 1]);

    // Sample 3

    let mut path = PathBuf::from_str(account_fixtures_path).unwrap();
    path.push("marginfi_account_sample_3.json");
    let mut file = File::open(&path).unwrap();
    let mut account_info_raw = String::new();
    file.read_to_string(&mut account_info_raw).unwrap();

    let account: CliAccount = serde_json::from_str(&account_info_raw).unwrap();
    let UiAccountData::Binary(data, _) = account.keyed_account.account.data else {
        bail!("Expecting Binary format for fixtures")
    };
    let account = MarginfiAccount::try_deserialize(&mut BASE64_STANDARD.decode(data)?.as_slice())?;

    assert_eq!(
        account.group,
        pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8")
    );
    assert_eq!(
        account.authority,
        pubkey!("7hmfVTuXc7HeX3YQjpiCXGVQuTeXonzjp795jorZukVR")
    );
    assert_eq!(account.account_flags, 0);
    assert_eq!(account.last_update, 0);
    assert_eq!(account.account_index, 0);
    assert_eq!(account.third_party_index, 0);
    assert_eq!(account.bump, 0);
    assert_eq!(account._pad0, [0; 3]);
    assert_eq!(account.liquidation_record, Pubkey::default());
    assert_eq!(account._padding0, [0; 7]);

    let balance_1 = account.lending_account.balances[0];
    assert!(!balance_1.is_active());
    assert_eq!(
        balance_1.bank_pk,
        pubkey!("11111111111111111111111111111111")
    );
    assert_eq!(balance_1.bank_asset_tag, ASSET_TAG_DEFAULT);
    assert_eq!(balance_1._pad0, [0; 6]);
    assert_eq!(
        I80F48::from(balance_1.asset_shares),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.liability_shares),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.emissions_outstanding),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(balance_1.last_update),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(balance_1._padding, [0; 1]);

    Ok(())
}

#[tokio::test]
async fn bank_field_values_reg() -> anyhow::Result<()> {
    let bank_fixtures_path = "tests/fixtures/bank";

    // Sample 1 (Jito)

    let mut path = PathBuf::from_str(bank_fixtures_path).unwrap();
    path.push("bank_sample_1.json");
    let mut file = File::open(&path).unwrap();
    let mut account_info_raw = String::new();
    file.read_to_string(&mut account_info_raw).unwrap();

    let account: CliAccount = serde_json::from_str(&account_info_raw).unwrap();
    let UiAccountData::Binary(data, _) = account.keyed_account.account.data else {
        bail!("Expecting Binary format for fixtures")
    };
    let bank = Bank::try_deserialize(&mut BASE64_STANDARD.decode(data)?.as_slice())?;

    assert_eq!(
        bank.mint,
        pubkey!("J1toso1uCk3RLmjorhTtrVwY9HJ7X8V9yYac6Y7kGCPn")
    );
    assert_eq!(bank.mint_decimals, 9);
    assert_eq!(
        bank.group,
        pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8")
    );
    assert_eq!(
        I80F48::from(bank.asset_share_value),
        I80F48::from_str("1.000561264812955").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.liability_share_value),
        I80F48::from_str("1.00737674726716").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.collected_insurance_fees_outstanding),
        I80F48::from_str("61174.580321107215052").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.collected_group_fees_outstanding),
        I80F48::from_str("35660072279.35465946938668").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.total_liability_shares),
        I80F48::from_str("79763493059362.858709822356737").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.total_asset_shares),
        I80F48::from_str("998366336320727.44918120920092").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.asset_weight_init),
        I80F48::from_str("0.649999976158142").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.asset_weight_maint),
        I80F48::from_str("0.80000001192093").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.liability_weight_init),
        I80F48::from_str("1.3").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.liability_weight_maint),
        I80F48::from_str("1.2").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.optimal_utilization_rate),
        I80F48::from_str("0.8").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.plateau_interest_rate),
        I80F48::from_str("0.1").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.max_interest_rate),
        I80F48::from_str("3").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.insurance_fee_fixed_apr),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.insurance_ir_fee),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.protocol_fixed_fee_apr),
        I80F48::from_str("0.01").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.protocol_ir_fee),
        I80F48::from_str("0.05").unwrap()
    );

    // Sample 2 (META)

    let mut path = PathBuf::from_str(bank_fixtures_path).unwrap();
    path.push("bank_sample_2.json");
    let mut file = File::open(&path).unwrap();
    let mut account_info_raw = String::new();
    file.read_to_string(&mut account_info_raw).unwrap();

    let account: CliAccount = serde_json::from_str(&account_info_raw).unwrap();
    let UiAccountData::Binary(data, _) = account.keyed_account.account.data else {
        bail!("Expecting Binary format for fixtures")
    };
    let bank = Bank::try_deserialize(&mut BASE64_STANDARD.decode(data)?.as_slice())?;

    assert_eq!(
        bank.mint,
        pubkey!("METADDFL6wWMWEoKTFJwcThTbUmtarRJZjRpzUvkxhr")
    );
    assert_eq!(bank.mint_decimals, 9);
    assert_eq!(
        bank.group,
        pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8")
    );
    assert_eq!(
        I80F48::from(bank.asset_share_value),
        I80F48::from_str("1").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.liability_share_value),
        I80F48::from_str("1").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.collected_insurance_fees_outstanding),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.collected_group_fees_outstanding),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.total_liability_shares),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.total_asset_shares),
        I80F48::from_str("698503862367").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.asset_weight_init),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.asset_weight_maint),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.liability_weight_init),
        I80F48::from_str("2.5").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.liability_weight_maint),
        I80F48::from_str("1.5").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.optimal_utilization_rate),
        I80F48::from_str("0.8").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.plateau_interest_rate),
        I80F48::from_str("0.1").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.max_interest_rate),
        I80F48::from_str("3").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.insurance_fee_fixed_apr),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.insurance_ir_fee),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.protocol_fixed_fee_apr),
        I80F48::from_str("0.01").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.protocol_ir_fee),
        I80F48::from_str("0.05").unwrap()
    );

    // Sample 3 (USDT)

    let mut path = PathBuf::from_str(bank_fixtures_path).unwrap();
    path.push("bank_sample_3.json");
    let mut file = File::open(&path).unwrap();
    let mut account_info_raw = String::new();
    file.read_to_string(&mut account_info_raw).unwrap();

    let account: CliAccount = serde_json::from_str(&account_info_raw).unwrap();
    let UiAccountData::Binary(data, _) = account.keyed_account.account.data else {
        bail!("Expecting Binary format for fixtures")
    };
    let bank = Bank::try_deserialize(&mut BASE64_STANDARD.decode(data)?.as_slice())?;

    assert_eq!(
        bank.mint,
        pubkey!("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB")
    );
    assert_eq!(bank.mint_decimals, 6);
    assert_eq!(
        bank.group,
        pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8")
    );
    assert_eq!(
        I80F48::from(bank.asset_share_value),
        I80F48::from_str("1.063003765188338").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.liability_share_value),
        I80F48::from_str("1.12089611736063").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.collected_insurance_fees_outstanding),
        I80F48::from_str("45839.746526861401865").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.collected_group_fees_outstanding),
        I80F48::from_str("28634360131.219557095675654").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.total_liability_shares),
        I80F48::from_str("32109684419718.204607882232235").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.total_asset_shares),
        I80F48::from_str("43231381120800.339303417329994").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.asset_weight_init),
        I80F48::from_str("1").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.asset_weight_maint),
        I80F48::from_str("1").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.liability_weight_init),
        I80F48::from_str("1.25").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.liability_weight_maint),
        I80F48::from_str("1.1").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.optimal_utilization_rate),
        I80F48::from_str("0.8").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.plateau_interest_rate),
        I80F48::from_str("0.2").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.max_interest_rate),
        I80F48::from_str("4").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.insurance_fee_fixed_apr),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.insurance_ir_fee),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.protocol_fixed_fee_apr),
        I80F48::from_str("0.01").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.protocol_ir_fee),
        I80F48::from_str("0.05").unwrap()
    );

    // Sample 4 (PyUSD)

    let mut path = PathBuf::from_str(bank_fixtures_path).unwrap();
    path.push("bank_pyusd_230822.json");
    let mut file = File::open(&path).unwrap();
    let mut account_info_raw = String::new();
    file.read_to_string(&mut account_info_raw).unwrap();

    let account: CliAccount = serde_json::from_str(&account_info_raw).unwrap();
    let UiAccountData::Binary(data, _) = account.keyed_account.account.data else {
        bail!("Expecting Binary format for fixtures")
    };
    let bank = Bank::try_deserialize(&mut BASE64_STANDARD.decode(data)?.as_slice())?;

    assert_eq!(
        bank.mint,
        pubkey!("2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo")
    );
    assert_eq!(bank.mint_decimals, 6);
    assert_eq!(
        bank.group,
        pubkey!("4qp6Fx6tnZkY5Wropq9wUYgtFxXKwE6viZxFHg3rdAG8")
    );
    assert_eq!(bank._pad0, [0; 7]);
    assert_eq!(
        I80F48::from(bank.asset_share_value),
        I80F48::from_str("1.000087706703773").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.liability_share_value),
        I80F48::from_str("1.00157842522853").unwrap()
    );
    assert_eq!(
        bank.liquidity_vault,
        pubkey!("HUmHLAXvcoUgWAtanAnCPNssBnAUzEfSRsb4MZYw7R73")
    );
    assert_eq!(bank.liquidity_vault_bump, 249);
    assert_eq!(bank.liquidity_vault_authority_bump, 255);
    assert_eq!(
        bank.insurance_vault,
        pubkey!("4jE6g1pLG5NmW19z9DgzjCwYEDy3jYSqix1Lv5BN4yEc")
    );
    assert_eq!(bank.insurance_vault_bump, 255);
    assert_eq!(bank.insurance_vault_authority_bump, 254);
    assert_eq!(bank._pad1, [0; 4]);
    assert_eq!(
        I80F48::from(bank.collected_insurance_fees_outstanding),
        I80F48::from_str("3459.51398842307891").unwrap()
    );
    assert_eq!(
        bank.fee_vault,
        pubkey!("AzVkUZFuvGwy3pD6U2eAJJNhHYMnzkzBFjjhn8CSVFRa")
    );
    assert_eq!(bank.fee_vault_bump, 255);
    assert_eq!(bank.fee_vault_authority_bump, 253);
    assert_eq!(bank._pad2, [0; 6]);
    assert_eq!(
        I80F48::from(bank.collected_group_fees_outstanding),
        I80F48::from_str("1015697146.551430542063226").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.total_liability_shares),
        I80F48::from_str("1997270567603.702517235172913").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.total_asset_shares),
        I80F48::from_str("39996492038102.214176640606695").unwrap()
    );
    assert_eq!(bank.last_update, 1724302256);

    assert_eq!(
        I80F48::from(bank.config.asset_weight_init),
        I80F48::from_str("0.899999976158142").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.asset_weight_maint),
        I80F48::from_str("0.94999998807907").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.liability_weight_init),
        I80F48::from_str("1.25").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.liability_weight_maint),
        I80F48::from_str("1.1").unwrap()
    );
    assert_eq!(bank.config.deposit_limit, 40000000000000);
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.optimal_utilization_rate),
        I80F48::from_str("0.8").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.plateau_interest_rate),
        I80F48::from_str("0.1").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.max_interest_rate),
        I80F48::from_str("3").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.insurance_fee_fixed_apr),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.insurance_ir_fee),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.protocol_fixed_fee_apr),
        I80F48::from_str("0.01").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.config.interest_rate_config.protocol_ir_fee),
        I80F48::from_str("0.05").unwrap()
    );
    assert_eq!(
        bank.config.operational_state,
        BankOperationalState::Operational
    );
    assert_eq!(bank.config.oracle_setup, OracleSetup::PythPushOracle);
    assert_eq!(
        bank.config.oracle_keys,
        [
            pubkey!("E3iagrg2kXyNJ9Ad2R2pNUsRmXutyQScu3m1FcQmBsAH"),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default(),
            Pubkey::default()
        ]
    );
    assert_eq!(bank.config._pad0, [0; 6]);
    assert_eq!(bank.config.borrow_limit, 2000000000000);
    assert_eq!(bank.config.risk_tier, RiskTier::Collateral);
    assert_eq!(bank.config.asset_tag, ASSET_TAG_DEFAULT);
    // Note: created before 0.1.4 Pyth pull migration.
    assert_eq!(bank.config.config_flags, 0);
    assert_eq!(bank.config._pad1, [0; 5]);
    assert_eq!(bank.config.total_asset_value_init_limit, 0);
    assert_eq!(bank.config.oracle_max_age, 300);
    assert_eq!(bank.config._padding0, [0; 2]);
    // Note: legacy banks that have a 0 value here will use 10%
    assert_eq!(bank.config.oracle_max_confidence, 0);
    assert_eq!(bank.config.fixed_price, I80F48::ZERO.into());
    assert_eq!(bank.config._padding1, [0; 16]);

    assert_eq!(
        I80F48::from(bank.emissions_rate),
        I80F48::from_str("170000").unwrap()
    );
    assert_eq!(
        I80F48::from(bank.emissions_remaining),
        I80F48::from_str("130585694893.67407796351017").unwrap()
    );
    assert_eq!(
        bank.emissions_mint,
        pubkey!("2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo")
    );
    // Legacy banks have no program fees
    assert_eq!(
        I80F48::from(bank.collected_program_fees_outstanding),
        I80F48::from_str("0").unwrap()
    );
    assert_eq!(bank.fees_destination_account, Pubkey::default());
    assert_eq!(bank.cache, BankCache::default());

    assert_eq!(bank.lending_position_count, 0);
    assert_eq!(bank.borrowing_position_count, 0);
    assert_eq!(bank._padding_0, [0; 16]);
    assert_eq!(bank.integration_acc_1, Pubkey::default());
    assert_eq!(bank.integration_acc_2, Pubkey::default());
    assert_eq!(bank.integration_acc_3, Pubkey::default());
    assert_eq!(bank._padding_1, [[0, 0]; 8]);

    Ok(())
}
