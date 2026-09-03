//! Borrow orders: a keeper borrows on the user's behalf while the bank's realized rate sits under
//! the order's level, and repays from the destination bank once it rises over the close level.
//!
//! The rate is the growth of `liability_share_value` since the bank's youngest reading at least a
//! window old (`Bank::rate_readings`), which no single transaction can move. Each side is a
//! sandwich: `start` gates and grants the keeper leg authority for the transaction, the keeper runs
//! the ordinary borrow/deposit or withdraw/repay legs, and `end` proves what moved: an open took
//! what fits under the level to within a granule, a close repaid all the destination could cover,
//! every other balance is untouched, and the account is healthy over the finished position.
//!
//! Residual risk, accepted as auto-rebalance accepts its equivalent: a keeper who also lends can
//! move a bank's utilization around a fill (lowering the rate before an open, or draining the
//! destination's liquidity to shrink what a close must repay); the window gate and `amount` bound
//! the damage, and it ties up the keeper's capital. Accrual excludes circuit-breaker halts but the
//! span does not, so the realized rate reads low for up to one window after a halt. A token-2022
//! transfer fee is not priced: an open on such a mint passes only for the post-fee amount, and a
//! redeploying open pays the fee again on the deposit.

use super::rebalance::pay_from_fee_pool;
use crate::{
    check, check_eq,
    constants::PROGRAM_VERSION,
    events::{
        AccountEventHeader, BorrowOrderCancelledEvent, BorrowOrderClosedEvent,
        BorrowOrderFilledEvent, BorrowOrderPlacedEvent, BorrowOrderUpdatedEvent,
    },
    ix_utils::{
        validate_borrow_order_instructions, validate_not_cpi_by_stack_height,
        BorrowOrderLegBinding, BorrowOrderSide,
    },
    math_error,
    prelude::*,
    state::{
        bank::BankImpl,
        borrow_order::{
            available_liquidity, realized_borrow_apr, record_native_reading,
            remaining_borrow_capacity, BorrowDestination, BorrowOrderImpl, BorrowOrderRecordImpl,
        },
        marginfi_account::{
            check_account_init_health, check_account_maint_health, run_cb_price_gate,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        premium::{MarginfiAccountPremiumImpl, PremiumScratch},
        rate::{borrow_rate_at, debt_index_of},
    },
};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DEFAULT, ASSET_TAG_SOL, BORROW_ORDER_FILL_DUST_ATOMS, BORROW_ORDER_RECORD_SEED,
        BORROW_ORDER_SEED, FEE_STATE_SEED, IS_T22, REBALANCE_FEE_POOL_SEED,
    },
    types::{
        Bank, BorrowOrder, BorrowOrderRecord, FeeState, HealthCache, MarginfiAccount,
        MarginfiGroup, RequirementType, ACCOUNT_IN_BORROW_ORDER, ACCOUNT_IN_BORROW_ORDER_INTERNAL,
        ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_REBALANCE, ORDER_BLOCKING_FLAGS,
    },
};
use std::cell::Ref;

fn event_header(
    signer: Pubkey,
    marginfi_account: Pubkey,
    account: &MarginfiAccount,
) -> AccountEventHeader {
    AccountEventHeader {
        signer: Some(signer),
        marginfi_account,
        marginfi_account_authority: account.authority,
        marginfi_group: account.group,
    }
}

/// Native banks only: the sandwich admits only the native legs.
fn is_supported_bank(bank: &Bank) -> bool {
    matches!(bank.config.asset_tag, ASSET_TAG_DEFAULT | ASSET_TAG_SOL)
}

fn liability_shares_in(account: &MarginfiAccount, bank: &Pubkey) -> I80F48 {
    account
        .lending_account
        .get_balance(bank)
        .map(|b| I80F48::from(b.liability_shares))
        .unwrap_or(I80F48::ZERO)
}

fn asset_shares_in(account: &MarginfiAccount, bank: &Pubkey) -> I80F48 {
    account
        .lending_account
        .get_balance(bank)
        .map(|b| I80F48::from(b.asset_shares))
        .unwrap_or(I80F48::ZERO)
}

