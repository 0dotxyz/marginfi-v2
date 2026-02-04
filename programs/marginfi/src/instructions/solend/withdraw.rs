use crate::{
    bank_signer, check,
    constants::{PROGRAM_VERSION, SOLEND_PROGRAM_ID},
    events::{AccountEventHeader, LendingAccountWithdrawEvent},
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            account_not_frozen_for_authority, calc_value, is_signer_authorized,
            validate_remaining_accounts_for_balances_close_last, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl, RiskEngine,
        },
        marginfi_group::MarginfiGroupImpl,
        rate_limiter::{should_skip_rate_limit, BankRateLimiterImpl, GroupRateLimiterImpl},
    },
    utils::{
        assert_within_one_token, fetch_asset_price_for_bank_low_bias,
        fetch_unbiased_price_for_bank, is_solend_asset_tag, validate_bank_state, InstructionKind,
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token::accessor;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::types::{
    Bank, HealthCache, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP,
};
use marginfi_type_crate::{
    constants::LIQUIDITY_VAULT_AUTHORITY_SEED, types::ACCOUNT_IN_DELEVERAGE,
};
use solend_mocks::cpi::accounts::WithdrawObligationCollateralAndRedeemReserveCollateral;
use solend_mocks::cpi::withdraw_obligation_collateral_and_redeem_reserve_collateral;
use solend_mocks::state::{
    get_solend_obligation_deposit_amount, validate_solend_obligation, SolendMinimalReserve,
};

/// Withdraw from a Solend reserve through a marginfi account
///
/// # Important Note on Token Amounts:
/// The `amount` parameter is specified in terms of COLLATERAL tokens (cTokens), not the
/// underlying liquidity tokens (e.g., USDC).
///
/// Collateral tokens represent shares in the Solend reserve. When withdrawing:
///
/// 1. The user specifies how many collateral tokens they want to withdraw.
///
/// 2. Solend calculates the corresponding amount of liquidity tokens (e.g., USDC)
///    to return based on the current exchange rate in the Solend reserve.
///
/// 3. If a user wants to withdraw a specific amount of liquidity tokens, they need
///    to calculate the required collateral tokens themselves using the reserve's current
///    exchange rate before making the withdrawal request.
///
/// 4. For withdrawing an entire position, use the `withdraw_all` option instead of
///    trying to calculate the exact amount.
///
/// This function performs the following steps:
/// 1. Gets the user's collateral balance and initial obligation data
/// 2. Calculates the appropriate number of collateral tokens to withdraw
/// 3. Performs a CPI call to Solend to withdraw tokens
/// 4. Verifies the withdrawal was successful
/// 5. Transfers funds to the user's account
/// 6. Updates the marginfi account's balance to reflect the withdrawal
pub fn solend_withdraw<'info>(
    ctx: Context<'_, '_, 'info, 'info, SolendWithdraw<'info>>,
    amount: u64, // Collateral token amount (cTokens)
    withdraw_all: Option<bool>,
) -> MarginfiResult {
    validate_solend_obligation(
        ctx.accounts.integration_acc_2.as_ref(),
        ctx.accounts.integration_acc_1.key(),
    )?;

    let withdraw_all = withdraw_all.unwrap_or(false);
    let authority_bump: u8;
    let bank_key = ctx.accounts.bank.key();
    if withdraw_all {
        let marginfi_account = ctx.accounts.marginfi_account.load()?;
        // Require remaining accounts for all active balances, including the one being closed.
        validate_remaining_accounts_for_balances_close_last(
            &marginfi_account.lending_account,
            ctx.remaining_accounts,
            &bank_key,
        )?;
    }
    let collateral_amount = {
        let mut bank = ctx.accounts.bank.load_mut()?;
        let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
        let mut group = ctx.accounts.group.load_mut()?;
        let clock = Clock::get()?;
        authority_bump = bank.liquidity_vault_authority_bump;

        validate_bank_state(&bank, InstructionKind::FailsInPausedState)?;

        check!(
            !marginfi_account.get_flag(ACCOUNT_DISABLED),
            MarginfiError::AccountDisabled
        );

        // Fetch oracle price for rate limiting and deleverage tracking
        let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
        let group_rate_limit_enabled = group.rate_limiter.is_enabled();
        let price = if in_receivership || group_rate_limit_enabled {
            let price = fetch_asset_price_for_bank_low_bias(
                &bank_key,
                &bank,
                &clock,
                ctx.remaining_accounts,
            )?;

            // Validate price is non-zero during liquidation/deleverage to prevent exploits with stale oracles
            if in_receivership {
                check!(price > I80F48::ZERO, MarginfiError::ZeroAssetPrice);
            }

            price
        } else {
            I80F48::ZERO
        };

        let mut bank_account = BankAccountWrapper::find(
            &ctx.accounts.bank.key(),
            &mut bank,
            &mut marginfi_account.lending_account,
        )?;

        let collateral_amount = if withdraw_all {
            bank_account.withdraw_all()?
        } else {
            bank_account.withdraw(I80F48::from_num(amount))?;
            amount
        };

        // Rate limiting tracks net outflow; skip for flashloan/liquidation/deleverage flows.
        let rate_limit_amount = if withdraw_all {
            collateral_amount
        } else {
            amount
        };
        if !should_skip_rate_limit(marginfi_account.account_flags) {
            // Bank-level rate limiting (native tokens)
            if bank.rate_limiter.is_enabled() {
                bank.rate_limiter
                    .try_record_outflow(rate_limit_amount, clock.unix_timestamp)?;
            }

            // Group-level rate limiting (USD) - use fresh oracle price
            if group_rate_limit_enabled {
                check!(price > I80F48::ZERO, MarginfiError::InvalidRateLimitPrice);
                let usd_value = calc_value(
                    I80F48::from_num(rate_limit_amount),
                    price,
                    bank.get_balance_decimals(),
                    None,
                )?;
                group
                    .rate_limiter
                    .try_record_outflow(usd_value.to_num::<u64>(), clock.unix_timestamp)?;
            }
        }

        // Track withdrawal limit for risk admin during deleverage
        if marginfi_account.get_flag(ACCOUNT_IN_DELEVERAGE) {
            let withdrawn_equity = calc_value(
                I80F48::from_num(collateral_amount),
                price,
                bank.get_balance_decimals(),
                None,
            )?;
            group.update_withdrawn_equity(withdrawn_equity, clock.unix_timestamp)?;
        }

        collateral_amount
    };

    // Get initial obligation data to verify withdrawal amount later
    let initial_obligation_deposited_amount =
        get_solend_obligation_deposit_amount(&ctx.accounts.integration_acc_2)?;

    // Get initial values to verify successful withdrawal later
    let pre_transfer_vault_balance =
        accessor::amount(&ctx.accounts.liquidity_vault.to_account_info())?;
    let expected_liquidity_amount = ctx
        .accounts
        .integration_acc_1
        .load()?
        .collateral_to_liquidity(collateral_amount)?;

    ctx.accounts
        .cpi_solend_withdraw(collateral_amount, authority_bump)?;

    // Verify the obligation deposit amount decreased by the correct amount
    let final_obligation_deposited_amount =
        get_solend_obligation_deposit_amount(&ctx.accounts.integration_acc_2)?;
    let obligation_collateral_change =
        initial_obligation_deposited_amount - final_obligation_deposited_amount;
    assert_within_one_token(
        obligation_collateral_change,
        collateral_amount,
        MarginfiError::SolendWithdrawFailed,
    )?;

    let post_transfer_vault_balance =
        accessor::amount(&ctx.accounts.liquidity_vault.to_account_info())?;
    let received = post_transfer_vault_balance - pre_transfer_vault_balance;
    assert_within_one_token(
        received,
        expected_liquidity_amount,
        MarginfiError::SolendWithdrawFailed,
    )?;

    ctx.accounts
        .cpi_transfer_liquidity_vault_to_destination(received, authority_bump)?;
    {
        let mut bank = ctx.accounts.bank.load_mut()?;
        let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
        let group = &ctx.accounts.group.load()?;

        // Update bank cache after modifying balances
        bank.update_bank_cache(group)?;

        marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;

        emit!(LendingAccountWithdrawEvent {
            header: AccountEventHeader {
                signer: Some(ctx.accounts.authority.key()),
                marginfi_account: ctx.accounts.marginfi_account.key(),
                marginfi_account_authority: marginfi_account.authority,
                marginfi_group: marginfi_account.group,
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            amount: collateral_amount,
            close_balance: withdraw_all,
        });

        let mut health_cache = HealthCache::zeroed();
        health_cache.timestamp = Clock::get()?.unix_timestamp;

        marginfi_account.lending_account.sort_balances();

        // Drop the bank mutable borrow before health check (bank is in remaining_accounts)
        drop(bank);

        let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);

        // Note: during liquidation, we skip all health checks until the end of the transaction,
        // but we still update the price cache.
        if !in_receivership {
            // Check account health, if below threshold fail transaction
            // Assuming `ctx.remaining_accounts` holds only oracle accounts
            let (risk_result, risk_engine) = RiskEngine::check_account_init_health(
                &marginfi_account,
                ctx.remaining_accounts,
                &mut Some(&mut health_cache),
            );
            risk_result?;
            health_cache.program_version = PROGRAM_VERSION;

            // Note: in flashloans, risk_engine is None, and we skip the cache price update.
            let bank_loader = &ctx.accounts.bank;
            if let Some(engine) = risk_engine {
                if let Ok(price) = engine.get_unbiased_price_for_bank(&bank_loader.key()) {
                    bank_loader.load_mut()?.update_cache_price(Some(price))?;
                }
            }

            health_cache.set_engine_ok(true);
            marginfi_account.health_cache = health_cache;
        } else {
            // Note: the caller can simply omit risk accounts since the risk check is ignored here,
            // that case the cache doesn't update and this does nothing.
            let mut bank = ctx.accounts.bank.load_mut()?;
            let clock = Clock::get()?;
            let price_for_cache =
                fetch_unbiased_price_for_bank(&bank_key, &bank, &clock, ctx.remaining_accounts)
                    .ok();

            bank.update_cache_price(price_for_cache)?;
        }
    }

    Ok(())
}

