use anchor_lang::prelude::*;

use crate::{
    constants::{FOUR_DECIMALS, THREE_DECIMALS, X30},
    errors::ErrorCodes,
    invokes::{OracleCpiAccounts, RATE_OUTPUT_DECIMALS},
    state::*,
};

use library::math::{
    casting::*,
    safe_math::*,
    tick::TickMath,
    u256::{safe_multiply_divide, safe_multiply_divide_result},
};

#[allow(clippy::too_many_arguments)]
pub fn end_liquidate<'info>(
    current_data: &mut CurrentLiquidity,
    tick_info: &mut TickMemoryVars,
    branch_in_memory: &mut BranchMemoryVars,
    debt_liquidated: &mut u128,
    col_liquidated: &mut u128,
    col_per_debt: u128,
    minimum_branch_debt: u128,
) -> Result<()> {
    if *debt_liquidated >= current_data.debt_remaining {
        // Liquidation ended between currentTick & refTick
        // Not all of liquidatable debt is actually liquidated -> recalculate
        *debt_liquidated = current_data.debt_remaining;
        // debt_liquidated here related to input param liquidation amount so using u256 safe multiply to be sure no overflow can happen
        *col_liquidated = safe_multiply_divide(
            *debt_liquidated,
            col_per_debt,
            10u128.pow(RATE_OUTPUT_DECIMALS),
        )?;

        // Liquidating to debt. Calculate final ratio after liquidation
        // liquidatable debt - debtLiquidated / liquidatable col - colLiquidated
        let final_ratio: u128 = current_data.get_final_ratio(*col_liquidated, *debt_liquidated)?;

        // Fetching tick of where liquidation ended
        let (mut final_tick, ratio_one_less) = TickMath::get_tick_at_ratio(final_ratio)?;

        if (final_tick < current_data.ref_tick) && (tick_info.partials == X30) {
            // This situation might never happen
            // If this happens then there might be some very edge case precision of few weis which is returning 1 tick less
            // If the above were to ever happen then tickInfo_.tick only be currentData_.refTick - 1
            // In this case the partial will be very very near to full (X30)
            // Increasing tick by 2 and making partial as 1 which is basically very very near to currentData_.refTick
            final_tick = final_tick.safe_add(2)?;
            tick_info.set_partials(1)?;
        } else {
            // Increasing tick by 1 as final ratio will probably be a partial
            final_tick = final_tick.safe_add(1)?;

            let existing_partials =
                if current_data.is_ref_tick_liquidated() && final_tick == current_data.ref_tick {
                    tick_info.partials
                } else {
                    0
                };

            // Taking edge cases where partial comes as 0 or X30 meaning perfect tick
            // Hence, increasing or reducing it by 1 as liquidation tick cannot be perfect tick
            tick_info.set_partials(get_tick_partials(ratio_one_less, final_ratio)?)?;

            current_data
                .check_is_ref_partials_safe_for_tick(existing_partials, tick_info.partials)?;
        }

        tick_info.set_tick(final_tick);
    } else {
        // End in liquidation threshold
        // finalRatio_ = current_data.ref_ratio
        // Increasing liquidation threshold tick by 1 partial. With 1 partial it'll reach to the next tick
        // Ratio change will be negligible. Doing this as liquidation threshold tick can also be a perfect non-liquidated tick
        tick_info.set_tick(current_data.ref_tick.safe_add(1)?);

        // Making partial as 1 so it doesn't stay perfect tick
        tick_info.set_partials(1)?;
        // Length is not needed as only partials are written to storage
    }

    // debtFactor = debtFactor * (liquidatableDebt - debtLiquidated) / liquidatableDebt
    // -> debtFactor * leftOverDebt / liquidatableDebt
    let debt_factor = current_data.get_debt_factor(*debt_liquidated)?;

    current_data.update_totals(*debt_liquidated, *col_liquidated)?;

    // Updating branch's debt factor & write to storage as liquidation is over
    // Using branch.debt_factor.mul_div_big_number() to match the Solidity .mulDivBigNumber()
    branch_in_memory.update_branch_debt_factor(debt_factor)?;

    if current_data.debt < minimum_branch_debt {
        // This can happen when someone tries to create a dust tick
        return Err(error!(ErrorCodes::VaultBranchDebtTooLow));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn get_ticks_from_oracle_price<'info>(
    ctx: &Context<'_, '_, 'info, 'info, Liquidate<'info>>,
    vault_config: &VaultConfig,
    supply_ex_price: u128,
    borrow_ex_price: u128,
    remaining_accounts_indices: &Vec<u8>,
) -> Result<(u128, i32, i32)> {
    let start_index: usize = 0;
    let end_index: usize = start_index + remaining_accounts_indices[0].cast::<usize>()?;

    if ctx.remaining_accounts.len() < end_index {
        return Err(error!(ErrorCodes::VaultLiquidateRemainingAccountsTooShort));
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

    let exchange_rate: u128 = oracle_cpi_accounts.get_exchange_rate_liquidate(nonce)?;

    // Note if price would come back as 0 `get_tick_at_ratio` will fail
    if exchange_rate > 10u128.pow(24) || exchange_rate == 0 {
        // capping to 1B USD per 1 Bitcoin at 15 oracle precision
        return Err(error!(ErrorCodes::VaultInvalidOraclePrice));
    }

    // max possible debt_per_col = 1e24 * 1e19 / 1e12 so 1e31. must be done in u256 to avoid potential overflow.
    // (exchange prices can only ever increase ensured in load_exchange_prices)
    let debt_per_col_result =
        safe_multiply_divide_result(exchange_rate, supply_ex_price, borrow_ex_price)?;

    let mut debt_per_col = debt_per_col_result.get(); // Use floor for ratio calculations

    if debt_per_col == 0 {
        return Err(error!(ErrorCodes::VaultInvalidOraclePrice));
    }

    // capping oracle pricing to 1e26 after applying exchange prices to guarantee enough precision
    if debt_per_col > 10u128.pow(26) {
        debt_per_col = 10u128.pow(26);
    }

    // For col_per_debt, we need to round up the debt_per_col to ensure col_per_debt rounds down
    let debt_per_col_rounded_up = debt_per_col_result.get_ceil()?;

    // Raw colPerDebt in 15 decimals, at minimum this comes out at 1e4 precision
    let raw_col_per_debt: u128 = 10u128
        .pow(RATE_OUTPUT_DECIMALS * 2)
        .safe_div(debt_per_col_rounded_up)?;

    // debt_per_col max possible = 1e26, min possible = 21646 (does not reach MIN_RATIO, see below)
    // raw_col_per_debt max possible = 1e30 / 21646 = ~4e25, min possible = 1e4

    // Calculate col_per_debt with liquidation penalty
    // Liquidation penalty in 4 decimals (1e2 = 1%)
    let col_per_debt: u128 = raw_col_per_debt
        .safe_mul(FOUR_DECIMALS.safe_add(vault_config.liquidation_penalty.cast()?)?)?
        .safe_div(FOUR_DECIMALS)?;

    // considering "ratioX48" supports between 6093 and 13002088133096036565414295:

    // when debt_per_col = 1 we do 281474976710656 / 1e15 = 0. <- below MIN RATIO, would error in get_tick_at_ratio()
    // error for any where debt_per_col < ~22000 (x * 281474976710656 / 1e15 = 6093 -> x = 21646, plus consider LT)

    // debt_per_col must max ever end up so that 4e25 * 281474976710656 / 1e15 = 11258999068426240000000000
    //                             fits into max ratio of                        13002088133096036565414295
    // setting to allow up to max 1e26 to cover range of possibilities around liquidation threshold.

    // Calculate liquidation tick (tick at liquidation threshold ratio)
    // Convert debt_per_col to get ratio at liquidation threshold
    let liquidation_ratio: u128 = safe_multiply_divide(
        debt_per_col,
        TickMath::ZERO_TICK_SCALED_RATIO,
        10u128.pow(RATE_OUTPUT_DECIMALS),
    )?;

    // Liquidation threshold in 3 decimals (900 = 90%)
    let threshold_ratio: u128 = liquidation_ratio
        .safe_mul(vault_config.liquidation_threshold.cast()?)?
        .safe_div(THREE_DECIMALS)?;

    let (liquidation_tick, _) = TickMath::get_tick_at_ratio(threshold_ratio)?;

    // Calculate liquidation max limit tick (tick at max limit ratio)
    let max_ratio: u128 = liquidation_ratio
        .safe_mul(vault_config.liquidation_max_limit.cast()?)?
        .safe_div(THREE_DECIMALS)?;

    // get liquidation max limit tick (tick at liquidation max limit ratio)
    let (max_tick, _) = TickMath::get_tick_at_ratio(max_ratio)?;

    Ok((col_per_debt, liquidation_tick, max_tick))
}
