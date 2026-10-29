use anchor_lang::prelude::*;
use fixed::types::I80F48;
use spl_stake_pool::state::StakePool;

use crate::{
    state::{
        bank::{BankImpl, BPS_DENOMINATOR},
        interest_rate::{
            calc_interest_payment_for_period, calculate_utilization_split, InterestRateConfigImpl,
        },
        lst_stake_price::MAX_LST_SOL_RATE,
        rate::measured_slots_per_second,
    },
    MarginfiError,
};
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup, UnwindConfig};

pub(super) fn min_pool_tokens_for_lamports(
    stake_pool: &StakePool,
    required_lamports: u64,
    charge_withdrawal_fee: bool,
) -> Result<(u64, u64)> {
    require!(
        stake_pool.pool_token_supply > 0 && stake_pool.total_lamports > 0,
        MarginfiError::ZeroSupplyInStakePool
    );

    let mut min_tokens = 1u64;
    let mut max_tokens = stake_pool.pool_token_supply;
    while min_tokens <= max_tokens {
        let candidate_tokens = min_tokens + (max_tokens - min_tokens) / 2;
        let withdrawal_fee = if charge_withdrawal_fee {
            stake_pool
                .calc_pool_tokens_stake_withdrawal_fee(candidate_tokens)
                .ok_or(MarginfiError::MathError)?
        } else {
            0
        };
        let tokens_after_fee = candidate_tokens
            .checked_sub(withdrawal_fee)
            .ok_or(MarginfiError::MathError)?;
        let lamports_out = stake_pool
            .calc_lamports_withdraw_amount(tokens_after_fee)
            .ok_or(MarginfiError::MathError)?;

        if lamports_out >= required_lamports {
            max_tokens = candidate_tokens.saturating_sub(1);
        } else {
            min_tokens = candidate_tokens.saturating_add(1);
        }
    }

    require!(
        min_tokens <= stake_pool.pool_token_supply,
        MarginfiError::StakePoolValidationFailed
    );

    let withdrawal_fee = if charge_withdrawal_fee {
        stake_pool
            .calc_pool_tokens_stake_withdrawal_fee(min_tokens)
            .ok_or(MarginfiError::MathError)?
    } else {
        0
    };
    Ok((min_tokens, withdrawal_fee))
}

pub(super) fn seconds_until_epoch_end(clock: &Clock) -> Result<u64> {
    let epoch_schedule = EpochSchedule::get()?;
    let slots_per_second = measured_slots_per_second(clock);
    require!(slots_per_second > I80F48::ZERO, MarginfiError::MathError);

    let next_epoch = clock.epoch.checked_add(1).ok_or(MarginfiError::MathError)?;
    let first_slot_next_epoch = epoch_schedule.get_first_slot_in_epoch(next_epoch);
    let last_slot_current_epoch = first_slot_next_epoch.saturating_sub(1);
    let slots_remaining = last_slot_current_epoch.saturating_sub(clock.slot);

    I80F48::from_num(slots_remaining)
        .checked_div(slots_per_second)
        .and_then(|seconds| seconds.checked_ceil())
        .and_then(|seconds| seconds.checked_to_num::<u64>())
        .ok_or_else(|| error!(MarginfiError::MathError))
}

pub(super) struct UnwindQuote {
    pub(super) debt_lamports: u64,
    pub(super) fee_lamports: u64,
}

