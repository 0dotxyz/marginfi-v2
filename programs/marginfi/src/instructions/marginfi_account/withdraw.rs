use crate::{
    bank_signer, check,
    constants::PROGRAM_VERSION,
    events::{AccountEventHeader, DeleverageWithdrawFlowEvent, LendingAccountWithdrawEvent},
    ix_utils::{get_discrim_hash, Hashable},
    prelude::*,
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            account_not_frozen_for_authority, calc_value, check_account_init_health,
            is_signer_authorized, BankAccountWrapper, LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
        rate_limiter::GroupRateLimiterImpl,
    },
    utils::{
        self, fetch_asset_price_for_bank_low_bias, fetch_unbiased_price_for_bank,
        is_marginfi_asset_tag, record_withdrawal_outflow, validate_bank_state, InstructionKind,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use anchor_spl::{
    token::accessor,
    token_interface::{TokenAccount, TokenInterface},
};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{LIQUIDITY_VAULT_AUTHORITY_SEED, TOKENLESS_REPAYMENTS_COMPLETE},
    types::{
        Bank, HealthCache, MarginfiAccount, MarginfiGroup, OraclePriceWithConfidence,
        ACCOUNT_DISABLED, ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_ORDER_EXECUTION,
        ACCOUNT_IN_RECEIVERSHIP,
    },
};

