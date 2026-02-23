use super::repay_common;
use crate::{
    prelude::{MarginfiError, MarginfiResult},
    state::{
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::is_marginfi_asset_tag,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_IN_FLASHLOAN};

/// Flashloan-only variant of `lending_account_repay`:
/// - Requires account to be in flashloan mode.
/// - Passes `group` as readonly and never mutates group rate-limiter state.
pub fn lending_account_repay_flashloan<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingAccountRepayFlashloan<'info>>,
    amount: u64,
    repay_all: Option<bool>,
) -> MarginfiResult {
    repay_common::lending_account_repay_common(
        &ctx.accounts.marginfi_account,
        &ctx.accounts.group,
        &ctx.accounts.bank,
        &ctx.accounts.signer_token_account,
        &ctx.accounts.liquidity_vault,
        &ctx.accounts.token_program,
        &ctx.accounts.authority,
        &mut ctx.remaining_accounts,
        amount,
        repay_all,
        false,
    )
}

#[derive(Accounts)]
pub struct LendingAccountRepayFlashloan<'info> {
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
            is_signer_authorized(&a, g.admin, authority.key(), true, true)
        } @ MarginfiError::Unauthorized,
        constraint = marginfi_account.load()?.get_flag(ACCOUNT_IN_FLASHLOAN)
            @ MarginfiError::IllegalFlashloan
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
            @ MarginfiError::WrongAssetTagForStandardInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: Token mint/authority are checked at transfer
    #[account(mut)]
    pub signer_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
