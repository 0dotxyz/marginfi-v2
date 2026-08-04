use crate::check;
use crate::ix_utils;
use crate::state::bank::BankImpl;
use crate::utils::NumTraitsWithTolerance;
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{CLOSE_ENABLED_FLAG, ZERO_AMOUNT_THRESHOLD},
    types::{Bank, MarginfiGroup},
};

/// * force_close - (admin escape hatch) when `Some(true)`, skips the `CLOSE_ENABLED_FLAG` and
///   open-position checks. Intended for legacy pre-0.1.4 banks whose position count is
///   non-authoritative. The zero-shares/zero-emissions checks are always enforced, so a bank still
///   holding value can never be force-closed.
pub fn lending_pool_close_bank(
    ctx: Context<LendingPoolCloseBank>,
    force_close: Option<bool>,
) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    let mut group = ctx.accounts.group.load_mut()?;
    // Note: Groups created prior to 0.1.2 have a non-authoritative count here, so subtraction
    // without saturation could reduce the count below zero.
    group.banks = group.banks.saturating_sub(1);

    let bank = ctx.accounts.bank.load()?;

    if !force_close.unwrap_or(false) {
        // banks created prior to 0.1.4 can never be closed because we cannot guarantee an accurate
        // position count for those banks.
        check!(
            bank.get_flag(CLOSE_ENABLED_FLAG),
            MarginfiError::BankCannotClose,
            "Only banks created in 0.1.4 and later can close"
        );
        check!(
            bank.lending_position_count == 0 && bank.borrowing_position_count == 0,
            MarginfiError::BankCannotClose,
            "Only banks with no open positions can close"
        );
    }
    check!(
        I80F48::from(bank.total_asset_shares).is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD)
            && I80F48::from(bank.total_liability_shares)
                .is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
        MarginfiError::BankCannotClose
    );
    check!(
        I80F48::from(bank.emissions_remaining).is_zero_with_tolerance(ZERO_AMOUNT_THRESHOLD),
        MarginfiError::BankCannotClose
    );

    drop(bank);

    // Bank will now be closed by anchor

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolCloseBank<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        close = admin
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}
