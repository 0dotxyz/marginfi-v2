use crate::{
    bank_signer, check,
    constants::DRIFT_PROGRAM_ID,
    events::{AccountEventHeader, LendingAccountDepositEvent},
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::{is_drift_asset_tag, validate_asset_tags, validate_bank_state, InstructionKind},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use drift_mocks::drift::cpi::accounts::{Deposit, UpdateSpotMarketCumulativeInterest};
use drift_mocks::drift::cpi::{deposit, update_spot_market_cumulative_interest};
use drift_mocks::state::MinimalUser;
use fixed::types::I80F48;
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP,
};

/// Deposit into a Drift spot market through a marginfi account
///
/// This function performs the following steps:
/// 1. Updates the spot market cumulative interest to ensure fresh calculations
/// 2. Transfers tokens from the user's source account to the liquidity vault
/// 3. Deposits the tokens into Drift through a CPI call
/// 4. Verifies the spot position was updated correctly
/// 5. Updates the marginfi account's balance to reflect the deposit
pub fn drift_deposit(ctx: Context<DriftDeposit>, amount: u64) -> MarginfiResult {
    let authority_bump: u8;
    let market_index: u16;
    {
        let marginfi_account = ctx.accounts.marginfi_account.load()?;
        let bank = ctx.accounts.bank.load()?;
        authority_bump = bank.liquidity_vault_authority_bump;

        validate_asset_tags(&bank, &marginfi_account)?;
        validate_bank_state(&bank, InstructionKind::FailsIfPausedOrReduceState)?;

        check!(
            !marginfi_account.get_flag(ACCOUNT_DISABLED)
                && !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
            MarginfiError::AccountDisabled
        );

        let integration_acc_1 = ctx.accounts.integration_acc_1.load()?;
        market_index = integration_acc_1.market_index;
    }

    ctx.accounts.cpi_update_spot_market_cumulative_interest()?;
    let expected_scaled_balance_change = ctx
        .accounts
        .integration_acc_1
        .load()?
        .get_scaled_balance_increment(amount)?;

    let initial_scaled_balance = {
        let integration_acc_2 = ctx.accounts.integration_acc_2.load()?;
        integration_acc_2.get_scaled_balance(market_index)
    };

    ctx.accounts.cpi_transfer_user_to_liquidity_vault(amount)?;
    ctx.accounts
        .cpi_drift_deposit(market_index, amount, authority_bump)?;

    let final_scaled_balance = {
        let integration_acc_2 = ctx.accounts.integration_acc_2.load()?;
        integration_acc_2.get_scaled_balance(market_index)
    };
    let scaled_balance_change = final_scaled_balance - initial_scaled_balance;
    require_eq!(
        scaled_balance_change,
        expected_scaled_balance_change,
        MarginfiError::DriftScaledBalanceMismatch
    );

    {
        let mut bank = ctx.accounts.bank.load_mut()?;
        let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
        let group = &ctx.accounts.group.load()?;

        let mut bank_account = BankAccountWrapper::find_or_create(
            &ctx.accounts.bank.key(),
            &mut bank,
            &mut marginfi_account.lending_account,
        )?;

        let scaled_balance_change_i80f48 = I80F48::from_num(scaled_balance_change);
        bank_account.deposit_no_repay(scaled_balance_change_i80f48)?;

        // Update bank cache after modifying balances
        bank.update_bank_cache(group)?;

        marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;
        marginfi_account.lending_account.sort_balances();

        emit!(LendingAccountDepositEvent {
            header: AccountEventHeader {
                signer: Some(ctx.accounts.authority.key()),
                marginfi_account: ctx.accounts.marginfi_account.key(),
                marginfi_account_authority: marginfi_account.authority,
                marginfi_group: marginfi_account.group,
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            amount,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct DriftDeposit<'info> {
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
        has_one = integration_acc_1 @ MarginfiError::InvalidDriftSpotMarket,
        has_one = integration_acc_2 @ MarginfiError::InvalidDriftUser,
        has_one = integration_acc_3 @ MarginfiError::InvalidDriftUserStats,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_drift_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForDriftOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// The oracle account for the asset (not needed if using oracle type QuoteAsset)
    /// CHECK: validated by Drift program
    pub drift_oracle: Option<UncheckedAccount<'info>>,

    /// The bank's liquidity vault authority, which owns the Drift user account
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Used as an intermediary to deposit tokens into Drift
    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// Owned by authority, the source account for the token deposit
    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The Drift state account
    /// CHECK: validated by the Drift program
    pub drift_state: UncheckedAccount<'info>,

    /// The Drift user account owned by liquidity_vault_authority
    #[account(
        mut,
        constraint = {
            let user = integration_acc_2.load()?;
            let spot_market = integration_acc_1.load()?;
            user.validate_spot_position(spot_market.market_index).is_ok()
        } @ MarginfiError::DriftInvalidSpotPositions
    )]
    pub integration_acc_2: AccountLoader<'info, MinimalUser>,

    /// The Drift user stats account owned by liquidity_vault_authority
    /// CHECK: validated by the Drift program
    #[account(mut)]
    pub integration_acc_3: UncheckedAccount<'info>,

    /// The Drift spot market for this asset
    #[account(
        mut,
        constraint = integration_acc_1.load()?.mint == mint.key()
            @ MarginfiError::DriftSpotMarketMintMismatch
    )]
    pub integration_acc_1: AccountLoader<'info, drift_mocks::state::MinimalSpotMarket>,

    /// The Drift spot market vault that will receive tokens
    /// CHECK: validated by the Drift program
    #[account(mut)]
    pub drift_spot_market_vault: UncheckedAccount<'info>,

    /// Bank's liquidity token mint
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: validated against hardcoded program id
    #[account(address = DRIFT_PROGRAM_ID)]
    pub drift_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> DriftDeposit<'info> {
    pub fn cpi_update_spot_market_cumulative_interest(&self) -> MarginfiResult {
        let accounts = UpdateSpotMarketCumulativeInterest {
            state: self.drift_state.to_account_info(),
            spot_market: self.integration_acc_1.to_account_info(),
            oracle: self
                .drift_oracle
                .as_ref()
                .map(|o| o.to_account_info())
                .unwrap_or(self.system_program.to_account_info()), // USDC uses system program
            spot_market_vault: self.drift_spot_market_vault.to_account_info(),
        };
        let program = self.drift_program.to_account_info();
        let cpi_ctx = CpiContext::new(program, accounts);
        update_spot_market_cumulative_interest(cpi_ctx)?;
        Ok(())
    }

    pub fn cpi_transfer_user_to_liquidity_vault(&self, amount: u64) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.signer_token_account.to_account_info(),
            to: self.liquidity_vault.to_account_info(),
            authority: self.authority.to_account_info(),
            mint: self.mint.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(program, accounts);
        let decimals = self.mint.decimals;
        transfer_checked(cpi_ctx, amount, decimals)?;
        Ok(())
    }

    pub fn cpi_drift_deposit(
        &self,
        market_index: u16,
        amount: u64,
        authority_bump: u8,
    ) -> MarginfiResult {
        let accounts = Deposit {
            state: self.drift_state.to_account_info(),
            user: self.integration_acc_2.to_account_info(),
            user_stats: self.integration_acc_3.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            spot_market_vault: self.drift_spot_market_vault.to_account_info(),
            user_token_account: self.liquidity_vault.to_account_info(),
            token_program: self.token_program.to_account_info(),
        };

        let program = self.drift_program.to_account_info();
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);
        let mut cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);

        // Construct remaining accounts in the required order for Drift:
        // 1. Oracle accounts (if provided)
        // 2. Spot market account (always required)
        // 3. Token mint (required for Token-2022, harmless to include for regular mints)
        let mut remaining_accounts = Vec::new();

        // Add oracle if provided (not needed if using oracle type QuoteAsset)
        if let Some(oracle) = &self.drift_oracle {
            remaining_accounts.push(oracle.to_account_info());
        }

        // Always add spot market account
        remaining_accounts.push(self.integration_acc_1.to_account_info());

        // Always add token mint (needed for Token-2022 support)
        remaining_accounts.push(self.mint.to_account_info());

        cpi_ctx = cpi_ctx.with_remaining_accounts(remaining_accounts);

        // Call drift deposit with reduce_only = false
        deposit(cpi_ctx, market_index, amount, false)?;
        Ok(())
    }
}
