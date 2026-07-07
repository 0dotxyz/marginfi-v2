use crate::{
    constants::SOLEND_PROGRAM_ID,
    instructions::integration::{self, impl_common_withdraw},
    ix_utils::{get_discrim_hash, Hashable},
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
use fixed::types::I80F48;
use marginfi_type_crate::constants::{ASSET_TAG_SOLEND, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_DELEVERAGE,
    ACCOUNT_IN_RECEIVERSHIP,
};
use solend_mocks::state::SolendMinimalReserve;

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
    ctx: Context<'info, SolendWithdraw<'info>>,
    amount: u64,
    withdraw_all: Option<bool>,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    // Leaked to get a true `'info` borrow (the bump allocator never frees, so this costs nothing).
    let protocol_accounts = ctx.accounts.protocol_accounts().leak();
    integration::integration_withdraw_impl(
        &common,
        protocol_accounts,
        ctx.remaining_accounts,
        amount,
        withdraw_all,
        Some(ASSET_TAG_SOLEND),
        false,
    )
}

#[derive(Accounts)]
pub struct SolendWithdraw<'info> {
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
            is_signer_authorized(&a, g.admin, authority.key(), true, false)
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
        constraint = {
            let a = marginfi_account.load()?;
            let b = bank.load()?;
            let weight: I80F48 = b.config.asset_weight_init.into();
            !(a.get_flag(ACCOUNT_IN_RECEIVERSHIP) && weight == I80F48::ZERO)
        } @ MarginfiError::LiquidationPremiumTooHigh
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Token account that will receive the withdrawn tokens. Mint/owner are validated by the
    /// SPL transfer; the caller controls the destination.
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
    /// CHECK: Validated in the integration handler
    #[account(mut)]
    pub integration_acc_2: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    #[account(mut)]
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
    /// liquidity_vault_authority that holds cTokens.
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub user_collateral: UncheckedAccount<'info>,

    /// CHECK: validated against hardcoded program id
    #[account(address = SOLEND_PROGRAM_ID)]
    pub solend_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl_common_withdraw!(SolendWithdraw);

impl<'info> SolendWithdraw<'info> {
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
            self.solend_program.to_account_info(),
        ]
    }
}

impl Hashable for SolendWithdraw<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "solend_withdraw")
    }
}
