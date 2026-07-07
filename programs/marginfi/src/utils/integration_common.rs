use crate::constants::PROGRAM_VERSION;
use crate::events::{AccountEventHeader, LendingAccountDepositEvent, LendingAccountWithdrawEvent};
use crate::state::bank::BankImpl;
use crate::state::marginfi_account::{
    check_account_init_health, BankAccountWrapper, LendingAccountImpl, MarginfiAccountImpl,
};
use crate::utils::{
    fetch_unbiased_price_for_bank_cache, record_deposit_inflow, validate_asset_tags,
    validate_bank_state, InstructionKind,
};
use crate::{check, MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_lang::solana_program::system_program;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::constants::{
    ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_SOLEND,
};
use marginfi_type_crate::types::{
    Bank, HealthCache, MarginfiAccount, MarginfiGroup, ACCOUNT_IN_ORDER_EXECUTION,
    ACCOUNT_IN_RECEIVERSHIP,
};

// ************* VENUE POLICY
// Per-venue behavioral differences, keyed by the bank's integration asset tag. Kept together so
// a venue's quirks (and its eventual removal) touch a single place.

/// Whether the venue permits withdrawals while the account is mid-order-execution.
pub fn allows_order_execution(asset_tag: u8) -> bool {
    asset_tag != ASSET_TAG_SOLEND
}

/// Whether the venue forbids deposits while the account is in receivership.
pub fn blocks_deposit_in_receivership(asset_tag: u8) -> bool {
    asset_tag == ASSET_TAG_SOLEND
}

/// Returns the total size of the venue's protocol account layout for a deposit.
pub fn deposit_protocol_account_count(asset_tag: u8) -> usize {
    match asset_tag {
        ASSET_TAG_KAMINO => crate::instructions::integration::kamino_handler::DEPOSIT_ACCOUNTS,
        ASSET_TAG_DRIFT => crate::instructions::integration::drift_handler::DEPOSIT_ACCOUNTS,
        ASSET_TAG_SOLEND => crate::instructions::integration::solend_handler::DEPOSIT_ACCOUNTS,
        ASSET_TAG_JUPLEND => crate::instructions::integration::juplend_handler::DEPOSIT_ACCOUNTS,
        _ => 0,
    }
}

/// Returns the total size of the venue's protocol account layout for a withdraw. Together with
/// the woven integration slots, this determines the split between protocol and oracle accounts
/// in the unified instruction's remaining accounts.
pub fn withdraw_protocol_account_count(asset_tag: u8) -> usize {
    match asset_tag {
        ASSET_TAG_KAMINO => crate::instructions::integration::kamino_handler::WITHDRAW_ACCOUNTS,
        ASSET_TAG_DRIFT => crate::instructions::integration::drift_handler::WITHDRAW_ACCOUNTS,
        ASSET_TAG_SOLEND => crate::instructions::integration::solend_handler::WITHDRAW_ACCOUNTS,
        ASSET_TAG_JUPLEND => crate::instructions::integration::juplend_handler::WITHDRAW_ACCOUNTS,
        _ => 0,
    }
}

/// Returns the layout slots the unified deposit fills from the bank's integration accounts.
pub fn deposit_integration_slots(asset_tag: u8) -> &'static [(usize, u8)] {
    match asset_tag {
        ASSET_TAG_KAMINO => crate::instructions::integration::kamino_handler::INTEGRATION_SLOTS,
        ASSET_TAG_DRIFT => crate::instructions::integration::drift_handler::INTEGRATION_SLOTS,
        ASSET_TAG_SOLEND => crate::instructions::integration::solend_handler::INTEGRATION_SLOTS,
        ASSET_TAG_JUPLEND => {
            crate::instructions::integration::juplend_handler::DEPOSIT_INTEGRATION_SLOTS
        }
        _ => &[],
    }
}

/// Returns the layout slots the unified withdraw fills from the bank's integration accounts.
pub fn withdraw_integration_slots(asset_tag: u8) -> &'static [(usize, u8)] {
    match asset_tag {
        ASSET_TAG_KAMINO => crate::instructions::integration::kamino_handler::INTEGRATION_SLOTS,
        ASSET_TAG_DRIFT => crate::instructions::integration::drift_handler::INTEGRATION_SLOTS,
        ASSET_TAG_SOLEND => crate::instructions::integration::solend_handler::INTEGRATION_SLOTS,
        ASSET_TAG_JUPLEND => {
            crate::instructions::integration::juplend_handler::WITHDRAW_INTEGRATION_SLOTS
        }
        _ => &[],
    }
}

// ************* END VENUE POLICY

/// Returns `Some(account)` if present and not the system program sentinel, `None` otherwise.
/// Used by integration handlers to resolve optional protocol accounts.
pub fn optional_account<'info>(info: Option<&AccountInfo<'info>>) -> Option<AccountInfo<'info>> {
    info.filter(|ai| ai.key() != system_program::ID).cloned()
}