#[derive(Accounts)]
pub struct SolendWithdraw<'info> {
    #[account(
        mut,
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
            is_signer_authorized(&a, g.admin, authority.key(), true)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        has_one = integration_acc_1 @ MarginfiError::InvalidSolendReserve,
        has_one = integration_acc_2 @ MarginfiError::InvalidSolendObligation,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_solend_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForSolendOperation,
        // Block withdraw of zero-weight assets during receivership - prevents unfair liquidation
        constraint = {
            let a = marginfi_account.load()?;
            let b = bank.load()?;
            let weight: I80F48 = b.config.asset_weight_init.into();
            !(a.get_flag(ACCOUNT_IN_RECEIVERSHIP) && weight == I80F48::ZERO)
        } @ MarginfiError::LiquidationPremiumTooHigh
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Token account that will get tokens back
    /// WARN: Completely unchecked!
    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// The Solend obligation account
    /// CHECK: Validated in instruction body
    #[account(
        mut,
        constraint = integration_acc_2.owner == &SOLEND_PROGRAM_ID @ MarginfiError::InvalidSolendAccount
    )]
    pub integration_acc_2: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub lending_market: UncheckedAccount<'info>,

    /// Derived from the lending market
    /// CHECK: validated by the Solend program
    pub lending_market_authority: UncheckedAccount<'info>,

    /// The Solend reserve that holds liquidity
    #[account(
        mut,
        constraint = !integration_acc_1.load()?.is_stale()? @ MarginfiError::SolendReserveStale
    )]
    pub integration_acc_1: AccountLoader<'info, SolendMinimalReserve>,

    /// Bank's liquidity token mint (e.g., USDC)
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Reserve's liquidity supply SPL Token account
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The reserve's mint for cTokens
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,

    /// The reserve's collateral supply account (where cTokens are stored)
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_collateral_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The user's destination for cTokens (collateral). This is a temporary account owned by
    /// liquidity_vault_authority that holds cTokens.
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub user_collateral: UncheckedAccount<'info>,

    /// CHECK: validated against hardcoded program id
    #[account(address = SOLEND_PROGRAM_ID)]
    pub solend_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> SolendWithdraw<'info> {
    pub fn cpi_solend_withdraw(
        &self,
        collateral_amount: u64,
        authority_bump: u8,
    ) -> MarginfiResult {
        let accounts = WithdrawObligationCollateralAndRedeemReserveCollateral {
            source_collateral_info: self.reserve_collateral_supply.to_account_info(),
            destination_collateral_info: self.user_collateral.to_account_info(),
            reserve_info: self.integration_acc_1.to_account_info(),
            obligation_info: self.integration_acc_2.to_account_info(),
            lending_market_info: self.lending_market.to_account_info(),
            lending_market_authority_info: self.lending_market_authority.to_account_info(),
            destination_liquidity_info: self.liquidity_vault.to_account_info(),
            reserve_collateral_mint_info: self.reserve_collateral_mint.to_account_info(),
            reserve_liquidity_supply_info: self.reserve_liquidity_supply.to_account_info(),
            obligation_owner_info: self.liquidity_vault_authority.to_account_info(),
            user_transfer_authority_info: self.liquidity_vault_authority.to_account_info(),
            token_program_info: self.token_program.to_account_info(),
            deposit_reserve_info: self.integration_acc_1.to_account_info(),
        };
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);

        // Create CPI context with signer
        let cpi_ctx = CpiContext::new_with_signer(
            self.solend_program.to_account_info(),
            accounts,
            signer_seeds,
        );
        withdraw_obligation_collateral_and_redeem_reserve_collateral(cpi_ctx, collateral_amount)?;
        Ok(())
    }

    pub fn cpi_transfer_liquidity_vault_to_destination(
        &self,
        amount: u64,
        authority_bump: u8,
    ) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.liquidity_vault.to_account_info(),
            to: self.destination_token_account.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            mint: self.mint.to_account_info(),
        };
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);

        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        let decimals = self.mint.decimals;
        transfer_checked(cpi_ctx, amount, decimals)?;
        Ok(())
    }
}
