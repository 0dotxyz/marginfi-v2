use crate::{
    bank_signer,
    constants::{SOLEND_OBLIGATION_SEED, SOLEND_PROGRAM_ID},
    state::bank::BankVaultType,
    utils::is_solend_asset_tag,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::types::Bank;
use solend_mocks::cpi::accounts::{DepositReserveLiquidityAndObligationCollateral, InitObligation};
use solend_mocks::cpi::{deposit_reserve_liquidity_and_obligation_collateral, init_obligation};
use solend_mocks::state::{SolendMinimalReserve, OBLIGATION_LEN as SOLEND_OBLIGATION_SIZE};

/// Initialize a Solend obligation for a marginfi account
pub fn solend_init_obligation(ctx: Context<SolendInitObligation>, amount: u64) -> MarginfiResult {
    // Arbitrarily setting minimum deposit to 10 absolute units to always keep the obligation alive.
    // Obligations auto close when empty, but we want to keep it open for future deposits.
    require_gte!(amount, 10, MarginfiError::ObligationInitDepositInsufficient);

    let authority_bump = ctx.accounts.bank.load()?.liquidity_vault_authority_bump;

    // Initialize the obligation
    ctx.accounts.cpi_init_obligation(authority_bump)?;

    // Transfer tokens from user (signer_token_account) -> liquidity vault
    ctx.accounts.cpi_transfer_user_to_liquidity_vault(amount)?;

    // Deposit into Solend (liquidity vault) -> (reserve_liquidity_supply)
    ctx.accounts.cpi_solend_deposit(amount, authority_bump)?;

    msg!("Solend obligation initialized with amount: {}", amount);

    Ok(())
}

#[derive(Accounts)]
pub struct SolendInitObligation<'info> {
    /// Pays to init the obligation and pays a nominal amount to ensure the obligation has a
    /// non-zero balance.
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        has_one = integration_acc_1 @ MarginfiError::InvalidSolendReserve,
        has_one = integration_acc_2 @ MarginfiError::InvalidSolendObligation,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_solend_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForSolendOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// The fee payer must provide a nominal amount of bank tokens so the obligation is not empty.
    /// This amount is irrecoverable and will prevent the obligation from ever being closed.
    #[account(
        mut,
        token::mint = mint,
        token::authority = fee_payer,
    )]
    pub signer_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The liquidity vault authority (PDA that will own the Solend obligation)
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Used as an intermediary to deposit a nominal amount of token into the obligation.
    #[account(mut)]
    pub liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: The obligation PDA to be initialized, owned by Solend
    #[account(
        init,
        payer = fee_payer,
        space = SOLEND_OBLIGATION_SIZE,
        owner = SOLEND_PROGRAM_ID,
        seeds = [
            SOLEND_OBLIGATION_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump,
    )]
    pub integration_acc_2: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    pub lending_market: UncheckedAccount<'info>,

    /// Derived from the lending market
    /// CHECK: validated by the Solend program
    pub lending_market_authority: UncheckedAccount<'info>,

    /// CHECK: validated by the Solend program
    #[account(mut)]
    pub integration_acc_1: AccountLoader<'info, SolendMinimalReserve>,

    /// Bank's liquidity token mint (e.g., USDC)
    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

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
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}

impl<'info> SolendInitObligation<'info> {
    pub fn cpi_init_obligation(&self, authority_bump: u8) -> MarginfiResult {
        let accounts = InitObligation {
            obligation_info: self.integration_acc_2.to_account_info(),
            lending_market_info: self.lending_market.to_account_info(),
            obligation_owner_info: self.liquidity_vault_authority.to_account_info(),
            rent_info: self.rent.to_account_info(),
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
        init_obligation(cpi_ctx)?;
        Ok(())
    }

    pub fn cpi_transfer_user_to_liquidity_vault(&self, amount: u64) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.signer_token_account.to_account_info(),
            to: self.liquidity_vault.to_account_info(),
            authority: self.fee_payer.to_account_info(),
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
