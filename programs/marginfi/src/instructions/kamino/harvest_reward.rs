use crate::state::bank::BankVaultType;
use crate::{
    bank_signer, constants::FARMS_PROGRAM_ID, optional_account, utils::is_kamino_asset_tag,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token::accessor;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use kamino_mocks::kamino_farms::cpi::accounts::HarvestReward;
use kamino_mocks::kamino_farms::cpi::harvest_reward;
use marginfi_type_crate::constants::{FEE_STATE_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED};
use marginfi_type_crate::types::{Bank, FeeState};

pub fn kamino_harvest_reward(
    ctx: Context<KaminoHarvestReward>,
    reward_index: u64,
) -> MarginfiResult {
    let pre_transfer_balance = accessor::amount(&ctx.accounts.user_reward_ata.to_account_info())?;
    ctx.accounts.cpi_harvest_rewards(reward_index)?;
    let post_transfer_balance = accessor::amount(&ctx.accounts.user_reward_ata.to_account_info())?;
    let received = post_transfer_balance - pre_transfer_balance;
    ctx.accounts
        .cpi_transfer_obligation_owner_to_destination(received)?;
    Ok(())
}

#[derive(Accounts)]
pub struct KaminoHarvestReward<'info> {
    #[account(
        constraint = is_kamino_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForKaminoInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// Global fee state that contains the global_fee_admin
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// Destination token account must be owned by the global fee admin
    #[account(
        mut,
        associated_token::mint = reward_mint,
        associated_token::authority = fee_state.load()?.global_fee_wallet,
        associated_token::token_program = token_program,
    )]
    pub destination_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// The bank's liquidity vault authority, which owns the Kamino obligation.
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref()
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: SystemAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub user_state: UncheckedAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub farm_state: UncheckedAccount<'info>,

    /// CHECK:
    pub global_config: UncheckedAccount<'info>,

    pub reward_mint: InterfaceAccount<'info, Mint>,

    /// An initialized ATA of type reward mint owned by liquidity vault
    /// CHECK:
    #[account(mut)]
    pub user_reward_ata: UncheckedAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub rewards_vault: UncheckedAccount<'info>,

    /// CHECK:
    #[account(mut)]
    pub rewards_treasury_vault: UncheckedAccount<'info>,

    /// CHECK:
    pub farm_vaults_authority: UncheckedAccount<'info>,

    /// CHECK:
    pub scope_prices: Option<UncheckedAccount<'info>>,

    /// CHECK: validated against hardcoded program id
    #[account(address = FARMS_PROGRAM_ID)]
    pub farms_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl<'info> KaminoHarvestReward<'info> {
    pub fn cpi_harvest_rewards(&self, reward_index: u64) -> MarginfiResult {
        let program = self.farms_program.to_account_info();
        let accounts = HarvestReward {
            owner: self.liquidity_vault_authority.to_account_info(),
            user_state: self.user_state.to_account_info(),
            farm_state: self.farm_state.to_account_info(),
            global_config: self.global_config.to_account_info(),
            reward_mint: self.reward_mint.to_account_info(),
            user_reward_ata: self.user_reward_ata.to_account_info(),
            rewards_vault: self.rewards_vault.to_account_info(),
            rewards_treasury_vault: self.rewards_treasury_vault.to_account_info(),
            farm_vaults_authority: self.farm_vaults_authority.to_account_info(),
            scope_prices: optional_account!(self.scope_prices),
            token_program: self.token_program.to_account_info(),
        };
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        harvest_reward(cpi_ctx, reward_index)?;
        Ok(())
    }

    pub fn cpi_transfer_obligation_owner_to_destination(&self, amount: u64) -> MarginfiResult {
        let program = self.token_program.to_account_info();
        let accounts = TransferChecked {
            from: self.user_reward_ata.to_account_info(),
            to: self.destination_token_account.to_account_info(),
            authority: self.liquidity_vault_authority.to_account_info(),
            mint: self.reward_mint.to_account_info(),
        };
        let bump = self.bank.load()?.liquidity_vault_authority_bump;
        let signer_seeds: &[&[&[u8]]] =
            bank_signer!(BankVaultType::Liquidity, self.bank.key(), bump);
        let cpi_ctx = CpiContext::new_with_signer(program, accounts, signer_seeds);
        let decimals = self.reward_mint.decimals;
        transfer_checked(cpi_ctx, amount, decimals)?;
        Ok(())
    }
}
