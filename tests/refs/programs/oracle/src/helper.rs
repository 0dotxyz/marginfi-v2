use anchor_lang::prelude::*;

use crate::constants::{
    AVG_SLOT_TIME_IN_MILLISECONDS, DEFAULT_DIVISOR, DEFAULT_MULTIPLIER, JUP_LEND_ACCOUNTS_COUNT,
    MAX_AGE_LIQUIDATE, MAX_AGE_OPERATE, RATE_OUTPUT_DECIMALS, SINGLE_POOL_ACCOUNTS_COUNT,
};
use crate::errors::ErrorCodes;
use crate::modules::{
    read_chainlink_source, read_jup_lend_source, read_msol_pool_source, read_pyth_source,
    read_redstone_source, read_single_pool_source, read_stake_pool_source,
};
use crate::state::{SourceType, Sources};
use library::math::safe_math::SafeMath;
use library::math::u256::safe_multiply_divide;

pub struct SourceAccounts<'info> {
    pub sources: Vec<AccountInfo<'info>>,
    pub source_info_index: usize,
}

// returns rate and multiplier delta
fn read_rate_from_source<'info>(
    source: Vec<AccountInfo<'info>>,
    source_info: Sources,
    is_liquidate: Option<bool>,
) -> Result<(u128, u128, u128)> {
    let price = match source_info.source_type {
        SourceType::Pyth => read_pyth_source(&source[0], is_liquidate)?.get()?,

        SourceType::StakePool => read_stake_pool_source(&source[0])?.get()?,

        SourceType::MsolPool => read_msol_pool_source(&source[0])?.get()?,

        SourceType::Redstone => read_redstone_source(&source[0], is_liquidate)?.get()?,

        SourceType::Chainlink => read_chainlink_source(&source[0], is_liquidate)?.get()?,

        // Two accounts are needed for single pool, the first is the stake account and the second is the mint account
        SourceType::SinglePool => read_single_pool_source(&source[0], &source[1])?.get()?,

        // Four accounts for JupLend: Lending, TokenReserve, RateModel, FTokenMint
        SourceType::JupLend => {
            read_jup_lend_source(&source[0], &source[1], &source[2], &source[3])?.get()?
        }

        #[allow(unreachable_patterns)]
        _ => {
            return err!(ErrorCodes::InvalidSource);
        }
    };

    Ok(price)
}

fn get_exchange_rate_for_hop<'info>(
    source: Vec<AccountInfo<'info>>,
    source_info: Sources,
    current_hop_rate: u128,
    is_liquidate: Option<bool>,
) -> Result<u128> {
    let (mut rate, multiplier, divisor) = read_rate_from_source(source, source_info, is_liquidate)?;

    rate = rate.safe_mul(multiplier)?.saturating_div(divisor);

    if rate > 0 && source_info.invert {
        rate = 10u128.pow(RATE_OUTPUT_DECIMALS * 2).saturating_div(rate);
    }

    rate = safe_multiply_divide(rate, current_hop_rate, 10u128.pow(RATE_OUTPUT_DECIMALS))?;

    Ok(rate)
}

