use crate::{
    instructions::integration::{self, CommonDeposit},
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

pub fn juplend_deposit<'info>(
    ctx: Context<'_, '_, 'info, 'info, JuplendDeposit<'info>>,
    amount: u64,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    let protocol_accounts = ctx.accounts.protocol_accounts();
    let protocol_accounts = integration::account_info_slice(&protocol_accounts);
    integration::integration_deposit_impl(
        &common,
        protocol_accounts,
        amount,
        Some(ASSET_TAG_JUPLEND),
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

    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, has_one = f_token_mint @ MarginfiError::InvalidJuplendLending)]
    pub integration_acc_1: AccountLoader<'info, JuplendLending>,

    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub integration_acc_2: InterfaceAccount<'info, TokenAccount>,

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
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> JuplendDeposit<'info> {
    fn to_common(&self) -> CommonDeposit<'_, 'info> {
        CommonDeposit {
            group: &self.group,
            marginfi_account: &self.marginfi_account,
            authority: &self.authority,
            bank: &self.bank,
            signer_token_account: self.signer_token_account.to_account_info(),
            liquidity_vault_authority: self.liquidity_vault_authority.to_account_info(),
            liquidity_vault: self.liquidity_vault.to_account_info(),
            mint: self.mint.to_account_info(),
            mint_decimals: self.mint.decimals,
            token_program: self.token_program.to_account_info(),
        }
    }

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
