use crate::{
    check,
    constants::{FARMS_PROGRAM_ID, KAMINO_PROGRAM_ID, PROGRAM_VERSION},
    events::{AccountEventHeader, LendingAccountWithdrawEvent},
    ix_utils::{get_discrim_hash, Hashable},
    optional_account,
    state::{
        bank::BankImpl,
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
        fetch_unbiased_price_for_bank, is_kamino_asset_tag, validate_bank_state, InstructionKind,
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_lang::solana_program::sysvar::{self, Sysvar};
use anchor_spl::token::{accessor, Token};
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use kamino_mocks::kamino_lending::cpi::withdraw_obligation_collateral_and_redeem_reserve_collateral_v2;
use kamino_mocks::{
    kamino_lending::cpi::accounts::{
        SocializeLossV2FarmsAccounts, WithdrawObligationCollateralAndRedeemReserveCollateral,
        WithdrawObligationCollateralAndRedeemReserveCollateralV2,
    },
    state::{MinimalObligation, MinimalReserve},
};
use marginfi_type_crate::types::{
    Bank, HealthCache, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP,
};
use marginfi_type_crate::{
    constants::{LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED},
    types::ACCOUNT_IN_DELEVERAGE,
};

/// Withdraw from a Kamino reserve through a marginfi account
///
/// # Important Note on Token Amounts:
/// The `amount` parameter is specified in terms of COLLATERAL tokens, not the underlying
/// liquidity tokens (e.g., USDC). This is important for users to understand.
///
/// Collateral tokens represent shares in the Kamino reserve. When withdrawing:
///
/// 1. The user specifies how many collateral tokens they want to withdraw.
///
/// 2. Kamino calculates the corresponding amount of liquidity tokens (e.g., USDC)
///    to return based on the current exchange rate in the Kamino reserve.
///
/// 3. If a user wants to withdraw a specific amount of liquidity tokens, they need
///    to calculate the required collateral tokens themselves using the reserve's current
///    exchange rate before making the withdrawal request.
///
/// 4. For withdrawing an entire position, use the `withdraw_all` option instead of
///    trying to calculate the exact amount.
///
/// This function performs the following steps:
/// 1. Gets the user's asset shares and initial obligation data
/// 2. Calculates the appropriate number of collateral tokens to withdraw
/// 3. Performs a CPI call to Kamino to withdraw tokens
/// 4. Verifies the obligation deposit amount was reduced correctly
/// 5. Transfers funds to the user's account
/// 6. Updates the marginfi account's balance to reflect the withdrawal
pub fn kamino_withdraw<'info>(
    ctx: Context<'_, '_, 'info, 'info, KaminoWithdraw<'info>>,
    amount: u64, // Collateral token amount
    withdraw_all: Option<bool>,
) -> MarginfiResult {
    let withdraw_all = withdraw_all.unwrap_or(false);

    // Get initial values to verify successful withdrawal later
    let pre_transfer_vault_balance =
        accessor::amount(&ctx.accounts.liquidity_vault.to_account_info())?;
    let initial_deposit_amount =
        ctx.accounts.integration_acc_2.load()?.deposits[0].deposited_amount;

    let collateral_amount;
    let bank_key = ctx.accounts.bank.key();
    let bank_mint = ctx.accounts.bank.load()?.mint;
    if withdraw_all {
        let marginfi_account = ctx.accounts.marginfi_account.load()?;
        // Require remaining accounts for all active balances, including the one being closed.
        validate_remaining_accounts_for_balances_close_last(
            &marginfi_account.lending_account,
            ctx.remaining_accounts,
            &bank_key,
        )?;
    }
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let clock = Clock::get()?;

    {
        let mut bank = ctx.accounts.bank.load_mut()?;
        let mut group = ctx.accounts.group.load_mut()?;
        validate_bank_state(&bank, InstructionKind::FailsInPausedState)?;

        check!(
            !marginfi_account.get_flag(ACCOUNT_DISABLED),
            MarginfiError::AccountDisabled
        );

        let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);

        // Fetch oracle price for rate limiting and deleverage tracking
        // When group rate limiter is enabled, oracle is required
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

        collateral_amount = if withdraw_all {
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

        // Note: we only care about the withdraw limit in case of deleverage
        if marginfi_account.get_flag(ACCOUNT_IN_DELEVERAGE) {
            let withdrawn_equity = calc_value(
                I80F48::from_num(collateral_amount),
                price,
                bank.get_balance_decimals(),
                None,
            )?;
            group.update_withdrawn_equity(withdrawn_equity, clock.unix_timestamp)?;
        }

        // Update bank cache after modifying balances (following pattern from regular withdraw)
        bank.update_bank_cache(&group)?;

        marginfi_account.last_update = clock.unix_timestamp as u64;
    }

    let expected_liquidity_amount = ctx
        .accounts
        .integration_acc_1
        .load()?
        .collateral_to_liquidity(collateral_amount)?;

    ctx.accounts.cpi_kamino_withdraw(collateral_amount)?;

    // Really just a sanity check, vault balance change is more important
    let final_deposit_amount = ctx.accounts.integration_acc_2.load()?.deposits[0].deposited_amount;
    let actual_deposit_decrease = initial_deposit_amount - final_deposit_amount;
    require_eq!(
        actual_deposit_decrease,
        collateral_amount,
        MarginfiError::KaminoWithdrawFailed
    );

    let post_transfer_vault_balance =
        accessor::amount(&ctx.accounts.liquidity_vault.to_account_info())?;
    let received = post_transfer_vault_balance - pre_transfer_vault_balance;
    assert_within_one_token(
        received,
        expected_liquidity_amount,
        MarginfiError::KaminoWithdrawFailed,
    )?;

    ctx.accounts
        .cpi_transfer_obligation_owner_to_destination(received)?;

    emit!(LendingAccountWithdrawEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.marginfi_account.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank: ctx.accounts.bank.key(),
        mint: bank_mint,
        amount: collateral_amount,
        close_balance: withdraw_all,
    });

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;

    marginfi_account.lending_account.sort_balances();

    let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);

    // Note: during liquidation/receivership, we skip health checks until the end of the transaction,
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
        // Note: the caller can simply omit risk accounts since the risk check is ignored here, in
        // that case the cache doesn't update and this does nothing.
        let mut bank = ctx.accounts.bank.load_mut()?;
        let price_for_cache =
            fetch_unbiased_price_for_bank(&bank_key, &bank, &clock, ctx.remaining_accounts).ok();

        bank.update_cache_price(price_for_cache)?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct KaminoWithdraw<'info> {
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
        has_one = integration_acc_1 @ MarginfiError::InvalidKaminoReserve,
        has_one = integration_acc_2 @ MarginfiError::InvalidKaminoObligation,
        constraint = is_kamino_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForKaminoInstructions,
        // Block withdraw of zero-weight assets during receivership - prevents unfair liquidation
        constraint = {
            let a = marginfi_account.load()?;
            let b = bank.load()?;
            let weight: I80F48 = b.config.asset_weight_init.into();
            !(a.get_flag(ACCOUNT_IN_RECEIVERSHIP) && weight == I80F48::ZERO)
        } @MarginfiError::LiquidationPremiumTooHigh
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

    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_bump,
    )]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut,
        // Only one reserve should be active on the obligation
        constraint = {
            let obligation = integration_acc_2.load()?;
            obligation.deposits.iter().skip(1).all(|d| d.deposited_amount == 0)
        } @ MarginfiError::InvalidObligationDepositCount,
        // The only reserve active should be the bank's linked reserve
        constraint = {
            let obligation = integration_acc_2.load()?;
            obligation.deposits[0].deposit_reserve == integration_acc_1.key()
        } @ MarginfiError::ObligationDepositReserveMismatch
    )]
    pub integration_acc_2: AccountLoader<'info, MinimalObligation>,

    /// The Kamino lending market
    /// CHECK: This is validated by the Kamino program
    pub lending_market: UncheckedAccount<'info>,

    /// The Kamino lending market authority
    /// CHECK: This is validated by the Kamino program
    pub lending_market_authority: UncheckedAccount<'info>,

    /// The Kamino reserve that holds liquidity
    #[account(mut)]
    pub integration_acc_1: AccountLoader<'info, MinimalReserve>,

    /// The liquidity token mint (e.g., USDC)
    /// Needs serde to get the mint decimals for transfer checked
    #[account(mut)]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    /// The reserve's liquidity supply account
    /// CHECK: This is validated by the Kamino program
    #[account(mut)]
    pub reserve_liquidity_supply: UncheckedAccount<'info>,

    /// The reserve's collateral mint
    /// CHECK: This is validated by the Kamino program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,

    /// The reserve's source for collateral tokens
    /// CHECK: This is validated by the Kamino program
    #[account(mut)]
    pub reserve_source_collateral: UncheckedAccount<'info>,

    /// Optional farms accounts for Kamino staking functionality
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub obligation_farm_user_state: Option<UncheckedAccount<'info>>,

    /// CHECK: validated by the Kamino program  
    #[account(mut)]
    pub reserve_farm_state: Option<UncheckedAccount<'info>>,

    /// CHECK: Use the cfg appropriate kamino program id
    #[account(address = KAMINO_PROGRAM_ID)]
    pub kamino_program: UncheckedAccount<'info>,

    /// Farms program for Kamino staking functionality
    /// CHECK: This is validated by the Kamino program
    #[account(address = FARMS_PROGRAM_ID)]
    pub farms_program: UncheckedAccount<'info>,

    /// The token program for the collateral token
    pub collateral_token_program: Program<'info, Token>,

    /// The token program for the liquidity token
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    /// Used by kamino validate CPI calls
    /// CHECK: read‑only Instructions sysvar
    #[account(address = sysvar::instructions::ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,
}

