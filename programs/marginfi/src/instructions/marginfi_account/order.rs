use crate::events::{
    AccountEventHeader, KeeperCloseOrderEvent, MarginfiAccountCloseOrderEvent,
    MarginfiAccountPlaceOrderEvent, SetKeeperCloseFlagsEvent,
};
use crate::instructions::marginfi_account::liquidate_start::validate_instructions;
use crate::ix_utils::{
    get_discrim_hash, keys_sha256_hash, validate_not_cpi_by_stack_height, Hashable,
};
use crate::state::marginfi_account::{
    account_not_frozen_for_authority, get_health_components, get_tagged_account_health_components,
    is_signer_authorized, run_cb_price_gate,
};
use crate::state::premium::{MarginfiAccountPremiumImpl, PremiumScratch};
use crate::state::rate::{debt_index_of, venue_multiplier, yield_index_of};
use crate::utils::is_integration_asset_tag;
use crate::{
    check,
    prelude::*,
    state::{
        bank::BankImpl,
        marginfi_account::{
            get_remaining_accounts_per_bank, LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        order::{ExecuteOrderRecordImpl, LegSpan, OrderImpl},
    },
};
use crate::{check_eq, math_error};
use anchor_lang::{prelude::*, system_program};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ix_discriminators, EXECUTE_ORDER_SEED, FEE_STATE_SEED, ORDER_ACTIVE_TAGS, ORDER_SEED,
    },
    types::{
        u32_to_milli, BalanceSide, Bank, ExecuteOrderRecord, FeeState, HealthCache,
        HealthPriceMode, InterestTriggerConfig, MarginfiAccount, MarginfiGroup, Order,
        OrderTrigger, OrderTriggerType, RequirementType, ACCOUNT_IN_ORDER_EXECUTION,
        ACCOUNT_IN_REBALANCE, ORDER_BLOCKING_FLAGS,
    },
};

pub fn place_order(
    ctx: Context<PlaceOrder>,
    bank_keys: Vec<Pubkey>,
    trigger: OrderTrigger,
) -> MarginfiResult {
    init_order(ctx, bank_keys, trigger, None)
}

/// [`place_order`] with a carry-exit policy. The rates are measured from the legs' banks, so the
/// order needs no accounts beyond [`PlaceOrder`] and is live from placement.
pub fn place_interest_order(
    ctx: Context<PlaceOrder>,
    bank_keys: Vec<Pubkey>,
    trigger: OrderTrigger,
    interest: InterestTriggerConfig,
) -> MarginfiResult {
    init_order(ctx, bank_keys, trigger, Some(interest))
}

