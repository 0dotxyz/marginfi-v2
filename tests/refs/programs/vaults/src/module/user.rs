use anchor_lang::prelude::*;

use crate::{constants::*, errors::ErrorCodes, events::*, invokes::*, state::*, utils::*};

use library::token::*;
use library::{
    math::{bn::*, casting::*, safe_math::*, tick::TickMath},
    structs::TokenTransferParams,
};
use liquidity::state::TransferType;

pub fn init_position<'info>(
    ctx: Context<InitPosition>,
    vault_id: u16,
    next_position_id: u32,
) -> Result<()> {
    verify_init_position(&ctx, vault_id, next_position_id)?;

    let mut position = ctx.accounts.position.load_init()?;

    position.tick = COLD_TICK;
    position.vault_id = vault_id;
    position.set_is_supply_only_position(true)?;
    position.nft_id = next_position_id;
    position.position_mint = ctx.accounts.position_mint.key();

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    vault_state.increase_total_positions();
    vault_state.update_next_position_id();

    let vault_admin = &ctx.accounts.vault_admin;
    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_ADMIN_SEED, &[vault_admin.bump]]];

    ctx.accounts.initialize_token_metadata(vault_id)?;

    mint_position_token_and_remove_authority(
        &ctx.accounts.vault_admin,
        &ctx.accounts.position_mint,
        &ctx.accounts.position_token_account,
        &ctx.accounts.token_program,
        &signer_seeds,
    )?;

    Ok(())
}

pub fn close_position<'info>(
    ctx: Context<ClosePosition>,
    vault_id: u16,
    position_id: u32,
) -> Result<()> {
    verify_close_position(&ctx, vault_id, position_id)?;

    // Burn the position token and metadata (also closes token account)
    ctx.accounts.burn_token_metadata()?;

    // Update vault state to decrement total positions
    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    vault_state.decrease_total_positions();

    emit!(LogClosePosition {
        signer: ctx.accounts.signer.key(),
        position_id,
        vault_id,
        position_mint: ctx.accounts.position_mint.key(),
    });

    Ok(())
}

