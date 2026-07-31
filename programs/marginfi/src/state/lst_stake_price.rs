//! Pricing helpers for internally-derived, multiplier-style oracles: native SVSP stake pools,
//! Marinade (mSOL), SPL stake-pool LSTs, and Exponent PT tokens. These all price a token as a base
//! value scaled by an exchange rate read from an on-chain account, and are kept out of `price.rs`
//! (which holds the oracle-adapter machinery and the venue integrations).

use crate::state::interest_rate::InterestRateCalc;
use crate::{check, check_eq, math_error, prelude::*};
use anchor_lang::prelude::*;
use exponent_mocks::state::MinimalExponentVault;
use fixed::types::I80F48;
use marginfi_type_crate::pdas::derive_staked_onramp_from_vote;
use marginfi_type_crate::types::{Bank, BankConfig};
use marinade_mocks::state::{MinimalMarinadeState, MSOL_PRICE_PRECISION};
use solana_borsh::v1::try_from_slice_unchecked;
use solana_stake_interface::state::StakeStateV2;

use crate::constants::{
    SANCTUM_SPL_MULTI_STAKE_POOL_ID, SANCTUM_SPL_STAKE_POOL_ID, SPL_STAKE_POOL_ID,
};

/// Sanity ceiling for an LST/mSOL SOL-denominated exchange rate. Guards against a bad byte-slice
/// yielding an absurd multiplier: real rates sit near 1.0, so 200 leaves ample headroom while
/// still catching garbage (e.g. a misaligned read).
const MAX_LST_SOL_RATE: i128 = 200;
/// Upper bound (~5 years) on how far in the future a PT vault may mature, a sanity check on the
/// timestamps decoded from the Exponent vault.
const PT_MAX_MATURITY_HORIZON: i64 = 5 * 365 * 24 * 60 * 60;

pub(crate) fn expected_staked_onramp(bank: &Bank) -> MarginfiResult<Pubkey> {
    if bank.config.oracle_keys[3] != Pubkey::default() {
        return Ok(bank.config.oracle_keys[3]);
    }

    check!(
        bank.integration_acc_1 != Pubkey::default(),
        MarginfiError::StakePoolValidationFailed
    );

    Ok(derive_staked_onramp_from_vote(bank.integration_acc_1))
}

pub(crate) fn staked_pool_net_asset_value(
    pool_stake_info: &AccountInfo,
    pool_onramp_info: &AccountInfo,
    rent: &Rent,
) -> MarginfiResult<u64> {
    let pool_rent_exempt_reserve = rent.minimum_balance(pool_stake_info.data_len());
    let onramp_rent_exempt_reserve = rent.minimum_balance(pool_onramp_info.data_len());

    let main_stake_value = pool_stake_info
        .lamports()
        .saturating_sub(pool_rent_exempt_reserve);
    let onramp_value = pool_onramp_info
        .lamports()
        .saturating_sub(onramp_rent_exempt_reserve);

    Ok(main_stake_value.saturating_add(onramp_value))
}

// To be removed once SVSP update is rolled out (likely in 1.10)
pub(crate) fn legacy_staked_pool_delegated_value(
    pool_stake_info: &AccountInfo,
) -> MarginfiResult<u64> {
    let stake_state = try_from_slice_unchecked::<StakeStateV2>(&pool_stake_info.data.borrow())?;
    let (_, stake) = match stake_state {
        StakeStateV2::Stake(meta, stake, _) => (meta, stake),
        _ => return err!(MarginfiError::StakePoolValidationFailed),
    };

    // Legacy pricing subtracts single-pool's initial non-refundable 1 SOL bootstrap stake.
    Ok(stake
        .delegation
        .stake
        .checked_sub(1_000_000_000)
        .ok_or_else(math_error!())?)
}

pub(crate) fn load_marinade_state<'info>(
    bank_config: &BankConfig,
    state_info: &'info AccountInfo<'info>,
    key_index: usize,
) -> MarginfiResult<AccountLoader<'info, MinimalMarinadeState>> {
    require_keys_eq!(
        *state_info.key,
        bank_config.oracle_keys[key_index],
        MarginfiError::MarinadeStateValidationFailed
    );

    let state_loader: AccountLoader<MinimalMarinadeState> = AccountLoader::try_from(state_info)
        .map_err(|_| MarginfiError::MarinadeStateValidationFailed)?;
    Ok(state_loader)
}

/// mSOL/SOL exchange rate = `msol_price / 2^32` (Marinade's cached rate).
pub(crate) fn marinade_price_multiplier(state: &MinimalMarinadeState) -> MarginfiResult<I80F48> {
    let rate = I80F48::from_num(state.msol_price())
        .checked_div(I80F48::from_num(MSOL_PRICE_PRECISION))
        .ok_or_else(math_error!())?;
    check!(
        rate > I80F48::ZERO && rate < I80F48::from_num(MAX_LST_SOL_RATE),
        MarginfiError::MarinadeStateValidationFailed
    );
    Ok(rate)
}

