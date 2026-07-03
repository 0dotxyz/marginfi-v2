use crate::{
    instructions::integration::{self, impl_common_withdraw},
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
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_DELEVERAGE,
    ACCOUNT_IN_RECEIVERSHIP,
};

/// Withdraw underlying tokens from a JupLend lending pool through a marginfi account.
///
/// Flow (program-first, exact-math):
/// 1. CPI `update_rate` to refresh `token_exchange_price`.
/// 2. Compute expected fTokens burned: `ceil(assets * 1e12 / token_exchange_price)`.
/// 3. Call `bank_account.withdraw()` for the expected burned shares.
/// 4. CPI `withdraw` (burn fTokens, receive underlying into withdraw intermediary ATA).
/// 5. Verify received underlying == requested and burned fTokens == expected.
/// 6. Transfer underlying from withdraw intermediary ATA -> destination token account.
/// 7. Update health cache (unless receivership).
pub fn juplend_withdraw<'info>(
    ctx: Context<'info, JuplendWithdraw<'info>>,
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
        Some(ASSET_TAG_JUPLEND),
        false,
    )
}

#[derive(Accounts)]
pub struct JuplendWithdraw<'info> {
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

    /// Token account that will receive the underlying withdrawal.
    /// WARN: Completely unchecked!
    #[account(mut)]
    pub destination_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).
    /// NOTE: JupLend marks the signer as writable in their withdraw instruction.
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

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

    /// Withdraw intermediary ATA (authority = liquidity_vault_authority).
    /// This must be an ATA to satisfy JupLend's withdraw constraints.
    #[account(
        mut,
        token::mint = mint,
        token::authority = liquidity_vault_authority,
        token::token_program = token_program,
    )]
    pub integration_acc_3: Box<InterfaceAccount<'info, TokenAccount>>,

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

    /// JupLend claim account for liquidity_vault_authority.
    /// TEMPORARY: Mainnet currently requires this account (passing None causes ConstraintMut errors),
    /// but an upcoming upgrade is expected to make it truly optional. The account is never actually
    /// validated or used - you can pass any mutable account. We create the canonical PDA for consistency.
    /// Seeds: ["user_claim", liquidity_vault_authority, mint] on Liquidity program.
    /// CHECK: not validated by JupLend - any mutable account works
    #[account(mut)]
    pub claim_account: UncheckedAccount<'info>,

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

// JupLend withdrawals flow through the intermediary ATA (integration_acc_3), which stands in for
// the liquidity vault in the common accounts.
impl_common_withdraw!(JuplendWithdraw, integration_acc_3, token_program);

impl<'info> JuplendWithdraw<'info> {
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
