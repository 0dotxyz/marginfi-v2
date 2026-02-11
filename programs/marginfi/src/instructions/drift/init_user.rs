use crate::{
    bank_signer,
    constants::{DRIFT_PROGRAM_ID, DRIFT_USER_SEED, DRIFT_USER_STATS_SEED},
    state::bank::BankVaultType,
    utils::is_drift_asset_tag,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use drift_mocks::drift::cpi::accounts::{
    Deposit, InitializeUser, InitializeUserStats, UpdateUserPoolId,
};
use drift_mocks::drift::cpi::{
    deposit, initialize_user, initialize_user_stats, update_user_pool_id,
};
use drift_mocks::state::MinimalSpotMarket;
use marginfi_type_crate::constants::LIQUIDITY_VAULT_AUTHORITY_SEED;
use marginfi_type_crate::types::Bank;

/// Initialize a Drift user and user stats for a marginfi account
/// Sub-account ID is always 0 for the first account
/// Requires a minimum deposit to ensure the account remains active
pub fn drift_init_user(ctx: Context<DriftInitUser>, amount: u64) -> MarginfiResult {
    let authority_bump: u8;
    let market_index: u16;
    let pool_id: u8;
    {
        let bank = ctx.accounts.bank.load()?;
        authority_bump = bank.liquidity_vault_authority_bump;

        let integration_acc_1 = ctx.accounts.integration_acc_1.load()?;
        market_index = integration_acc_1.market_index;
        pool_id = integration_acc_1.pool_id;
    }

    // Arbitrarily setting minimum deposit to 10 absolute units to always keep the account alive.
    require_gte!(amount, 10, MarginfiError::DriftUserInitDepositInsufficient);

    ctx.accounts.cpi_init_user_stats(authority_bump)?;

    ctx.accounts.cpi_init_user(authority_bump)?;

    ctx.accounts.cpi_transfer_user_to_liquidity_vault(amount)?;

    if pool_id != 0 {
        ctx.accounts
            .cpi_update_user_pool_id(pool_id, authority_bump)?;
    }

    ctx.accounts
        .cpi_drift_deposit(amount, market_index, authority_bump)?;

    // Drift user initialized with sub_account_id: 0

    Ok(())
}

#[derive(Accounts)]
pub struct DriftInitUser<'info> {
    /// Pays to init the drift user and user stats accounts and provides initial deposit
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// The fee payer must provide a nominal amount of bank tokens so the account is not empty.
    /// This amount is irrecoverable and will prevent the account from being closed.
    #[account(
        mut,
        token::mint = mint,
        token::authority = fee_payer,
    )]
    pub signer_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        has_one = integration_acc_1 @ MarginfiError::InvalidDriftSpotMarket,
        has_one = integration_acc_2 @ MarginfiError::InvalidDriftUser,
        has_one = integration_acc_3 @ MarginfiError::InvalidDriftUserStats,
        has_one = mint @ MarginfiError::InvalidMint,
        constraint = is_drift_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForDriftOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// The liquidity vault authority (PDA that will own the Drift user)
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// Used as an intermediary to deposit a nominal amount of token into Drift.
    #[account(mut)]
    pub liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Bank's liquidity token mint (e.g., USDC)
    #[account(mut)]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    /// The user stats account to be created
    #[account(
        mut,
        seeds = [
            DRIFT_USER_STATS_SEED.as_bytes(),
            liquidity_vault_authority.key().as_ref(),
        ],
        bump,
        seeds::program = DRIFT_PROGRAM_ID
    )]
    pub integration_acc_3: SystemAccount<'info>,

    /// The user account to be created (sub_account_id = 0)
    #[account(
        mut,
        seeds = [
            DRIFT_USER_SEED.as_bytes(),
            liquidity_vault_authority.key().as_ref(),
            &0u16.to_le_bytes()
        ],
        bump,
        seeds::program = DRIFT_PROGRAM_ID
    )]
    pub integration_acc_2: SystemAccount<'info>,

    /// CHECK: validated by the Drift program
    #[account(mut)]
    pub drift_state: UncheckedAccount<'info>,

    /// CHECK: validated by the Drift program
    #[account(mut)]
    pub integration_acc_1: AccountLoader<'info, MinimalSpotMarket>,

    /// The Drift spot market vault where tokens will be deposited
    /// CHECK: validated by the Drift program
    #[account(mut)]
    pub drift_spot_market_vault: UncheckedAccount<'info>,

    /// Oracle for the asset (can be null for USDC/market 0)
    /// CHECK: validated by the Drift program
    pub drift_oracle: Option<UncheckedAccount<'info>>,

    /// CHECK: validated against hardcoded program id
    #[account(address = DRIFT_PROGRAM_ID)]
    pub drift_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}

