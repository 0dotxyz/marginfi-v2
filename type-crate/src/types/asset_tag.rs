use crate::constants::{
    ASSET_TAG_DEFAULT, ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOL,
    ASSET_TAG_SOLEND, ASSET_TAG_STAKED,
};

use super::{Bank, MarginfiAccount};

pub fn is_integration_asset_tag(asset_tag: u8) -> bool {
    matches!(
        asset_tag,
        ASSET_TAG_KAMINO | ASSET_TAG_DRIFT | ASSET_TAG_SOLEND | ASSET_TAG_JUPLEND
    )
}

pub fn is_marginfi_asset_tag(asset_tag: u8) -> bool {
    matches!(
        asset_tag,
        ASSET_TAG_DEFAULT | ASSET_TAG_SOL | ASSET_TAG_STAKED
    )
}

pub fn is_closeable_asset_tag(asset_tag: u8) -> bool {
    is_marginfi_asset_tag(asset_tag) || is_integration_asset_tag(asset_tag)
}

fn is_default_like(asset_tag: u8) -> bool {
    asset_tag == ASSET_TAG_DEFAULT || is_integration_asset_tag(asset_tag)
}

/// Validate that after a deposit to Bank, the users's account contains either all Default/SOL
/// balances, or all Staked/Sol balances. Default and Staked assets cannot mix.
pub fn validate_asset_tags(bank: &Bank, marginfi_account: &MarginfiAccount) -> bool {
    let mut has_default_asset = false;
    let mut has_staked_asset = false;

    for balance in marginfi_account.lending_account.balances.iter() {
        if balance.is_active() {
            match balance.bank_asset_tag {
                ASSET_TAG_DEFAULT => has_default_asset = true,
                ASSET_TAG_SOL => { /* Do nothing, SOL can mix with any asset type */ }
                ASSET_TAG_STAKED => has_staked_asset = true,
                // Kamino/Drift/Solend/JupLend assets behave like default assets
                tag if is_integration_asset_tag(tag) => has_default_asset = true,
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
