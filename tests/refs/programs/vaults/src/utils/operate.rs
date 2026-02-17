use anchor_lang::prelude::*;

use crate::{
    constants::{
        EXCHANGE_PRICES_PRECISION, FOUR_DECIMALS, LOWER_DECIMALS_OPERATE, MAX_OPERATE, MIN_OPERATE,
        MIN_OPERATE_LOWER_DECIMALS_AMOUNT, THREE_DECIMALS,
    },
    errors::ErrorCodes,
    invokes::{OracleCpiAccounts, RATE_OUTPUT_DECIMALS},
    state::*,
};

use library::math::{casting::*, safe_math::*, tick::TickMath, u256::safe_multiply_divide};
use liquidity::state::UserSupplyPosition;

pub fn get_sanitized_exchange_rate(
    exchange_rate: &mut u128,
    supply_ex_price: u128,
    borrow_ex_price: u128,
) -> Result<u128> {
    let mut exchange_rate = exchange_rate.clone();

    // Note if price would come back as 0 `get_tick_at_ratio` will fail
    if exchange_rate > 10u128.pow(24) || exchange_rate < 10u128.pow(6) {
        // capping to 1B USD per 1 Bitcoin at 15 oracle precision
        return Err(error!(ErrorCodes::VaultInvalidOraclePrice));
    }

    exchange_rate = safe_multiply_divide(exchange_rate, supply_ex_price, borrow_ex_price)?;

    if exchange_rate == 0 {
        return Err(error!(ErrorCodes::VaultInvalidOraclePrice));
    }

    // capping oracle pricing: This means we are restricting collateral price to never go above 1e26
    // Above 1e26 precisions gets too low for calculations
    // This will never happen for all good token pairs (for example, WBTC/USD pair when WBTC price is $1M, oracle price will come as 1e21)
    // Restricting oracle price doesn't pose any risk to protocol as we are capping collateral price, meaning if price is above 1e26
    // user is simply not able to borrow more

    if exchange_rate > 10u128.pow(26) {
        exchange_rate = 10u128.pow(26);
    }

    Ok(exchange_rate)
}

pub fn check_if_position_safe<'info>(
    ctx: &Context<'_, '_, 'info, 'info, Operate<'info>>,
    memory_vars: &Box<OperateMemoryVars>,
    collateral_factor: u128,
    liquidation_threshold: u128,
    supply_ex_price: u128,
    borrow_ex_price: u128,
    new_col: i128,
    new_debt: i128,
    remaining_accounts_indices: &Vec<u8>,
) -> Result<()> {
    // if debt is greater than 0 & transaction includes borrow or withdraw (incl. combinations such as deposit + borrow etc.)
    // -> check collateral factor
    // for deposit / payback -> check liquidation threshold.
    // calc for net debt can be unchecked as memory_vars.dust_debt_raw can not be > memory_vars.debt_raw:
    // memory_vars.dust_debt_raw is the result of memory_vars.debt_raw - x where x > 0 see _addDebtToTickWrite()

    // Oracle returns price at 100% ratio.
    // debt price w.r.t to col in 1e15
    let start_index: usize = 0;
    let end_index: usize = start_index + remaining_accounts_indices[0].cast::<usize>()?;

    if ctx.remaining_accounts.len() < end_index {
        return Err(error!(ErrorCodes::VaultOperateRemainingAccountsTooShort));
    }

    let remaining_accounts = ctx
        .remaining_accounts
        .iter()
        .take(end_index)
        .skip(start_index)
        .map(|x| x.to_account_info())
        .collect::<Vec<_>>();

    let nonce: u16 = ctx.accounts.oracle.nonce;
    let oracle_cpi_accounts = OracleCpiAccounts {
        oracle_program: ctx.accounts.oracle_program.to_account_info(),
        oracle: ctx.accounts.oracle.to_account_info(),
        remaining_accounts: remaining_accounts,
    };

    let (mut exchange_rate_liquidate, mut exchange_rate_operate): (u128, u128) =
        oracle_cpi_accounts.get_both_exchange_rate(nonce)?;

    let (exchange_rate, threshold, error_code) = if new_col < 0 || new_debt > 0 {
        // withdraw or borrow, check collateral factor
        (
            &mut exchange_rate_operate,
            collateral_factor,
            ErrorCodes::VaultPositionAboveCF,
        )
    } else {
        // deposit or payback, check liquidation threshold
        (
            &mut exchange_rate_liquidate,
            liquidation_threshold,
            ErrorCodes::VaultPositionAboveLiquidationThreshold,
        )
    };

    *exchange_rate = get_sanitized_exchange_rate(exchange_rate, supply_ex_price, borrow_ex_price)?;

    // Calculate ratio (at cf or lt depending above), then convert to tick boundary
    let mut check_ratio: u128 = (*exchange_rate)
        .safe_mul(threshold)?
        .safe_div(THREE_DECIMALS)?;

    check_ratio = safe_multiply_divide(
        check_ratio,
        TickMath::ZERO_TICK_SCALED_RATIO,
        10u128.pow(RATE_OUTPUT_DECIMALS),
    )?;

    if memory_vars.tick > TickMath::get_tick_at_ratio(check_ratio)?.0 {
        // Exceeded safety boundary
        // For CF: Above CF, user should only be allowed to reduce ratio either by paying debt or by depositing more collateral.
        //          Not comparing collateral as user can potentially use safe/deleverage to reduce tick & debt.
        //          On use of safe/deleverage, collateral will decrease but debt will decrease as well making the overall position safer.
        // For LT: User must bring position into safe territory with deposit or payback, not just slightly safer but still above LT.
        return Err(error!(error_code));
    }

    Ok(())
}