pub fn operate<'info>(
    ctx: Context<'_, '_, 'info, 'info, Operate<'info>>,
    new_col: i128,
    new_debt: i128,
    transfer_type: Option<TransferType>,
    remaining_accounts_indices: Vec<u8>,
) -> Result<(u32, i128, i128)> {
    verify_operate(&ctx)?;

    // remaining_accounts_indices[0] is oracle sources length
    // remaining_accounts_indices[1] is branch accounts length
    // remaining_accounts_indices[2] is tick has debt array length
    if remaining_accounts_indices.len() != 3 {
        return Err(error!(ErrorCodes::VaultInvalidRemainingAccountsIndices));
    }

    verify_operate_amounts(
        new_col,
        new_debt,
        ctx.accounts.supply_token.decimals,
        ctx.accounts.borrow_token.decimals,
    )?;

    // checking owner only in case of withdraw or borrow
    if new_col < 0 || new_debt > 0 {
        verify_position_authority_interface(
            &ctx.accounts.position_token_account,
            &ctx.accounts.signer,
        )?;
    }

    // scale, except in max withdraw of payback cases
    let mut new_col: i128 = if new_col == i128::MIN {
        i128::MIN
    } else {
        scale_amounts(new_col, ctx.accounts.supply_token.decimals)?
    };

    let mut new_debt: i128 = if new_debt == i128::MIN {
        i128::MIN
    } else {
        scale_amounts(new_debt, ctx.accounts.borrow_token.decimals)?
    };

    let mut position = ctx.accounts.position.load_mut()?;

    let (vault_config_bump, vault_id, memory_vars, old_state, new_col_final, new_debt_final) = {
        let vault_config = ctx.accounts.vault_config.load()?;
        let mut vault_state = ctx.accounts.vault_state.load_mut()?;

        let mut memory_vars: Box<OperateMemoryVars> = Box::new(OperateMemoryVars::default());
        let mut top_tick = vault_state.get_top_tick();

        (
            memory_vars.col_raw,
            memory_vars.dust_debt_raw,
            memory_vars.debt_raw,
            memory_vars.tick,
            memory_vars.tick_id,
        ) = position.get_position_info()?;

        let tick_has_debt_accounts = get_tick_has_debt_from_remaining_accounts_operate(
            &ctx.remaining_accounts,
            &remaining_accounts_indices,
            vault_state.vault_id,
        )?;

        // Get latest updated Position's debt & supply (if position is with debt -> not new / supply position)
        if memory_vars.tick > COLD_TICK {
            let mut branch_accounts = get_branches_from_remaining_accounts(
                &ctx.remaining_accounts,
                &remaining_accounts_indices,
                vault_state.vault_id,
            )?;

            let mut tick_data = ctx.accounts.current_position_tick.load_mut()?;

            // Check if user got liquidated
            // Checking if tick is liquidated OR if the total IDs of tick is greater than user's tick ID
            if tick_data.is_liquidated() || tick_data.total_ids > memory_vars.tick_id {
                // User got liquidated - fetch the latest position
                // Get updated position after liquidation
                let branch_id = memory_vars.fetch_latest_position(
                    &tick_data,
                    &ctx.accounts.current_position_tick_id,
                    &branch_accounts,
                )?;
                // @dev memory_vars gets updated inside the fetch_latest_position

                if memory_vars.debt_raw > memory_vars.dust_debt_raw {
                    let mut branch_to_update = branch_accounts.load_mut(branch_id)?;
                    branch_to_update.update_debt_liquidity(
                        memory_vars.debt_raw,
                        get_minimum_branch_debt(ctx.accounts.borrow_token.decimals)?,
                    )?;

                    // Convert position raw debt to net position debt
                    memory_vars.debt_raw = memory_vars.get_net_debt_raw()?;
                } else {
                    // Liquidated 100% or almost 100%
                    // Absorb dust debt into the vault state
                    vault_state.update_absorbed_dust_debt_amount(
                        memory_vars.dust_debt_raw.cast()?,
                        memory_vars.debt_raw.cast()?,
                    )?;

                    memory_vars.debt_raw = 0;
                    memory_vars.col_raw = 0;
                }
            } else {
                // Check if tick has debt (if not, something is wrong)
                // below require can fail when a user liquidity is extremely low (talking about way less than even $1)
                // adding require meaning this vault user won't be able to interact unless someone makes the liquidity in tick as non 0.
                // reason of adding is the tick has already removed from everywhere. Can removing it again break something? Better to simply remove that case entirely
                if tick_data.raw_debt == 0 {
                    return Err(error!(ErrorCodes::VaultTickIsEmpty));
                }

                let debt_in_tick = tick_data.get_raw_debt()?;

                // Calculate remaining debt in tick
                let mut remaining_debt: u128 = debt_in_tick.saturating_sub(memory_vars.debt_raw);

                // If debt is too low, set it to zero
                if remaining_debt < get_minimum_tick_debt(ctx.accounts.borrow_token.decimals)? {
                    remaining_debt = 0;

                    // If debt becomes 0, remove the tick from tick_has_debt
                    if memory_vars.tick == top_tick {
                        // Set new top tick and update vault state
                        top_tick = vault_state
                            .set_top_tick(&mut branch_accounts, &tick_has_debt_accounts)?;
                    }

                    tick_has_debt_accounts.update_tick_has_debt(memory_vars.tick, false)?;
                }

                // Update tick data with remaining debt
                tick_data.set_raw_debt(remaining_debt)?;
                // Convert position raw debt to net position debt
                memory_vars.debt_raw = memory_vars.get_net_debt_raw()?;
            }

            // Reset dust debt after processing
            memory_vars.dust_debt_raw = 0;
        }

        let (
            liquidity_supply_ex_price,
            _, // liquidity_borrow_ex_price
            supply_ex_price,
            borrow_ex_price,
        ) = vault_state.load_exchange_prices(
            &vault_config,
            &ctx.accounts.supply_token_reserves_liquidity,
            &ctx.accounts.borrow_token_reserves_liquidity,
        )?;

        let old_state: Box<OldState> = Box::new(OldState {
            old_col_raw: memory_vars.col_raw,
            old_net_debt_raw: memory_vars.debt_raw,
            old_tick: memory_vars.tick,
        });

        (new_col, new_debt, memory_vars.col_raw, memory_vars.debt_raw) = position
            .get_new_position_info(
                Box::new(AmountInfo {
                    new_col,
                    new_debt,
                    col_raw: memory_vars.col_raw,
                    debt_raw: memory_vars.debt_raw,
                }),
                Box::new(VaultExchangePrices {
                    supply_ex_price,
                    borrow_ex_price,
                    borrow_fee: vault_config.borrow_fee,
                }),
            )?;

        // if position has no collateral or debt and user sends type(int).min for withdraw and payback then this results in 0
        // there's is no issue if it stays 0 but better to throw here to avoid checking for potential issues if there could be
        if new_col == 0 && new_debt == 0 {
            return Err(error!(ErrorCodes::VaultInvalidOperateAmount));
        }

        if memory_vars.debt_raw > 0 {
            (
                memory_vars.tick,
                memory_vars.tick_id,
                memory_vars.debt_raw,
                memory_vars.dust_debt_raw,
            ) = add_debt_to_tick(
                &mut ctx.accounts.final_position_tick, // Update debt in new tick
                &mut ctx.accounts.final_position_tick_id,
                &tick_has_debt_accounts,
                memory_vars.col_raw,
                memory_vars.get_scaled_debt(None)?,
                get_minimum_debt(ctx.accounts.borrow_token.decimals)?,
            )?;

            let new_net_debt_raw = memory_vars.get_net_debt_raw()?;

            if new_debt < 0 && new_net_debt_raw > old_state.old_net_debt_raw {
                // anyone can payback debt of any position
                // hence, explicitly checking the debt should decrease
                return Err(error!(ErrorCodes::VaultInvalidPaybackOrDeposit));
            }

            if new_col > 0 && new_debt == 0 {
                check_if_ratio_safe_for_deposit(&memory_vars, &old_state, new_net_debt_raw)?;
            }

            if memory_vars.tick >= top_tick {
                // Update the topmost tick in vault state as new top tick is available
                vault_state.update_topmost_tick(memory_vars.tick, &mut ctx.accounts.new_branch)?;
            }
        } else {
            memory_vars.tick = COLD_TICK;
            // debtRaw_ remains 0 in this situation
            // This kind of position will not have any tick. Meaning it'll be a supply position.
        }

        // if position is not a supply position, check if it's safe for borrow or withdraw
        if memory_vars.tick != COLD_TICK {
            // if debt is greater than 0 & transaction includes borrow or withdraw (incl. combinations such as deposit + borrow etc.)
            // -> check collateral factor
            check_if_position_safe(
                &ctx,
                &memory_vars,
                vault_config.collateral_factor.cast()?,
                vault_config.liquidation_threshold.cast()?,
                supply_ex_price,
                borrow_ex_price,
                new_col,
                new_debt,
                &remaining_accounts_indices,
            )?;
        }

        position.update_position_after_operate(&memory_vars)?;

        // Emit user position after operate
        emit!(LogUserPosition {
            user: ctx.accounts.signer.key(),
            nft_id: position.nft_id,
            vault_id: vault_state.vault_id,
            position_mint: position.position_mint,
            tick: memory_vars.tick,
            col: unscale_amounts(
                memory_vars.col_raw.cast()?,
                ctx.accounts.supply_token.decimals
            )?
            .cast()?,
            borrow: unscale_amounts(
                memory_vars.debt_raw.cast()?,
                ctx.accounts.borrow_token.decimals
            )?
            .cast()?,
        });

        // unscale new_col for withdrawal gap check
        new_col = unscale_amounts(new_col, ctx.accounts.supply_token.decimals)?;

        // Withdrawal gap to make sure there's always liquidity for liquidation
        // For example if withdrawal allowance is 15% on liquidity then we can limit operate's withdrawal allowance to 10%
        // this will allow liquidate function to get extra 5% buffer for potential liquidations.
        if new_col < 0 {
            check_if_withdrawal_safe_for_withdrawal_gap(
                vault_config.withdraw_gap.cast()?,
                new_col,
                liquidity_supply_ex_price,
                &ctx.accounts.vault_supply_position_on_liquidity,
            )?;
        } else if new_col > 0 {
            new_col = new_col.safe_add(1)?; // safe round up for deposit after unscaling, so that deposit towards LL happens with > accounted amount
        }

        new_debt = unscale_amounts(new_debt, ctx.accounts.borrow_token.decimals)?;
        if new_debt < 0 {
            new_debt = new_debt.safe_sub(1)?;
            // safe round up for payback after unscaling, so that payback towards LL happens with > accounted amount
            // rounding like this will lead to rebalance() turning a profit:
            // supply amounts tracked for vault at LL > amounts tracked at vault itself
            // borrow amounts tracked for vault at LL < amounts tracked at vault itself
        }

        (
            vault_config.bump,
            vault_state.vault_id,
            memory_vars,
            old_state,
            new_col,  // return unscaled amount
            new_debt, // return unscaled amount
        )
    };

    // Now we can safely make CPI calls without borrow conflicts
    let vault_id_bytes = vault_id.to_le_bytes();
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_CONFIG_SEED,
        vault_id_bytes.as_slice(),
        &[vault_config_bump],
    ]];

    if new_col_final > 0 {
        let accounts = ctx.accounts.get_deposit_accounts();

        accounts.pre_operate_with_signer(
            PreOperateInstructionParams {
                mint: ctx.accounts.supply_token.key(),
            },
            signer_seeds,
            true,
        )?;

        // First, handle the token transfer
        transfer_spl_tokens(TokenTransferParams {
            source: ctx.accounts.signer_supply_token_account.to_account_info(),
            destination: ctx.accounts.vault_supply_token_account.to_account_info(),
            authority: ctx.accounts.signer.to_account_info(),
            amount: new_col_final.cast()?,
            token_program: ctx.accounts.supply_token_program.to_account_info(),
            signer_seeds: None,
            mint: *ctx.accounts.supply_token.clone(),
        })?;

        accounts.operate_with_signer(
            OperateInstructionParams {
                supply_amount: new_col_final,
                borrow_amount: 0,
                withdraw_to: ctx.accounts.liquidity.key(), // default withdraw_to will be liquidity PDA itself
                borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity PDA itself
                mint: ctx.accounts.supply_token.key(),
                transfer_type: None, // Not used for deposit
            },
            signer_seeds,
        )?;
    }

    if new_debt_final < 0 {
        let accounts = ctx.accounts.get_payback_accounts();

        accounts.pre_operate_with_signer(
            PreOperateInstructionParams {
                mint: ctx.accounts.borrow_token.key(),
            },
            signer_seeds,
            false,
        )?;

        // PAYBACK
        transfer_spl_tokens(TokenTransferParams {
            source: ctx.accounts.signer_borrow_token_account.to_account_info(),
            destination: ctx.accounts.vault_borrow_token_account.to_account_info(),
            authority: ctx.accounts.signer.to_account_info(),
            amount: new_debt_final.abs().cast()?, // Since new_debt_final < 0, we need to payback debt, but transfer amount > 0, hence abs()
            token_program: ctx.accounts.borrow_token_program.to_account_info(),
            signer_seeds: None,
            mint: *ctx.accounts.borrow_token.clone(),
        })?;

        accounts.operate_with_signer(
            OperateInstructionParams {
                supply_amount: 0,
                borrow_amount: new_debt_final,
                withdraw_to: ctx.accounts.liquidity.key(), // default withdraw_to will be liquidity PDA itself
                borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity PDA itself
                mint: ctx.accounts.borrow_token.key(),
                transfer_type: None, // Not used for payback
            },
            signer_seeds,
        )?;
    }

    if new_col_final < 0 {
        let is_claim_type =
            transfer_type.is_some() && transfer_type.clone().unwrap() == TransferType::CLAIM;

        // WITHDRAW
        ctx.accounts
            .get_withdraw_accounts(is_claim_type)?
            .operate_with_signer(
                OperateInstructionParams {
                    supply_amount: new_col_final,
                    borrow_amount: 0,
                    withdraw_to: ctx.accounts.get_recipient()?.key(), //  withdraw_to will be recipient address
                    borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity PDA itself
                    mint: ctx.accounts.supply_token.key(),
                    transfer_type: if is_claim_type {
                        Some(TransferType::CLAIM)
                    } else {
                        None
                    },
                },
                signer_seeds,
            )?;
    }

    if new_debt_final > 0 {
        let is_claim_type =
            transfer_type.is_some() && transfer_type.unwrap() == TransferType::CLAIM;

        // BORROW
        ctx.accounts
            .get_borrow_accounts(is_claim_type)?
            .operate_with_signer(
                OperateInstructionParams {
                    supply_amount: 0,
                    borrow_amount: new_debt_final,
                    withdraw_to: ctx.accounts.liquidity.key(), //  withdraw_to will be liquidity PDA itself
                    borrow_to: ctx.accounts.get_recipient()?.key(), // borrow_to will be recipient address
                    mint: ctx.accounts.borrow_token.key(),
                    transfer_type: if is_claim_type {
                        Some(TransferType::CLAIM)
                    } else {
                        None
                    },
                },
                signer_seeds,
            )?;
    }

    {
        let mut vault_state = ctx.accounts.vault_state.load_mut()?;

        // Update final vault variables on storage
        vault_state.update_total_supply(memory_vars.col_raw, old_state.old_col_raw)?;
        vault_state
            .update_total_borrow(memory_vars.get_net_debt_raw()?, old_state.old_net_debt_raw)?;
    }

    emit!(LogOperate {
        signer: ctx.accounts.signer.key(),
        nft_id: position.nft_id,
        new_col: new_col_final,
        new_debt: new_debt_final,
        to: ctx.accounts.get_recipient()?.key(),
    });

    Ok((position.nft_id, new_col_final, new_debt_final))
}

