use anchor_lang::prelude::*;
#[allow(deprecated)]
use anchor_lang::solana_program::{
    account_info::AccountInfo, program::get_return_data, program_pack::Pack,
};
use solana_program::program::invoke;
#[allow(deprecated)]
use solana_program::{
    borsh0_10::try_from_slice_unchecked,
    stake::instruction::get_minimum_delegation,
    stake::program::id as stake_program_id,
    stake::state::{Stake, StakeStateV2},
};
use spl_single_pool::{find_pool_address, find_pool_mint_address, find_pool_stake_address};
use spl_token::state::Mint;

use crate::constants::{FACTOR, LAMPORTS_PER_SOL, SINGLE_POOL_ACCOUNTS_COUNT};
use crate::errors::ErrorCodes;
use crate::state::{Price, Sources};
use library::math::casting::Cast;
use library::math::safe_math::SafeMath;

fn fetch_minimum_delegation_via_cpi() -> Result<u64> {
    let ix = get_minimum_delegation();

    invoke(&ix, &[])?;

    if let Some((pid, data)) = get_return_data() {
        if pid != stake_program_id() || data.len() < 8 {
            return err!(ErrorCodes::InvalidStakePoolReturnParams);
        }

        let mut buf = [0u8; 8];
        buf.copy_from_slice(&data[..8]);
        Ok(u64::from_le_bytes(buf))
    } else {
        err!(ErrorCodes::CpiToStakeProgramFailed)
    }
}

pub fn validate_single_pool_sources(sources: &[Sources]) -> Result<()> {
    if sources.len() != SINGLE_POOL_ACCOUNTS_COUNT {
        return err!(ErrorCodes::InvalidSourcesLength);
    }

    let stake_account_source = &sources[0];
    if !stake_account_source.is_valid() {
        return err!(ErrorCodes::InvalidParams);
    }

    if !stake_account_source.is_single_pool_source() {
        return err!(ErrorCodes::InvalidSource);
    }

    let mint_source = &sources[1];
    if !mint_source.is_single_pool_source() {
        return err!(ErrorCodes::InvalidSource);
    }

    if !mint_source.is_valid() {
        return err!(ErrorCodes::InvalidParams);
    }

    let stake_program_source = &sources[2];
    if stake_program_source.source != stake_program_id() {
        return err!(ErrorCodes::InvalidSource);
    }

    if !stake_program_source.is_single_pool_source() {
        return err!(ErrorCodes::InvalidSource);
    }

    if !stake_program_source.is_valid() {
        return err!(ErrorCodes::InvalidParams);
    }

    Ok(())
}

fn validate_stake_and_mint_accounts(
    voter_pubkey: Pubkey,
    single_pool_stake_account: &AccountInfo,
    single_pool_mint: &AccountInfo,
) -> Result<()> {
    let program_id = spl_single_pool::id();
    let pool = find_pool_address(&program_id, &voter_pubkey);
    let expected_stake = find_pool_stake_address(&program_id, &pool);
    let expected_mint = find_pool_mint_address(&program_id, &pool);

    if expected_stake != single_pool_stake_account.key() {
        return err!(ErrorCodes::SinglePoolInvalidStakeAccount);
    }

    if expected_mint != single_pool_mint.key() {
        return err!(ErrorCodes::SinglePoolInvalidMint);
    }

    Ok(())
}

fn get_single_pool_stake(
    single_pool_stake_account: &AccountInfo,
    single_pool_mint: &AccountInfo,
) -> Result<Stake> {
    #[allow(deprecated)]
    let stake_state =
        try_from_slice_unchecked::<StakeStateV2>(&single_pool_stake_account.data.borrow())?;

    let stake = stake_state
        .stake()
        .ok_or(ErrorCodes::SinglePoolInvalidStakeAccount)?;

    validate_stake_and_mint_accounts(
        stake.delegation.voter_pubkey,
        single_pool_stake_account,
        single_pool_mint,
    )?;

    Ok(stake)
}

pub fn read_single_pool_source(
    single_pool_stake_account: &AccountInfo,
    single_pool_mint: &AccountInfo,
) -> Result<Price> {
    let mint_data = single_pool_mint.try_borrow_data()?;
    let mint = Mint::unpack_from_slice(&mint_data)?;
    let token_supply: u64 = mint.supply;

    if token_supply == 0 {
        return err!(ErrorCodes::SinglePoolTokenSupplyZero);
    }

    let stake = get_single_pool_stake(single_pool_stake_account, single_pool_mint)?;
    let delegation_stake = stake.delegation.stake;

    let minimum_delegation = fetch_minimum_delegation_via_cpi()?;
    let minimum_pool_balance = core::cmp::max(minimum_delegation, LAMPORTS_PER_SOL);

    let active_stake: u128 = delegation_stake.safe_sub(minimum_pool_balance)?.cast()?;

    let price = Price {
        price: active_stake
            .safe_mul(FACTOR)?
            .safe_div(token_supply.cast()?)?,
        exponent: None,
    };

    Ok(price)
}