pub fn check_if_withdrawal_safe_for_withdrawal_gap(
    withdrawal_gap: u64,
    new_col: i128,
    liquidity_ex_price: u128,
    user_supply_position: &AccountLoader<UserSupplyPosition>,
) -> Result<()> {
    if withdrawal_gap == 0 {
        return Ok(());
    }

    let user_supply_position_account = user_supply_position.load()?;

    let (liquidity_withdrawal_limit, vault_supply_position_amount) =
        user_supply_position_account.calc_withdrawal_limit_before_operate()?;

    if liquidity_withdrawal_limit == 0 {
        return Ok(());
    }

    let user_withdrawal = new_col
        .abs()
        .cast::<u128>()?
        .safe_mul(EXCHANGE_PRICES_PRECISION)?
        .safe_div(liquidity_ex_price)?;

    // max is vault's supply * 1000 -> Overflowing is impossible.
    let available_withdrawal = vault_supply_position_amount
        .safe_mul(THREE_DECIMALS.safe_sub(withdrawal_gap.cast()?)?)?
        .safe_div(THREE_DECIMALS)?;

    // (liquidityUserSupply - withdrawalGap - liquidityWithdrawalLimit) should NOT be less than user's withdrawal
    if (available_withdrawal.saturating_sub(liquidity_withdrawal_limit)) < user_withdrawal {
        return Err(error!(ErrorCodes::VaultWithdrawMoreThanOperateLimit));
    }

    Ok(())
}

pub fn check_if_ratio_safe_for_deposit(
    memory_vars: &Box<OperateMemoryVars>,
    old_state: &OldState,
    new_net_debt_raw: u128,
) -> Result<()> {
    let new_ratio = memory_vars.get_new_ratio(new_net_debt_raw)?;
    let old_ratio = old_state.get_old_ratio()?;

    if new_ratio > old_ratio {
        return Err(error!(ErrorCodes::VaultInvalidPaybackOrDeposit));
    }

    Ok(())
}

