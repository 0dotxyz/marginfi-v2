use crate::{
    bank_signer, check,
    constants::PROGRAM_VERSION,
    events::{AccountEventHeader, LendingAccountBorrowEvent},
    math_error,
    prelude::{MarginfiError, MarginfiResult},
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            calc_value, check_account_init_health, BankAccountWrapper, LendingAccountImpl,
            MarginfiAccountImpl,
        },
        rate_limiter::{
            should_skip_rate_limit, BankRateLimiterImpl, BankRateLimiterUntrackedImpl,
            GroupRateLimiterImpl,
        },
    },
    utils::{
        self, fetch_unbiased_price_for_bank, validate_asset_tags, validate_bank_state,
        InstructionKind,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    Bank, HealthCache, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_FLASHLOAN,
    ACCOUNT_IN_RECEIVERSHIP,
};

/// Shared implementation for borrow instructions.
///
/// `allow_group_rate_limit_mutation` must be `true` for normal borrows and `false` for
/// flashloan-only variants that pass `group` as readonly.
#[allow(clippy::too_many_arguments)]
pub fn lending_account_borrow_common<'info>(
    marginfi_account_loader: &AccountLoader<'info, MarginfiAccount>,
    marginfi_group_loader: &AccountLoader<'info, MarginfiGroup>,
    bank_loader: &AccountLoader<'info, Bank>,
    destination_token_account: &InterfaceAccount<'info, TokenAccount>,
    bank_liquidity_vault: &InterfaceAccount<'info, TokenAccount>,
    bank_liquidity_vault_authority: &AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
    authority: &Signer<'info>,
    remaining_accounts: &mut &'info [AccountInfo<'info>],
    amount: u64,
    allow_group_rate_limit_mutation: bool,
) -> MarginfiResult {
    let clock = Clock::get()?;
    let maybe_bank_mint =
        utils::maybe_take_bank_mint(remaining_accounts, &*bank_loader.load()?, token_program.key)?;

    let mut marginfi_account = marginfi_account_loader.load_mut()?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED)
        // Sanity check: liquidation doesn't allow the borrow ix, but just in case
            && !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
        MarginfiError::AccountDisabled
    );

    if allow_group_rate_limit_mutation {
        check!(
            !marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
            MarginfiError::IllegalFlashloan
        );
    } else {
        check!(
            marginfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
            MarginfiError::IllegalFlashloan
        );
    }

    let (program_fee_rate, group_rate_limit_enabled): (I80F48, bool) = {
        let group = marginfi_group_loader.load()?;
        let program_fee_rate: I80F48 = group.fee_state_cache.program_fee_rate.into();
        let group_rate_limit_enabled = group.rate_limiter.is_enabled();
        bank_loader.load_mut()?.accrue_interest(
            clock.unix_timestamp,
            &group,
            #[cfg(not(feature = "client"))]
            bank_loader.key(),
        )?;
        (program_fee_rate, group_rate_limit_enabled)
    };

    let mut origination_fee: I80F48 = I80F48::ZERO;
    {
        let mut bank = bank_loader.load_mut()?;

        validate_asset_tags(&bank, &marginfi_account)?;
        validate_bank_state(&bank, InstructionKind::FailsIfPausedOrReduceState)?;

        let liquidity_vault_authority_bump = bank.liquidity_vault_authority_bump;
        let origination_fee_rate: I80F48 = bank
            .config
            .interest_rate_config
            .protocol_origination_fee
            .into();

        let lending_account = &mut marginfi_account.lending_account;
        let mut bank_account =
            BankAccountWrapper::find_or_create(&bank_loader.key(), &mut bank, lending_account)?;

        // User needs to borrow amount + fee to receive amount
        let amount_pre_fee = maybe_bank_mint
            .as_ref()
            .map(|mint| {
                utils::calculate_pre_fee_spl_deposit_amount(
                    mint.to_account_info(),
                    amount,
                    clock.epoch,
                )
            })
            .transpose()?
            .unwrap_or(amount);

        let origination_fee_u64: u64;
        if !origination_fee_rate.is_zero() {
            origination_fee = I80F48::from_num(amount_pre_fee)
                .checked_mul(origination_fee_rate)
                .ok_or_else(math_error!())?;
            origination_fee_u64 = origination_fee.checked_to_num().ok_or_else(math_error!())?;

            // Incurs a borrow that includes the origination fee (but withdraws just the amt)
            bank_account.borrow(I80F48::from_num(amount_pre_fee) + origination_fee)?;
        } else {
            // Incurs a borrow for the amount without any fee
            origination_fee_u64 = 0;
            bank_account.borrow(I80F48::from_num(amount_pre_fee))?;
        }

        marginfi_account.last_update = clock.unix_timestamp as u64;

        bank.withdraw_spl_transfer(
            amount_pre_fee,
            bank_liquidity_vault.to_account_info(),
            destination_token_account.to_account_info(),
            bank_liquidity_vault_authority.to_account_info(),
            maybe_bank_mint.as_ref(),
            token_program.to_account_info(),
            bank_signer!(
                BankVaultType::Liquidity,
                bank_loader.key(),
                liquidity_vault_authority_bump
            ),
            remaining_accounts,
        )?;

        emit!(LendingAccountBorrowEvent {
            header: AccountEventHeader {
                signer: Some(authority.key()),
                marginfi_account: marginfi_account_loader.key(),
                marginfi_account_authority: marginfi_account.authority,
                marginfi_group: marginfi_account.group,
            },
            bank: bank_loader.key(),
            mint: bank.mint,
            amount: amount_pre_fee + origination_fee_u64,
        });
    } // release mutable borrow of bank

    // The program and/or group fee account gains the origination fee
    {
        let mut bank = bank_loader.load_mut()?;

        if !origination_fee.is_zero() {
            let mut bank_fees_after: I80F48 = bank.collected_group_fees_outstanding.into();

            if !program_fee_rate.is_zero() {
                // Some portion of the origination fee to goes to program fees
                let program_fee_amount: I80F48 = origination_fee
                    .checked_mul(program_fee_rate)
                    .ok_or_else(math_error!())?;
                // The remainder of the origination fee goes to group fees
                bank_fees_after = bank_fees_after
                    .saturating_add(origination_fee.saturating_sub(program_fee_amount));

                // Update the bank's program fees
                let program_fees_before: I80F48 = bank.collected_program_fees_outstanding.into();
                bank.collected_program_fees_outstanding = program_fees_before
                    .saturating_add(program_fee_amount)
                    .into();
            } else {
                // If program fee rate is zero, add the full origination fee to group fees
                bank_fees_after = bank_fees_after.saturating_add(origination_fee);
            }

            // Update the bank's group fees
            bank.collected_group_fees_outstanding = bank_fees_after.into();
        }
    }

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;
    marginfi_account.lending_account.sort_balances();

    // Check account health, if below threshold fail transaction
    // Assuming `remaining_accounts` holds only oracle accounts
    check_account_init_health(
        &marginfi_account,
        remaining_accounts,
        &mut Some(&mut health_cache),
    )?;
    health_cache.program_version = PROGRAM_VERSION;

    let bank_pk = bank_loader.key();
    let bank = bank_loader.load()?;
    let price = fetch_unbiased_price_for_bank(&bank_pk, &bank, &clock, remaining_accounts).ok();
    drop(bank);

    let mut bank = bank_loader.load_mut()?;

    // Rate limiting tracks net outflow; skip for flashloan/liquidation/deleverage flows.
    if !should_skip_rate_limit(marginfi_account.account_flags) {
        // Bank-level rate limiting (native tokens)
        if bank.rate_limiter.is_enabled() {
            bank.rate_limiter
                .try_record_outflow(amount, clock.unix_timestamp)?;
        }

        // Group-level rate limiting (USD) - reuse risk-engine price used for cache.
        if group_rate_limit_enabled {
            check!(
                allow_group_rate_limit_mutation,
                MarginfiError::IllegalFlashloan
            );

            let rate_limit_price = price.as_ref().map(|p| p.price).unwrap_or(I80F48::ZERO);
            check!(
                rate_limit_price > I80F48::ZERO,
                MarginfiError::InvalidRateLimitPrice
            );

            // Apply any pending untracked inflows before recording the outflow
            if bank.rate_limiter.untracked_inflow != 0 {
                let mint_decimals = bank.mint_decimals;
                let mut group = marginfi_group_loader.load_mut()?;
                bank.rate_limiter.apply_untracked_inflow(
                    &mut group.rate_limiter,
                    rate_limit_price,
                    mint_decimals,
                    clock.unix_timestamp,
                )?;
            }

            let usd_value = calc_value(
                I80F48::from_num(amount),
                rate_limit_price,
                bank.mint_decimals,
                None,
            )?;

            let mut group = marginfi_group_loader.load_mut()?;
            group
                .rate_limiter
                .try_record_outflow(usd_value.to_num::<u64>(), clock.unix_timestamp)?;
        }
    }

    let group = marginfi_group_loader.load()?;
    bank.update_bank_cache(&group)?;
    bank.update_cache_price(price)?;

    health_cache.set_engine_ok(true);
    marginfi_account.health_cache = health_cache;

    Ok(())
}