/// 1. Accrue interest
/// 2. Find the user's existing bank account for the asset withdrawn
/// 3. Record asset decrease in the bank account
/// 4. Transfer funds from the bank's liquidity vault to the signer's token account
/// 5. Verify that the user account is in a healthy state
///
/// Will error if there is no existing asset <=> borrowing is not allowed.
pub fn lending_account_withdraw<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdraw<'info>>,
    amount: u64,
    withdraw_all: Option<bool>,
) -> MarginfiResult {
    let LendingAccountWithdraw {
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

    let withdraw_all = withdraw_all.unwrap_or(false);
    let mut marginfi_account = marginfi_account_loader.load_mut()?;

    {
        let maybe_bank_mint = {
            let bank = bank_loader.load()?;
            utils::maybe_take_bank_mint(&mut ctx.remaining_accounts, &bank, token_program.key)?
        };

        let in_receivership_or_order_execution =
            marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_ORDER_EXECUTION);
        let group = marginfi_group_loader.load()?;
        let mut bank = bank_loader.load_mut()?;
        validate_bank_state(&bank, InstructionKind::FailsInPausedState, false)?;

        // Fetch oracle price for rate limiting and deleverage tracking
        // When group rate limiter is enabled, oracle is required
        let group_rate_limit_enabled = group.rate_limiter.is_enabled();
        let price = if in_receivership_or_order_execution || group_rate_limit_enabled {
            let price = fetch_asset_price_for_bank_low_bias(
                &bank_loader.key(),
                &bank,
                &clock,
                ctx.remaining_accounts,
            )?;

            // Validate price is non-zero during liquidation/deleverage to prevent exploits
            if in_receivership_or_order_execution {
                check!(price > I80F48::ZERO, MarginfiError::ZeroAssetPrice);
            }

            price
        } else {
            I80F48::ZERO
        };

        bank.accrue_interest(
            clock.unix_timestamp,
            &group,
            #[cfg(not(feature = "client"))]
            bank_loader.key(),
        )?;

        let liquidity_vault_authority_bump = bank.liquidity_vault_authority_bump;

        let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
        let lending_account = &mut marginfi_account.lending_account;
        let mut bank_account =
            BankAccountWrapper::find(&bank_loader.key(), &mut bank, lending_account)?;

        let amount_pre_fee = if withdraw_all {
            // Note: In liquidation, we still want this passed on the books
            bank_account.withdraw_all(in_receivership)?
        } else {
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

            bank_account.withdraw(I80F48::from_num(amount_pre_fee))?;

            amount_pre_fee
        };

        // If in deleverage mode and deleverage is complete, you get what's left!
        let amount_pre_fee = if bank.get_flag(TOKENLESS_REPAYMENTS_COMPLETE) {
            let actual = accessor::amount(&bank_liquidity_vault.to_account_info())?;
            msg!(
                "amount expected withdrawn: {:?}, actual: {:?}",
                amount_pre_fee,
                actual
            );
            u64::min(amount_pre_fee, actual)
        } else {
            amount_pre_fee
        };

        record_withdrawal_outflow(
            group_rate_limit_enabled,
            amount_pre_fee,
            amount_pre_fee,
            price,
            &mut bank,
            &group,
            marginfi_group_loader.key(),
            bank_loader.key(),
            &marginfi_account,
            &clock,
        )?;
        // Note: we only care about the withdraw limit in case of deleverage
        if marginfi_account.get_flag(ACCOUNT_IN_DELEVERAGE) {
            let withdrawn_equity = calc_value(
                I80F48::from_num(amount_pre_fee),
                price,
                bank.get_balance_decimals(),
                None,
            )?;
            group.check_deleverage_withdraw_limit(withdrawn_equity, clock.unix_timestamp)?;
            emit!(DeleverageWithdrawFlowEvent {
                group: marginfi_group_loader.key(),
                bank: bank_loader.key(),
                mint: bank.mint,
                outflow_usd: withdrawn_equity.to_num(),
                current_timestamp: clock.unix_timestamp,
            });
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
        bank.update_bank_cache(&group)?;

        emit!(LendingAccountWithdrawEvent {
            header: AccountEventHeader {
                signer: Some(ctx.accounts.authority.key()),
                marginfi_account: marginfi_account_loader.key(),
                marginfi_account_authority: marginfi_account.authority,
                marginfi_group: marginfi_account.group,
            },
            bank: bank_loader.key(),
            mint: bank.mint,
            amount: amount_pre_fee,
            close_balance: withdraw_all,
        });
    }

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;

    marginfi_account.lending_account.sort_balances();

    // To update the bank's price cache
    let maybe_price: Option<OraclePriceWithConfidence>;
    let bank_pk = bank_loader.key();

    // Note: during receivership and order execution, we skip all health checks until the end of the transaction.
    if !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP | ACCOUNT_IN_ORDER_EXECUTION) {
        // Check account health, if below threshold fail transaction
        // Assuming `ctx.remaining_accounts` holds only oracle accounts
        // Uses heap-efficient health check to support accounts with up to 16 positions
        check_account_init_health(
            &marginfi_account,
            ctx.remaining_accounts,
            &mut Some(&mut health_cache),
        )?;
        health_cache.program_version = PROGRAM_VERSION;

        health_cache.set_engine_ok(true);
        marginfi_account.health_cache = health_cache;
    }

    // Fetch unbiased price for cache update
    // Note: during receivership, callers may omit oracle accounts; the cache simply won't update.
    {
        let bank = bank_loader.load()?;
        maybe_price =
            fetch_unbiased_price_for_bank(&bank_pk, &bank, &clock, ctx.remaining_accounts).ok();
    }

    bank_loader.load_mut()?.update_cache_price(maybe_price)?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountWithdraw<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
            || marginfi_account.load()?.get_flag(ACCOUNT_IN_DELEVERAGE)
        ) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let acc = marginfi_account.load()?;
            !acc.get_flag(ACCOUNT_DISABLED)
        } @MarginfiError::AccountDisabled,
        constraint = {
            let a = marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), true, true)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// Must be marginfi_account's authority, unless in liquidation/deleverage receivership or order execution
    ///
    /// Note: during receivership and order execution, there are no signer checks whatsoever: any key can repay as
    /// long as the invariants checked at the end of execution are met.
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
        // We want to block withdraw of assets with no weight (e.g. isolated) otherwise the
        // liquidator can just take all of them and the user gets nothing back, which is unfair. For
        // assets with any nominal weight, e.g. 10%, caveat emptor
        constraint = {
            let a = marginfi_account.load()?;
            let b = bank.load()?;
            let weight: I80F48 = b.config.asset_weight_init.into();
            !(a.get_flag(ACCOUNT_IN_RECEIVERSHIP) && weight == I80F48::ZERO)
        } @MarginfiError::LiquidationPremiumTooHigh
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
        bump = bank.load()?.liquidity_vault_authority_bump,
    )]
    pub bank_liquidity_vault_authority: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl Hashable for LendingAccountWithdraw<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "lending_account_withdraw")
    }
}