fn load_sources_from_remaining_accounts<'info>(
    source_infos: &Vec<Sources>,
    remaining_accounts: &[AccountInfo<'info>],
) -> Result<Vec<SourceAccounts<'info>>> {
    if remaining_accounts.len() != source_infos.len() {
        return err!(ErrorCodes::InvalidParams);
    }

    let mut source_accounts_list: Vec<SourceAccounts<'info>> = Vec::new();
    let mut i = 0;

    while i < remaining_accounts.len() {
        let account = remaining_accounts[i].clone();

        // Verify that source is valid and matches the source info
        source_infos[i].verify_source(&account)?;

        // In single pool, two source accounts are needed
        if source_infos[i].is_single_pool_source() {
            if !source_infos[i + 1].is_single_pool_source() {
                return err!(ErrorCodes::InvalidSource);
            }

            let next_account = remaining_accounts[i + 1].clone();
            source_infos[i + 1].verify_source(&next_account)?;

            source_accounts_list.push(SourceAccounts {
                sources: vec![account, next_account],
                source_info_index: i,
            });

            i += SINGLE_POOL_ACCOUNTS_COUNT;
        } else if source_infos[i].is_jup_lend_source() {
            // JupLend needs 4 accounts: Lending, TokenReserve, RateModel, FTokenMint
            let mut jup_lend_accounts = vec![account];

            for j in 1..JUP_LEND_ACCOUNTS_COUNT {
                if !source_infos[i + j].is_jup_lend_source() {
                    return err!(ErrorCodes::InvalidSource);
                }
                let acc = remaining_accounts[i + j].clone();
                source_infos[i + j].verify_source(&acc)?;
                jup_lend_accounts.push(acc);
            }

            source_accounts_list.push(SourceAccounts {
                sources: jup_lend_accounts,
                source_info_index: i,
            });

            i += JUP_LEND_ACCOUNTS_COUNT;
        } else {
            source_accounts_list.push(SourceAccounts {
                sources: vec![account],
                source_info_index: i,
            });

            i += 1;
        }
    }

    Ok(source_accounts_list)
}

pub fn get_hops_exchange_rate<'info>(
    source_infos: &Vec<Sources>,
    remaining_accounts: &[AccountInfo<'info>],
    is_liquidate: Option<bool>,
) -> Result<u128> {
    let mut rate: u128 = 10u128.pow(RATE_OUTPUT_DECIMALS);

    let source_accounts_list =
        load_sources_from_remaining_accounts(source_infos, remaining_accounts)?;

    for source_account in source_accounts_list.iter() {
        rate = get_exchange_rate_for_hop(
            source_account.sources.clone(),
            source_infos[source_account.source_info_index].clone(),
            rate,
            is_liquidate,
        )?;

        if rate == 0 {
            return err!(ErrorCodes::RateZero);
        }
    }

    Ok(rate)
}

pub fn verify_publish_time<'info>(publish_time: u64, is_liquidate: Option<bool>) -> Result<()> {
    let current_time = Clock::get()?.unix_timestamp as u64;

    // @dev reverts if current_time < publish_time
    if is_liquidate.is_some() && is_liquidate.unwrap() {
        if current_time.safe_sub(publish_time)? > MAX_AGE_LIQUIDATE {
            return err!(ErrorCodes::PriceTooOld);
        }
    } else {
        if current_time.safe_sub(publish_time)? > MAX_AGE_OPERATE {
            return err!(ErrorCodes::PriceTooOld);
        }
    }

    Ok(())
}

pub fn verify_publish_time_with_slot<'info>(
    publish_slot: u64,
    is_liquidate: Option<bool>,
) -> Result<()> {
    let current_slot = Clock::get()?.slot;

    let time_elapsed = current_slot
        .safe_sub(publish_slot)?
        .safe_mul(AVG_SLOT_TIME_IN_MILLISECONDS)?
        .safe_div(1000)?;

    // @dev reverts if current_time < publish_time
    if is_liquidate.is_some() && is_liquidate.unwrap() {
        if time_elapsed > MAX_AGE_LIQUIDATE {
            return err!(ErrorCodes::PriceTooOld);
        }
    } else {
        if time_elapsed > MAX_AGE_OPERATE {
            return err!(ErrorCodes::PriceTooOld);
        }
    }

    Ok(())
}

pub fn get_multiplier_and_divisor(exponent: u32) -> (u128, u128) {
    match exponent > RATE_OUTPUT_DECIMALS {
        true => (
            DEFAULT_MULTIPLIER,
            10u128.pow(exponent - RATE_OUTPUT_DECIMALS),
        ),
        false => (10u128.pow(RATE_OUTPUT_DECIMALS - exponent), DEFAULT_DIVISOR),
    }
}
