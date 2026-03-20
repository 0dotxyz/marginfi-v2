use crate::{
    constants::DRIFT_PROGRAM_ID,
    instructions::integration::{self, CommonDeposit},
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
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED};

pub fn drift_deposit<'info>(
    ctx: Context<'_, '_, 'info, 'info, DriftDeposit<'info>>,
    amount: u64,
) -> MarginfiResult {
    let common = ctx.accounts.to_common();
    let protocol_accounts = ctx.accounts.protocol_accounts();
    let protocol_accounts = integration::account_info_slice(&protocol_accounts);
    integration::integration_deposit_impl(&common, protocol_accounts, amount, Some(ASSET_TAG_DRIFT))
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

    /// CHECK: validated by Drift program
    pub drift_oracle: Option<UncheckedAccount<'info>>,

    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: validated by Drift program
    pub drift_state: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = {
            let user = integration_acc_2.load()?;
            let spot_market = integration_acc_1.load()?;
            user.validate_spot_position(spot_market.market_index).is_ok()
        } @ MarginfiError::DriftInvalidSpotPositions
    )]
    pub integration_acc_2: AccountLoader<'info, MinimalUser>,

    /// CHECK: validated by Drift program
    #[account(mut)]
    pub integration_acc_3: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = integration_acc_1.load()?.mint == mint.key()
            @ MarginfiError::DriftSpotMarketMintMismatch
    )]
    pub integration_acc_1: AccountLoader<'info, drift_mocks::state::MinimalSpotMarket>,

    /// CHECK: validated by Drift program
    #[account(mut)]
    pub drift_spot_market_vault: UncheckedAccount<'info>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: validated against hardcoded program id
    #[account(address = DRIFT_PROGRAM_ID)]
    pub drift_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> DriftDeposit<'info> {
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
