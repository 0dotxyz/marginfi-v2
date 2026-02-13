use crate::{
    bank_signer, check,
    constants::PROGRAM_VERSION,
    events::{AccountEventHeader, LendingAccountWithdrawEvent},
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            calc_value, check_account_init_health, BankAccountWrapper, LendingAccountImpl,
            MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::{
        fetch_asset_price_for_bank_low_bias, fetch_unbiased_price_for_bank, is_juplend_asset_tag,
        validate_bank_state, InstructionKind,
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
use juplend_mocks::juplend_earn::cpi::accounts::{UpdateRate, Withdraw as WithdrawCpi};
use juplend_mocks::juplend_earn::cpi::{update_rate, withdraw as cpi_withdraw};
use juplend_mocks::state::Lending as JuplendLending;
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::types::{
    Bank, HealthCache, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP,
};

/// Withdraw underlying tokens from a JupLend lending pool through a marginfi account.
///
/// Flow (program-first, exact-math):
/// 1. CPI `update_rate` to refresh `token_exchange_price`.
/// 2. Compute expected fTokens burned: `ceil(assets * 1e12 / token_exchange_price)`.
/// 3. Call `bank_account.withdraw()` for the expected burned shares.
/// 4. CPI `withdraw` (burn fTokens, receive underlying into withdraw intermediary ATA).
/// 5. Verify received underlying == requested and burned fTokens == expected.
/// 6. Transfer underlying from withdraw intermediary ATA -> destination token account.
/// 7. Update health cache (unless receivership).
pub fn juplend_withdraw<'info>(
    ctx: Context<'_, '_, 'info, 'info, JuplendWithdraw<'info>>,
    amount: u64,
    withdraw_all: Option<bool>,
) -> MarginfiResult {
    let withdraw_all = withdraw_all.unwrap_or(false);

    // Refresh exchange pricing (interest/rewards) for this slot.
    ctx.accounts.cpi_update_rate()?;

    let bank_key = ctx.accounts.bank.key();
    let authority_bump: u8;

    // Update marginfi internal balances first (tx will revert if CPI fails later).
    //
    // For `withdraw_all`, we:
    // - call `bank_account.withdraw_all()` to close the marginfi position and obtain the full fToken share balance
    // - compute the redeemable underlying = floor(shares * exchange_rate)
    // - CPI JupLend `withdraw` for that underlying amount
    //
    // For partial withdraw, we:
    // - compute shares_to_burn = ceil(assets / exchange_rate)
    // - call `bank_account.withdraw(shares_to_burn)`
    // - CPI JupLend `withdraw` for the requested underlying `amount`
    let clock = Clock::get()?;
    let (token_amount, shares_to_burn) = {
        let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
        let mut bank = ctx.accounts.bank.load_mut()?;
        let mut group = ctx.accounts.group.load_mut()?;

        authority_bump = bank.liquidity_vault_authority_bump;
        validate_bank_state(&bank, InstructionKind::FailsInPausedState)?;

        check!(
            !marginfi_account.get_flag(ACCOUNT_DISABLED),
            MarginfiError::AccountDisabled
        );

        // Validate price is non-zero during liquidation/deleverage to prevent exploits with stale oracles.
        let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
        let price = if in_receivership {
            let price = fetch_asset_price_for_bank_low_bias(
                &bank_key,
                &bank,
                &clock,
                ctx.remaining_accounts,
            )?;
            check!(price > I80F48::ZERO, MarginfiError::ZeroAssetPrice);
            price
        } else {
            I80F48::ZERO
        };

        let mut bank_account = BankAccountWrapper::find(
            &ctx.accounts.bank.key(),
            &mut bank,
            &mut marginfi_account.lending_account,
        )?;

        let (token_amount, shares_to_burn) = if withdraw_all {
            // `withdraw_all` returns the user's full fToken share balance (u64).
            let total_shares = bank_account.withdraw_all()?;
            // Redeemable underlying = floor(shares * price / 1e12)
            // Then recalculate shares_to_burn from token_amount to guarantee we match
            // JupLend's expected burn amount (should be identical, but this is safer).
            let (token_amount, shares_to_burn) = {
                let lending = ctx.accounts.integration_acc_1.load()?;
                let token_amount = lending
                    .expected_assets_for_redeem(total_shares)
                    .map_err(|_| error!(MarginfiError::MathError))?;
                let shares_to_burn = lending
                    .expected_shares_for_withdraw(token_amount)
                    .map_err(|_| error!(MarginfiError::MathError))?;
                (token_amount, shares_to_burn)
            };

            // Sanity check: recalculated shares should never exceed what we have
            require!(shares_to_burn <= total_shares, MarginfiError::MathError);

            (token_amount, shares_to_burn)
        } else {
            // shares = ceil(assets * 1e12 / token_exchange_price)
            let shares_to_burn = {
                let lending = ctx.accounts.integration_acc_1.load()?;
                lending
                    .expected_shares_for_withdraw(amount)
                    .map_err(|_| error!(MarginfiError::MathError))?
            };

            bank_account.withdraw(I80F48::from_num(shares_to_burn))?;

            (amount, shares_to_burn)
        };

        // Track withdrawal limit for risk admin during deleverage.
        if ctx.accounts.authority.key() == group.risk_admin {
            let withdrawn_equity = calc_value(
                I80F48::from_num(shares_to_burn),
                price,
                bank.mint_decimals,
                None,
            )?;
            group.update_withdrawn_equity(withdrawn_equity, clock.unix_timestamp)?;
        }

        (token_amount, shares_to_burn)
    };

    // Record balances to verify exact deltas.
    let pre_withdraw_intermediary_ata_balance =
        accessor::amount(&ctx.accounts.integration_acc_3.to_account_info())?;
    let pre_f_token_balance = accessor::amount(&ctx.accounts.integration_acc_2.to_account_info())?;

    // Handle potential dust case where remaining shares are worth less than 1 underlying unit.
    //
    // NOTE: Unlike Drift (which has reachable dust due to double-rounding in its
    // assets → scaled_balance → assets conversion), this case is UNREACHABLE in JupLend
    // under normal operation because:
    //
    // - JupLend uses single-level math: shares = floor(assets * 1e12 / price)
    // - Minimum shares = 1 (u64 integer, not fractional)
    // - Exchange price >= 1e12 (starts at 1:1, only increases with yield)
    // - Therefore: floor(1 * 1e12 / 1e12) = 1 (always at least 1 underlying)
    //
    // Drift's dust is reachable because it uses multi-step rounding:
    // 1. assets → scaled_balance (floor + variable precision per token)
    // 2. scaled_balance + 1 (round up for safety)
    // 3. scaled_balance → assets (floor again)
    // This cascading rounding can produce 0 tokens from small positions.
    //
    // This defensive code exists for potential edge cases:
    // - Socialized loss reducing JupLend's exchange price below 1e12
    // - Future protocol changes affecting share/price invariants
    //
    // If we can guarantee that JupLend's exchange price never drops below 1e12, this branch is dead code.
    let received_underlying = if withdraw_all && token_amount == 0 {
        0
    } else {
        // CPI withdraw: burns fTokens and credits underlying into withdraw intermediary ATA.
        ctx.accounts
            .cpi_juplend_withdraw(token_amount, authority_bump)?;

        let post_withdraw_intermediary_ata_balance =
            accessor::amount(&ctx.accounts.integration_acc_3.to_account_info())?;
        let post_f_token_balance =
            accessor::amount(&ctx.accounts.integration_acc_2.to_account_info())?;

        let received_underlying = post_withdraw_intermediary_ata_balance
            .checked_sub(pre_withdraw_intermediary_ata_balance)
            .ok_or_else(|| error!(MarginfiError::MathError))?;
        require_eq!(
            received_underlying,
            token_amount,
            MarginfiError::JuplendWithdrawFailed
        );

        let burned_shares = pre_f_token_balance
            .checked_sub(post_f_token_balance)
            .ok_or_else(|| error!(MarginfiError::MathError))?;
        require_eq!(
            burned_shares,
            shares_to_burn,
            MarginfiError::JuplendWithdrawFailed
        );

        // Transfer underlying from withdraw intermediary ATA -> destination.
        ctx.accounts
            .cpi_transfer_withdraw_intermediary_ata_to_destination(
                received_underlying,
                authority_bump,
            )?;

        received_underlying
    };

    // Post-withdraw accounting + health check.
    {
        let mut bank = ctx.accounts.bank.load_mut()?;
        let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
        let group = &ctx.accounts.group.load()?;

        bank.update_bank_cache(group)?;

        marginfi_account.last_update = clock.unix_timestamp as u64;

        emit!(LendingAccountWithdrawEvent {
            header: AccountEventHeader {
                signer: Some(ctx.accounts.authority.key()),
                marginfi_account: ctx.accounts.marginfi_account.key(),
                marginfi_account_authority: marginfi_account.authority,
                marginfi_group: marginfi_account.group,
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            amount: received_underlying,
            close_balance: withdraw_all,
        });

        let mut health_cache = HealthCache::zeroed();
        health_cache.timestamp = clock.unix_timestamp;

        marginfi_account.lending_account.sort_balances();

        // Drop bank mutable borrow before health check (bank is in remaining_accounts).
        drop(bank);

        // Skip health checks during liquidation; checked at end of tx.
        // During receivership we still update the price cache but skip the risk check.
        if !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP) {
            check_account_init_health(
                &marginfi_account,
                ctx.remaining_accounts,
                &mut Some(&mut health_cache),
            )?;
            health_cache.program_version = PROGRAM_VERSION;

            let bank_loader = &ctx.accounts.bank;
            let bank = bank_loader.load()?;
            let clock = Clock::get()?;
            let price_for_cache = fetch_unbiased_price_for_bank(
                &bank_loader.key(),
                &bank,
                &clock,
                ctx.remaining_accounts,
            )
            .ok();
            drop(bank);
            bank_loader
                .load_mut()?
                .update_cache_price(price_for_cache)?;

            health_cache.set_engine_ok(true);
            marginfi_account.health_cache = health_cache;
        } else {
            // Note: the caller can simply omit risk accounts since the risk check is ignored here,
            // in that case the cache doesn't update and this does nothing.
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
pub struct JuplendWithdraw<'info> {
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
            a.authority == authority.key() || a.get_flag(ACCOUNT_IN_RECEIVERSHIP)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = integration_acc_1 @ MarginfiError::InvalidJuplendLending,
        has_one = integration_acc_2 @ MarginfiError::InvalidJuplendFTokenVault,
        has_one = integration_acc_3 @ MarginfiError::InvalidJuplendWithdrawIntermediaryAta,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_juplend_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForJuplendOperation,
        // Block withdraw of zero-weight assets during receivership - prevents unfair liquidation
        constraint = {
            let a = marginfi_account.load()?;
            let b = bank.load()?;
            let weight: I80F48 = b.config.asset_weight_init.into();
            !(a.get_flag(ACCOUNT_IN_RECEIVERSHIP) && weight == I80F48::ZERO)
        } @ MarginfiError::LiquidationPremiumTooHigh
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Token account that will receive the underlying withdrawal.
    /// WARN: Completely unchecked!
    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).
    /// NOTE: JupLend marks the signer as writable in their withdraw instruction.
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Underlying mint.
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// JupLend lending state account.
    #[account(mut, has_one = f_token_mint @ MarginfiError::InvalidJuplendLending)]
    pub integration_acc_1: AccountLoader<'info, JuplendLending>,

    /// JupLend fToken mint.
    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Bank's fToken vault (validated via has_one on bank).
    #[account(mut)]
    pub integration_acc_2: InterfaceAccount<'info, TokenAccount>,

    /// Withdraw intermediary ATA (authority = liquidity_vault_authority).
    /// This must be an ATA to satisfy JupLend's withdraw constraints.
    #[account(
        mut,
        token::mint = mint,
        token::authority = liquidity_vault_authority,
        token::token_program = token_program,
    )]
    pub integration_acc_3: InterfaceAccount<'info, TokenAccount>,

    // ---- JupLend CPI accounts ----
    /// CHECK: validated by the JupLend program
    pub lending_admin: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    #[account(
        mut,
        constraint = supply_token_reserves_liquidity.key() == integration_acc_1.load()?.token_reserves_liquidity
            @ MarginfiError::InvalidJuplendLending,
    )]
    pub supply_token_reserves_liquidity: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    #[account(
        mut,
        constraint = lending_supply_position_on_liquidity.key() == integration_acc_1.load()?.supply_position_on_liquidity
            @ MarginfiError::InvalidJuplendLending,
    )]
    pub lending_supply_position_on_liquidity: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    pub rate_model: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    #[account(mut)]
    pub vault: UncheckedAccount<'info>,

    /// JupLend claim account for liquidity_vault_authority.
    /// TEMPORARY: Mainnet currently requires this account (passing None causes ConstraintMut errors),
    /// but an upcoming upgrade is expected to make it truly optional. The account is never actually
    /// validated or used - you can pass any mutable account. We create the canonical PDA for consistency.
    /// Seeds: ["user_claim", liquidity_vault_authority, mint] on Liquidity program.
    /// CHECK: not validated by JupLend - any mutable account works
    #[account(mut)]
    pub claim_account: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    #[account(mut)]
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    pub liquidity_program: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    pub rewards_rate_model: UncheckedAccount<'info>,

    /// CHECK: validated against hardcoded program id
    #[account(address = juplend_mocks::ID)]
    pub juplend_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> JuplendWithdraw<'info> {
    pub fn cpi_update_rate(&self) -> MarginfiResult {
        let accounts = UpdateRate {
            lending: self.integration_acc_1.to_account_info(),
            mint: self.mint.to_account_info(),
            f_token_mint: self.f_token_mint.to_account_info(),
            supply_token_reserves_liquidity: self.supply_token_reserves_liquidity.to_account_info(),
            rewards_rate_model: self.rewards_rate_model.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(self.juplend_program.to_account_info(), accounts);
        update_rate(cpi_ctx)?;
        Ok(())
    }

    pub fn cpi_juplend_withdraw(&self, amount: u64, authority_bump: u8) -> MarginfiResult {
        let accounts = WithdrawCpi {
            signer: self.liquidity_vault_authority.to_account_info(),
            owner_token_account: self.integration_acc_2.to_account_info(),
            recipient_token_account: self.integration_acc_3.to_account_info(),
            lending_admin: self.lending_admin.to_account_info(),
            lending: self.integration_acc_1.to_account_info(),
            mint: self.mint.to_account_info(),
            f_token_mint: self.f_token_mint.to_account_info(),
            supply_token_reserves_liquidity: self.supply_token_reserves_liquidity.to_account_info(),
            lending_supply_position_on_liquidity: self
                .lending_supply_position_on_liquidity
                .to_account_info(),
            rate_model: self.rate_model.to_account_info(),
            vault: self.vault.to_account_info(),
            claim_account: Some(self.claim_account.to_account_info()),
            liquidity: self.liquidity.to_account_info(),
            liquidity_program: self.liquidity_program.to_account_info(),
            rewards_rate_model: self.rewards_rate_model.to_account_info(),
            token_program: self.token_program.to_account_info(),
            associated_token_program: self.associated_token_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };

        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);

        let cpi_ctx = CpiContext::new_with_signer(
            self.juplend_program.to_account_info(),
            accounts,
            signer_seeds,
        );

        cpi_withdraw(cpi_ctx, amount)?;
        Ok(())
    }

    pub fn cpi_transfer_withdraw_intermediary_ata_to_destination(
        &self,
        amount: u64,
        authority_bump: u8,
    ) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.integration_acc_3.to_account_info(),
            to: self.destination_token_account.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            mint: self.mint.to_account_info(),
        };

        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        transfer_checked(cpi_ctx, amount, self.mint.decimals)?;
        Ok(())
    }
}
