pub mod bank;
pub mod bank_cache;
pub mod bank_config;
pub mod bank_metadata;
pub mod emode;
pub mod fee_state;
pub mod group;
pub mod health_cache;
pub mod history_archive;
pub mod interest_rate;
pub mod liquidation_record;
pub mod monitor_snapshot;
pub mod order;
pub mod panic_state_cache;
pub mod price;
pub mod pubkey;
pub mod rate_limiter;
pub mod staked_settings;
pub mod user_account;
pub mod wrapped_i80f48;

pub use bank::*;
pub use bank_cache::*;
pub use bank_config::*;
pub use bank_metadata::*;
pub use emode::*;
pub use fee_state::*;
pub use group::*;
pub use health_cache::*;
pub use history_archive::*;
pub use interest_rate::*;
pub use liquidation_record::*;
pub use monitor_snapshot::*;
pub use order::*;
pub use panic_state_cache::*;
pub use price::*;
pub use pubkey::*;
pub use rate_limiter::*;
pub use staked_settings::*;
pub use user_account::*;
pub use wrapped_i80f48::*;

use crate::constants::{
    ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOL,
    ASSET_TAG_SOLEND, ASSET_TAG_STAKED,
};

#[derive(Copy, Clone, Debug)]
pub enum OraclePriceType {
    /// Time weighted price
    /// EMA for PythEma
    TimeWeighted,
    /// Real time price
    RealTime,
}

#[derive(Copy, Clone)]
pub enum RequirementType {
    Initial,
    Maintenance,
    Equity,
}

impl RequirementType {
    /// Get oracle price type for the requirement type.
    ///
    /// Initial and equity requirements use the time weighted price feed.
    /// Maintenance requirement uses the real time price feed, as its more accurate for triggering liquidations.
    pub fn get_oracle_price_type(&self) -> OraclePriceType {
        match self {
            RequirementType::Initial | RequirementType::Equity => OraclePriceType::TimeWeighted,
            RequirementType::Maintenance => OraclePriceType::RealTime,
        }
    }
}

/// Validate that after a deposit to Bank, the users's account contains either all Default/SOL
/// balances, or all Staked/Sol balances. Default and Staked assets cannot mix.
pub fn validate_asset_tags(bank: &Bank, marginfi_account: &MarginfiAccount) -> bool {
    let mut has_default_asset = false;
    let mut has_staked_asset = false;

    let is_default_like = |asset_tag: u8| {
        matches!(
            asset_tag,
            ASSET_TAG_DEFAULT
                | ASSET_TAG_KAMINO
                | ASSET_TAG_DRIFT
                | ASSET_TAG_SOLEND
                | ASSET_TAG_JUPLEND
        )
    };

    for balance in marginfi_account.lending_account.balances.iter() {
        if balance.is_active() {
            match balance.bank_asset_tag {
                ASSET_TAG_DEFAULT => has_default_asset = true,
                ASSET_TAG_SOL => { /* Do nothing, SOL can mix with any asset type */ }
                ASSET_TAG_STAKED => has_staked_asset = true,
                // Kamino/Drift/Solend/JupLend assets behave like default assets
                ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND | ASSET_TAG_JUPLEND => {
                    has_default_asset = true
                }
                _ => panic!("unsupported asset tag"),
            }
        }
    }

    // 1. Default-like assets cannot mix with Staked assets
    if is_default_like(bank.config.asset_tag) && has_staked_asset {
        return false;
    }

    // 2. Staked SOL cannot mix with Default-like assets
    if bank.config.asset_tag == ASSET_TAG_STAKED && has_default_asset {
        return false;
    }

    true
}

/// Validate that two banks are compatible based on their asset tags. See the following combinations
/// (* is wildcard, e.g. any tag):
///
/// Allowed:
/// 1) Default/Default
/// 2) Sol/*
/// 3) Staked/Staked
///
/// Forbidden:
/// 1) Default/Staked
///
/// Returns an error if the two banks have mismatching asset tags according to the above.
pub fn validate_bank_asset_tags(bank_a: &Bank, bank_b: &Bank) -> bool {
    let is_default_like = |asset_tag: u8| {
        matches!(
            asset_tag,
            ASSET_TAG_DEFAULT
                | ASSET_TAG_KAMINO
                | ASSET_TAG_DRIFT
                | ASSET_TAG_SOLEND
                | ASSET_TAG_JUPLEND
        )
    };

    let is_bank_a_default = is_default_like(bank_a.config.asset_tag);
    let is_bank_a_staked = bank_a.config.asset_tag == ASSET_TAG_STAKED;
    let is_bank_b_default = is_default_like(bank_b.config.asset_tag);
    let is_bank_b_staked = bank_b.config.asset_tag == ASSET_TAG_STAKED;
    // Note: Sol is compatible with all other tags and doesn't matter...

    // 1. Default assets cannot mix with Staked assets
    if is_bank_a_default && is_bank_b_staked {
        return false;
    }
    if is_bank_a_staked && is_bank_b_default {
        return false;
    }

    true
}
