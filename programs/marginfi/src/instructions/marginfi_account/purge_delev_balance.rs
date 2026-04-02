use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{TOKENLESS_REPAYMENTS_COMPLETE, ZERO_AMOUNT_THRESHOLD},
    types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_IN_RECEIVERSHIP},
};

use crate::{
    prelude::*,
    state::{
        bank::BankImpl,
        marginfi_account::{BalanceImpl, LendingAccountImpl, MarginfiAccountImpl},
    },
};

pub fn lending_account_purge_delev_balance(
    ctx: Context<LendingAccountPurgeDelevBalance>,
) -> MarginfiResult {
    let bank_pk = &ctx.accounts.bank.key();
    let mut bank = ctx.accounts.bank.load_mut()?;
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let in_receivership = marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP);
    let lending_account = &mut marginfi_account.lending_account;

    let balance = lending_account
        .balances
        .iter_mut()
        .find(|balance| balance.is_active() && balance.bank_pk.eq(bank_pk))
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

    lending_account.sort_balances();
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
}
