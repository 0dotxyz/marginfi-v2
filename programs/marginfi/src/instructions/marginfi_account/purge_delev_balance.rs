use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        INSURANCE_VAULT_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED,
        TOKENLESS_REPAYMENTS_COMPLETE, ZERO_AMOUNT_THRESHOLD,
    },
    types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_IN_RECEIVERSHIP},
};

use crate::{
    bank_signer,
    prelude::*,
    state::{
        bank::{BankImpl, BankVaultType},
        marginfi_account::{BalanceImpl, LendingAccountImpl, MarginfiAccountImpl},
    },
    utils,
};

pub fn lending_account_purge_delev_balance<'info>(
    mut ctx: Context<'info, LendingAccountPurgeDelevBalance<'info>>,
) -> MarginfiResult {
    let bank_key = ctx.accounts.bank.key();
    let mut bank = ctx.accounts.bank.load_mut()?;
    let maybe_bank_mint = utils::maybe_take_bank_mint(
        &mut ctx.remaining_accounts,
        &bank,
        ctx.accounts.token_program.key,
    )?;
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
    let lending_account = &mut marginfi_account.lending_account;

    let balance = lending_account
        .balances
        .iter_mut()
        .find(|balance| balance.is_active() && balance.bank_pk.eq(&bank_key))
        .ok_or_else(|| error!(MarginfiError::BankAccountNotFound))?;

    // Sanity check the balance is not a liability
    let liab_shares: I80F48 = balance.liability_shares.into();
    if liab_shares.abs() > ZERO_AMOUNT_THRESHOLD {
        msg!("liab with shares: {:?}", liab_shares.to_num::<f64>());
        return err!(MarginfiError::OperationWithdrawOnly);
    }

    let asset_shares: I80F48 = balance.asset_shares.into();
    msg!("Balance had: {:?}", asset_shares.to_num::<f64>());
    balance.close()?;
    if in_receivership {
        bank.cache.clear_liquidation_price_cache_locked();
    }
    bank.decrement_lending_position_count();
    bank.change_asset_shares(-asset_shares, false)?;

    // Purging moves no funds: whatever remains in the liquidity vault stays claimable by the
    // other lenders, first-come-first-served. The purge closing the bank's last lending position
    // sweeps the vault into the insurance vault — no on-chain claims remain at that point, and
    // the funds would otherwise be stranded once the bank closes. The admin can withdraw the
    // swept funds to repay purged lenders off-chain
    if bank.lending_position_count == 0 {
        bank.withdraw_spl_transfer(
            ctx.accounts.liquidity_vault.amount,
            ctx.accounts.liquidity_vault.to_account_info(),
            ctx.accounts.insurance_vault.to_account_info(),
            ctx.accounts.liquidity_vault_authority.to_account_info(),
            maybe_bank_mint.as_ref(),
            ctx.accounts.token_program.to_account_info(),
            bank_signer!(
                BankVaultType::Liquidity,
                bank_key,
                bank.liquidity_vault_authority_bump
            ),
            ctx.remaining_accounts,
        )?;
    }

    lending_account.sort_balances();
    marginfi_account.sync_indexer_flags();
    marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountPurgeDelevBalance<'info> {
    #[account(
        has_one = risk_admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub risk_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = bank.load()?.get_flag(TOKENLESS_REPAYMENTS_COMPLETE)
            @ MarginfiError::ForbiddenIx
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: Seed constraint check
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_bump
    )]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            INSURANCE_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.insurance_vault_bump
    )]
    pub insurance_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
