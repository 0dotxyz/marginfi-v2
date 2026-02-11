use crate::{
    bank_signer, state::bank::BankVaultType, utils::is_juplend_asset_tag, MarginfiError,
    MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};
use juplend_mocks::juplend_earn::cpi::accounts::Deposit;
use juplend_mocks::juplend_earn::cpi::deposit;
use juplend_mocks::state::Lending as JuplendLending;
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::types::{Bank, BankOperationalState};

/// Initialize the bank-level JupLend position used by marginfi.
///
/// This instruction:
/// 1. Transfers a small amount of underlying from the fee payer into the bank liquidity vault
/// 2. Performs a `deposit` CPI into JupLend (seed deposit)
/// 3. Flips the bank operational state from `Paused` -> `Operational`
///
/// Notes:
/// - The fToken ATA is created during `add_pool`, not here.
/// - The seed deposit is intentionally not credited to any marginfi account.
/// - The bank is created in `Paused` state; this instruction is what activates it.
pub fn juplend_init_position(ctx: Context<JuplendInitPosition>, amount: u64) -> MarginfiResult {
    // Require minimum seed deposit amount (same as other integrations)
    require_gte!(
        amount,
        10,
        MarginfiError::JuplendInitPositionDepositInsufficient
    );

    // Transfer underlying tokens from fee payer -> liquidity vault
    ctx.accounts.cpi_transfer_user_to_liquidity_vault(amount)?;

    // Deposit into JupLend via CPI: liquidity_vault -> f_token_vault
    let authority_bump = ctx.accounts.bank.load()?.liquidity_vault_authority_bump;
    ctx.accounts.cpi_juplend_deposit(amount, authority_bump)?;

    // Activate the bank (operational).
    {
        let mut bank = ctx.accounts.bank.load_mut()?;
        bank.config.operational_state = BankOperationalState::Operational;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct JuplendInitPosition<'info> {
    /// Pays for the ATA creation (if needed) and provides a nominal deposit amount.
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// Token account owned by the fee payer holding the underlying mint.
    #[account(
        mut,
        token::mint = mint,
        token::authority = fee_payer,
    )]
    pub signer_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        has_one = integration_acc_1 @ MarginfiError::InvalidJuplendLending,
        has_one = integration_acc_2 @ MarginfiError::InvalidJuplendFTokenVault,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_juplend_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForJuplendOperation,
        constraint = bank.load()?.config.operational_state == BankOperationalState::Paused
            @ MarginfiError::JuplendBankAlreadyActivated
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// The bank's liquidity vault authority PDA (acts as signer for JupLend CPIs).
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Bank liquidity vault (holds underlying mint and is used as depositor_token_account).
    #[account(mut)]
    pub liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Underlying mint (must match bank mint and JupLend lending state mint).
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// JupLend lending state account.
    #[account(mut, has_one = f_token_mint @ MarginfiError::InvalidJuplendLending)]
    pub integration_acc_1: AccountLoader<'info, JuplendLending>,

    /// JupLend fToken mint.
    #[account(mut)]
    pub f_token_mint: Box<InterfaceAccount<'info, Mint>>,

    /// Bank's fToken vault (validated via has_one on bank).
    #[account(mut)]
    pub integration_acc_2: Box<InterfaceAccount<'info, TokenAccount>>,

    // ---- JupLend CPI accounts ----
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
    /// NOTE: JupLend marks this as writable in their Lending program (unusual for a program account)
    #[account(mut)]
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

impl<'info> JuplendInitPosition<'info> {
    pub fn cpi_transfer_user_to_liquidity_vault(&self, amount: u64) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.signer_token_account.to_account_info(),
            to: self.liquidity_vault.to_account_info(),
            authority: self.fee_payer.to_account_info(),
            mint: self.mint.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(program, accounts);
        transfer_checked(cpi_ctx, amount, self.mint.decimals)?;
        Ok(())
    }

    pub fn cpi_juplend_deposit(&self, amount: u64, authority_bump: u8) -> MarginfiResult {
        let accounts = Deposit {
            signer: self.liquidity_vault_authority.to_account_info(),
            depositor_token_account: self.liquidity_vault.to_account_info(),
            recipient_token_account: self.integration_acc_2.to_account_info(),
            mint: self.mint.to_account_info(),
            lending_admin: self.lending_admin.to_account_info(),
            lending: self.integration_acc_1.to_account_info(),
            f_token_mint: self.f_token_mint.to_account_info(),
            supply_token_reserves_liquidity: self.supply_token_reserves_liquidity.to_account_info(),
            lending_supply_position_on_liquidity: self
                .lending_supply_position_on_liquidity
                .to_account_info(),
            rate_model: self.rate_model.to_account_info(),
            vault: self.vault.to_account_info(),
            liquidity: self.liquidity.to_account_info(),
            liquidity_program: self.liquidity_program.to_account_info(),
            rewards_rate_model: self.rewards_rate_model.to_account_info(),
            token_program: self.token_program.to_account_info(),
            associated_token_program: self.associated_token_program.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };

        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);

        let cpi_ctx = CpiContext::new_with_signer(
            self.juplend_program.to_account_info(),
            accounts,
            signer_seeds,
        );

        deposit(cpi_ctx, amount)?;
        Ok(())
    }
}
