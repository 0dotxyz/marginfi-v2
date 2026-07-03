use crate::{
    instructions::integration::{self, impl_common_deposit},
    state::{
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::is_juplend_asset_tag,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use juplend_mocks::state::Lending as JuplendLending;
use marginfi_type_crate::constants::{ASSET_TAG_JUPLEND, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED};

/// Deposit into a JupLend lending pool through a marginfi account.
///
/// Flow (program-first, exact-math):
/// 1. CPI `update_rate` to refresh `token_exchange_price`.
/// 2. Compute expected fTokens minted: `assets * 1e12 / token_exchange_price` (floor).
/// 3. Transfer underlying from user -> bank liquidity vault.
/// 4. CPI `deposit` (bank vault -> fToken vault).
/// 5. Verify minted fTokens == expected.
/// 6. Credit marginfi asset_shares by minted fTokens.
pub fn juplend_deposit<'info>(
    ctx: Context<'info, JuplendDeposit<'info>>,
    amount: u64,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    // Leaked to get a true `'info` borrow (the bump allocator never frees, so this costs nothing).
    let protocol_accounts = ctx.accounts.protocol_accounts().leak();
    integration::integration_deposit_impl(
        &common,
        protocol_accounts,
        amount,
        Some(ASSET_TAG_JUPLEND),
        false,
    )
}

#[derive(Accounts)]
pub struct JuplendDeposit<'info> {
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
        has_one = integration_acc_1 @ MarginfiError::InvalidJuplendLending,
        has_one = integration_acc_2 @ MarginfiError::InvalidJuplendFTokenVault,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_juplend_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForJuplendOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Owned by authority, the source account for the token deposit.
    #[account(mut)]
    pub signer_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).
    /// NOTE: JupLend marks the signer as writable in their deposit instruction.
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Bank liquidity vault (holds underlying mint and is used as depositor_token_account).
    #[account(mut)]
    pub liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Underlying mint.
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// JupLend lending state account.
    #[account(mut)]
    pub integration_acc_1: AccountLoader<'info, JuplendLending>,

    /// JupLend fToken mint.
    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Bank's fToken vault (validated via has_one on bank).
    #[account(mut)]
    pub integration_acc_2: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: validated by the JupLend program
    pub lending_admin: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    #[account(mut)]
    pub supply_token_reserves_liquidity: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    #[account(mut)]
    pub lending_supply_position_on_liquidity: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    pub rate_model: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    #[account(mut)]
    pub vault: UncheckedAccount<'info>,

    /// CHECK: validated by the JupLend program
    #[account(mut)]
    pub liquidity: UncheckedAccount<'info>,

    /// CHECK: pinned to the JupLend liquidity program
    pub liquidity_program: UncheckedAccount<'info>,

    /// CHECK: cross-checked against integration_acc_1.rewards_rate_model
    pub rewards_rate_model: UncheckedAccount<'info>,

    /// CHECK: validated against hardcoded program id
    #[account(address = juplend_mocks::ID)]
    pub juplend_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl_common_deposit!(JuplendDeposit);

impl<'info> JuplendDeposit<'info> {
    fn protocol_accounts(&self) -> Vec<AccountInfo<'info>> {
        vec![
            self.integration_acc_1.to_account_info(),
            self.f_token_mint.to_account_info(),
            self.integration_acc_2.to_account_info(),
            self.lending_admin.to_account_info(),
            self.supply_token_reserves_liquidity.to_account_info(),
            self.lending_supply_position_on_liquidity.to_account_info(),
            self.rate_model.to_account_info(),
            self.vault.to_account_info(),
            self.liquidity.to_account_info(),
            self.liquidity_program.to_account_info(),
            self.rewards_rate_model.to_account_info(),
            self.juplend_program.to_account_info(),
            self.associated_token_program.to_account_info(),
            self.system_program.to_account_info(),
        ]
    }
}