/// Minimal borsh view of an SPL `StakePool`: the fixed-size prefix up to `total_lamports` /
/// `pool_token_supply`. Trailing bytes are ignored; `account_type` (1 = StakePool) sits at byte 0
/// rather than an 8-byte discriminator, so it deserializes from the start.
#[derive(AnchorDeserialize)]
#[allow(dead_code)] // fields before the two we read only advance the borsh cursor
struct MinimalStakePool {
    account_type: u8,
    manager: Pubkey,
    staker: Pubkey,
    stake_deposit_authority: Pubkey,
    stake_withdraw_bump_seed: u8,
    validator_list: Pubkey,
    reserve_stake: Pubkey,
    pool_mint: Pubkey,
    manager_fee_account: Pubkey,
    token_program_id: Pubkey,
    total_lamports: u64,
    pool_token_supply: u64,
}

/// Parses and validates the *identity* of an SPL / Sanctum stake pool — key, owner (vanilla SPL or
/// Sanctum's SPL / SPL-Multi forks), and StakePool account type — and returns the decoded view.
fn load_stake_pool(
    bank_config: &BankConfig,
    stake_pool_info: &AccountInfo,
    key_index: usize,
) -> MarginfiResult<MinimalStakePool> {
    require_keys_eq!(
        *stake_pool_info.key,
        bank_config.oracle_keys[key_index],
        MarginfiError::StakePoolValidationFailed
    );

    let owner = stake_pool_info.owner;
    check!(
        owner == &SPL_STAKE_POOL_ID
            || owner == &SANCTUM_SPL_STAKE_POOL_ID
            || owner == &SANCTUM_SPL_MULTI_STAKE_POOL_ID,
        MarginfiError::StakePoolValidationFailed
    );

    let data = stake_pool_info.try_borrow_data()?;
    let pool = try_from_slice_unchecked::<MinimalStakePool>(&data)
        .map_err(|_| MarginfiError::StakePoolValidationFailed)?;
    check!(
        pool.account_type == 1,
        MarginfiError::StakePoolValidationFailed
    );
    Ok(pool)
}

pub(crate) fn validate_stake_pool_account(
    bank_config: &BankConfig,
    stake_pool_info: &AccountInfo,
    key_index: usize,
    bank_mint: Pubkey,
) -> MarginfiResult {
    let pool = load_stake_pool(bank_config, stake_pool_info, key_index)?;
    check_eq!(
        pool.pool_mint,
        bank_mint,
        MarginfiError::StakePoolValidationFailed
    );
    Ok(())
}

/// Pricing: LST/SOL exchange rate = `total_lamports / pool_token_supply`, with sanity bounds.
pub(crate) fn stake_pool_price_multiplier(
    bank_config: &BankConfig,
    stake_pool_info: &AccountInfo,
    key_index: usize,
) -> MarginfiResult<I80F48> {
    let pool = load_stake_pool(bank_config, stake_pool_info, key_index)?;
    check!(
        pool.pool_token_supply > 0,
        MarginfiError::ZeroSupplyInStakePool
    );

    let rate = I80F48::from_num(pool.total_lamports)
        .checked_div(I80F48::from_num(pool.pool_token_supply))
        .ok_or_else(math_error!())?;
    check!(
        rate < I80F48::from_num(MAX_LST_SOL_RATE),
        MarginfiError::StakePoolValidationFailed
    );
    Ok(rate)
}

pub(crate) fn validate_marinade_state_account<'info>(
    bank_config: &BankConfig,
    info: &'info AccountInfo<'info>,
    key_index: usize,
    bank_mint: Pubkey,
) -> MarginfiResult {
    // Constructing the loader validates the key, owner, and discriminator.
    let state_loader = load_marinade_state(bank_config, info, key_index)?;
    check_eq!(
        state_loader.load()?.msol_mint(),
        bank_mint,
        MarginfiError::MarinadeStateValidationFailed
    );
    Ok(())
}

pub(crate) fn load_exponent_vault<'info>(
    bank_config: &BankConfig,
    vault_info: &'info AccountInfo<'info>,
    key_index: usize,
) -> MarginfiResult<AccountLoader<'info, MinimalExponentVault>> {
    require_keys_eq!(
        *vault_info.key,
        bank_config.oracle_keys[key_index],
        MarginfiError::ExponentVaultValidationFailed
    );
    let loader: AccountLoader<MinimalExponentVault> = AccountLoader::try_from(vault_info)
        .map_err(|_| MarginfiError::ExponentVaultValidationFailed)?;
    Ok(loader)
}

/// PT linear rate: accretion from `start_price` to par (1.0) over the vault's
/// `[start_ts, start_ts + duration]`, clamped at both ends.
pub(crate) fn pt_linear_multiplier(
    vault: &MinimalExponentVault,
    clock: &Clock,
    start_price: I80F48,
) -> MarginfiResult<I80F48> {
    let start_ts = vault.start_ts() as i64;
    let maturity = start_ts
        .checked_add(vault.duration() as i64)
        .ok_or_else(math_error!())?;
    let now = clock.unix_timestamp;

    // Sanity-check the decoded schedule against a garbage byte-slice: maturity must be after the
    // start and no more than ~5 years out, which comfortably bounds any real Exponent vault.
    let horizon = now
        .checked_add(PT_MAX_MATURITY_HORIZON)
        .ok_or_else(math_error!())?;
    check!(
        maturity > start_ts && maturity <= horizon,
        MarginfiError::ExponentVaultValidationFailed
    );

    Ok(InterestRateCalc::lerp(
        I80F48::from_num(start_ts),
        start_price,
        I80F48::from_num(maturity),
        I80F48::ONE,
        I80F48::from_num(now),
    )
    .ok_or_else(math_error!())?)
}
