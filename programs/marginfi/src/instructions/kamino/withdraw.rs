use crate::{
    check,
    instructions::integration::{self, impl_common_withdraw},
    ix_utils::{get_discrim_hash, Hashable},
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
use fixed::types::I80F48;
use kamino_mocks::state::{MinimalObligation, MinimalReserve};
use marginfi_type_crate::constants::{
    ASSET_TAG_KAMINO, LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED,
};
use marginfi_type_crate::pdas::{FARMS_PROGRAM_ID, KAMINO_PROGRAM_ID};
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_DELEVERAGE,
    ACCOUNT_IN_RECEIVERSHIP,
};

/// Withdraw from a Kamino reserve through a marginfi account
///
/// # Important Note on Token Amounts:
/// The `amount` parameter is specified in terms of COLLATERAL tokens, not the underlying
/// liquidity tokens (e.g., USDC). This is important for users to understand.
///
/// Collateral tokens represent shares in the Kamino reserve. When withdrawing:
///
/// 1. The user specifies how many collateral tokens they want to withdraw.
///
/// 2. Kamino calculates the corresponding amount of liquidity tokens (e.g., USDC)
///    to return based on the current exchange rate in the Kamino reserve.
///
/// 3. If a user wants to withdraw a specific amount of liquidity tokens, they need
///    to calculate the required collateral tokens themselves using the reserve's current
///    exchange rate before making the withdrawal request.
///
/// 4. For withdrawing an entire position, use the `withdraw_all` option instead of
///    trying to calculate the exact amount.
///
/// This function performs the following steps:
/// 1. Gets the user's asset shares and initial obligation data
/// 2. Calculates the appropriate number of collateral tokens to withdraw
/// 3. Performs a CPI call to Kamino to withdraw tokens
/// 4. Verifies the obligation deposit amount was reduced correctly
/// 5. Transfers funds to the user's account
/// 6. Updates the marginfi account's balance to reflect the withdrawal
pub fn kamino_withdraw<'info>(
    ctx: Context<'info, KaminoWithdraw<'info>>,
    amount: u64,
    flags: Option<u8>,
) -> MarginfiResult {
    // bit 0: withdraw all; bit 1: refresh reserve via batch refresh
    let flags = flags.unwrap_or(0);
    let withdraw_all = flags & 1 != 0;
    let refresh_reserve = flags & 2 != 0;
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
    integration::integration_withdraw_impl(
        &common,
        protocol_accounts,
        ctx.remaining_accounts,
        amount,
        Some(withdraw_all),
        Some(ASSET_TAG_KAMINO),
        refresh_reserve,
    )
}

#[derive(Accounts)]
pub struct KaminoWithdraw<'info> {
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
        has_one = integration_acc_1 @ MarginfiError::InvalidKaminoReserve,
        has_one = integration_acc_2 @ MarginfiError::InvalidKaminoObligation,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_kamino_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForKaminoInstructions,
        constraint = {
            let a = marginfi_account.load()?;
            let b = bank.load()?;
            let weight: I80F48 = b.config.asset_weight_init.into();
            !(a.get_flag(ACCOUNT_IN_RECEIVERSHIP) && weight == I80F48::ZERO)
        } @MarginfiError::LiquidationPremiumTooHigh
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Token account that will receive the withdrawn tokens. Mint/owner are validated by the
    /// SPL transfer; the caller controls the destination.
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

    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_bump,
    )]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// The Kamino obligation owned by liquidity_vault_authority
    #[account(mut)]
    pub integration_acc_2: AccountLoader<'info, MinimalObligation>,

    /// The Kamino lending market
    /// CHECK: This is validated by the Kamino program
    pub lending_market: UncheckedAccount<'info>,

    /// The Kamino lending market authority
    /// CHECK: This is validated by the Kamino program
    pub lending_market_authority: UncheckedAccount<'info>,

    /// The Kamino reserve that holds liquidity
    #[account(mut)]
    pub integration_acc_1: AccountLoader<'info, MinimalReserve>,

    /// The liquidity token mint (e.g., USDC)
    /// Needs serde to get the mint decimals for transfer checked
    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// The reserve's liquidity supply account
    /// CHECK: This is validated by the Kamino program
    #[account(mut)]
    pub reserve_liquidity_supply: UncheckedAccount<'info>,

    /// The reserve's collateral mint
    /// CHECK: This is validated by the Kamino program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,

    /// The reserve's source for collateral tokens
    /// CHECK: This is validated by the Kamino program
    #[account(mut)]
    pub reserve_source_collateral: UncheckedAccount<'info>,

    /// Optional farms accounts for Kamino staking functionality
    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub obligation_farm_user_state: Option<UncheckedAccount<'info>>,

    /// CHECK: validated by the Kamino program
    #[account(mut)]
    pub reserve_farm_state: Option<UncheckedAccount<'info>>,

    /// CHECK: Use the cfg appropriate kamino program id
    #[account(address = KAMINO_PROGRAM_ID)]
    pub kamino_program: UncheckedAccount<'info>,

    /// Farms program for Kamino staking functionality
    /// CHECK: This is validated by the Kamino program
    #[account(address = FARMS_PROGRAM_ID)]
    pub farms_program: UncheckedAccount<'info>,

    /// The token program for the collateral token
    pub collateral_token_program: Program<'info, Token>,
    /// The token program for the liquidity token
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    /// Used by kamino validate CPI calls
    /// CHECK: read‑only Instructions sysvar
    #[account(address = solana_instructions_sysvar::ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,
}

impl_common_withdraw!(KaminoWithdraw, liquidity_vault, liquidity_token_program);

impl<'info> KaminoWithdraw<'info> {
    fn protocol_accounts(&self) -> Vec<AccountInfo<'info>> {
        let mut accounts = vec![
            self.integration_acc_2.to_account_info(),
            self.lending_market.to_account_info(),
            self.lending_market_authority.to_account_info(),
            self.integration_acc_1.to_account_info(),
            self.reserve_liquidity_supply.to_account_info(),
            self.reserve_collateral_mint.to_account_info(),
            self.reserve_source_collateral.to_account_info(),
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

impl Hashable for KaminoWithdraw<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "kamino_withdraw")
    }
}
