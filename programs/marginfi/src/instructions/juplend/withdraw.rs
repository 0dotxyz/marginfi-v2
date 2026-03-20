use crate::{
    instructions::integration::{self, CommonWithdraw},
    ix_utils::{get_discrim_hash, Hashable},
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
use fixed::types::I80F48;
use juplend_mocks::state::Lending as JuplendLending;
use marginfi_type_crate::constants::{ASSET_TAG_JUPLEND, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP,
};

pub fn juplend_withdraw<'info>(
    ctx: Context<'_, '_, 'info, 'info, JuplendWithdraw<'info>>,
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
        Some(ASSET_TAG_JUPLEND),
    )
}

#[derive(Accounts)]
pub struct JuplendWithdraw<'info> {
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
            is_signer_authorized(&a, g.admin, authority.key(), true, true)
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
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, has_one = f_token_mint @ MarginfiError::InvalidJuplendLending)]
    pub integration_acc_1: AccountLoader<'info, JuplendLending>,

    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub integration_acc_2: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = mint,
        token::authority = liquidity_vault_authority,
        token::token_program = token_program,
    )]
    pub integration_acc_3: InterfaceAccount<'info, TokenAccount>,

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

    /// CHECK: not validated by JupLend, but must be mutable
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
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> JuplendWithdraw<'info> {
    fn to_common(&self) -> CommonWithdraw<'_, 'info> {
        CommonWithdraw {
            group: &self.group,
            marginfi_account: &self.marginfi_account,
            authority: &self.authority,
            bank: &self.bank,
            destination_token_account: self.destination_token_account.to_account_info(),
            liquidity_vault_authority: self.liquidity_vault_authority.to_account_info(),
            liquidity_vault: self.integration_acc_3.to_account_info(),
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
            self.integration_acc_3.to_account_info(),
            self.lending_admin.to_account_info(),
            self.supply_token_reserves_liquidity.to_account_info(),
            self.lending_supply_position_on_liquidity.to_account_info(),
            self.rate_model.to_account_info(),
            self.vault.to_account_info(),
            self.claim_account.to_account_info(),
            self.liquidity.to_account_info(),
            self.liquidity_program.to_account_info(),
            self.rewards_rate_model.to_account_info(),
            self.juplend_program.to_account_info(),
            self.associated_token_program.to_account_info(),
            self.system_program.to_account_info(),
        ]
    }
}

impl Hashable for JuplendWithdraw<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "juplend_withdraw")
    }
}