/// Tag both legs of the pair, write the order account, charge the flat init fee and emit the place
/// event.
fn init_order(
    ctx: Context<PlaceOrder>,
    bank_keys: Vec<Pubkey>,
    trigger: OrderTrigger,
    interest: Option<InterestTriggerConfig>,
) -> MarginfiResult {
    let PlaceOrder {
        marginfi_account: marginfi_account_loader,
        order: order_loader,
        fee_state: fee_state_loader,
        ..
    } = &ctx.accounts;

    let mut marginfi_account = marginfi_account_loader.load_mut()?;

    check!(
        bank_keys.len() == ORDER_ACTIVE_TAGS,
        MarginfiError::InvalidBalanceCount
    );

    // ORDER_ACTIVE_TAGS == 2
    let bank_key_1 = &bank_keys[0];
    let bank_key_2 = &bank_keys[1];

    check!(bank_key_1 != bank_key_2, MarginfiError::DuplicateBalance);

    let lending_account = &mut marginfi_account.lending_account;

    let balance_index_1 = lending_account.get_balance_index(bank_key_1)?;
    let balance_index_2 = lending_account.get_balance_index(bank_key_2)?;

    // Ensure we have one asset and one liability
    match (
        lending_account.balances[balance_index_1].get_side(),
        lending_account.balances[balance_index_2].get_side(),
    ) {
        (Some(BalanceSide::Assets), Some(BalanceSide::Liabilities)) => {}
        (Some(BalanceSide::Liabilities), Some(BalanceSide::Assets)) => {}
        _ => return err!(MarginfiError::InvalidAssetOrLiabilitiesCount),
    };

    // Reserve tags for the balances if necessary
    let balance_1_needs_tag = lending_account.balances[balance_index_1].tag == 0;
    let balance_2_needs_tag = lending_account.balances[balance_index_2].tag == 0;

    let empty_tag_count = balance_1_needs_tag as usize + balance_2_needs_tag as usize;

    if empty_tag_count > 0 {
        let new_tags = lending_account.reserve_n_tags(empty_tag_count);
        let mut tag_index = 0;

        if balance_1_needs_tag {
            lending_account.balances[balance_index_1].tag = new_tags[tag_index];
            tag_index += 1;
        }

        if balance_2_needs_tag {
            lending_account.balances[balance_index_2].tag = new_tags[tag_index];
        }
    }

    let tags = [
        lending_account.balances[balance_index_1].tag,
        lending_account.balances[balance_index_2].tag,
    ];

    let marginfi_account_key = marginfi_account_loader.key();

    let mut order = order_loader.load_init()?;
    order.initialize(
        marginfi_account_key,
        trigger,
        interest,
        tags,
        ctx.bumps.order,
        Clock::get()?.unix_timestamp,
    )?;
    marginfi_account.increment_active_orders()?;

    let order_init_flat_sol_fee = fee_state_loader.load()?.order_init_flat_sol_fee;
    if order_init_flat_sol_fee > 0 {
        anchor_lang::system_program::transfer(
            ctx.accounts.transfer_flat_fee(),
            order_init_flat_sol_fee as u64,
        )?;
    }

    emit!(MarginfiAccountPlaceOrderEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: marginfi_account_key,
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        order: order_loader.key(),
        trigger: order.trigger,
        stop_loss: order.stop_loss,
        take_profit: order.take_profit,
        tags,
        interest_window_seconds: order.interest_window_seconds,
        interest_exit_budget_seconds: order.interest_exit_budget_seconds,
        interest_min_negative_apr: order.interest_min_negative_apr,
    });

    Ok(())
}

pub fn close_order(ctx: Context<CloseOrder>) -> MarginfiResult {
    let CloseOrder {
        marginfi_account: marginfi_account_loader,
        authority,
        order: order_loader,
        ..
    } = &ctx.accounts;

    let mut marginfi_account = marginfi_account_loader.load_mut()?;
    marginfi_account.decrement_active_orders()?;

    emit!(MarginfiAccountCloseOrderEvent {
        header: AccountEventHeader {
            signer: Some(authority.key()),
            marginfi_account: marginfi_account_loader.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        order: order_loader.key(),
    });

    Ok(())
}

pub fn keeper_close_order(ctx: Context<KeeperCloseOrder>) -> MarginfiResult {
    let KeeperCloseOrder {
        order: order_loader,
        marginfi_account,
        ..
    } = &ctx.accounts;

    let order = order_loader.load()?;
    let marginfi_account_info = marginfi_account.to_account_info();

    // Manual owner check: Only attempt to deserialize when the account is not closed
    let (authority_pk, group_pk, can_close) = if marginfi_account_info.owner.eq(&system_program::ID)
        && marginfi_account_info.data_is_empty()
    {
        (Pubkey::default(), Pubkey::default(), true)
    } else {
        // Deserialize manually using bytemuck to avoid lifetime issues
        let mut data = marginfi_account_info.try_borrow_mut_data()?;

        // Check discriminator
        require!(
            data.len() >= 8 + std::mem::size_of::<MarginfiAccount>(),
            MarginfiError::InternalLogicError
        );

        let disc = &data[..8];
        check_eq!(
            disc,
            MarginfiAccount::DISCRIMINATOR,
            MarginfiError::InternalLogicError
        );

        let marginfi_account: &mut MarginfiAccount =
            bytemuck::from_bytes_mut(&mut data[8..8 + std::mem::size_of::<MarginfiAccount>()]);

        let balances = &marginfi_account.lending_account.balances;
        // Can close if any of the balances used in the order no longer exists (or if its tag was cleared)
        let can_close = order.tags.iter().any(|tag| {
            !balances
                .iter()
                .any(|balance| balance.is_active() && balance.tag == *tag)
        });
        if can_close {
            marginfi_account.decrement_active_orders()?;
        }
        (
            marginfi_account.authority,
            marginfi_account.group,
            can_close,
        )
    };

    check!(can_close, MarginfiError::LiquidatorOrderCloseNotAllowed);

    emit!(KeeperCloseOrderEvent {
        header: AccountEventHeader {
            signer: None,
            marginfi_account: marginfi_account_info.key(),
            marginfi_account_authority: authority_pk,
            marginfi_group: group_pk,
        },
        order: order_loader.key(),
    });

    Ok(())
}

pub fn set_keeper_close_flags(
    ctx: Context<SetKeeperCloseFlags>,
    bank_keys_opt: Option<Vec<Pubkey>>,
) -> MarginfiResult {
    let SetKeeperCloseFlags {
        marginfi_account, ..
    } = &ctx.accounts;

    let mut marginfi_account = marginfi_account.load_mut()?;

    let lending_account = &mut marginfi_account.lending_account;

    match bank_keys_opt {
        Some(ref keys) => {
            for bank_key in keys.iter() {
                let index = lending_account.get_balance_index(bank_key)?;

                let balance = &mut lending_account.balances[index];
                balance.tag = 0;
            }
        }
        None => {
            for balance in lending_account.balances.iter_mut() {
                balance.tag = 0;
            }
        }
    }

    emit!(SetKeeperCloseFlagsEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank_keys: bank_keys_opt,
    });

    Ok(())
}

