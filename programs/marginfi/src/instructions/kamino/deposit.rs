use crate::{
    check,
    instructions::integration::{self, impl_common_deposit},
    state::{
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::is_kamino_asset_tag,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use kamino_mocks::state::{MinimalObligation, MinimalReserve};
use marginfi_type_crate::constants::{ASSET_TAG_KAMINO, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::pdas::{FARMS_PROGRAM_ID, KAMINO_PROGRAM_ID};
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED};

/// Deposit into a Kamino pool through a marginfi account
///
/// This function performs the following steps:
/// 1. Transfers tokens from the user's source account to the obligation owner's account
/// 2. Deposits the tokens into Kamino through a CPI call
/// 3. Verifies the obligation deposit amount was increased correctly
/// 4. Updates the marginfi account's balance to reflect the deposit
pub fn kamino_deposit<'info>(
    ctx: Context<'info, KaminoDeposit<'info>>,
    amount: u64,
    refresh_reserve: Option<bool>,
) -> MarginfiResult {
    // `protocol_accounts` flattens the optional farm accounts positionally, so a reserve farm
    // state without the obligation farm user state would land in the wrong slot.
    check!(
        ctx.accounts.obligation_farm_user_state.is_some()
            || ctx.accounts.reserve_farm_state.is_none(),
        MarginfiError::KaminoObligationFarmUserStateMissing
    );
    let common = ctx.accounts.to_common();
    // Leaked to get a true `'info` borrow (the bump allocator never frees, so this costs nothing).
    let protocol_accounts = ctx.accounts.protocol_accounts().leak();
    integration::integration_deposit_impl(
        &common,
        protocol_accounts,
        amount,
        Some(ASSET_TAG_KAMINO),
        refresh_reserve.unwrap_or(false),
    )
}

#[derive(Accounts)]
pub struct KaminoDeposit<'info> {
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
        has_one = integration_acc_1 @ MarginfiError::InvalidKaminoReserve,
        has_one = integration_acc_2 @ MarginfiError::InvalidKaminoObligation,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_kamino_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForKaminoInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Owned by authority, the source account for the token deposit.
    /// CHECK: Mint and owner are checked at transfer time
    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The bank's liquidity vault authority, which owns the Kamino obligation. Note: Kamino needs
    /// this to be mut because `deposit` might return the rent here
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Used as an intermediary to deposit token into Kamino
    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// The Kamino obligation owned by liquidity_vault_authority
    #[account(mut)]
    pub integration_acc_2: AccountLoader<'info, MinimalObligation>,

    /// CHECK: validated by the Kamino program
    pub lending_market: UncheckedAccount<'info>,

    /// CHECK: validated by the Kamino program
    pub lending_market_authority: UncheckedAccount<'info>,

    /// The Kamino reserve that holds liquidity
    #[account(mut)]
    pub integration_acc_1: AccountLoader<'info, MinimalReserve>,

    /// Bank's liquidity token mint (e.g., USDC). Kamino calls this the `reserve_liquidity_mint`
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub reserve_liquidity_supply: UncheckedAccount<'info>,

    /// The reserve's mint for tokenized representations of Kamino deposits.
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,

    /// The reserve's destination for tokenized representations of deposits. Note: the
    /// `reserve_collateral_mint` will mint tokens directly to this account.
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub reserve_destination_deposit_collateral: UncheckedAccount<'info>,

    /// Required if the Kamino reserve has an active farm.
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub obligation_farm_user_state: Option<UncheckedAccount<'info>>,

    /// Required if the Kamino reserve has an active farm.
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub reserve_farm_state: Option<UncheckedAccount<'info>>,

    /// CHECK: validated against hardcoded program id
    #[account(address = KAMINO_PROGRAM_ID)]
    pub kamino_program: UncheckedAccount<'info>,

    /// Farms program for Kamino staking functionality
    /// CHECK: validated against hardcoded program id
    #[account(address = FARMS_PROGRAM_ID)]
    pub farms_program: UncheckedAccount<'info>,

    pub collateral_token_program: Program<'info, Token>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    /// CHECK: validated against hardcoded program id
    #[account(address = solana_instructions_sysvar::ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,
}

impl_common_deposit!(KaminoDeposit, liquidity_token_program);

impl<'info> KaminoDeposit<'info> {
    fn protocol_accounts(&self) -> Vec<AccountInfo<'info>> {
        let mut accounts = vec![
            self.integration_acc_2.to_account_info(),
            self.lending_market.to_account_info(),
            self.lending_market_authority.to_account_info(),
            self.integration_acc_1.to_account_info(),
            self.reserve_liquidity_supply.to_account_info(),
            self.reserve_collateral_mint.to_account_info(),
            self.reserve_destination_deposit_collateral
                .to_account_info(),
            self.kamino_program.to_account_info(),
            self.farms_program.to_account_info(),
            self.collateral_token_program.to_account_info(),
            self.instruction_sysvar_account.to_account_info(),
        ];
        if let Some(ref account) = self.obligation_farm_user_state {
            accounts.push(account.to_account_info());
        }
        if let Some(ref account) = self.reserve_farm_state {
            accounts.push(account.to_account_info());
        }
        accounts
    }
}
