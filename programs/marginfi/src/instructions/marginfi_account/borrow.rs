use crate::{
    bank_signer, check,
    constants::PROGRAM_VERSION,
    events::{AccountEventHeader, LendingAccountBorrowEvent},
    math_error,
    prelude::{MarginfiError, MarginfiResult},
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            account_not_frozen_for_authority, check_account_init_health, is_signer_authorized,
            BankAccountWrapper, LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        rate_limiter::GroupRateLimiterImpl,
    },
    utils::{
        self, fetch_unbiased_price_for_bank, is_marginfi_asset_tag, record_withdrawal_outflow,
        validate_asset_tags, validate_bank_state, InstructionKind,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{LIQUIDITY_VAULT_AUTHORITY_SEED, TOKENLESS_REPAYMENTS_ALLOWED},
    types::{
        Bank, HealthCache, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED,
        ACCOUNT_IN_RECEIVERSHIP,
    },
};

/// 1. Accrue interest
/// 2. Create the user's bank account for the asset borrowed if it does not exist yet
/// 3. Record liability increase in the bank account
/// 4. Transfer funds from the bank's liquidity vault to the signer's token account
/// 5. Verify that the user account is in a healthy state
///
/// Will error if there is an existing asset <=> withdrawing is not allowed.
pub fn lending_account_borrow<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingAccountBorrow<'info>>,
    amount: u64,
) -> MarginfiResult {
    let LendingAccountBorrow {
        marginfi_account: marginfi_account_loader,
        destination_token_account,
        liquidity_vault: bank_liquidity_vault,
        token_program,
        bank_liquidity_vault_authority,
        bank: bank_loader,
        group: marginfi_group_loader,
        ..
    } = ctx.accounts;
    let clock = Clock::get()?;
    let maybe_bank_mint = utils::maybe_take_bank_mint(
        &mut ctx.remaining_accounts,
        &*bank_loader.load()?,
        token_program.key,
    )?;
    utils::validate_bank_mint(maybe_bank_mint.as_ref())?;

    let mut marginfi_account = marginfi_account_loader.load_mut()?;
    let group = marginfi_group_loader.load()?;

    let program_fee_rate: I80F48 = group.fee_state_cache.program_fee_rate.into();

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED)
        // Sanity check: liquidation doesn't allow the borrow ix, but just in case
            && !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
        MarginfiError::AccountDisabled
    );

    bank_loader.load_mut()?.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let group_rate_limit_enabled = group.rate_limiter.is_enabled();

    let mut origination_fee: I80F48 = I80F48::ZERO;
    let amount_pre_fee;
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
        amount_pre_fee = amount;

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
            ctx.remaining_accounts,
        )?;

        // Fetch liability price to cache it (borrow creates a liability)
        emit!(LendingAccountBorrowEvent {
            header: AccountEventHeader {
                signer: Some(ctx.accounts.authority.key()),
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
    // Assuming `ctx.remaining_accounts` holds only oracle accounts
    check_account_init_health(
        &marginfi_account,
        ctx.remaining_accounts,
        &mut Some(&mut health_cache),
    )?;
    health_cache.program_version = PROGRAM_VERSION;

    let bank_pk = ctx.accounts.bank.key();
    let mut bank = ctx.accounts.bank.load_mut()?;
    let price = fetch_unbiased_price_for_bank(&bank_pk, &bank, &clock, ctx.remaining_accounts).ok();

    let rate_limit_price = price.as_ref().map(|p| p.price).unwrap_or(I80F48::ZERO);
    record_withdrawal_outflow(
        group_rate_limit_enabled,
        amount_pre_fee,
        amount_pre_fee,
        rate_limit_price,
        &mut bank,
        &group,
        marginfi_group_loader.key(),
        bank_pk,
        &marginfi_account,
        &clock,
    )?;

    bank.update_bank_cache(&group)?;
    bank.update_cache_price(price)?;

    health_cache.set_engine_ok(true);
    marginfi_account.health_cache = health_cache;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountBorrow<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
        ) @ MarginfiError::ProtocolPaused
    )]
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
            is_signer_authorized(&a, g.admin, authority.key(), false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
        // Prevents footgun where admin forgot to put a deleveraging bank into reduce-only mode
        constraint = !bank.load()?.get_flag(TOKENLESS_REPAYMENTS_ALLOWED)
            @MarginfiError::ForbiddenIx
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Seed constraint check
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load() ?.liquidity_vault_authority_bump,
    )]
    pub bank_liquidity_vault_authority: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