impl<'info> KaminoWithdraw<'info> {
    pub fn cpi_kamino_withdraw(&self, collateral_amount: u64) -> MarginfiResult {
        let withdraw_accounts = WithdrawObligationCollateralAndRedeemReserveCollateral {
            collateral_token_program: self.collateral_token_program.to_account_info(),
            instruction_sysvar_account: self.instruction_sysvar_account.to_account_info(),
            lending_market: self.lending_market.to_account_info(),
            lending_market_authority: self.lending_market_authority.to_account_info(),
            liquidity_token_program: self.liquidity_token_program.to_account_info(),
            obligation: self.integration_acc_2.to_account_info(),
            owner: self.liquidity_vault_authority.to_account_info(),
            placeholder_user_destination_collateral: None,
            reserve_collateral_mint: self.reserve_collateral_mint.to_account_info(),
            reserve_liquidity_mint: self.reserve_liquidity_mint.to_account_info(),
            reserve_liquidity_supply: self.reserve_liquidity_supply.to_account_info(),
            reserve_source_collateral: self.reserve_source_collateral.to_account_info(),
            user_destination_liquidity: self.liquidity_vault.to_account_info(),
            withdraw_reserve: self.integration_acc_1.to_account_info(),
        };
        let farms_accounts = SocializeLossV2FarmsAccounts {
            obligation_farm_user_state: optional_account!(self.obligation_farm_user_state),
            reserve_farm_state: optional_account!(self.reserve_farm_state),
        };
        let accounts = WithdrawObligationCollateralAndRedeemReserveCollateralV2 {
            withdraw_obligation_collateral_and_redeem_reserve_collateral_v2_withdraw_accounts:
                withdraw_accounts,
            withdraw_obligation_collateral_and_redeem_reserve_collateral_v2_farms_accounts:
                farms_accounts,
            farms_program: self.farms_program.to_account_info(),
        };
        let program = self.kamino_program.to_account_info();
        let bank_key = self.bank.key();
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let seeds = &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank_key.as_ref(),
            &[bump],
        ];
        let signer_seeds: &[&[&[u8]]] = &[seeds];
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        withdraw_obligation_collateral_and_redeem_reserve_collateral_v2(
            cpi_ctx,
            collateral_amount,
        )?;
        Ok(())
    }

    pub fn cpi_transfer_obligation_owner_to_destination(&self, amount: u64) -> MarginfiResult {
        let program = self.liquidity_token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.liquidity_vault.to_account_info(),
            to: self.destination_token_account.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            mint: self.reserve_liquidity_mint.to_account_info(),
        };
        let bank_key = self.bank.key();
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let seeds = &[
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank_key.as_ref(),
            &[bump],
        ];
        let signer_seeds: &[&[&[u8]]] = &[seeds];
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        let decimals = self.reserve_liquidity_mint.decimals;
        transfer_checked(cpi_ctx, amount, decimals)?;
        Ok(())
    }
}

impl Hashable for KaminoWithdraw<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "kamino_withdraw")
    }
}