impl<'info> DriftInitUser<'info> {
    pub fn cpi_init_user_stats(&self, authority_bump: u8) -> MarginfiResult {
        let accounts = InitializeUserStats {
            user_stats: self.integration_acc_3.to_account_info(),
            state: self.drift_state.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            payer: self.fee_payer.to_account_info(),
            rent: self.rent.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };
        let program = self.drift_program.to_account_info();
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        initialize_user_stats(cpi_ctx)?;
        Ok(())
    }

    pub fn cpi_init_user(&self, authority_bump: u8) -> MarginfiResult {
        let accounts = InitializeUser {
            user: self.integration_acc_2.to_account_info(),
            user_stats: self.integration_acc_3.to_account_info(),
            state: self.drift_state.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            payer: self.fee_payer.to_account_info(),
            rent: self.rent.to_account_info(),
            system_program: self.system_program.to_account_info(),
        };
        let program = self.drift_program.to_account_info();
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        // Initialize with sub_account_id = 0 and empty name
        let name: [u8; 32] = [0; 32];
        initialize_user(cpi_ctx, 0, name)?;
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

    pub fn cpi_drift_deposit(
        &self,
        amount: u64,
        market_index: u16,
        authority_bump: u8,
    ) -> MarginfiResult {
        let accounts = Deposit {
            state: self.drift_state.to_account_info(),
            user: self.integration_acc_2.to_account_info(),
            user_stats: self.integration_acc_3.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            spot_market_vault: self.drift_spot_market_vault.to_account_info(),
            user_token_account: self.liquidity_vault.to_account_info(),
            token_program: self.token_program.to_account_info(),
        };
        let program = self.drift_program.to_account_info();
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);
        let mut cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);

        // Construct remaining accounts in the required order for Drift:
        // 1. Oracle accounts (if provided)
        // 2. Spot market account (always required)
        // 3. Token mint (required for Token-2022, harmless to include for regular mints)
        let mut remaining_accounts = Vec::new();

        // Add oracle if provided (not needed for USDC/market 0)
        if let Some(oracle) = &self.drift_oracle {
            remaining_accounts.push(oracle.to_account_info());
        }

        // Always add spot market account
        remaining_accounts.push(self.integration_acc_1.to_account_info());

        // Add token mint (needed for Token-2022 support)
        remaining_accounts.push(self.mint.to_account_info());

        cpi_ctx = cpi_ctx.with_remaining_accounts(remaining_accounts);

        // Call drift deposit with reduce_only = false
        deposit(cpi_ctx, market_index, amount, false)?;
        Ok(())
    }

    pub fn cpi_update_user_pool_id(&self, pool_id: u8, authority_bump: u8) -> MarginfiResult {
        let accounts = UpdateUserPoolId {
            user: self.integration_acc_2.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
        };
        let program = self.drift_program.to_account_info();
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), authority_bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        // Update pool ID for sub_account_id = 0
        update_user_pool_id(cpi_ctx, 0, pool_id)?;
        Ok(())
    }
}
