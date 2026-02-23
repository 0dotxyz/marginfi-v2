use super::borrow_common;
use crate::{
    prelude::{MarginfiError, MarginfiResult},
    state::{
        bank::BankImpl,
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::is_marginfi_asset_tag,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use marginfi_type_crate::{
    constants::{LIQUIDITY_VAULT_AUTHORITY_SEED, TOKENLESS_REPAYMENTS_ALLOWED},
    types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_IN_FLASHLOAN},
};

/// Flashloan-only variant of `lending_account_borrow`:
/// - Requires account to be in flashloan mode.
/// - Passes `group` as readonly and never mutates group rate-limiter state.
pub fn lending_account_borrow_flashloan<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingAccountBorrowFlashloan<'info>>,
    amount: u64,
) -> MarginfiResult {
    borrow_common::lending_account_borrow_common(
        &ctx.accounts.marginfi_account,
        &ctx.accounts.group,
        &ctx.accounts.bank,
        &ctx.accounts.destination_token_account,
        &ctx.accounts.liquidity_vault,
        &ctx.accounts.bank_liquidity_vault_authority,
        &ctx.accounts.token_program,
        &ctx.accounts.authority,
        &mut ctx.remaining_accounts,
        amount,
        false,
    )
}

#[derive(Accounts)]
pub struct LendingAccountBorrowFlashloan<'info> {
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
        } @ MarginfiError::Unauthorized,
        constraint = marginfi_account.load()?.get_flag(ACCOUNT_IN_FLASHLOAN)
            @ MarginfiError::IllegalFlashloan
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