pub fn place_borrow_order(
    ctx: Context<PlaceBorrowOrder>,
    amount: u64,
    open_below_apr: u32,
    close_above_apr: Option<u32>,
    cooldown_seconds: Option<u32>,
    window_seconds: Option<u32>,
    keeper_tip: Option<u64>,
) -> MarginfiResult {
    let now = Clock::get()?.unix_timestamp;
    let mut account = ctx.accounts.marginfi_account.load_mut()?;
    let bank_key = ctx.accounts.bank.key();
    // The window the order measures over starts counting from this reading at the latest.
    accrue(&ctx.accounts.bank, &*ctx.accounts.group.load()?, now)?;
    let bank_mint = {
        let bank = ctx.accounts.bank.load()?;
        check!(
            is_supported_bank(&bank),
            MarginfiError::BorrowOrderUnsupportedBank
        );
        // The borrow leg refuses a bank the account already lends in.
        check!(
            asset_shares_in(&account, &bank_key) == I80F48::ZERO,
            MarginfiError::BorrowOrderInvalidConfig
        );
        bank.mint
    };

    let destination = match ctx.accounts.destination_bank.as_ref() {
        Some(destination_loader) => {
            let destination = destination_loader.load()?;
            check!(
                is_supported_bank(&destination),
                MarginfiError::BorrowOrderUnsupportedBank
            );
            // Same asset, and the deposit leg refuses a bank the account already borrows from.
            check_eq!(
                destination.mint,
                bank_mint,
                MarginfiError::BorrowOrderInvalidConfig
            );
            check!(
                liability_shares_in(&account, &destination_loader.key()) == I80F48::ZERO,
                MarginfiError::BorrowOrderInvalidConfig
            );
            BorrowDestination::Bank(destination_loader.key())
        }
        None => BorrowDestination::Wallet,
    };

    // The discriminator lands when the instruction exits, so the event reads the initialized view.
    let (
        order_amount,
        order_open,
        order_close,
        order_window,
        order_cooldown,
        order_tip,
        order_destination,
    ) = {
        let mut order = ctx.accounts.borrow_order.load_init()?;
        order.initialize(
            ctx.accounts.marginfi_account.key(),
            ctx.accounts.authority.key(),
            bank_key,
            amount,
            open_below_apr,
            close_above_apr,
            cooldown_seconds,
            window_seconds,
            keeper_tip,
            destination,
            ctx.bumps.borrow_order,
        )?;
        (
            order.amount,
            order.open_below_apr,
            order.close_above_apr,
            order.window_seconds,
            order.cooldown_seconds,
            order.keeper_tip,
            order.destination_bank,
        )
    };
    account.increment_active_orders()?;

    let order_init_flat_sol_fee = ctx.accounts.fee_state.load()?.order_init_flat_sol_fee;
    if order_init_flat_sol_fee > 0 {
        anchor_lang::system_program::transfer(
            ctx.accounts.transfer_flat_fee(),
            u64::from(order_init_flat_sol_fee),
        )?;
    }

    emit!(BorrowOrderPlacedEvent {
        header: event_header(
            ctx.accounts.authority.key(),
            ctx.accounts.marginfi_account.key(),
            &account,
        ),
        order: ctx.accounts.borrow_order.key(),
        bank: bank_key,
        amount: order_amount,
        open_below_apr: order_open,
        close_above_apr: order_close,
        window_seconds: order_window,
        cooldown_seconds: order_cooldown,
        keeper_tip: order_tip,
        destination_bank: order_destination,
    });
    Ok(())
}

