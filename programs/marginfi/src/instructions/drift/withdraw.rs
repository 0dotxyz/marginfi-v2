use crate::{
    instructions::integration::{self, impl_common_withdraw},
    ix_utils::{get_discrim_hash, Hashable},
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
use fixed::types::I80F48;
use marginfi_type_crate::constants::{ASSET_TAG_DRIFT, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::pdas::DRIFT_PROGRAM_ID;
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_DELEVERAGE,
    ACCOUNT_IN_RECEIVERSHIP,
};

pub fn drift_withdraw<'info>(
    ctx: Context<'info, DriftWithdraw<'info>>,
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
        Some(ASSET_TAG_DRIFT),
        false,
    )
}

#[derive(Accounts)]
pub struct DriftWithdraw<'info> {
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
            is_signer_authorized(&a, g.admin, authority.key(), true, true)
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
            @ MarginfiError::WrongBankAssetTagForDriftOperation,
        constraint = {
            let a = marginfi_account.load()?;
            let b = bank.load()?;
            let weight: I80F48 = b.config.asset_weight_init.into();
            !(a.get_flag(ACCOUNT_IN_RECEIVERSHIP) && weight == I80F48::ZERO)
        } @ MarginfiError::LiquidationPremiumTooHigh
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
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: validated by Drift program
    pub drift_state: UncheckedAccount<'info>,

    // Spot position, reward accounts, bricked state, and spot market mint are validated in the
    // integration handler.
    #[account(mut)]
    pub integration_acc_2: AccountLoader<'info, MinimalUser>,

    /// CHECK: validated by Drift program
    #[account(mut)]
    pub integration_acc_3: UncheckedAccount<'info>,

    #[account(mut)]
    pub integration_acc_1: AccountLoader<'info, drift_mocks::state::MinimalSpotMarket>,

    /// CHECK: validated by Drift program
    #[account(mut)]
    pub drift_spot_market_vault: UncheckedAccount<'info>,

    /// CHECK: validated by Drift program
    pub drift_reward_oracle: Option<UncheckedAccount<'info>>,

    /// CHECK: validated by Drift program
    pub drift_reward_spot_market: Option<UncheckedAccount<'info>>,

    /// CHECK: validated by Drift program
    pub drift_reward_mint: Option<UncheckedAccount<'info>>,

    /// CHECK: validated by Drift program
    pub drift_reward_oracle_2: Option<UncheckedAccount<'info>>,

    /// CHECK: validated by Drift program
    pub drift_reward_spot_market_2: Option<UncheckedAccount<'info>>,

    /// CHECK: validated by Drift program
    pub drift_reward_mint_2: Option<UncheckedAccount<'info>>,

    /// CHECK: validated by Drift program
    pub drift_signer: UncheckedAccount<'info>,

    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: validated against hardcoded program id
    #[account(address = DRIFT_PROGRAM_ID)]
    pub drift_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// Resolves an optional account to its `AccountInfo`, or the system program sentinel expected by
/// `optional_account` when absent.
fn opt_or<'info>(
    opt: &Option<UncheckedAccount<'info>>,
    sentinel: AccountInfo<'info>,
) -> AccountInfo<'info> {
    opt.as_ref()
        .map(|a| a.to_account_info())
        .unwrap_or(sentinel)
}

impl_common_withdraw!(DriftWithdraw);

impl<'info> DriftWithdraw<'info> {
    fn protocol_accounts(&self) -> Vec<AccountInfo<'info>> {
        let sentinel = self.system_program.to_account_info();
        vec![
            self.drift_state.to_account_info(),
            self.integration_acc_2.to_account_info(),
            self.integration_acc_3.to_account_info(),
            self.integration_acc_1.to_account_info(),
            self.drift_spot_market_vault.to_account_info(),
            self.drift_signer.to_account_info(),
            self.drift_program.to_account_info(),
            self.system_program.to_account_info(),
            opt_or(&self.drift_oracle, sentinel.clone()),
            opt_or(&self.drift_reward_oracle, sentinel.clone()),
            opt_or(&self.drift_reward_oracle_2, sentinel.clone()),
            opt_or(&self.drift_reward_spot_market, sentinel.clone()),
            opt_or(&self.drift_reward_spot_market_2, sentinel.clone()),
            opt_or(&self.drift_reward_mint, sentinel.clone()),
            opt_or(&self.drift_reward_mint_2, sentinel),
        ]
    }
}

impl Hashable for DriftWithdraw<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "drift_withdraw")
    }
}