/// Both legs of an interest-triggered order, read once their banks are current.
struct OrderLegs {
    asset: LegSpan,
    debt: LegSpan,
    premium_apr: I80F48,
}

/// Accrue the order's two banks and read each leg's share index out of the health observation
/// stream, spanned from the bank reading nearest `window` seconds old.
fn read_order_legs<'info>(
    marginfi_account: &MarginfiAccount,
    remaining_ais: &'info [AccountInfo<'info>],
    order_tags: &[u16; ORDER_ACTIVE_TAGS],
    group: &MarginfiGroup,
    clock: &Clock,
    window: i64,
) -> MarginfiResult<OrderLegs> {
    let mut asset: Option<LegSpan> = None;
    let mut debt: Option<(LegSpan, I80F48)> = None;
    let mut account_index = 0usize;

    for balance in marginfi_account
        .lending_account
        .balances
        .iter()
        .filter(|b| b.is_active())
    {
        let bank_ai = remaining_ais
            .get(account_index)
            .ok_or(MarginfiError::InvalidBankAccount)?;
        let bank_al = AccountLoader::<Bank>::try_from(bank_ai)?;
        check_eq!(
            balance.bank_pk,
            *bank_ai.key,
            MarginfiError::InvalidBankAccount
        );
        let num_accounts = {
            let bank = bank_al.load()?;
            get_remaining_accounts_per_bank(&bank)?
        };

        if !order_tags.contains(&balance.tag) {
            account_index += num_accounts;
            continue;
        }

        let end_idx = account_index + num_accounts;
        require_gte!(
            remaining_ais.len(),
            end_idx,
            MarginfiError::WrongNumberOfOracleAccounts
        );
        let oracle_ais = &remaining_ais[account_index + 1..end_idx];

        check!(
            bank_ai.is_writable,
            MarginfiError::OrderInterestBankNotWritable
        );
        let side = balance
            .get_side()
            .ok_or_else(|| error!(MarginfiError::IllegalBalanceState))?;

        {
            let mut bank = bank_al.load_mut()?;
            if !is_integration_asset_tag(bank.config.asset_tag) {
                bank.accrue_interest(
                    clock.unix_timestamp,
                    group,
                    #[cfg(not(feature = "client"))]
                    *bank_ai.key,
                )?;
                bank.update_bank_cache(group)?;
            }
        }

        let bank = bank_al.load()?;
        let multiplier = venue_multiplier(&bank, oracle_ais, clock)?;
        let reading = bank
            .rate_reading_at_least(window, clock.unix_timestamp)
            .ok_or(MarginfiError::OrderInterestHistoryTooShort)?;
        let elapsed = clock
            .unix_timestamp
            .checked_sub(reading.timestamp)
            .ok_or_else(math_error!())?;
        match side {
            BalanceSide::Assets => {
                asset = Some(LegSpan {
                    start: reading.asset_index(),
                    end: yield_index_of(&bank, multiplier)?,
                    elapsed,
                });
            }
            BalanceSide::Liabilities => {
                debt = Some((
                    LegSpan {
                        start: reading.debt_index(),
                        end: debt_index_of(&bank, multiplier)?,
                        elapsed,
                    },
                    u32_to_milli(balance.premium_rate_snapshot),
                ));
            }
        }
        account_index += num_accounts;
    }

    let asset = asset.ok_or(MarginfiError::LendingAccountBalanceNotFound)?;
    let (debt, premium_apr) = debt.ok_or(MarginfiError::LendingAccountBalanceNotFound)?;
    Ok(OrderLegs {
        asset,
        debt,
        premium_apr,
    })
}

