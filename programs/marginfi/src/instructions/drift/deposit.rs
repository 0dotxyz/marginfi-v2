use crate::{
    instructions::integration::{self, impl_common_deposit},
    state::{
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::is_drift_asset_tag,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use drift_mocks::state::MinimalUser;
use marginfi_type_crate::constants::{ASSET_TAG_DRIFT, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::pdas::DRIFT_PROGRAM_ID;
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED};

/// Deposit into a Drift spot market through a marginfi account
///
/// This function performs the following steps:
/// 1. Updates the spot market cumulative interest to ensure fresh calculations
/// 2. Transfers tokens from the user's source account to the liquidity vault
/// 3. Deposits the tokens into Drift through a CPI call
/// 4. Verifies the spot position was updated correctly
/// 5. Updates the marginfi account's balance to reflect the deposit
pub fn drift_deposit<'info>(
    ctx: Context<'info, DriftDeposit<'info>>,
    amount: u64,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    // Leaked to get a true `'info` borrow (the bump allocator never frees, so this costs nothing).
    let protocol_accounts = ctx.accounts.protocol_accounts().leak();
    integration::integration_deposit_impl(
        &common,
        protocol_accounts,
        amount,
        Some(ASSET_TAG_DRIFT),
        false,
    )
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
    #[account(mut)]
    pub integration_acc_2: AccountLoader<'info, MinimalUser>,

    /// The Drift user stats account owned by liquidity_vault_authority
    /// CHECK: validated by the Drift program
    #[account(mut)]
    pub integration_acc_3: UncheckedAccount<'info>,

    /// The Drift spot market for this asset
    #[account(mut)]
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

impl_common_deposit!(DriftDeposit);

impl<'info> DriftDeposit<'info> {
    fn protocol_accounts(&self) -> Vec<AccountInfo<'info>> {
        let mut accounts = vec![
            self.drift_state.to_account_info(),
            self.integration_acc_2.to_account_info(),
            self.integration_acc_3.to_account_info(),
            self.integration_acc_1.to_account_info(),
            self.drift_spot_market_vault.to_account_info(),
            self.drift_program.to_account_info(),
            self.system_program.to_account_info(),
        ];
        if let Some(ref oracle) = self.drift_oracle {
            accounts.push(oracle.to_account_info());
        }
        accounts
    }
}
