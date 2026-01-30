use crate::{
    bank_signer, constants::DRIFT_PROGRAM_ID, state::bank::BankVaultType,
    utils::is_drift_asset_tag, MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token::accessor,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};
use drift_mocks::drift::cpi::accounts::Withdraw;
use drift_mocks::drift::cpi::withdraw;
use drift_mocks::state::{MinimalSpotMarket, MinimalUser};
use marginfi_type_crate::constants::{FEE_STATE_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::types::{Bank, FeeState};

/// Harvest rewards from admin deposits in Drift spot markets
/// This instruction allows withdrawing from positions that were created by admin deposits
/// at indices 2-7 (index 0 is for USDC, 1 is for any other token mint)
/// Has a number of checks to ensure this only withdraws rewards
/// - Checks that harvest spot market does not match the bank's spot market
/// - Checks that harvest spot market mint does not match bank's mint
/// - Checks that the harvest spot market has a balance in index 2 - 7 on the user account
///     The only possible exception to index 2-7 is if someone rewards USDC usage which is unlikely.
///
/// Remaining accounts should be passed in the order required by Drift's withdraw instruction:
/// 1. Oracle accounts (optional)
/// 2. Spot market accounts (always required)
/// 3. Token mint (optional for Token-2022)
pub fn drift_harvest_reward<'info>(
    ctx: Context<'_, '_, 'info, 'info, DriftHarvestReward<'info>>,
) -> MarginfiResult {
    let spot_market_index = {
        let harvest_spot_market = ctx.accounts.harvest_drift_spot_market.load()?;
        harvest_spot_market.market_index
    };

    ctx.accounts
        .cpi_withdraw_from_position(spot_market_index, ctx.remaining_accounts)?;

    ctx.accounts.cpi_transfer_to_destination()?;
    Ok(())
}

#[derive(Accounts)]
pub struct DriftHarvestReward<'info> {
    #[account(
        has_one = integration_acc_2 @ MarginfiError::InvalidDriftUser,
        has_one = integration_acc_3 @ MarginfiError::InvalidDriftUserStats,
        constraint = is_drift_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongBankAssetTagForDriftOperation
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Global fee state that contains the global_fee_wallet
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// The bank's liquidity vault authority
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// To create this manually just send some of the reward token
    /// to the liquidity vault authority address before claiming
    #[account(
        mut,
        associated_token::mint = reward_mint,
        associated_token::authority = liquidity_vault_authority,
        associated_token::token_program = token_program,
    )]
    pub intermediary_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Destination token account must be owned by the global fee wallet
    #[account(
        mut,
        associated_token::mint = reward_mint,
        associated_token::authority = fee_state.load()?.global_fee_wallet,
        associated_token::token_program = token_program,
    )]
    pub destination_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Drift accounts
    /// CHECK: Validated in cpi
    pub drift_state: UncheckedAccount<'info>,

    #[account(
        mut,
        constraint = {
            let user = integration_acc_2.load()?;
            user.has_admin_deposit(harvest_drift_spot_market.load()?.market_index).is_ok()
        } @ MarginfiError::DriftNoAdminDeposit
    )]
    pub integration_acc_2: AccountLoader<'info, MinimalUser>,

    /// CHECK: Validated in cpi
    #[account(mut)]
    pub integration_acc_3: UncheckedAccount<'info>,

    /// The harvest spot market - MUST be different from bank's Drift spot market (integration_acc_1)
    /// This is the market that contains admin deposits to harvest
    #[account(
        mut,
        owner = DRIFT_PROGRAM_ID,
        constraint = harvest_drift_spot_market.load()?.mint != bank.load()?.mint
            @ MarginfiError::DriftSpotMarketMintMismatch,
        constraint = harvest_drift_spot_market.key() != bank.load()?.integration_acc_1
            @ MarginfiError::DriftHarvestSameMarket,
    )]
    pub harvest_drift_spot_market: AccountLoader<'info, MinimalSpotMarket>,

    /// The harvest spot market vault - derived from harvest_drift_spot_market
    /// CHECK: Validated in CPI
    #[account(mut)]
    pub harvest_drift_spot_market_vault: UncheckedAccount<'info>,

    /// The Drift signer PDA
    /// CHECK: Validated via seeds
    pub drift_signer: UncheckedAccount<'info>,

    pub reward_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: validated against hardcoded program id
    #[account(address = DRIFT_PROGRAM_ID)]
    pub drift_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> DriftHarvestReward<'info> {
    pub fn cpi_withdraw_from_position(
        &self,
        market_index: u16,
        remaining_accounts: &[AccountInfo<'info>],
    ) -> MarginfiResult {
        let program = self.drift_program.to_account_info();

        let accounts = Withdraw {
            state: self.drift_state.to_account_info(),
            user: self.integration_acc_2.to_account_info(),
            user_stats: self.integration_acc_3.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            spot_market_vault: self.harvest_drift_spot_market_vault.to_account_info(),
            drift_signer: self.drift_signer.to_account_info(),
            user_token_account: self.intermediary_token_account.to_account_info(),
            token_program: self.token_program.to_account_info(),
        };

        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);

        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds)
            .with_remaining_accounts(remaining_accounts.to_vec());

        withdraw(cpi_ctx, market_index, u64::MAX, true)?;
        Ok(())
    }

    pub fn cpi_transfer_to_destination(&self) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.intermediary_token_account.to_account_info(),
            to: self.destination_token_account.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            mint: self.reward_mint.to_account_info(),
        };

        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);

        let decimals = self.reward_mint.decimals;
        // Transfer entire balance
        let amount = accessor::amount(&self.intermediary_token_account.to_account_info())?;
        transfer_checked(cpi_ctx, amount, decimals)?;
        Ok(())
    }
}
