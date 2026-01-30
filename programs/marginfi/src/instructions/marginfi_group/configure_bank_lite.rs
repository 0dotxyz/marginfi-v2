use crate::set_if_some;
use crate::state::bank::BankImpl;
use crate::state::interest_rate::InterestRateConfigImpl;
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::constants::{TOKENLESS_REPAYMENTS_ALLOWED, TOKENLESS_REPAYMENTS_COMPLETE};
use marginfi_type_crate::{
    constants::FREEZE_SETTINGS,
    types::{Bank, InterestRateConfigOpt, MarginfiGroup},
};

pub fn lending_pool_configure_bank_interest_only(
    ctx: Context<LendingPoolConfigureBankInterestOnly>,
    interest_rate_config: InterestRateConfigOpt,
) -> MarginfiResult {
    msg!("Configuring bank : {:?}", ctx.accounts.bank.key());
    let mut bank = ctx.accounts.bank.load_mut()?;

    // If settings are frozen, interest rates can't update.
    if bank.get_flag(FREEZE_SETTINGS) {
        msg!("WARN: Bank settings frozen, did nothing.");
    } else {
        bank.config
            .interest_rate_config
            .update(&interest_rate_config);
        bank.config.interest_rate_config.validate()?;
        msg!("Bank configured!");
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBankInterestOnly<'info> {
    #[account(
        has_one = delegate_curve_admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub delegate_curve_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}

pub fn lending_pool_configure_bank_limits_only(
    ctx: Context<LendingPoolConfigureBankLimitsOnly>,
    deposit_limit: Option<u64>,
    borrow_limit: Option<u64>,
    total_asset_value_init_limit: Option<u64>,
) -> MarginfiResult {
    msg!("Configuring bank : {:?}", ctx.accounts.bank.key());
    let mut bank = ctx.accounts.bank.load_mut()?;

    // If settings are frozen, only deposit and borrow limits can update
    if bank.get_flag(FREEZE_SETTINGS) {
        msg!("WARN: Bank settings frozen, only deposit/borrow limits update.");
        // Note: total_asset_value_init_limit is somewhat risky because it can reduce the value of
        // existing deposited assets.
        set_if_some!(bank.config.deposit_limit, deposit_limit);
        set_if_some!(bank.config.borrow_limit, borrow_limit);
    } else {
        set_if_some!(bank.config.deposit_limit, deposit_limit);
        set_if_some!(bank.config.borrow_limit, borrow_limit);
        set_if_some!(
            bank.config.total_asset_value_init_limit,
            total_asset_value_init_limit
        );
        msg!("Bank configured!");
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBankLimitsOnly<'info> {
    #[account(
        has_one = delegate_limit_admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub delegate_limit_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}

pub fn lending_pool_force_tokenless_repay_complete(
    ctx: Context<LendingPoolForceTokenlessRepayComplete>,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    if bank.get_flag(TOKENLESS_REPAYMENTS_ALLOWED) {
        bank.update_flag(true, TOKENLESS_REPAYMENTS_COMPLETE);
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolForceTokenlessRepayComplete<'info> {
    #[account(
        has_one = risk_admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub risk_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}