pub(super) fn quote_unwind_debt_and_fee(
    sol_bank: &Bank,
    sol_bank_key: &Pubkey,
    group: &MarginfiGroup,
    unwind_config: &UnwindConfig,
    marginfi_account: &MarginfiAccount,
    epoch_seconds_remaining: u64,
) -> Result<UnwindQuote> {
    let debt_amount = {
        let balance = marginfi_account
            .lending_account
            .balances
            .iter()
            .find(|balance| balance.is_active() && balance.bank_pk == *sol_bank_key)
            .ok_or(MarginfiError::NoLiabilityFound)?;
        sol_bank.get_liability_amount(balance.liability_shares.into())?
    };

    let total_assets = sol_bank.get_asset_amount(sol_bank.total_asset_shares.into())?;
    let actual_liabilities = sol_bank.get_actual_liability_amount()?;
    let effective_liabilities = sol_bank.get_effective_liability_amount()?;
    let utilization =
        calculate_utilization_split(total_assets, actual_liabilities, effective_liabilities)?;

    let borrowing_rate = sol_bank
        .config
        .interest_rate_config
        .create_interest_rate_calculator(group)
        .calc_interest_rates_split_utilization(utilization.effective, utilization.actual)
        .map_err(|_| MarginfiError::MathError)?
        .borrowing_rate_apr;

    let projected_interest =
        calc_interest_payment_for_period(borrowing_rate, epoch_seconds_remaining, debt_amount)
            .ok_or(MarginfiError::MathError)?;

    // Fee uses simple APR, not compounding.
    let fee_multiplier = I80F48::from_num(unwind_config.fee_multiplier_bps)
        .checked_div(I80F48::from_num(BPS_DENOMINATOR))
        .ok_or(MarginfiError::MathError)?;
    let fee_amount = projected_interest
        .checked_mul(fee_multiplier)
        .ok_or(MarginfiError::MathError)?;

    let debt_lamports = debt_amount
        .checked_ceil()
        .and_then(|value| value.checked_to_num::<u64>())
        .ok_or(MarginfiError::MathError)?;
    let fee_lamports = fee_amount
        .checked_ceil()
        .and_then(|value| value.checked_to_num::<u64>())
        .ok_or(MarginfiError::MathError)?;

    Ok(UnwindQuote {
        debt_lamports,
        fee_lamports,
    })
}

pub(super) struct PoolRedemptionQuote {
    pub(super) pool_tokens_in: u64,
    pub(super) pool_withdrawal_fee_lst: u64,
    pub(super) unwind_fee_lst: u64,
    pub(super) total_lst_required: u64,
}

pub(super) fn convert_lamport_fee_to_lst(
    fee_lamports: u64,
    pool_token_supply: u64,
    total_lamports: u64,
) -> Result<u64> {
    require!(total_lamports > 0, MarginfiError::MathError);
    let fee_lst =
        ((fee_lamports as u128) * (pool_token_supply as u128)).div_ceil(total_lamports as u128);
    u64::try_from(fee_lst).map_err(|_| MarginfiError::MathError.into())
}

pub(super) fn quote_pool_redemption(
    pool: &StakePool,
    stake_pool_key: &Pubkey,
    lst_bank: &Bank,
    manager_fee_account: &AccountInfo<'_>,
    source_pool_token_account: &Pubkey,
    quote: &UnwindQuote,
    max_lst_in: u64,
) -> Result<PoolRedemptionQuote> {
    require_keys_eq!(
        *stake_pool_key,
        lst_bank.config.oracle_keys[1],
        MarginfiError::StakePoolValidationFailed
    );
    require!(
        pool.pool_token_supply > 0,
        MarginfiError::ZeroSupplyInStakePool
    );
    let lst_sol_rate = I80F48::from_num(pool.total_lamports)
        .checked_div(I80F48::from_num(pool.pool_token_supply))
        .ok_or(MarginfiError::MathError)?;
    require!(
        lst_sol_rate > I80F48::ZERO && lst_sol_rate < I80F48::from_num(MAX_LST_SOL_RATE),
        MarginfiError::StakePoolValidationFailed
    );

    // Match the CPI's manager-fee waiver rules.
    let charge_withdrawal_fee = pool.manager_fee_account != *source_pool_token_account
        && pool.check_manager_fee_info(manager_fee_account).is_ok();
    let (pool_tokens_in, pool_withdrawal_fee_lst) =
        min_pool_tokens_for_lamports(pool, quote.debt_lamports, charge_withdrawal_fee)?;

    let unwind_fee_lst = convert_lamport_fee_to_lst(
        quote.fee_lamports,
        pool.pool_token_supply,
        pool.total_lamports,
    )?;

    let total_lst_required = pool_tokens_in
        .checked_add(unwind_fee_lst)
        .ok_or(MarginfiError::MathError)?;
    require!(
        total_lst_required <= max_lst_in,
        MarginfiError::SlippageTooHigh
    );

    Ok(PoolRedemptionQuote {
        pool_tokens_in,
        pool_withdrawal_fee_lst,
        unwind_fee_lst,
        total_lst_required,
    })
}

pub(super) struct UnwindPlan {
    pub(super) quote: UnwindQuote,
    pub(super) redemption: PoolRedemptionQuote,
    pub(super) epoch_seconds_remaining: u64,
}