pub fn start_execute_order<'info>(ctx: Context<'info, StartExecuteOrder<'info>>) -> MarginfiResult {
    let StartExecuteOrder {
        marginfi_account: marginfi_account_loader,
        fee_payer: _fee_payer,
        executor,
        order: order_loader,
        execute_record: execute_record_loader,
        instruction_sysvar,
        ..
    } = &ctx.accounts;

    let clock = Clock::get()?;
    let mut marginfi_account = marginfi_account_loader.load_mut()?;
    let mut order = order_loader.load_mut()?;

    marginfi_account.set_flag(ACCOUNT_IN_ORDER_EXECUTION, false);

    // Both legs are brought current first, so the equity below and the rates share one accrued
    // state. A leg error surfaces only if the price condition does not carry the execution.
    let (legs, leg_error) = if order.interest_trigger_enabled() {
        let group = ctx.accounts.group.load()?;
        match read_order_legs(
            &marginfi_account,
            ctx.remaining_accounts,
            &order.tags,
            &group,
            &clock,
            i64::from(order.interest_window_seconds),
        ) {
            Ok(legs) => (Some(legs), None),
            Err(err) => (None, Some(err)),
        }
    } else {
        (None, None)
    };

    run_cb_price_gate(&marginfi_account, ctx.remaining_accounts)?;

    let (order_assets_in_equity, order_liabs_in_equity, order_asset_count, order_liab_count) =
        get_tagged_account_health_components(
            &marginfi_account,
            ctx.remaining_accounts,
            &order.tags,
        )?;

    check!(
        order_asset_count + order_liab_count == ORDER_ACTIVE_TAGS,
        MarginfiError::LendingAccountBalanceNotFound
    );

    // Also gate at start: the order can close a tagged balance before the end gate runs, so a bank
    // whose breaching price sets the trigger must be caught here while it's still active.
    run_cb_price_gate(&marginfi_account, ctx.remaining_accounts)?;

    let net = order_assets_in_equity
        .checked_sub(order_liabs_in_equity)
        .ok_or_else(math_error!())?;

    // Either condition can trigger the execution; the record remembers which, for the end-side bound.
    let price_met = match order.trigger {
        OrderTriggerType::StopLoss => net <= I80F48::from(order.stop_loss),
        OrderTriggerType::TakeProfit => net >= I80F48::from(order.take_profit),
        OrderTriggerType::Both => {
            net <= I80F48::from(order.stop_loss) || net >= I80F48::from(order.take_profit)
        }
    };

    let (interest_met, interest_carry) = match legs {
        Some(legs) => {
            let carry = order.realized_carry(
                &legs.asset,
                &legs.debt,
                order_assets_in_equity,
                order_liabs_in_equity,
                legs.premium_apr,
            )?;
            (
                order.interest_condition_met(carry, order_assets_in_equity)?,
                carry,
            )
        }
        None => (false, I80F48::ZERO),
    };

    if !(price_met || interest_met) {
        return Err(leg_error.unwrap_or_else(|| {
            error!(if order.interest_trigger_enabled() {
                MarginfiError::OrderInterestNotNegative
            } else {
                MarginfiError::OrderTriggerNotMet
            })
        }));
    }

    // An execution triggered only by carry leaves the user's configured level untouched.
    if price_met
        && matches!(
            order.trigger,
            OrderTriggerType::StopLoss | OrderTriggerType::Both
        )
    {
        order.stop_loss = net.into();
    }

    let mut met_conditions = 0u8;
    if price_met {
        met_conditions |= ExecuteOrderRecord::MET_PRICE;
    }
    if interest_met {
        met_conditions |= ExecuteOrderRecord::MET_INTEREST;
    }

    // Create execution record
    let mut execute_record = execute_record_loader.load_init()?;

    // Store the order, executor, health of the order balances as well as all the active non-order balances.
    execute_record.initialize(
        order_loader.key(),
        executor.key(),
        &marginfi_account,
        &order.tags,
        &net,
        met_conditions,
        interest_carry,
    )?;

    validate_instructions(
        instruction_sysvar,
        ctx.program_id,
        &ix_discriminators::START_EXECUTE_ORDER,
        &ix_discriminators::END_EXECUTE_ORDER,
    )
}