/// Modify a live order in place. `None` fields are left as they are.
pub fn update_borrow_order(
    ctx: Context<UpdateBorrowOrder>,
    amount: Option<u64>,
    open_below_apr: Option<u32>,
    close_above_apr: Option<u32>,
    cooldown_seconds: Option<u32>,
    window_seconds: Option<u32>,
    keeper_tip: Option<u64>,
) -> MarginfiResult {
    let account = ctx.accounts.marginfi_account.load()?;
    check!(
        !account.get_flag(ACCOUNT_IN_BORROW_ORDER),
        MarginfiError::UnexpectedOrderExecutionState
    );
    let mut order = ctx.accounts.borrow_order.load_mut()?;

    if let Some(amount) = amount {
        // Shrinking below what is already borrowed would leave the order owing more than it holds.
        check!(
            amount >= order.filled,
            MarginfiError::BorrowOrderExceedsRemaining
        );
        order.amount = amount;
    }
    if let Some(apr) = open_below_apr {
        order.open_below_apr = apr;
    }
    if let Some(apr) = close_above_apr {
        order.close_above_apr = apr;
    }
    if let Some(cooldown) = cooldown_seconds {
        order.cooldown_seconds = cooldown;
    }
    if let Some(window) = window_seconds {
        order.window_seconds = window;
    }
    if let Some(tip) = keeper_tip {
        order.keeper_tip = tip;
    }
    order.validate()?;

    emit!(BorrowOrderUpdatedEvent {
        header: event_header(
            ctx.accounts.authority.key(),
            ctx.accounts.marginfi_account.key(),
            &account,
        ),
        order: ctx.accounts.borrow_order.key(),
        amount: order.amount,
        open_below_apr: order.open_below_apr,
        close_above_apr: order.close_above_apr,
        window_seconds: order.window_seconds,
        cooldown_seconds: order.cooldown_seconds,
        keeper_tip: order.keeper_tip,
    });
    Ok(())
}

pub fn cancel_borrow_order(ctx: Context<CancelBorrowOrder>) -> MarginfiResult {
    let mut account = ctx.accounts.marginfi_account.load_mut()?;
    account.decrement_active_orders()?;

    let order = ctx.accounts.borrow_order.load()?;
    emit!(BorrowOrderCancelledEvent {
        header: event_header(
            ctx.accounts.authority.key(),
            ctx.accounts.marginfi_account.key(),
            &account,
        ),
        order: ctx.accounts.borrow_order.key(),
        filled: order.filled,
    });
    Ok(())
}

/// Gate an open and hand the keeper borrow authority for the rest of the transaction.
pub fn start_borrow_order_open<'info>(
    ctx: Context<'info, StartBorrowOrderFill<'info>>,
) -> MarginfiResult {
    start_fill(ctx, BorrowOrderSide::Open)
}

/// Gate a close and hand the keeper withdraw and repay authority for the rest of the transaction.
pub fn start_borrow_order_close<'info>(
    ctx: Context<'info, StartBorrowOrderFill<'info>>,
) -> MarginfiResult {
    start_fill(ctx, BorrowOrderSide::Close)
}