pub fn liquidate<'info>(
    ctx: Context<'_, '_, 'info, 'info, Liquidate<'info>>,
    debt_amt: u64,
    col_per_unit_debt: u128, // min collateral needed to receive per unit of debt paid back in 1e15
    absorb: bool,
    transfer_type: Option<TransferType>,
    remaining_accounts_indices: Vec<u8>, // first index is sources, second is branches, third is ticks, fourth is tick has debt
) -> Result<(u128, u128)> {
    verify_liquidate(&ctx)?;

    // remaining_accounts_indices[0] is oracle sources length
    // remaining_accounts_indices[1] is branch accounts length
    // remaining_accounts_indices[2] is tick accounts length
    // remaining_accounts_indices[3] is tick has debt array length
    if remaining_accounts_indices.len() != 4 {
        return Err(error!(ErrorCodes::VaultInvalidRemainingAccountsIndices));
    }

    let (vault_id, bump, actual_debt_amt, actual_col_amt) = {
        let mut vault_state = ctx.accounts.vault_state.load_mut()?;
        let vault_config = ctx.accounts.vault_config.load()?;

        // Check for valid input
        if vault_state.topmost_tick == COLD_TICK {
            return Err(error!(ErrorCodes::VaultTopTickDoesNotExist));
        }

        let debt_amt = scale_amounts(debt_amt.cast()?, ctx.accounts.borrow_token.decimals)?;

        // Below are exchange prices of vaults
        let (
            _,               // liquidity_supply_ex_price
            _,               // liquidity_borrow_ex_price
            supply_ex_price, // vault_supply_ex_price
            borrow_ex_price, // vault_borrow_ex_price
        ) = vault_state.load_exchange_prices(
            &vault_config,
            &ctx.accounts.supply_token_reserves_liquidity,
            &ctx.accounts.borrow_token_reserves_liquidity,
        )?;

        let mut current_data: Box<CurrentLiquidity> = Box::new(CurrentLiquidity::default());

        // @dev setup tick in memory vars for liquidation
        let (col_per_debt, liquidation_tick, max_tick) = get_ticks_from_oracle_price(
            &ctx,
            &vault_config,
            supply_ex_price,
            borrow_ex_price,
            &remaining_accounts_indices,
        )?;

        let tick_has_debt_accounts = get_tick_has_debt_from_remaining_accounts_liquidate(
            &ctx.remaining_accounts,
            &remaining_accounts_indices,
            vault_state.vault_id,
        )?;

        let branch_accounts = get_branches_from_remaining_accounts(
            &ctx.remaining_accounts,
            &remaining_accounts_indices,
            vault_state.vault_id,
        )?;

        let tick_accounts = get_ticks_from_remaining_accounts(
            &ctx.remaining_accounts,
            &remaining_accounts_indices,
            vault_state.vault_id,
        )?;

        let vault_liquidation_start_tick = vault_state.topmost_tick;

        // Check if tick is above max limit, absorb it first
        if vault_state.topmost_tick > max_tick {
            // Call absorb function to handle bad debt above max limit
            // @dev passing vault_state as a mut reference, that means it will be updated inside the absorb function
            let (col_absorbed, debt_absorbed) = self::absorb(
                &mut vault_state,
                &tick_accounts,
                &tick_has_debt_accounts,
                &branch_accounts,
                &mut ctx.accounts.new_branch,
                max_tick,
            )?;

            emit!(LogAbsorb {
                col_amount: unscale_amounts(col_absorbed, ctx.accounts.supply_token.decimals)?
                    .cast()?,
                debt_amount: unscale_amounts(debt_absorbed, ctx.accounts.borrow_token.decimals)?
                    .cast()?,
            });

            if debt_amt == 0 {
                // If debt_amt was 0, we just wanted to absorb
                return Ok((0, 0));
            }
        }

        // Get current tick from vault state
        current_data.tick = vault_state.topmost_tick;

        if debt_amt < get_minimum_debt(ctx.accounts.borrow_token.decimals)?.cast::<i128>()? {
            return Err(error!(ErrorCodes::VaultInvalidLiquidationAmt));
        }

        current_data.tick_status = vault_state.get_tick_status();

        let mut tick_info: Box<TickMemoryVars> = Box::new(TickMemoryVars::default());
        tick_info.tick = current_data.tick;

        // Calculate debt remaining to liquidate
        // debtAmt_ should be less than 2**128 & EXCHANGE_PRICES_PRECISION is 1e12
        current_data.debt_remaining = debt_amt
            .cast::<u128>()?
            .safe_mul(EXCHANGE_PRICES_PRECISION)?
            .safe_div(borrow_ex_price)?;

        // Get total debt for minimum check
        let total_debt: u128 = vault_state.get_total_borrow()?;

        if total_debt.safe_div(BILLION)? > current_data.debt_remaining {
            // if liquidation amount is less than 1e9 of total debt then revert
            // so if total debt is $1B then minimum liquidation limit = $1
            // so if total debt is $1T then minimum liquidation limit = $1000
            // partials precision is slightly above 1e9 so this will make sure that on every liquidation at least 1 partial gets liquidated
            // not sure if it can result in any issue but restricting amount further more to remove very low amount scenarios totally
            return Err(error!(ErrorCodes::VaultInvalidLiquidationAmt));
        }

        // Handle absorbed liquidity first if requested
        if absorb {
            vault_state.absorb_dust_amount_for_liquidate(&mut current_data)?;
        }

        // current tick should be greater than liquidationTick and it cannot be greater than maxTick as absorb will run
        if current_data.tick > liquidation_tick && current_data.debt_remaining > 0 {
            // branch related stuffs
            let mut branch: Box<BranchMemoryVars> = Box::new(BranchMemoryVars::default());

            {
                // @dev current branch should exist in the branch accounts
                let branch_0 = branch_accounts.load(vault_state.current_branch_id)?;
                branch.set_branch_data_in_memory(&branch_0)?;
            }

            let mut next_tick = COLD_TICK;

            if current_data.is_perfect_tick() {
                // top tick is not liquidated. Hence it's a perfect tick.
                current_data.ratio = TickMath::get_ratio_at_tick(tick_info.tick)?;
                // if current tick in liquidation is a perfect tick then it is also the next tick that has debt.
                next_tick = current_data.tick;
            } else {
                (current_data.ratio, tick_info.partials) = get_current_partials_ratio(
                    branch.data.minima_tick_partials,
                    TickMath::get_ratio_at_tick(tick_info.tick)?,
                )?;

                // Check for edge case: liquidation tick+1 == current tick and partials == 1
                // This means there's nothing to liquidate anymore
                if liquidation_tick + 1 == tick_info.tick && tick_info.partials == 1 {
                    if ctx.accounts.to.key() == ADDRESS_DEAD {
                        // revert with liquidated amounts if to_ address is the dead address.
                        // this can be used in a resolver to find the max liquidatable amounts
                        let topmost_tick = vault_state.topmost_tick;
                        msg!("VaultLiquidationResult: [{}, {}, {}]", 0, 0, topmost_tick);
                        return Err(error!(ErrorCodes::VaultLiquidationResult));
                    }

                    return Err(error!(ErrorCodes::VaultInvalidLiquidation));
                }
            }

            let mut is_first_iteration = true;
            // Main liquidation loop
            loop {
                let additional_debt: u128 = if current_data.is_perfect_tick() {
                    // not liquidated -> Getting the debt from tick data itself
                    // Updating tick on storage with removing debt & adding connection to branch
                    let mut tick_data = tick_accounts.load_mut(current_data.tick)?;

                    let debt = tick_data.get_raw_debt()?;
                    tick_data.set_liquidated(branch.id, branch.debt_factor);

                    debt
                } else {
                    // Tick is already liquidated - Get debt from branch data
                    branch.data.debt_liquidity.cast()?
                    // debt in branch
                };

                // Adding new debt into active debt for liquidation
                current_data.debt = current_data.debt.safe_add(additional_debt)?;

                // Adding new col into active col for liquidation
                // Ratio is in 2**48 decimals hence multiplying debt with 2**48 to get proper collateral
                current_data.col = current_data.col.safe_add(
                    additional_debt
                        .safe_mul(TickMath::ZERO_TICK_SCALED_RATIO)?
                        .safe_div(current_data.ratio)?,
                )?;

                // Find next tick with debt or check if we reach liquidation threshold
                if (next_tick == current_data.tick && current_data.is_perfect_tick())
                    || is_first_iteration
                {
                    is_first_iteration = false;

                    next_tick = tick_has_debt_accounts.fetch_next_tick_liquidate(
                        current_data.tick,
                        liquidation_tick,
                        current_data.is_perfect_tick(), // in 1st loop tickStatus can be 2. Meaning not a perfect current tick
                    )?;
                }

                (current_data.ref_tick, current_data.ref_tick_status) =
                    get_next_ref_tick(branch.minima_tick, next_tick, liquidation_tick)?;

                if current_data.is_ref_tick_liquidated() {
                    // Merge current branch with base branch
                    // Fetching base branch data to get the base branch's partial
                    let base_branch_id = branch.data.connected_branch_id;

                    // Find the corresponding branch in our list
                    let base_branch = branch_accounts.load(base_branch_id)?;
                    branch.set_base_branch_data(&base_branch)?;

                    (current_data.ref_ratio, tick_info.partials) = get_current_partials_ratio(
                        base_branch.minima_tick_partials,
                        TickMath::get_ratio_at_tick(current_data.ref_tick)?,
                    )?;
                } else {
                    // refTickStatus can only be 1 (next tick from perfect tick) or 3 (liquidation threshold tick)
                    current_data.ref_ratio = TickMath::get_ratio_at_tick(current_data.ref_tick)?;
                    tick_info.partials = X30;
                }

                // Formula: (debt_ - x) / (col_ - (x * colPerDebt_)) = ratioEnd_
                // x = ((ratioEnd_ * col) - debt_) / ((colPerDebt_ * ratioEnd_) - 1)
                // x is debt_liquidated
                // col_ = debt_ / ratioStart_ -> (current_data.debt / current_data.ratio)
                // ratioEnd_ is current_data.ref_ratio
                //
                // Calculation results of numerator & denominator is always negative
                // which will cancel out to give positive output in the end so we can safely cast to u128.
                // For numerator:
                // ratioStart can only be >= ratioEnd so first part can only be reducing current_data.debt leading to
                // current_data.debt reduced - current_data.debt original * 1e27 -> can only be a negative number
                // For denominator:
                // col_per_debt and current_data.ref_ratio are inversely proportional to each other.
                // The maximum value they can ever be is ~9.97e26 which is the 0.3% away from 100% because liquidation
                // threshold + liquidation penalty can never be > 99.7%. This can also be verified by going back from
                // min / max ratio values further up where we fetch oracle price etc.
                // As optimization we can inverse numerator and denominator subtraction to directly get a positive number.

                // Calculate debt to liquidate
                let mut debt_liquidated: u128 = current_data.get_debt_liquidated(col_per_debt)?;

                // Calculate collateral to liquidate
                // extremely unlikely to overflow considering debt_liquidated is realistically within u64.
                let mut col_liquidated: u128 = debt_liquidated
                    .safe_mul(col_per_debt)?
                    .safe_div(10u128.pow(RATE_OUTPUT_DECIMALS))?;

                // Adjust for edge case
                if current_data.debt == debt_liquidated {
                    debt_liquidated = debt_liquidated.safe_sub(1)?;
                }

                if debt_liquidated >= current_data.debt_remaining
                    || current_data.is_ref_tick_liquidation_threshold()
                {
                    // @dev tick_has_debt_data already updated in fetch_next_tick_liquidate

                    // End of liquidation as full amount to liquidate or liquidation threshold tick has been reached
                    end_liquidate(
                        &mut current_data,
                        &mut tick_info,
                        &mut branch,
                        &mut debt_liquidated,
                        &mut col_liquidated,
                        col_per_debt,
                        get_minimum_branch_debt(ctx.accounts.borrow_token.decimals)?,
                    )?;

                    let mut branch_to_update = branch_accounts.load_mut(branch.id)?;

                    branch_to_update.update_state_at_liq_end(
                        tick_info.tick,
                        tick_info.partials,
                        current_data.debt,
                        branch.debt_factor.cast()?,
                    )?;

                    vault_state.update_state_at_liq_end(tick_info.tick, branch.id)?;

                    emit!(LogLiquidateInfo {
                        vault_id: vault_config.vault_id,
                        start_tick: vault_liquidation_start_tick, // start tick of liquidation, pre absorb
                        end_tick: vault_state.topmost_tick,
                    });

                    break;
                }

                // Calculate new debt factor
                // debtFactor = debtFactor * (liquidatableDebt - debtLiquidated) / liquidatableDebt
                // -> debtFactor * leftOverDebt / liquidatableDebt
                let debt_factor = current_data.get_debt_factor(debt_liquidated)?;

                current_data.reduce_debt_remaining(debt_liquidated)?;

                // Update totals
                current_data.update_totals(debt_liquidated, col_liquidated)?;

                // Update branch debt factor using mulDivBigNumber equivalent
                branch.update_branch_debt_factor(debt_factor)?;

                if current_data.is_ref_tick_liquidated() {
                    // Ref tick is base branch's minima, so we need to merge the current branch to base branch
                    // and make base branch the current branch

                    let new_branch_debt_factor = branch.base_branch_data.debt_factor;

                    let mut current_branch = branch_accounts.load_mut(branch.id)?;

                    current_branch.merge_with_base_branch(div_big_number(
                        new_branch_debt_factor,
                        branch.debt_factor,
                    )?)?;

                    branch.debt_factor = new_branch_debt_factor;

                    // Update branch variables to use base branch now
                    branch.update_branch_to_base_branch();
                }

                // Make reference tick the current tick for next iteration
                current_data.update_next_iterations_with_ref();
            }
        }

        // Calculate net token amounts using exchange price
        let (mut actual_debt_amt, mut actual_col_amt) = current_data.get_actual_amounts(
            borrow_ex_price,
            supply_ex_price,
            debt_amt.cast()?,
            vault_config.vault_id,
        )?;

        actual_debt_amt =
            unscale_amounts_up(actual_debt_amt.cast()?, ctx.accounts.borrow_token.decimals)?
                .cast::<u128>()?;

        actual_col_amt =
            unscale_amounts(actual_col_amt.cast()?, ctx.accounts.supply_token.decimals)?
                .cast::<u128>()?;

        // Check if slippage tolerance is maintained
        if actual_col_amt
            .safe_mul(10u128.pow(RATE_OUTPUT_DECIMALS))?
            .safe_div(actual_debt_amt)?
            < col_per_unit_debt
        {
            return Err(error!(ErrorCodes::VaultExcessSlippageLiquidation));
        }

        vault_state.reduce_total_supply(current_data.total_col_liq)?;
        vault_state.reduce_total_borrow(current_data.total_debt_liq)?;

        (
            vault_config.vault_id,
            vault_config.bump,
            actual_debt_amt, // return unscaled amount
            actual_col_amt,  // return unscaled amount
        )
    };

    if ctx.accounts.to.key() == ADDRESS_DEAD {
        let topmost_tick = ctx.accounts.vault_state.load()?.topmost_tick;
        // revert with liquidated amounts if to_ address is the dead address.
        // this can be used in a resolver to find the max liquidatable amounts.
        msg!(
            "VaultLiquidationResult: [{}, {}, {}]",
            actual_col_amt,
            actual_debt_amt,
            topmost_tick
        );
        return Err(error!(ErrorCodes::VaultLiquidationResult));
    }

    // PAYBACK AND WITHDRAW AT LIQUIDITY
    {
        let vault_id_bytes = vault_id.to_le_bytes();
        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_CONFIG_SEED, vault_id_bytes.as_slice(), &[bump]]];

        let accounts = &ctx.accounts.get_payback_accounts();

        // call pre_operate to setup the env for incoming borrow token on LL::operate
        accounts.pre_operate_with_signer(
            PreOperateInstructionParams {
                mint: ctx.accounts.borrow_token.key(),
            },
            signer_seeds,
            false,
        )?;

        // transfer the borrow token to the vault
        transfer_spl_tokens(TokenTransferParams {
            source: ctx.accounts.signer_token_account.to_account_info(), // signer borrow token account
            destination: ctx.accounts.vault_borrow_token_account.to_account_info(), // vault borrow token account
            authority: ctx.accounts.signer.to_account_info(),                       // msg.sender
            amount: actual_debt_amt.cast()?,
            token_program: ctx.accounts.borrow_token_program.to_account_info(),
            signer_seeds: None,
            mint: *ctx.accounts.borrow_token.clone(),
        })?;

        // payback at liquidity
        accounts.operate_with_signer(
            OperateInstructionParams {
                supply_amount: 0,
                borrow_amount: actual_debt_amt.cast::<i128>()?.safe_mul(-1)?,
                withdraw_to: ctx.accounts.liquidity.key(), // default withdraw_to will be liquidity PDA itself
                borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity PDA itself
                mint: ctx.accounts.borrow_token.key(),
                transfer_type: None, // not used for payback
            },
            signer_seeds,
        )?;

        let is_claim_type =
            transfer_type.is_some() && transfer_type.unwrap() == TransferType::CLAIM;

        let accounts = ctx.accounts.get_withdraw_accounts(is_claim_type)?;

        // withdraw at liquidity
        accounts.operate_with_signer(
            OperateInstructionParams {
                supply_amount: actual_col_amt.cast::<i128>()?.safe_mul(-1)?,
                borrow_amount: 0,
                withdraw_to: ctx.accounts.to.key(), // withdraw_to will be to address
                borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity PDA itself
                mint: ctx.accounts.supply_token.key(),
                transfer_type: if is_claim_type {
                    Some(TransferType::CLAIM)
                } else {
                    None
                },
            },
            signer_seeds,
        )?;
    }

    emit!(LogLiquidate {
        signer: ctx.accounts.signer.key(),
        col_amount: actual_col_amt.cast()?,
        debt_amount: actual_debt_amt.cast()?,
        to: ctx.accounts.to.key(),
    });

    Ok((actual_col_amt.cast()?, actual_debt_amt.cast()?))
}

