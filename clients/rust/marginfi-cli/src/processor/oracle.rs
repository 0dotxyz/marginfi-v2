use crate::config::Config;
use anchor_client::anchor_lang::AnchorDeserialize;
use chrono::{DateTime, Local, TimeZone};
use fixed::types::I80F48;
use marginfi::state::price::{PriceAdapter, PythPushOraclePriceFeed};
use marginfi_type_crate::constants::EXP_10_I80F48;
use pyth_solana_receiver_sdk::price_update::{FeedId, PriceUpdateV2};
use solana_account_decoder::UiAccountEncoding;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_sdk::account_info::IntoAccountInfo;
use solana_sdk::pubkey::Pubkey;
use std::time::{SystemTime, UNIX_EPOCH};
use switchboard_on_demand::PullFeedAccountData;

pub fn find_pyth_push_oracles_for_feed_id(
    rpc_client: &RpcClient,
    feed_id: FeedId,
) -> anyhow::Result<()> {
    let mut res = rpc_client.get_program_accounts_with_config(
        &pyth_solana_receiver_sdk::ID,
        RpcProgramAccountsConfig {
            filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                8 + 32 + 1,
                feed_id.to_vec(),
            ))]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                ..Default::default()
            },
            ..Default::default()
        },
    )?;

    println!("Found {} price feeds", res.len());

    for (ref address, account) in res.iter_mut() {
        let ai = (address, account).into_account_info();
        let price_update_v2 = marginfi::state::price::load_price_update_v2_checked(&ai)?;

        let feed_id = &price_update_v2.price_message.feed_id;
        let current_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let age_secs = current_timestamp - price_update_v2.price_message.publish_time;
        let verification_level = price_update_v2.verification_level;

        let feed_id_hex = hex::encode(feed_id);
        println!(
            "Found pyth account {}, feed_id: 0x{}, min_verification_level: {:?}, age: {}s",
            address, feed_id_hex, verification_level, age_secs
        );
    }

    Ok(())
}

pub fn inspect_pyth_push_feed(config: &Config, address: Pubkey) -> anyhow::Result<()> {
    let mut account = config.mfi_program.rpc().get_account(&address)?;
    let ai = (&address, &mut account).into_account_info();

    let mut data = &ai.try_borrow_data()?[8..];
    let price_update = PriceUpdateV2::deserialize(&mut data)?;

    println!("Pyth Push Feed: {}", address);
    let feed = PythPushOraclePriceFeed::load_unchecked(&ai)?;

    println!(
        "Price: {}",
        feed.get_price_of_type_ignore_conf(
            marginfi::state::price::OraclePriceType::RealTime,
            None
        )?
    );

    let feed_id = price_update.price_message.feed_id;

    println!("Feed id: {:?}", feed_id);
    println!("Feed id hex: 0x{}", hex::encode(feed_id));

    Ok(())
}

pub fn inspect_swb_pull_feed(config: &Config, address: Pubkey) -> anyhow::Result<()> {
    let mut account = config.mfi_program.rpc().get_account(&address)?;

    let ai = (&address, &mut account).into_account_info();
    let feed = PullFeedAccountData::parse(ai.data.borrow())?;

    let price: I80F48 = I80F48::from_num(feed.result.value)
        .checked_div(EXP_10_I80F48[switchboard_on_demand::PRECISION as usize])
        .ok_or_else(|| anyhow::anyhow!("failed to compute SWB price: division overflow"))?;

    let last_updated = feed.last_update_timestamp;
    let current_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let age = current_timestamp.saturating_sub(last_updated);
    let datetime: DateTime<Local> = Local
        .timestamp_opt(last_updated, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("invalid timestamp: {}", last_updated))?;

    println!("price: {}", price);
    println!(
        "last updated: {} (ts: {}; slot {})",
        datetime,
        last_updated,
        feed.result.result_slot().unwrap_or(0)
    );
    println!("age: {}s", age);

    Ok(())
}
