use anchor_lang::prelude::*;
use marginfi_type_crate::types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED};

use crate::{
    check,
    prelude::*,
    state::{
        bank::BankImpl,
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl,
        },
    },
    utils::is_marginfi_asset_tag,
};

pub fn lending_account_close_balance(ctx: Context<LendingAccountCloseBalance>) -> MarginfiResult {
    let LendingAccountCloseBalance {
        marginfi_account,
        bank: bank_loader,
        group: marginfi_group_loader,
        ..
    } = ctx.accounts;

    let mut marginfi_account = marginfi_account.load_mut()?;
    let mut bank = bank_loader.load_mut()?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );

    let group = &*marginfi_group_loader.load()?;
    bank.accrue_interest(
        Clock::get()?.unix_timestamp,
        group,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    bank.update_bank_cache(group)?;

    let lending_account: &mut marginfi_type_crate::types::LendingAccount =
        &mut marginfi_account.lending_account;
    let mut bank_account =
        BankAccountWrapper::find(&bank_loader.key(), &mut bank, lending_account)?;

    bank_account.close_balance()?;
    lending_account.sort_balances();
    marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountCloseBalance<'info> {
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
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,
}
