use crate::{
    check,
    constants::PROGRAM_VERSION,
    events::{AccountEventHeader, LendingAccountDepositEvent, LendingAccountWithdrawEvent},
    state::{
        bank::BankImpl,
        marginfi_account::{
            calc_value, check_account_init_health, LendingAccountImpl, MarginfiAccountImpl,
        },
        rate_limiter::{
            should_skip_rate_limit, BankRateLimiterImpl, BankRateLimiterUntrackedImpl,
            GroupRateLimiterImpl,
        },
    },
    utils::{fetch_rate_limit_price_for_inflow, fetch_unbiased_price_for_bank},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    Bank, HealthCache, MarginfiAccount, MarginfiGroup, ACCOUNT_IN_ORDER_EXECUTION,
    ACCOUNT_IN_RECEIVERSHIP,
};

/// Records withdrawal outflow on bank-level and group-level rate limiters.
pub fn record_withdrawal_outflow(
    group_rate_limit_enabled: bool,
    token_amount: u64,
    price: I80F48,
    bank: &mut Bank,
    group: &mut MarginfiGroup,
    marginfi_account: &MarginfiAccount,
    clock: &Clock,
) -> MarginfiResult<()> {
    // Rate limiting tracks net outflow; skip for flashloan/liquidation/deleverage flows.
    if !should_skip_rate_limit(marginfi_account.account_flags) {
        // Bank-level rate limiting (native tokens)
        if bank.rate_limiter.is_enabled() {
            bank.rate_limiter
                .try_record_outflow(token_amount, clock.unix_timestamp)?;
        }

        // Group-level rate limiting (USD) - use fresh oracle price
        if group_rate_limit_enabled {
            check!(price > I80F48::ZERO, MarginfiError::InvalidRateLimitPrice);

            // Apply any pending untracked inflows before recording the outflow
            if bank.rate_limiter.untracked_inflow != 0 {
                let mint_decimals = bank.mint_decimals;
                bank.rate_limiter.apply_untracked_inflow(
                    &mut group.rate_limiter,
                    price,
                    mint_decimals,
                    clock.unix_timestamp,
                )?;
            }

            let usd_value = calc_value(
                I80F48::from_num(token_amount),
                price,
                bank.mint_decimals,
                None,
            )?;
            group
                .rate_limiter
                .try_record_outflow(usd_value.to_num::<u64>(), clock.unix_timestamp)?;
        }
    }
    Ok(())
}

/// Post-withdrawal finalization: emits the withdraw event, runs the health check,
/// and updates the bank price cache.
pub fn finalize_withdrawal<'info>(
    marginfi_account: &mut MarginfiAccount,
    bank_loader: &AccountLoader<'info, Bank>,
    bank_key: Pubkey,
    bank_mint: Pubkey,
    signer_key: Pubkey,
    marginfi_account_key: Pubkey,
    actual_amount_received: u64,
    withdraw_all: bool,
    clock: &Clock,
    remaining_accounts: &'info [AccountInfo<'info>],
) -> MarginfiResult {
    marginfi_account.last_update = clock.unix_timestamp as u64;

    emit!(LendingAccountWithdrawEvent {
        header: AccountEventHeader {
            signer: Some(signer_key),
            marginfi_account: marginfi_account_key,
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank: bank_key,
        mint: bank_mint,
        amount: actual_amount_received,
        close_balance: withdraw_all,
    });

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;

    marginfi_account.lending_account.sort_balances();

    // Note: during liquidation/deleverage or order execution, we skip all health checks until
    // the end of the transaction.
    if !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_ORDER_EXECUTION) {
        check_account_init_health(
            marginfi_account,
            remaining_accounts,
            &mut Some(&mut health_cache),
        )?;

        health_cache.program_version = PROGRAM_VERSION;
        health_cache.set_engine_ok(true);
        marginfi_account.health_cache = health_cache;
    }

    let bank = bank_loader.load()?;
    let price_for_cache =
        fetch_unbiased_price_for_bank(&bank_key, &bank, clock, remaining_accounts).ok();
    drop(bank);

    bank_loader.load_mut()?.update_cache_price(price_for_cache)?;

    Ok(())
}

/// Records deposit inflow on bank-level and group-level rate limiters.
pub fn record_deposit_inflow(
    bank: &mut Bank,
    group: &mut MarginfiGroup,
    account_flags: u64,
    amount: u64,
    clock: &Clock,
) -> MarginfiResult<()> {
    // Rate limiting tracks net outflow; inflows release capacity.
    if !should_skip_rate_limit(account_flags) {
        if bank.rate_limiter.is_enabled() {
            bank.rate_limiter
                .record_inflow(amount, clock.unix_timestamp);
        }

        if group.rate_limiter.is_enabled() {
            let rate_limit_price = fetch_rate_limit_price_for_inflow(bank, clock)?;
            match rate_limit_price {
                Some(price) => {
                    let usd_value =
                        calc_value(I80F48::from_num(amount), price, bank.mint_decimals, None)?;
                    group
                        .rate_limiter
                        .record_inflow(usd_value.to_num::<u64>(), clock.unix_timestamp);
                }
                None => {
                    bank.rate_limiter.record_untracked_inflow(amount);
                }
            }
        }
    }
    Ok(())
}

/// Post-deposit finalization: updates bank cache, sets last_update, sorts balances,
/// and emits the deposit event.
pub fn finalize_deposit(
    bank: &mut Bank,
    group: &MarginfiGroup,
    marginfi_account: &mut MarginfiAccount,
    signer_key: Pubkey,
    marginfi_account_key: Pubkey,
    bank_key: Pubkey,
    amount: u64,
    clock: &Clock,
) -> MarginfiResult<()> {
    bank.update_bank_cache(group)?;

    marginfi_account.last_update = clock.unix_timestamp as u64;
    marginfi_account.lending_account.sort_balances();

    emit!(LendingAccountDepositEvent {
        header: AccountEventHeader {
            signer: Some(signer_key),
            marginfi_account: marginfi_account_key,
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank: bank_key,
        mint: bank.mint,
        amount,
    });

    Ok(())
}