pub fn add_debt_to_tick<'info>(
    tick_data: &mut AccountLoader<'info, Tick>,
    tick_id_liquidation: &mut AccountLoader<'info, TickIdLiquidation>,
    tick_has_debt_accounts: &Box<TickHasDebtAccounts<'info>>,
    total_col_raw: u128,
    net_debt_raw: u128,
    min_debt: u128,
) -> Result<(i32, u32, u128, u128)> {
    if net_debt_raw < min_debt {
        return Err(error!(ErrorCodes::VaultUserDebtTooLow));
    }

    // Calculate ratio
    let ratio: u128 = net_debt_raw
        .safe_mul(TickMath::ZERO_TICK_SCALED_RATIO)?
        .safe_div(total_col_raw)?;

    // Get tick at ratio (floor)
    let (mut tick, ratio_at_tick) = TickMath::get_tick_at_ratio(ratio)?;

    // Increase tick by 1 and ratio by one tick (1.0015x)
    tick = tick.safe_add(1)?;
    let ratio_new: u128 = ratio_at_tick
        .safe_mul(TickMath::TICK_SPACING)?
        .safe_div(FOUR_DECIMALS)?;

    // Calculate user's debt with the adjusted ratio
    let user_raw_debt: u128 = ratio_new
        .safe_mul(total_col_raw)?
        .safe_shr(TickMath::SHIFT)?;

    let dust_debt: u128 = user_raw_debt.safe_sub(net_debt_raw)?;

    let mut tick_data_load = tick_data.load_mut()?;
    tick_data_load.validate(tick)?;

    let mut tick_id: u32 = tick_data_load.total_ids;
    let tick_new_debt;

    if tick_id > 0 && !tick_data_load.is_liquidated() {
        // Current debt in the tick
        let tick_existing_raw_debt = tick_data_load.get_raw_debt()?;

        // Tick's already initialized and not liquidated. Hence simply add the debt
        tick_new_debt = tick_existing_raw_debt.safe_add(user_raw_debt)?;

        if tick_existing_raw_debt == 0 {
            tick_has_debt_accounts.update_tick_has_debt(tick, true)?;
        }
    } else {
        // Liquidation happened or tick getting initialized for the first time
        if tick_id > 0 {
            let mut tick_id_liquidation_load = tick_id_liquidation.load_mut()?;

            // Check that tick_id_liquidation matches the tick we're updating
            tick_id_liquidation_load.validate(tick, tick_id)?;

            // Liquidation happened, move data to tickID
            // Find the right set in the liquidation data
            tick_id_liquidation_load.set_tick_status(
                tick_id,
                tick_data_load.is_fully_liquidated(),
                tick_data_load.liquidation_branch_id,
                tick_data_load.debt_factor,
            );
        }

        tick_new_debt = user_raw_debt;
        tick_has_debt_accounts.update_tick_has_debt(tick, true)?;
        tick_id = tick_id + 1;
        tick_data_load.total_ids = tick_id;
    }

    if tick_new_debt < min_debt {
        return Err(error!(ErrorCodes::VaultTickDebtTooLow));
    }

    // reset data
    tick_data_load.is_liquidated = 0;
    tick_data_load.is_fully_liquidated = 0;
    tick_data_load.liquidation_branch_id = 0;
    tick_data_load.debt_factor = 0;
    tick_data_load.set_raw_debt(tick_new_debt)?;

    Ok((tick, tick_id, user_raw_debt, dust_debt))
}

fn check_if_amount_is_valid(amount: i128, decimals: u8) -> Result<()> {
    // If greater than 4 decimals, use MIN_OPERATE, otherwise use MIN_OPERATE_LOWER_DECIMALS_AMOUNT (10)
    let minimum_amount = if decimals >= LOWER_DECIMALS_OPERATE {
        MIN_OPERATE
    } else {
        MIN_OPERATE_LOWER_DECIMALS_AMOUNT
    };

    if
    // withdrawal or deposit or borrow or payback cannot be too small
    (amount != 0 && amount.unsigned_abs() < minimum_amount )
        // amounts must not be too big
        || (amount != i128::MIN && amount.unsigned_abs() > MAX_OPERATE)
    {
        return Err(error!(ErrorCodes::VaultInvalidOperateAmount));
    }

    Ok(())
}

pub fn verify_operate_amounts(
    new_col: i128,
    new_debt: i128,
    col_decimals: u8,
    debt_decimals: u8,
) -> Result<()> {
    if new_col == 0 && new_debt == 0 {
        return Err(error!(ErrorCodes::VaultInvalidOperateAmount));
    }

    check_if_amount_is_valid(new_col, col_decimals)?;
    check_if_amount_is_valid(new_debt, debt_decimals)?;

    Ok(())
}