/// Checks that `accounts` has at least `min_count` entries and that each `(index, key)` pair in
/// `expected_keys` matches the account at that position.
pub fn expect_protocol_accounts(
    accounts: &[AccountInfo],
    min_count: usize,
    expected_keys: &[(usize, Pubkey)],
) -> MarginfiResult {
    check!(
        accounts.len() >= min_count,
        MarginfiError::IntegrationAccountCountMismatch
    );
    for (index, key) in expected_keys {
        check!(
            accounts[*index].key() == *key,
            MarginfiError::IntegrationAccountKeyMismatch
        );
    }
    Ok(())
}

/// Pre-deposit validation shared by all integration deposits.
/// Returns the `liquidity_vault_authority_bump` needed for CPI signing.
pub fn validate_integration_deposit(
    marginfi_account: &AccountLoader<MarginfiAccount>,
    bank: &AccountLoader<Bank>,
) -> MarginfiResult<u8> {
    let marginfi_account = marginfi_account.load()?;
    let bank = bank.load()?;
    let authority_bump = bank.liquidity_vault_authority_bump;

    validate_asset_tags(&bank, &marginfi_account)?;
    validate_bank_state(&bank, InstructionKind::FailsIfPausedOrReduceState)?;
    check!(
        !(blocks_deposit_in_receivership(bank.config.asset_tag)
            && marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP)),
        MarginfiError::AccountDisabled
    );

    Ok(authority_bump)
}

/// Post-deposit finalization shared by all integration deposits.
/// Records balance change, rate limiting, cache update, and emits event.
pub fn finalize_integration_deposit(
    group: &AccountLoader<MarginfiGroup>,
    marginfi_account: &AccountLoader<MarginfiAccount>,
    bank: &AccountLoader<Bank>,
    authority_key: Pubkey,
    balance_change: u64,
    inflow_amount: u64,
) -> MarginfiResult {
    let group_key = group.key();
    let marginfi_account_key = marginfi_account.key();
    let bank_key = bank.key();

    let mut bank = bank.load_mut()?;
    let mut marginfi_account = marginfi_account.load_mut()?;
    let group = group.load()?;
    let clock = Clock::get()?;

    let mut bank_account = BankAccountWrapper::find_or_create(
        &bank_key,
        &mut bank,
        &mut marginfi_account.lending_account,
    )?;

    let balance_change_i80f48 = I80F48::from_num(balance_change);
    let share_amount = bank_account.deposit_no_repay(balance_change_i80f48)?;

    record_deposit_inflow(
        &mut bank,
        &group,
        group_key,
        bank_key,
        marginfi_account.account_flags,
        inflow_amount,
        &clock,
    )?;

    bank.update_bank_cache(&group)?;

    marginfi_account.last_update = clock.unix_timestamp as u64;
    marginfi_account.lending_account.sort_balances();
    marginfi_account.sync_indexer_flags();

    emit!(LendingAccountDepositEvent {
        header: AccountEventHeader {
            signer: Some(authority_key),
            marginfi_account: marginfi_account_key,
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank: bank_key,
        mint: bank.mint,
        amount: inflow_amount,
        share_amount: share_amount.into(),
    });

    Ok(())
}

/// Post-withdraw finalization shared by all integration withdrawals.
/// Emits event, sorts balances, runs health check (unless receivership/order execution),
/// and updates bank price cache.
pub fn finalize_integration_withdraw<'info>(
    marginfi_account: &AccountLoader<'info, MarginfiAccount>,
    bank: &AccountLoader<'info, Bank>,
    authority_key: Pubkey,
    event_amount: u64,
    share_amount: I80F48,
    withdraw_all: bool,
    oracle_accounts: &'info [AccountInfo<'info>],
    clock: &Clock,
) -> MarginfiResult {
    let marginfi_account_key = marginfi_account.key();
    let bank_key = bank.key();
    let bank_mint = bank.load()?.mint;

    let mut marginfi_account = marginfi_account.load_mut()?;

    emit!(LendingAccountWithdrawEvent {
        header: AccountEventHeader {
            signer: Some(authority_key),
            marginfi_account: marginfi_account_key,
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank: bank_key,
        mint: bank_mint,
        amount: event_amount,
        share_amount: share_amount.into(),
        close_balance: withdraw_all,
    });

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;

    marginfi_account.lending_account.sort_balances();
    marginfi_account.sync_indexer_flags();

    let in_receivership_or_order_execution =
        marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_ORDER_EXECUTION);

    if !in_receivership_or_order_execution {
        check_account_init_health(
            &marginfi_account,
            oracle_accounts,
            &mut Some(&mut health_cache),
        )?;
        health_cache.program_version = PROGRAM_VERSION;
        health_cache.set_engine_ok(true);
        marginfi_account.health_cache = health_cache;
    }

    // Update price cache regardless of receivership status
    let mut bank = bank.load_mut()?;
    let price_for_cache =
        fetch_unbiased_price_for_bank_cache(&bank_key, &bank, clock, oracle_accounts).ok();
    bank.update_cache_price(price_for_cache)?;

    Ok(())
}