/// The side's gates, then the record, the in-fill flags, and the pin on the transaction's shape.
fn start_fill<'info>(
    ctx: Context<'info, StartBorrowOrderFill<'info>>,
    side: BorrowOrderSide,
) -> MarginfiResult {
    let now = Clock::get()?.unix_timestamp;
    let accounts = &ctx.accounts;
    let group = accounts.group.load()?;
    accrue(&accounts.bank, &group, now)?;

    let binding = {
        let order = accounts.borrow_order.load()?;
        let bank = accounts.bank.load()?;
        let account = accounts.marginfi_account.load()?;
        check!(
            order.cooldown_elapsed(now),
            MarginfiError::BorrowOrderCooldown
        );
        let pre_liability_shares = liability_shares_in(&account, &order.bank);
        // The smoothed condition: what the bank has actually charged over the order's window.
        let realized_apr = realized_borrow_apr(
            &bank,
            order.window_seconds,
            debt_index_of(&bank, I80F48::ONE)?,
            now,
        )?;

        let (destination_bank, wallet_destination) = match side {
            BorrowOrderSide::Open => {
                check!(
                    order.remaining() > 0,
                    MarginfiError::BorrowOrderExceedsRemaining
                );
                check!(
                    order.opens_at(realized_apr),
                    MarginfiError::BorrowOrderRateNotLowEnough
                );
                // Nothing else observes where a borrow's tokens go, so the leg is pinned to the ATA.
                let wallet = order.to_wallet().then(|| {
                    let token_program = if bank.flags & IS_T22 != 0 {
                        anchor_spl::token_2022::ID
                    } else {
                        anchor_spl::token::ID
                    };
                    get_associated_token_address_with_program_id(
                        &account.authority,
                        &bank.mint,
                        &token_program,
                    )
                });
                (order.to_bank().then_some(order.destination_bank), wallet)
            }
            BorrowOrderSide::Close => {
                check!(
                    order.has_close_side(),
                    MarginfiError::BorrowOrderNoCloseSide
                );
                check!(
                    order.closable_shares(pre_liability_shares) > I80F48::ZERO,
                    MarginfiError::BorrowOrderNothingToClose
                );
                // Both the window's average and the rate right now must sit over the level.
                check!(
                    order.closes_at(realized_apr)
                        && order.closes_at(borrow_rate_at(&bank, &group, 0)?),
                    MarginfiError::BorrowOrderRateNotHighEnough
                );
                (Some(order.destination_bank), None)
            }
        };

        let mut record = accounts.borrow_order_record.load_init()?;
        record.order = accounts.borrow_order.key();
        record.executor = accounts.executor.key();
        record.kind = match side {
            BorrowOrderSide::Open => BorrowOrderRecord::RECORD_OPEN,
            BorrowOrderSide::Close => BorrowOrderRecord::RECORD_CLOSE,
        };
        record.pre_liability_shares = pre_liability_shares.into();
        record.pre_destination_shares = asset_shares_in(&account, &order.destination_bank).into();
        record.realized_apr = realized_apr.into();
        record.pre_collected_premium = bank.collected_premium_outstanding;
        record.snapshot_others(&account, &order.banks())?;

        BorrowOrderLegBinding {
            side,
            marginfi_account: accounts.marginfi_account.key(),
            bank: order.bank,
            destination_bank,
            wallet_destination,
        }
    };

    // A fill between two marginfi banks moves nothing out of the protocol.
    let mut flags = ACCOUNT_IN_BORROW_ORDER;
    if binding.destination_bank.is_some() {
        flags |= ACCOUNT_IN_BORROW_ORDER_INTERNAL;
    }
    accounts.marginfi_account.load_mut()?.set_flag(flags, false);
    validate_borrow_order_instructions(&accounts.instruction_sysvar, &binding)
}

/// Accrue to `now` and take a reading, so every index and utilization read after is current.
fn accrue(bank_loader: &AccountLoader<Bank>, group: &MarginfiGroup, now: i64) -> MarginfiResult {
    let mut bank = bank_loader.load_mut()?;
    bank.accrue_interest(
        now,
        group,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;
    bank.update_bank_cache(group)?;
    record_native_reading(&mut bank, now)
}

/// The record an `end` closes out, once the instruction is confirmed top-level and of `kind`.
fn fill_record<'a, 'info>(
    ctx: &'a Context<'info, EndBorrowOrderFill<'info>>,
    kind: u8,
) -> MarginfiResult<Ref<'a, BorrowOrderRecord>> {
    validate_not_cpi_by_stack_height()?;
    let record = ctx.accounts.borrow_order_record.load()?;
    check_eq!(
        record.kind,
        kind,
        MarginfiError::BorrowOrderMalformedSandwich
    );
    Ok(record)
}

/// The destination bank an `end` was given, which must be the order's own.
fn destination_bank<'a, 'info>(
    accounts: &'a EndBorrowOrderFill<'info>,
    order: &BorrowOrder,
) -> MarginfiResult<&'a AccountLoader<'info, Bank>> {
    let destination = accounts
        .destination_bank
        .as_ref()
        .ok_or_else(|| error!(MarginfiError::BorrowOrderLegBankMismatch))?;
    check_eq!(
        destination.key(),
        order.destination_bank,
        MarginfiError::BorrowOrderLegBankMismatch
    );
    Ok(destination)
}

