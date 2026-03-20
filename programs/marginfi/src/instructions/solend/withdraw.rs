use crate::{
    constants::SOLEND_PROGRAM_ID,
    instructions::integration::{self, CommonWithdraw},
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
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP,
};
use solend_mocks::state::SolendMinimalReserve;

pub fn solend_withdraw<'info>(
    ctx: Context<'_, '_, 'info, 'info, SolendWithdraw<'info>>,
    amount: u64,
    withdraw_all: Option<bool>,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    let protocol_accounts = ctx.accounts.protocol_accounts();
    let protocol_accounts = integration::account_info_slice(&protocol_accounts);
    integration::integration_withdraw_impl(
        &common,
        protocol_accounts,
        ctx.remaining_accounts,
        amount,
        withdraw_all,
        Some(ASSET_TAG_SOLEND),
    )
}

#[derive(Accounts)]
pub struct SolendWithdraw<'info> {
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

    /// CHECK: validated in instruction body
    #[account(
        mut,
        constraint = integration_acc_2.owner == &SOLEND_PROGRAM_ID @ MarginfiError::InvalidSolendAccount
    )]
    pub integration_acc_2: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub lending_market: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    pub lending_market_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = !integration_acc_1.load()?.is_stale()? @ MarginfiError::SolendReserveStale
    )]
    pub integration_acc_1: AccountLoader<'info, SolendMinimalReserve>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_collateral_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub user_collateral: UncheckedAccount<'info>,

    /// CHECK: validated against hardcoded program id
    #[account(address = SOLEND_PROGRAM_ID)]
    pub solend_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> SolendWithdraw<'info> {
    fn to_common(&self) -> CommonWithdraw<'_, 'info> {
        CommonWithdraw {
            group: &self.group,
            marginfi_account: &self.marginfi_account,
            authority: &self.authority,
            bank: &self.bank,
            destination_token_account: self.destination_token_account.to_account_info(),
            liquidity_vault_authority: self.liquidity_vault_authority.to_account_info(),
            liquidity_vault: self.liquidity_vault.to_account_info(),
            mint: self.mint.to_account_info(),
            mint_decimals: self.mint.decimals,
            token_program: self.token_program.to_account_info(),
        }
    }

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
