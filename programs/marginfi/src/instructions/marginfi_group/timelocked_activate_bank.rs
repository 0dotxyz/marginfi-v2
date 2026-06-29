use super::timelocked_utils::*;
use crate::events::{GroupEventHeader, LendingPoolBankConfigureEvent};
use crate::prelude::MarginfiError;
use crate::state::bank::BankImpl;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::TIMELOCKED_OPERATION_SEED,
    types::{
        operation_type, Bank, BankConfigOpt, BankOperationalState, MarginfiGroup,
        TimelockedOperation,
    },
};

/// Schedule bank activation (ReduceOnly/Paused → Operational).
pub fn lending_pool_schedule_activate_bank(
    ctx: Context<LendingPoolScheduleActivateBank>,
) -> MarginfiResult {
    let marginfi_group = ctx.accounts.marginfi_group.load()?;

    assert_timelocked_admin_authorized(&marginfi_group, &ctx.accounts.timelocked_admin.key())?;

    let bank = ctx.accounts.bank.load()?;
    require!(
        bank.group == ctx.accounts.marginfi_group.key(),
        MarginfiError::InvalidGroup
    );
    require!(bank.is_activatable(), MarginfiError::InvalidConfig);

    let clock = Clock::get()?;
    let mut timelocked_op = ctx.accounts.timelocked_operation.load_init()?;
    init_timelocked_operation(
        &mut timelocked_op,
        ctx.accounts.marginfi_group.key(),
        ctx.accounts.timelocked_admin.key(),
        operation_type::ACTIVATE_BANK,
        bank.mint,
        ctx.bumps.timelocked_operation,
        marginfi_group.timelocked_operation_delay_seconds,
        clock.unix_timestamp,
    )?;

    msg!(
        "Scheduled activate bank for bank: {:?}, available at timestamp: {}",
        ctx.accounts.bank.key(),
        timelocked_op.execution_available_at
    );

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolScheduleActivateBank<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub bank: AccountLoader<'info, Bank>,

    #[account(
        init,
        space = 8 + std::mem::size_of::<TimelockedOperation>(),
        payer = timelocked_admin,
        seeds = [
            TIMELOCKED_OPERATION_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            bank.key().as_ref(),
            &[operation_type::ACTIVATE_BANK],
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    #[account(mut)]
    pub timelocked_admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Execute scheduled bank activation.
pub fn lending_pool_execute_timelocked_activate_bank(
    ctx: Context<LendingPoolExecuteTimelockedActivateBank>,
) -> MarginfiResult {
    let mut timelocked_op = ctx.accounts.timelocked_operation.load_mut()?;
    let marginfi_group = ctx.accounts.marginfi_group.load()?;
    let clock = Clock::get()?;

    assert_ready_for_execution(
        &timelocked_op,
        &ctx.accounts.marginfi_group.key(),
        operation_type::ACTIVATE_BANK,
        clock.unix_timestamp,
    )?;

    assert_signer_authorized(
        &timelocked_op,
        &ctx.accounts.signer.key(),
        &marginfi_group.admin,
    )?;

    let mut bank = ctx.accounts.bank.load_mut()?;
    require!(
        bank.group == ctx.accounts.marginfi_group.key(),
        MarginfiError::InvalidGroup
    );
    require!(
        timelocked_op.bank_mint == bank.mint,
        MarginfiError::InvalidConfig
    );
    require!(bank.is_activatable(), MarginfiError::InvalidConfig);

    bank.config.operational_state = BankOperationalState::Operational;

    msg!(
        "Executing activate bank for bank: {:?} after timelock",
        ctx.accounts.bank.key()
    );

    timelocked_op.executed = 1;

    emit!(LendingPoolBankConfigureEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(ctx.accounts.signer.key())
        },
        bank: ctx.accounts.bank.key(),
        mint: bank.mint,
        config: BankConfigOpt::default(),
    });

    drop(timelocked_op);
    close_timelocked_account(
        &ctx.accounts.timelocked_operation,
        &ctx.accounts.signer.to_account_info(),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolExecuteTimelockedActivateBank<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        seeds = [
            TIMELOCKED_OPERATION_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            bank.key().as_ref(),
            &[operation_type::ACTIVATE_BANK],
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    pub signer: Signer<'info>,
}