#[allow(clippy::too_many_arguments)]
fn absorb<'info>(
    vault_state: &mut VaultState,
    tick_accounts: &Box<TickAccounts<'info>>,
    tick_has_debt_accounts: &Box<TickHasDebtAccounts<'info>>,
    branch_accounts: &BranchAccounts,
    new_branch: &AccountLoader<'_, Branch>,
    max_tick: i32,
) -> Result<(i128, i128)> {
    let (next_tick, mut col_absorbed, mut debt_absorbed) = tick_has_debt_accounts
        .fetch_next_tick_absorb(
            tick_accounts,
            vault_state.topmost_tick.safe_add(1)?,
            max_tick,
        )?;

    // Process branches
    let mut branch_data = BranchMemoryVars::default();
    let mut new_branch_id = 0; // If this remains 0, it means create a new branch

    branch_data.id = vault_state.current_branch_id;
    branch_data.set_branch_data_load(&branch_accounts.get(branch_data.id)?)?;

    if !vault_state.is_branch_liquidated() {
        // Current branch is not liquidated, can be used as a new branch if needed
        new_branch_id = branch_data.id;

        // Check base branch minima tick
        if branch_data.data.connected_minima_tick != COLD_TICK {
            // Setting the base branch as current liquidatable branch
            branch_data.id = branch_data.data.connected_branch_id;
            branch_data.minima_tick = branch_data.data.connected_minima_tick;

            branch_data.set_branch_data_load(&branch_accounts.get(branch_data.id)?)?;
        } else {
            // The current branch is base branch, need to setup a new base branch
            branch_data.id = 0;
            branch_data.reset_branch_data();
            branch_data.minima_tick = COLD_TICK;
        }
    } else {
        // Current branch is liquidated
        branch_data.minima_tick = branch_data.data.minima_tick;
    }

    while branch_data.minima_tick > max_tick {
        // Check base branch, if exists then check if minima tick is above max tick then liquidate it.
        let current_ratio = branch_data.get_current_ratio_from_minima_tick()?;
        let branch_debt: u128 = branch_data.data.debt_liquidity.cast()?;

        // Absorb branch's debt & collateral
        debt_absorbed = debt_absorbed.safe_add(branch_debt)?;
        col_absorbed = col_absorbed.safe_add(
            branch_debt
                .safe_mul(TickMath::ZERO_TICK_SCALED_RATIO)?
                .safe_div(current_ratio)?,
        )?;

        // Close the branch (mark as status 3)
        let mut branch_to_update = branch_accounts.load_mut(branch_data.id)?;

        branch_to_update.set_state_after_absorb(&branch_data.data);

        // Find the next branch to process
        if branch_data.data.connected_minima_tick != COLD_TICK {
            // Set the base branch as current liquidatable branch
            branch_data.id = branch_data.data.connected_branch_id;
            branch_data.minima_tick = branch_data.data.connected_minima_tick;

            branch_data.set_branch_data_load(&branch_accounts.get(branch_data.id)?)?;
        } else {
            // The current branch is base branch, no more branches to process
            branch_data.id = 0;
            branch_data.reset_branch_data();
            branch_data.minima_tick = COLD_TICK;
        }
    }

    // Update vault state based on next tick and branch status
    if next_tick >= branch_data.minima_tick {
        // New top tick is not liquidated
        if next_tick > COLD_TICK {
            vault_state.topmost_tick = next_tick;
        } else {
            vault_state.reset_top_tick();
        }

        let init_new_branch = new_branch_id == 0;
        if init_new_branch {
            vault_state.update_branch_info_by_one();
            new_branch_id = vault_state.total_branch_id;
        } else {
            // using already initialized non liquidated branch
            vault_state.reset_branch_liquidated();
        }

        if branch_data.minima_tick > COLD_TICK {
            let mut new_branch_loader = new_branch.load_mut()?;
            if new_branch_id != new_branch_loader.branch_id {
                return Err(error!(ErrorCodes::VaultNewBranchInvalid));
            }

            new_branch_loader.reset_branch_data();
            new_branch_loader.set_connections(branch_data.id, branch_data.minima_tick)?;
        } else {
            let mut branch_to_clear = if init_new_branch {
                let new_branch_loader = new_branch.load_mut()?;
                if new_branch_id != new_branch_loader.branch_id {
                    return Err(error!(ErrorCodes::VaultNewBranchInvalid));
                }

                new_branch_loader
            } else {
                branch_accounts.load_mut(new_branch_id)?
            };

            branch_to_clear.reset_branch_data();
        }
    } else {
        // New top tick is liquidated
        vault_state.update_state_at_liq_end(branch_data.minima_tick, branch_data.id)?;

        if new_branch_id != 0 {
            vault_state.total_branch_id = new_branch_id.safe_sub(1)?; // decreasing total branch by 1

            let mut branch_to_clear = branch_accounts.load_mut(new_branch_id)?;

            if new_branch_id != branch_to_clear.branch_id {
                return Err(error!(ErrorCodes::VaultNewBranchInvalid));
            }

            branch_to_clear.reset_branch_data();
        }
    }

    vault_state.add_absorbed_col_amount(col_absorbed)?;
    vault_state.add_absorbed_debt_amount(debt_absorbed)?;

    Ok((col_absorbed.cast()?, debt_absorbed.cast()?))
}