/// A share-derived delta must match its target within the share round-trip's rounding.
fn check_moved(actual: I80F48, expected: I80F48) -> MarginfiResult {
    let dust = I80F48::from_num(BORROW_ORDER_FILL_DUST_ATOMS);
    check!(
        actual
            .checked_sub(expected)
            .ok_or_else(math_error!())?
            .abs()
            <= dust,
        MarginfiError::BorrowOrderFillMismatch
    );
    Ok(())
}

/// Prove the open took what fits under the level, redeployed it when the order says so, and left
/// the account at initial health.
pub fn end_borrow_order_open<'info>(
    ctx: Context<'info, EndBorrowOrderFill<'info>>,
) -> MarginfiResult {
    let record = fill_record(&ctx, BorrowOrderRecord::RECORD_OPEN)?;
    let clock = Clock::get()?;

    let (delivered, filled, remaining, spot_apr_after) = {
        let account = ctx.accounts.marginfi_account.load()?;
        let mut order = ctx.accounts.borrow_order.load_mut()?;
        let bank = ctx.accounts.bank.load()?;
        let group = ctx.accounts.group.load()?;

        // The debt delta carries the origination fee; price it back to what the leg delivered.
        let borrowed_shares = liability_shares_in(&account, &order.bank)
            .checked_sub(record.pre_liability_shares.into())
            .ok_or_else(math_error!())?;
        let fee_rate: I80F48 = bank
            .config
            .interest_rate_config
            .protocol_origination_fee
            .into();
        let delivered = bank
            .get_liability_amount(borrowed_shares)?
            .checked_div(
                I80F48::ONE
                    .checked_add(fee_rate)
                    .ok_or_else(math_error!())?,
            )
            .ok_or_else(math_error!())?;
        let delivered_atoms: u64 = delivered.round().to_num();
        let granule = order.granule();
        check!(
            delivered_atoms >= granule.min(order.remaining()),
            MarginfiError::BorrowOrderFillBelowGranule
        );

        // The rate left behind sits under the level, and one more granule would not have fit.
        let spot_apr_after = borrow_rate_at(&bank, &group, 0)?;
        check!(
            order.opens_at(spot_apr_after),
            MarginfiError::BorrowOrderFillOvershoots
        );
        let one_more = I80F48::from_num(granule);
        let mut maximal = delivered_atoms.saturating_add(granule) >= order.remaining()
            || one_more > available_liquidity(&bank)?
            || one_more > remaining_borrow_capacity(&bank)?
            || !order.opens_at(borrow_rate_at(&bank, &group, granule)?);

        if order.to_bank() {
            let destination = destination_bank(ctx.accounts, &order)?.load()?;
            let deposited_shares = asset_shares_in(&account, &order.destination_bank)
                .checked_sub(record.pre_destination_shares.into())
                .ok_or_else(math_error!())?;
            check_moved(destination.get_asset_amount(deposited_shares)?, delivered)?;
            maximal |= granule > destination.get_remaining_deposit_capacity()?;
        }
        check!(maximal, MarginfiError::BorrowOrderFillNotMaximal);

        record.verify_others_unchanged(&account, &order.banks())?;
        order.record_fill(delivered_atoms, borrowed_shares, clock.unix_timestamp)?;
        (
            delivered_atoms,
            order.filled,
            order.remaining(),
            spot_apr_after,
        )
    };

    end_sandwich(
        &ctx,
        RequirementType::Initial,
        delivered,
        clock.unix_timestamp,
    )?;

    let account = ctx.accounts.marginfi_account.load()?;
    emit!(BorrowOrderFilledEvent {
        header: event_header(
            ctx.accounts.executor.key(),
            ctx.accounts.marginfi_account.key(),
            &account,
        ),
        order: ctx.accounts.borrow_order.key(),
        bank: ctx.accounts.bank.key(),
        executor: ctx.accounts.executor.key(),
        amount: delivered,
        filled,
        remaining,
        realized_apr: record.realized_apr,
        spot_apr_after: spot_apr_after.into(),
    });
    Ok(())
}