pub fn end_execute_order<'info>(ctx: Context<'info, EndExecuteOrder<'info>>) -> MarginfiResult {
    let EndExecuteOrder {
        marginfi_account: marginfi_account_loader,
        order: order_loader,
        execute_record: execute_record_loader,
        fee_state: fee_state_loader,
        ..
    } = &ctx.accounts;

    validate_not_cpi_by_stack_height()?;

    let mut marginfi_account = marginfi_account_loader.load_mut()?;
    let order = order_loader.load()?;
    let execute_record = execute_record_loader.load()?;
    let fee_state = fee_state_loader.load()?;

    let mut health_cache = HealthCache::zeroed();
    let group = ctx.accounts.group.load()?;
    let mut premium_scratch = PremiumScratch::default();
    let (
        (order_assets_in_equity, _order_liabs_in_equity, _order_asset_count, order_liab_count),
        is_healthy,
    ) = {
        let (assets, liabs) = get_health_components(
            &marginfi_account,
            &group,
            ctx.remaining_accounts,
            RequirementType::Maintenance,
            &mut Some(&mut health_cache),
            HealthPriceMode::Live { liq_cache: None },
            &mut Some(&mut premium_scratch),
        )?;

        let account_health = assets.checked_sub(liabs).ok_or_else(math_error!())?;

        let is_healthy = account_health >= I80F48::ZERO;

        health_cache.set_healthy(is_healthy);

        (
            get_tagged_account_health_components(
                &marginfi_account,
                ctx.remaining_accounts,
                &order.tags,
            )?,
            is_healthy,
        )
    };

    marginfi_account.health_cache = health_cache;

    // Inline CB gate: order execution moves funds at oracle prices, so revert if any involved
    // bank's live price has jumped past the breach threshold relative to its reference.
    run_cb_price_gate(&marginfi_account, ctx.remaining_accounts)?;

    check!(
        order_liab_count.eq(&0), // All order liabilities should be closed
        MarginfiError::OrderLiabilityNotClosed
    );

    let net = order_assets_in_equity;

    // The user slippage constraint we want to enforce is:
    // net >= (1 - slippage) * (tp or sl)
    // where slippage is encoded as a u32 percent (0..100% mapped to 0..u32::MAX).

    // For the TP case another constraint(the max fee constraint) we want to enforce is:
    // net >= (1 - max_fee) * (start health)
    // It may be possible that the value (1 - max_fee) * (start health) is less than the
    // min allowed value based on the user's slippage constraint alone, in that case
    // we clamp it to the slippage, allowing for that much.
    // In the case where the value was greater we use (1 - max_fee) * (start health) instead.

    // Check that the liquidator did not over-withdraw.

    let slippage_frac = {
        let slippage = marginfi_type_crate::types::u32_to_centi(order.max_slippage);
        I80F48::ONE
            .checked_sub(slippage)
            .ok_or_else(math_error!())?
    };

    let max_fee_frac = {
        let max_fee: I80F48 = fee_state.order_execution_max_fee.into();
        I80F48::ONE.checked_sub(max_fee).ok_or_else(math_error!())?
    };

    let start_health: I80F48 = execute_record.order_start_health.into();

    // Each met condition brings its own cost bound; one that was not met is not evaluated.
    let price_ok = execute_record.met_price()
        && match order.trigger {
            OrderTriggerType::StopLoss => {
                let sl: I80F48 = order.stop_loss.into();
                net >= sl.checked_mul(slippage_frac).ok_or_else(math_error!())?
            }
            OrderTriggerType::TakeProfit => {
                let tp: I80F48 = order.take_profit.into();
                let allowed_tp = tp.checked_mul(slippage_frac).ok_or_else(math_error!())?;
                let allowed_diff = start_health
                    .checked_mul(max_fee_frac)
                    .ok_or_else(math_error!())?;
                net >= allowed_diff && net >= allowed_tp
            }
            OrderTriggerType::Both => {
                // This branch relies on sl being < tp, which is enforced in the code to tell them apart
                let tp: I80F48 = order.take_profit.into();
                if start_health >= tp {
                    let allowed_tp = tp.checked_mul(slippage_frac).ok_or_else(math_error!())?;
                    let allowed_diff = start_health
                        .checked_mul(max_fee_frac)
                        .ok_or_else(math_error!())?;
                    net >= allowed_diff && net >= allowed_tp
                } else {
                    let sl: I80F48 = order.stop_loss.into();
                    net >= sl.checked_mul(slippage_frac).ok_or_else(math_error!())?
                }
            }
        };

    // The user's slippage ceiling binds on top, so the budget never widens the exit past it.
    let (interest_ok, over_carry_budget) = if execute_record.met_interest() {
        let carry: I80F48 = execute_record.interest_carry.into();
        let realized_cost = start_health.checked_sub(net).ok_or_else(math_error!())?;
        let ceiling = start_health
            .checked_mul(slippage_frac)
            .ok_or_else(math_error!())?;
        let within_budget = realized_cost <= order.interest_allowed_cost(carry)?;
        (within_budget && net >= ceiling, !within_budget)
    } else {
        (false, false)
    };

    check!(
        price_ok || interest_ok,
        if execute_record.met_interest() && !execute_record.met_price() && over_carry_budget {
            MarginfiError::OrderInterestCostExceedsCarry
        } else {
            MarginfiError::OrderExecutionOverWithdrawal
        }
    );

    // Only one asset and liab are currently involved in a balance, with the single liability being
    // closed.
    // * Note: There is a trivial edge case where e.g. a user has $50 A, $50 B, borrowing $50 C,
    // sets an order that would normally close A AND C but cannot execute because orders cannot
    // close two balances. See `limit_orders_overlap_ab_nearly_closes_a_ad_fails_start` for a demo.
    // We expect to eventually handle this once orders can support multiple positions (allowing the
    // keeper e.g. take $25 each from A/B and repay $50 C instead)
    let closed_order_balances_count = 1;

    // order_liab_in_equity = 0
    let order_current_health = order_assets_in_equity;

    // Check that the non-order balances remain unchanged, including inactive ones. Also check that
    // the account is at least as healthy as it was at the start of execution, If it wasn't since
    // the user specified the slippage, we at least make sure the account is still healthy after
    // execution to avoid the position taking on more risk.
    execute_record.check_health_and_verify_unchanged(
        &marginfi_account,
        closed_order_balances_count,
        &order_current_health,
        is_healthy,
    )?;

    // At this point we know that all non order balances were not touched and the order
    // balances that were touched:
    // 1) Is still above or equal to the trigger price (in equity terms).
    // 2) Did not make the account less healthy and if at all we did, the account is
    //    still healthy overall.

    // Withdraw defers its snapshot refresh while ACCOUNT_IN_ORDER_EXECUTION is set, so this
    // handler owns it: claim at old rates and re-weight surviving liabilities against the
    // post-order collateral mix.
    marginfi_account.update_premium_snapshots(
        &group,
        &premium_scratch,
        Clock::get()?.unix_timestamp as u64,
    )?;

    marginfi_account.unset_flag(ACCOUNT_IN_ORDER_EXECUTION, false);
    marginfi_account.decrement_active_orders()?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(bank_keys: Vec<Pubkey>)]
