use crate::{
    bank_signer, check,
    constants::SOLEND_PROGRAM_ID,
    events::{AccountEventHeader, LendingAccountDepositEvent},
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::{
        assert_within_one_token, is_solend_asset_tag, validate_asset_tags, validate_bank_state,
        InstructionKind,
    },
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use fixed::types::I80F48;
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP,
};
use solend_mocks::cpi::accounts::DepositReserveLiquidityAndObligationCollateral;
use solend_mocks::cpi::deposit_reserve_liquidity_and_obligation_collateral;
use solend_mocks::state::{
    get_solend_obligation_deposit_amount, validate_solend_obligation, SolendMinimalReserve,
};

/// Deposit into a Solend reserve through a marginfi account
///
/// This function performs the following steps:
/// 1. Transfers tokens from the user's source account to the liquidity vault
/// 2. Deposits the tokens into Solend through a CPI call
/// 3. Verifies the obligation collateral was increased correctly
/// 4. Updates the marginfi account's balance to reflect the deposit
pub fn solend_deposit(ctx: Context<SolendDeposit>, amount: u64) -> MarginfiResult {
    // Forced to validate here as unable to load obligation as ref in constraints
    validate_solend_obligation(
        ctx.accounts.integration_acc_2.as_ref(),
        ctx.accounts.integration_acc_1.key(),
    )?;

    let authority_bump: u8;
    {
        let marginfi_account = ctx.accounts.marginfi_account.load()?;
        let bank = ctx.accounts.bank.load()?;
        authority_bump = bank.liquidity_vault_authority_bump;

        validate_asset_tags(&bank, &marginfi_account)?;
        validate_bank_state(&bank, InstructionKind::FailsIfPausedOrReduceState)?;

        check!(
            !marginfi_account.get_flag(ACCOUNT_DISABLED)
                && !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
            MarginfiError::AccountDisabled
        );
    }

    // Get initial obligation data to verify deposit amount later
    let initial_obligation_deposited_amount =
        get_solend_obligation_deposit_amount(&ctx.accounts.integration_acc_2)?;

    let expected_collateral_amount = ctx
        .accounts
        .integration_acc_1
        .load()?
        .liquidity_to_collateral(amount)?;

    ctx.accounts.cpi_transfer_user_to_liquidity_vault(amount)?;
    ctx.accounts.cpi_solend_deposit(amount, authority_bump)?;

    let final_obligation_deposited_amount =
        get_solend_obligation_deposit_amount(&ctx.accounts.integration_acc_2)?;

    // Verify the deposit was successful by checking obligation balance increased by correct amount
    let obligation_collateral_change =
        final_obligation_deposited_amount - initial_obligation_deposited_amount;

    assert_within_one_token(
        obligation_collateral_change,
        expected_collateral_amount,
        MarginfiError::SolendDepositFailed,
    )?;

    {
        let mut bank = ctx.accounts.bank.load_mut()?;
        let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
        let group = &ctx.accounts.group.load()?;

        let mut bank_account = BankAccountWrapper::find_or_create(
            &ctx.accounts.bank.key(),
            &mut bank,
            &mut marginfi_account.lending_account,
        )?;

        // Convert deposit amount to I80F48 for calculations
        let obligation_collateral_change_i80f48 = I80F48::from_num(obligation_collateral_change);
        bank_account.deposit_no_repay(obligation_collateral_change_i80f48)?;

        // Update bank cache after modifying balances
        bank.update_bank_cache(group)?;

        marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;
        marginfi_account.lending_account.sort_balances();

        emit!(LendingAccountDepositEvent {
            header: AccountEventHeader {
                signer: Some(ctx.accounts.authority.key()),
                marginfi_account: ctx.accounts.marginfi_account.key(),
                marginfi_account_authority: marginfi_account.authority,
                marginfi_group: marginfi_account.group,
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            amount,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct SolendDeposit<'info> {
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
            is_signer_authorized(&a, g.admin, authority.key(), false)
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
            @ MarginfiError::WrongBankAssetTagForSolendOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Owned by authority, the source account for the token deposit.
    #[account(mut)]
    pub signer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// The bank's liquidity vault authority, which owns the Solend obligation
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Used as an intermediary to deposit tokens into Solend
    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    /// The Solend obligation account
    /// CHECK: Validated in instruction body
    #[account(
        mut,
        constraint = integration_acc_2.owner == &SOLEND_PROGRAM_ID @ MarginfiError::InvalidSolendAccount
    )]
    pub integration_acc_2: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    pub lending_market: UncheckedAccount<'info>,

    /// Derived from the lending market
    /// CHECK: validated by the Solend program
    pub lending_market_authority: UncheckedAccount<'info>,

    /// The Solend reserve that holds liquidity
    #[account(
        mut,
        constraint = !integration_acc_1.load()?.is_stale()? @ MarginfiError::SolendReserveStale
    )]
    pub integration_acc_1: AccountLoader<'info, SolendMinimalReserve>,

    /// Bank's liquidity token mint (e.g., USDC)
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// Reserve's liquidity supply SPL Token account
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The reserve's mint for cTokens
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,

    /// The reserve's collateral supply account (where cTokens are stored)
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub reserve_collateral_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The user's destination for cTokens (collateral). This is a temporary account owned by
    /// liquidity_vault_authority that will hold cTokens between deposit and obligation update.
    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub user_collateral: UncheckedAccount<'info>,

    /// Oracle accounts - required by Solend even if not actively used
    /// CHECK: validated by the Solend program
    pub pyth_price: UncheckedAccount<'info>,
    /// CHECK: validated by the Solend program
    pub switchboard_feed: UncheckedAccount<'info>,

    /// CHECK: validated against hardcoded program id
    #[account(address = SOLEND_PROGRAM_ID)]
    pub solend_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> SolendDeposit<'info> {
    pub fn cpi_transfer_user_to_liquidity_vault(&self, amount: u64) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.signer_token_account.to_account_info(),
            to: self.liquidity_vault.to_account_info(),
            authority: self.authority.to_account_info(),
            mint: self.mint.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(program, accounts);
        let decimals = self.mint.decimals;
        transfer_checked(cpi_ctx, amount, decimals)?;
        Ok(())
    }

    pub fn cpi_solend_deposit(&self, amount: u64, authority_bump: u8) -> MarginfiResult {
        let accounts = DepositReserveLiquidityAndObligationCollateral {
            source_liquidity_info: self.liquidity_vault.to_account_info(),
            user_collateral_info: self.user_collateral.to_account_info(),
            reserve_info: self.integration_acc_1.to_account_info(),
            reserve_liquidity_supply_info: self.reserve_liquidity_supply.to_account_info(),
            reserve_collateral_mint_info: self.reserve_collateral_mint.to_account_info(),
            lending_market_info: self.lending_market.to_account_info(),
            lending_market_authority_info: self.lending_market_authority.to_account_info(),
            destination_deposit_collateral_info: self.reserve_collateral_supply.to_account_info(),
            obligation_info: self.integration_acc_2.to_account_info(),
            obligation_owner_info: self.liquidity_vault_authority.to_account_info(),
            pyth_price_info: self.pyth_price.to_account_info(),
            switchboard_feed_info: self.switchboard_feed.to_account_info(),
            user_transfer_authority_info: self.liquidity_vault_authority.to_account_info(),
            token_program_info: self.token_program.to_account_info(),
        };
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);

        // Create CPI context with signer
        let cpi_ctx = CpiContext::new_with_signer(
            self.solend_program.to_account_info(),
            accounts,
            signer_seeds,
        );
        deposit_reserve_liquidity_and_obligation_collateral(cpi_ctx, amount)?;
        Ok(())
    }
}