pub fn rebalance<'info>(
    ctx: Context<'_, '_, 'info, 'info, Rebalance<'info>>,
) -> Result<(i128, i128)> {
    verify_rebalance(&ctx)?;

    let vault_state = ctx.accounts.vault_state.load()?;
    let vault_config = ctx.accounts.vault_config.load()?;

    // Get exchange prices
    let (liq_supply_ex_price, liq_borrow_ex_price, vault_supply_ex_price, vault_borrow_ex_price) =
        vault_state.load_exchange_prices(
            &vault_config,
            &ctx.accounts.supply_token_reserves_liquidity,
            &ctx.accounts.borrow_token_reserves_liquidity,
        )?;

    let supply_position = ctx.accounts.vault_supply_position_on_liquidity.load()?;

    let total_supply_liquidity = supply_position
        .get_amount()?
        .safe_mul(liq_supply_ex_price)?
        .safe_div(EXCHANGE_PRICES_PRECISION)?;

    let borrow_position = ctx.accounts.vault_borrow_position_on_liquidity.load()?;

    let total_borrow_liquidity = borrow_position
        .get_amount()?
        .safe_mul(liq_borrow_ex_price)?
        .safe_div(EXCHANGE_PRICES_PRECISION)?;

    let mut total_supply_vault = vault_state
        .get_total_supply()?
        .safe_mul(vault_supply_ex_price)?
        .safe_div(EXCHANGE_PRICES_PRECISION)?;

    // Unscale the total supply vault
    total_supply_vault = unscale_amounts(
        total_supply_vault.cast()?,
        ctx.accounts.supply_token.decimals,
    )?
    .cast::<u128>()?;

    let mut total_borrow_vault = vault_state
        .get_total_borrow()?
        .safe_mul(vault_borrow_ex_price)?
        .safe_div(EXCHANGE_PRICES_PRECISION)?;

    // Unscale the total borrow vault
    total_borrow_vault = unscale_amounts(
        total_borrow_vault.cast()?,
        ctx.accounts.borrow_token.decimals,
    )?
    .cast::<u128>()?;

    let mut supply_amt: i128 = 0;
    let mut borrow_amt: i128 = 0;

    let vault_id = vault_config.vault_id;
    let vault_id_bytes = vault_id.to_le_bytes();
    let signer_seeds: &[&[&[u8]]] = &[&[
        VAULT_CONFIG_SEED,
        vault_id_bytes.as_slice(),
        &[vault_config.bump],
    ]];

    drop(supply_position);
    drop(borrow_position);

    // Rebalance supply
    if total_supply_vault > total_supply_liquidity {
        // Fetch tokens from rebalancer and supply to liquidity
        supply_amt = (total_supply_vault - total_supply_liquidity).cast()?;
        let accounts = ctx.accounts.get_deposit_accounts();

        accounts.pre_operate_with_signer(
            PreOperateInstructionParams {
                mint: ctx.accounts.supply_token.key(),
            },
            signer_seeds,
            true,
        )?;

        transfer_spl_tokens(TokenTransferParams {
            source: ctx
                .accounts
                .rebalancer_supply_token_account
                .to_account_info(),
            destination: ctx.accounts.vault_supply_token_account.to_account_info(),
            authority: ctx.accounts.rebalancer.to_account_info(),
            amount: supply_amt.cast()?,
            token_program: ctx.accounts.supply_token_program.to_account_info(),
            signer_seeds: None,
            mint: *ctx.accounts.supply_token.clone(),
        })?;

        // deposit at liquidity
        accounts.operate_with_signer(
            OperateInstructionParams {
                supply_amount: supply_amt,
                borrow_amount: 0,
                withdraw_to: ctx.accounts.liquidity.key(), // default withdraw_to will be liquidity PDA itself
                borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity PDA itself
                mint: ctx.accounts.supply_token.key(),
                transfer_type: None, // Not used for deposit
            },
            signer_seeds,
        )?;
    } else if total_supply_liquidity > total_supply_vault {
        // Withdraw from Liquidity and send to rebalancer
        supply_amt = (total_supply_liquidity - total_supply_vault)
            .cast::<i128>()?
            .safe_mul(-1)?;

        // Withdraw from Liquidity contract and send it to revenue contract.
        // This is the scenario when the vault user's are getting less ETH APR than what's going on Liquidity contract.
        // When supply rate magnifier is less than 1.
        ctx.accounts.get_withdraw_accounts().operate_with_signer(
            OperateInstructionParams {
                supply_amount: supply_amt, // withdraw from liquidity
                borrow_amount: 0,
                withdraw_to: ctx.accounts.rebalancer.key(), // withdraw_to will be rebalancer address
                borrow_to: ctx.accounts.liquidity.key(), // borrow_to will be liquidity PDA itself
                mint: ctx.accounts.supply_token.key(),
                transfer_type: None, // Not used for withdraw in rebalance
            },
            signer_seeds,
        )?;
    }

    // Rebalance borrow
    if total_borrow_vault > total_borrow_liquidity {
        borrow_amt = (total_borrow_vault - total_borrow_liquidity).cast()?;

        // Borrow from Liquidity contract and send to revenue/rebalance contract
        // This is the scenario when the vault is charging more borrow to user than the Liquidity contract.
        // When borrow rate magnifier is greater than 1.
        ctx.accounts.get_borrow_accounts().operate_with_signer(
            OperateInstructionParams {
                supply_amount: 0,
                borrow_amount: borrow_amt,
                withdraw_to: ctx.accounts.liquidity.key(), // default withdraw_to will be liquidity PDA itself
                borrow_to: ctx.accounts.rebalancer.key(),  // borrow_to will be rebalancer address
                mint: ctx.accounts.borrow_token.key(),
                transfer_type: None, // Not used for borrow in rebalance
            },
            signer_seeds,
        )?;
    } else if total_borrow_liquidity > total_borrow_vault {
        // Transfer from rebalancer and payback on Liquidity
        borrow_amt = (total_borrow_liquidity - total_borrow_vault)
            .cast::<i128>()?
            .safe_mul(-1)?;

        // Transfer from revenue/rebalance contract and payback on Liquidity contract
        // This is the scenario when vault protocol is earning rewards so effective borrow rate for users is low.
        // Or the case where borrow rate magnifier is less than 1

        let accounts = ctx.accounts.get_payback_accounts();

        accounts.pre_operate_with_signer(
            PreOperateInstructionParams {
                mint: ctx.accounts.borrow_token.key(),
            },
            signer_seeds,
            false,
        )?;

        transfer_spl_tokens(TokenTransferParams {
            source: ctx
                .accounts
                .rebalancer_borrow_token_account
                .to_account_info(),
            destination: ctx.accounts.vault_borrow_token_account.to_account_info(),
            authority: ctx.accounts.rebalancer.to_account_info(),
            amount: borrow_amt.abs().cast()?,
            token_program: ctx.accounts.borrow_token_program.to_account_info(),
            signer_seeds: None,
            mint: *ctx.accounts.borrow_token.clone(),
        })?;

        accounts.operate_with_signer(
            OperateInstructionParams {
                supply_amount: 0,
                borrow_amount: borrow_amt, // payback on liquidity
                withdraw_to: ctx.accounts.liquidity.key(), // default withdraw_to will be liquidity PDA itself
                borrow_to: ctx.accounts.liquidity.key(), // default borrow_to will be liquidity PDA itself
                mint: ctx.accounts.borrow_token.key(),
                transfer_type: None, // Not used for payback in rebalance
            },
            signer_seeds,
        )?;
    }

    if supply_amt == 0 && borrow_amt == 0 {
        return Err(error!(ErrorCodes::VaultNothingToRebalance));
    }

    emit!(LogRebalance {
        supply_amt,
        borrow_amt,
    });

    Ok((supply_amt, borrow_amt))
}