/// Prove the close repaid all the destination could cover and no more than the order owes, took no
/// more from the destination than it paid, and left the account at maintenance health.
pub fn end_borrow_order_close<'info>(
    ctx: Context<'info, EndBorrowOrderFill<'info>>,
) -> MarginfiResult {
    let record = fill_record(&ctx, BorrowOrderRecord::RECORD_CLOSE)?;
    let clock = Clock::get()?;

    let (paid, filled) = {
        let account = ctx.accounts.marginfi_account.load()?;
        let mut order = ctx.accounts.borrow_order.load_mut()?;
        let bank = ctx.accounts.bank.load()?;
        let dust = I80F48::from_num(BORROW_ORDER_FILL_DUST_ATOMS);

        let repaid_shares = I80F48::from(record.pre_liability_shares)
            .checked_sub(liability_shares_in(&account, &order.bank))
            .ok_or_else(math_error!())?;
        let repaid = bank.get_liability_amount(repaid_shares)?;
        // The repay leg settles premium ahead of principal; it counts as paid, not as debt retired.
        let premium_settled = I80F48::from(bank.collected_premium_outstanding)
            .checked_sub(record.pre_collected_premium.into())
            .ok_or_else(math_error!())?;
        let paid = repaid
            .checked_add(premium_settled)
            .ok_or_else(math_error!())?;
        check!(paid > I80F48::ZERO, MarginfiError::BorrowOrderFillMismatch);
        let closable =
            bank.get_liability_amount(order.closable_shares(record.pre_liability_shares.into()))?;
        check!(
            repaid <= closable + dust,
            MarginfiError::BorrowOrderExceedsRemaining
        );

        let destination = destination_bank(ctx.accounts, &order)?.load()?;
        let withdrawn_shares = I80F48::from(record.pre_destination_shares)
            .checked_sub(asset_shares_in(&account, &order.destination_bank))
            .ok_or_else(math_error!())?;
        let withdrawn = destination.get_asset_amount(withdrawn_shares)?;
        // The keeper may top a payment up from their own funds, never take more than they paid.
        check!(
            withdrawn <= paid + dust,
            MarginfiError::BorrowOrderFillMismatch
        );
        // What the destination held and could pay out before the withdraw, up to the debt.
        let reachable = closable
            .min(destination.get_asset_amount(record.pre_destination_shares.into())?)
            .min(
                available_liquidity(&destination)?
                    .checked_add(withdrawn)
                    .ok_or_else(math_error!())?,
            );
        check!(
            reachable > I80F48::ZERO,
            MarginfiError::BorrowOrderNothingToClose
        );
        check!(
            paid + dust >= I80F48::from_num(order.close_floor(reachable)),
            MarginfiError::BorrowOrderCloseIncomplete
        );

        record.verify_others_unchanged(&account, &order.banks())?;
        order.record_repay(
            repaid_shares,
            record.pre_liability_shares.into(),
            clock.unix_timestamp,
        )?;
        (paid.round().to_num::<u64>(), order.filled)
    };

    end_sandwich(
        &ctx,
        RequirementType::Maintenance,
        paid,
        clock.unix_timestamp,
    )?;

    let account = ctx.accounts.marginfi_account.load()?;
    emit!(BorrowOrderClosedEvent {
        header: event_header(
            ctx.accounts.executor.key(),
            ctx.accounts.marginfi_account.key(),
            &account,
        ),
        order: ctx.accounts.borrow_order.key(),
        bank: ctx.accounts.bank.key(),
        executor: ctx.accounts.executor.key(),
        amount: paid,
        filled,
        realized_apr: record.realized_apr,
    });
    Ok(())
}