pub struct PlaceOrder<'info> {
    #[account(
        constraint = (!group.load()?.is_protocol_paused()) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = authority @ MarginfiError::Unauthorized,
        constraint = !marginfi_account.load()?.get_flag(
            ORDER_BLOCKING_FLAGS | ACCOUNT_IN_ORDER_EXECUTION | ACCOUNT_IN_REBALANCE
        ) @ MarginfiError::UnexpectedOrderExecutionState
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub authority: Signer<'info>,

    #[account(
        init,
        payer = fee_payer,
        space = 8 + std::mem::size_of::<Order>(),
        seeds = [
            ORDER_SEED.as_bytes(),
            marginfi_account.key().as_ref(),
            &keys_sha256_hash(&bank_keys) // This ensures each combination of balances has only one order.
        ],
        bump
    )]
    pub order: AccountLoader<'info, Order>,

    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_wallet @ MarginfiError::InvalidFeeWallet
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// CHECK: The fee admin's native SOL wallet, validated against fee state
    #[account(mut)]
    pub global_fee_wallet: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> PlaceOrder<'info> {
    fn transfer_flat_fee(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, anchor_lang::system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.key(),
            anchor_lang::system_program::Transfer {
                from: self.fee_payer.to_account_info(),
                to: self.global_fee_wallet.to_account_info(),
            },
        )
    }
}

