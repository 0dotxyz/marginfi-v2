use crate::{
    constants::SOLEND_PROGRAM_ID,
    instructions::integration::{self, impl_common_deposit},
    state::{
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::is_solend_asset_tag,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use marginfi_type_crate::constants::{ASSET_TAG_SOLEND, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED};
use solend_mocks::state::SolendMinimalReserve;

/// Deposit into a Solend reserve through a marginfi account
///
/// This function performs the following steps:
/// 1. Transfers tokens from the user's source account to the liquidity vault
/// 2. Deposits the tokens into Solend through a CPI call
/// 3. Verifies the obligation collateral was increased correctly
/// 4. Updates the marginfi account's balance to reflect the deposit
pub fn solend_deposit<'info>(
    ctx: Context<'info, SolendDeposit<'info>>,
    amount: u64,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    // Leaked to get a true `'info` borrow (the bump allocator never frees, so this costs nothing).
    let protocol_accounts = ctx.accounts.protocol_accounts().leak();
    integration::integration_deposit_impl(
        &common,
        protocol_accounts,
        amount,
        Some(ASSET_TAG_SOLEND),
        false,
    )
}

#[derive(Accounts)]
pub struct SolendDeposit<'info> {
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
        } @ MarginfiError::AccountDisabled,
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
        has_one = integration_acc_1 @ MarginfiError::InvalidSolendReserve,
        has_one = integration_acc_2 @ MarginfiError::InvalidSolendObligation,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_solend_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForSolendOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Owned by authority, the source account for the token deposit.
    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The bank's liquidity vault authority, which owns the Solend obligation
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Used as an intermediary to deposit tokens into Solend
    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// The Solend obligation account
    /// CHECK: Validated in the integration handler
    #[account(mut)]
    pub integration_acc_2: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    pub lending_market: UncheckedAccount<'info>,

    /// Derived from the lending market
    /// CHECK: validated by the Solend program
    pub lending_market_authority: UncheckedAccount<'info>,

    /// The Solend reserve that holds liquidity
    #[account(mut)]
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
    /// liquidity_vault_authority that will hold cTokens between deposit and obligation update.
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub user_collateral: UncheckedAccount<'info>,

    /// Oracle accounts - required by Solend even if not actively used
    /// CHECK: validated by the Solend program
    pub pyth_price: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    pub switchboard_feed: UncheckedAccount<'info>,

    /// CHECK: validated against hardcoded program id
    #[account(address = SOLEND_PROGRAM_ID)]
    pub solend_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl_common_deposit!(SolendDeposit);

impl<'info> SolendDeposit<'info> {
    fn protocol_accounts(&self) -> Vec<AccountInfo<'info>> {
        vec![
            self.integration_acc_2.to_account_info(),
            self.lending_market.to_account_info(),
            self.lending_market_authority.to_account_info(),
            self.integration_acc_1.to_account_info(),
            self.reserve_liquidity_supply.to_account_info(),
            self.reserve_collateral_mint.to_account_info(),
            self.reserve_collateral_supply.to_account_info(),
            self.user_collateral.to_account_info(),
            self.pyth_price.to_account_info(),
            self.switchboard_feed.to_account_info(),
            self.solend_program.to_account_info(),
        ]
    }
}