/// The passes the legs skipped, once over the post-fill balances, then the keeper's tip scaled by
/// the share of `amount` `moved`.
fn end_sandwich<'info>(
    ctx: &Context<'info, EndBorrowOrderFill<'info>>,
    requirement: RequirementType,
    moved: u64,
    now: i64,
) -> MarginfiResult {
    {
        let mut account = ctx.accounts.marginfi_account.load_mut()?;
        let group = ctx.accounts.group.load()?;
        let health_cache = check_fill_health_and_refresh_premium(
            &mut account,
            &group,
            ctx.remaining_accounts,
            requirement,
            now,
        )?;
        drop(group);
        if account.lending_account.has_liabilities() {
            run_cb_price_gate(&account, ctx.remaining_accounts)?;
        }
        account.health_cache = health_cache;
        account.unset_flag(
            ACCOUNT_IN_BORROW_ORDER | ACCOUNT_IN_BORROW_ORDER_INTERNAL,
            false,
        );
        account.last_update = now as u64;
    }

    let spendable = ctx
        .accounts
        .fee_pool
        .lamports()
        .saturating_sub(Rent::get()?.minimum_balance(0));
    let order = ctx.accounts.borrow_order.load()?;
    let earned = u128::from(order.keeper_tip) * u128::from(moved) / u128::from(order.amount);
    let tip = u64::try_from(earned)
        .unwrap_or(u64::MAX)
        .min(order.keeper_tip)
        .min(spendable);
    pay_from_fee_pool(
        &ctx.accounts.fee_pool,
        &ctx.accounts.executor.to_account_info(),
        &ctx.accounts.system_program,
        &ctx.accounts.marginfi_account.key(),
        ctx.bumps.fee_pool,
        tip,
    )
}

/// Health at `requirement` plus the premium refresh; out of line for the end instruction's stack.
#[inline(never)]
fn check_fill_health_and_refresh_premium<'info>(
    account: &mut MarginfiAccount,
    group: &MarginfiGroup,
    health_obs: &'info [AccountInfo<'info>],
    requirement: RequirementType,
    now: i64,
) -> MarginfiResult<HealthCache> {
    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = now;
    let mut premium_scratch = PremiumScratch::default();
    match requirement {
        RequirementType::Initial => {
            check_account_init_health(
                account,
                group,
                health_obs,
                &mut Some(&mut health_cache),
                &mut Some(&mut premium_scratch),
            )?;
            check!(
                !premium_scratch.refresh_unavailable(),
                MarginfiError::PremiumSnapshotUnavailable
            );
        }
        _ => check_account_maint_health(
            account,
            group,
            health_obs,
            &mut Some(&mut health_cache),
            &mut Some(&mut premium_scratch),
        )?,
    }
    account.update_premium_snapshots(group, &premium_scratch, now as u64)?;
    health_cache.program_version = PROGRAM_VERSION;
    health_cache.set_engine_ok(true);
    Ok(health_cache)
}

#[derive(Accounts)]
pub struct PlaceBorrowOrder<'info> {
    #[account(constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = authority @ MarginfiError::Unauthorized,
        constraint = !marginfi_account.load()?.get_flag(
            ORDER_BLOCKING_FLAGS | ACCOUNT_IN_ORDER_EXECUTION | ACCOUNT_IN_REBALANCE
                | ACCOUNT_IN_BORROW_ORDER
        ) @ MarginfiError::UnexpectedOrderExecutionState,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(mut, has_one = group @ MarginfiError::InvalidGroup)]
    pub bank: AccountLoader<'info, Bank>,

    /// The same-mint bank borrowed funds are redeployed into. Omitted when they go to the wallet.
    #[account(has_one = group @ MarginfiError::InvalidGroup)]
    pub destination_bank: Option<AccountLoader<'info, Bank>>,

    #[account(
        init,
        payer = fee_payer,
        space = 8 + BorrowOrder::LEN,
        seeds = [
            BORROW_ORDER_SEED.as_bytes(),
            marginfi_account.key().as_ref(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub borrow_order: AccountLoader<'info, BorrowOrder>,

    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_wallet @ MarginfiError::InvalidFeeWallet
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// CHECK: validated against the fee state.
    #[account(mut)]
    pub global_fee_wallet: UncheckedAccount<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> PlaceBorrowOrder<'info> {
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
pub struct UpdateBorrowOrder<'info> {
    #[account(has_one = authority @ MarginfiError::Unauthorized)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = marginfi_account @ MarginfiError::Unauthorized,
        has_one = authority @ MarginfiError::Unauthorized
    )]
    pub borrow_order: AccountLoader<'info, BorrowOrder>,
}