#[derive(Accounts)]
pub struct CloseOrder<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let a = marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), false, false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = marginfi_account,
        close = fee_recipient
    )]
    pub order: AccountLoader<'info, Order>,

    /// CHECK: no checks whatsoever, marginfi account authority decides this without restriction
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct KeeperCloseOrder<'info> {
    /// CHECK: This uses an unchecked account here so the instruction can be called even when the
    /// marginfi account was closed.
    /// The ownership check is checked in the handler or/and type checks are made in the handler.
    #[account(mut)]
    pub marginfi_account: UncheckedAccount<'info>,

    /// CHECK: no checks whatsoever, keeper decides this without restriction
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,

    #[account(
        mut,
        has_one = marginfi_account,
        close = fee_recipient
    )]
    pub order: AccountLoader<'info, Order>,
}

#[derive(Accounts)]
pub struct SetKeeperCloseFlags<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let a = marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), false, false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct StartExecuteOrder<'info> {
    #[account(
        constraint = (!group.load()?.is_protocol_paused()) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// The account owning the order
    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = !marginfi_account.load()?.get_flag(
            ORDER_BLOCKING_FLAGS | ACCOUNT_IN_ORDER_EXECUTION | ACCOUNT_IN_REBALANCE
        ) @ MarginfiError::UnexpectedOrderExecutionState
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// This account will have the authority to withdraw/repay as if they are the user authority
    /// until the end of the tx.
    ///
    /// CHECK: no checks whatsoever, executor decides this without restriction
    pub executor: UncheckedAccount<'info>,

    #[account(
        mut,
        has_one = marginfi_account
    )]
    pub order: AccountLoader<'info, Order>,

    /// This keeps track of the relevant state to be checked at the end of execution.
    #[account(
        init,
        payer = fee_payer,
        space = 8 + std::mem::size_of::<ExecuteOrderRecord>(),
        seeds = [
            EXECUTE_ORDER_SEED.as_bytes(),
            order.key().as_ref()
        ],
        bump
    )]
    pub execute_record: AccountLoader<'info, ExecuteOrderRecord>,

    /// CHECK: validated against known hard-coded sysvar key
    #[account(
        address = solana_instructions_sysvar::id()
    )]
    pub instruction_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl Hashable for StartExecuteOrder<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_start_execute_order")
    }
}

#[derive(Accounts)]
pub struct EndExecuteOrder<'info> {
    #[account(
        constraint = (!group.load()?.is_protocol_paused()) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// The account owning the order
    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let acc = marginfi_account.load()?;
            acc.get_flag(ACCOUNT_IN_ORDER_EXECUTION) && !acc.get_flag(ORDER_BLOCKING_FLAGS)
        } @ MarginfiError::UnexpectedOrderExecutionState
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// The executioner ☠️
    pub executor: Signer<'info>,

    /// CHECK: no checks whatsoever, executor decides this without restriction
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,

    #[account(
        mut,
        has_one = marginfi_account,
        close = fee_recipient
    )]
    pub order: AccountLoader<'info, Order>,

    /// This keeps track of the relevant state to be checked at the end of execution.
    #[account(
        mut,
        has_one = order,
        has_one = executor,
        close = fee_recipient
    )]
    pub execute_record: AccountLoader<'info, ExecuteOrderRecord>,

    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,
}

impl Hashable for EndExecuteOrder<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_end_execute_order")
    }
}
