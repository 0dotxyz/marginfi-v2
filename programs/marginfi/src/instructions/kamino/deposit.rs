use crate::state::bank::BankVaultType;
use crate::{
    bank_signer, check,
    constants::{FARMS_PROGRAM_ID, KAMINO_PROGRAM_ID},
    events::{AccountEventHeader, LendingAccountDepositEvent},
    optional_account,
    state::{
        bank::BankImpl,
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::is_kamino_asset_tag,
    utils::{assert_within_one_token, validate_asset_tags, validate_bank_state, InstructionKind},
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_lang::solana_program::sysvar::{self, Sysvar};
use anchor_spl::token::Token;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use fixed::types::I80F48;
use kamino_mocks::kamino_lending::cpi::deposit_reserve_liquidity_and_obligation_collateral_v2;
use kamino_mocks::{
    kamino_lending::cpi::accounts::{
        DepositReserveLiquidityAndObligationCollateral,
        DepositReserveLiquidityAndObligationCollateralV2, SocializeLossV2FarmsAccounts,
    },
    state::{MinimalObligation, MinimalReserve},
};
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::types::{
    Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP,
};

/// Deposit into a Kamino pool through a marginfi account
///
/// This function performs the following steps:
/// 1. Transfers tokens from the user's source account to the obligation owner's account
/// 2. Deposits the tokens into Kamino through a CPI call
/// 3. Verifies the obligation deposit amount was increased correctly
/// 4. Updates the marginfi account's balance to reflect the deposit
pub fn kamino_deposit(ctx: Context<KaminoDeposit>, amount: u64) -> MarginfiResult {
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
        ctx.accounts.integration_acc_2.load()?.deposits[0].deposited_amount;
    let expected_collateral_amount = ctx
        .accounts
        .integration_acc_1
        .load()?
        .liquidity_to_collateral(amount)?;

    ctx.accounts.cpi_transfer_user_to_obligation_owner(amount)?;
    ctx.accounts.cpi_kamino_deposit(amount, authority_bump)?;

    let final_obligation_deposited_amount =
        ctx.accounts.integration_acc_2.load()?.deposits[0].deposited_amount;

    // Verifying the deposit was successful by checking obligation balance increased by the correct amount
    let obligation_collateral_change =
        final_obligation_deposited_amount - initial_obligation_deposited_amount;
    assert_within_one_token(
        obligation_collateral_change,
        expected_collateral_amount,
        MarginfiError::KaminoDepositFailed,
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

    #[account(
        mut,
        // The first deposit in the obligation is for `integration_acc_1`.
        constraint = {
            let obligation = integration_acc_2.load()?;
            obligation.deposits[0].deposit_reserve == integration_acc_1.key()
        } @ MarginfiError::ObligationDepositReserveMismatch,
        // The rest of the obligation is always empty
        constraint = {
            let obligation = integration_acc_2.load()?;
            obligation.deposits.iter().skip(1).all(|d| d.deposited_amount == 0)
        } @ MarginfiError::InvalidObligationDepositCount
    )]
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
    #[account(address = sysvar::instructions::ID)]
    pub instruction_sysvar_account: UncheckedAccount<'info>,
}

impl<'info> KaminoDeposit<'info> {
    pub fn cpi_transfer_user_to_obligation_owner(&self, amount: u64) -> MarginfiResult {
        let program = self.liquidity_token_program.to_account_info();
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

    pub fn cpi_kamino_deposit(&self, amount: u64, authority_bump: u8) -> MarginfiResult {
        let deposit_accounts = DepositReserveLiquidityAndObligationCollateral {
            owner: self.liquidity_vault_authority.to_account_info(),
            obligation: self.integration_acc_2.to_account_info(),
            lending_market: self.lending_market.to_account_info(),
            lending_market_authority: self.lending_market_authority.to_account_info(),
            reserve: self.integration_acc_1.to_account_info(),
            reserve_liquidity_mint: self.mint.to_account_info(),
            reserve_liquidity_supply: self.reserve_liquidity_supply.to_account_info(),
            reserve_collateral_mint: self.reserve_collateral_mint.to_account_info(),
            reserve_destination_deposit_collateral: self
                .reserve_destination_deposit_collateral
                .to_account_info(),
            user_source_liquidity: self.liquidity_vault.to_account_info(),
            placeholder_user_destination_collateral: None,
            collateral_token_program: self.collateral_token_program.to_account_info(),
            liquidity_token_program: self.liquidity_token_program.to_account_info(),
            instruction_sysvar_account: self.instruction_sysvar_account.to_account_info(),
        };

        // --- optional “farms_accounts” group ---
        let farms_accounts = SocializeLossV2FarmsAccounts {
            obligation_farm_user_state: optional_account!(self.obligation_farm_user_state),
            reserve_farm_state: optional_account!(self.reserve_farm_state),
        };

        // --- wrap both groups in the outer struct ---
        let accounts = DepositReserveLiquidityAndObligationCollateralV2 {
            deposit_reserve_liquidity_and_obligation_collateral_v2_deposit_accounts:
                deposit_accounts,
            deposit_reserve_liquidity_and_obligation_collateral_v2_farms_accounts: farms_accounts,
            farms_program: self.farms_program.to_account_info(),
        };
        let program = self.kamino_program.to_account_info();
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        deposit_reserve_liquidity_and_obligation_collateral_v2(cpi_ctx, amount)?;
        Ok(())
    }
}