#[derive(Accounts)]
pub struct CancelBorrowOrder<'info> {
    #[account(mut, has_one = authority @ MarginfiError::Unauthorized)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = marginfi_account @ MarginfiError::Unauthorized,
        has_one = authority @ MarginfiError::Unauthorized,
        close = fee_recipient
    )]
    pub borrow_order: AccountLoader<'info, BorrowOrder>,

    /// CHECK: no checks whatsoever, the authority decides this without restriction
    #[account(mut)]
    pub fee_recipient: UncheckedAccount<'info>,
}

/// Shared by both `start` instructions.
#[derive(Accounts)]
pub struct StartBorrowOrderFill<'info> {
    #[account(constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = !marginfi_account.load()?.get_flag(
            ORDER_BLOCKING_FLAGS | ACCOUNT_IN_ORDER_EXECUTION | ACCOUNT_IN_REBALANCE
                | ACCOUNT_IN_BORROW_ORDER
        ) @ MarginfiError::UnexpectedOrderExecutionState,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        has_one = marginfi_account @ MarginfiError::Unauthorized,
        has_one = bank @ MarginfiError::InvalidBankAccount
    )]
    pub borrow_order: AccountLoader<'info, BorrowOrder>,

    #[account(mut, has_one = group @ MarginfiError::InvalidGroup)]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: the keeper; gains temporary leg authority for this transaction only.
    pub executor: UncheckedAccount<'info>,

    #[account(
        init,
        payer = fee_payer,
        space = 8 + BorrowOrderRecord::LEN,
        seeds = [BORROW_ORDER_RECORD_SEED.as_bytes(), borrow_order.key().as_ref()],
        bump,
    )]
    pub borrow_order_record: AccountLoader<'info, BorrowOrderRecord>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// CHECK: validated by address.
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Shared by both `end` instructions.
#[derive(Accounts)]
pub struct EndBorrowOrderFill<'info> {
    #[account(constraint = !group.load()?.is_protocol_paused() @ MarginfiError::ProtocolPaused)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let acc = marginfi_account.load()?;
            acc.get_flag(ACCOUNT_IN_BORROW_ORDER) && !acc.get_flag(ORDER_BLOCKING_FLAGS)
        } @ MarginfiError::UnexpectedOrderExecutionState,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        mut,
        has_one = marginfi_account @ MarginfiError::Unauthorized,
        has_one = bank @ MarginfiError::InvalidBankAccount
    )]
    pub borrow_order: AccountLoader<'info, BorrowOrder>,

    #[account(has_one = group @ MarginfiError::InvalidGroup)]
    pub bank: AccountLoader<'info, Bank>,

    /// The order's destination bank. Omitted only for a wallet order's open.
    #[account(has_one = group @ MarginfiError::InvalidGroup)]
    pub destination_bank: Option<AccountLoader<'info, Bank>>,

    #[account(mut)]
    pub executor: Signer<'info>,

    #[account(
        mut,
        constraint = borrow_order_record.load()?.order == borrow_order.key()
            @ MarginfiError::Unauthorized,
        has_one = executor @ MarginfiError::Unauthorized,
        close = executor
    )]
    pub borrow_order_record: AccountLoader<'info, BorrowOrderRecord>,

    /// The account's keeper-tip pool, shared with auto-rebalance.
    #[account(
        mut,
        seeds = [REBALANCE_FEE_POOL_SEED.as_bytes(), marginfi_account.key().as_ref()],
        bump,
    )]
    pub fee_pool: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
    // The post-fill health observation set follows in remaining_accounts.
}
