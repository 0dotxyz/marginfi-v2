use super::timelocked_utils::*;
use crate::events::{GroupEventHeader, LendingPoolBankConfigureEvent};
use crate::prelude::MarginfiError;
use crate::state::bank::BankImpl;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::TIMELOCKED_OPERATION_SEED,
    types::{operation_type, Bank, BankConfigOpt, MarginfiGroup, TimelockedOperation},
};

/// Schedule risk tier configuration (Isolated ↔ Collateral transition).
pub fn lending_pool_schedule_configure_bank_risk_tier(
    ctx: Context<LendingPoolScheduleConfigureBankRiskTier>,
    new_risk_tier: u8,
) -> MarginfiResult {
    let marginfi_group = ctx.accounts.marginfi_group.load()?;

    assert_timelocked_admin_authorized(&marginfi_group, &ctx.accounts.timelocked_admin.key())?;

    let bank = ctx.accounts.bank.load()?;
    require!(
        bank.group == ctx.accounts.marginfi_group.key(),
        MarginfiError::InvalidGroup
    );

    let _new_tier = parse_risk_tier(new_risk_tier)?;

    let clock = Clock::get()?;
    let mut timelocked_op = ctx.accounts.timelocked_operation.load_init()?;
    init_timelocked_operation(
        &mut timelocked_op,
        ctx.accounts.marginfi_group.key(),
        ctx.accounts.timelocked_admin.key(),
        operation_type::CONFIGURE_BANK_RISK_TIER,
        bank.mint,
        ctx.bumps.timelocked_operation,
        marginfi_group.timelocked_operation_delay_seconds,
        clock.unix_timestamp,
    )?;

    timelocked_op.data.value_u64_1 = new_risk_tier as u64;

    msg!(
        "Scheduled configure bank risk tier for bank: {:?}, new tier: {}, available at timestamp: {}",
        ctx.accounts.bank.key(),
        new_risk_tier,
        timelocked_op.execution_available_at
    );

    Ok(())
}

/// Execute scheduled risk tier configuration.
pub fn lending_pool_execute_timelocked_configure_bank_risk_tier(
    ctx: Context<LendingPoolExecuteTimelockedConfigureBankRiskTier>,
    new_risk_tier: u8,
) -> MarginfiResult {
    let timelocked_op = ctx.accounts.timelocked_operation.load()?;
    let marginfi_group = ctx.accounts.marginfi_group.load()?;
    let clock = Clock::get()?;

    assert_ready_for_execution(
        &timelocked_op,
        &ctx.accounts.marginfi_group.key(),
        operation_type::CONFIGURE_BANK_RISK_TIER,
        clock.unix_timestamp,
    )?;

    assert_signer_authorized(
        &timelocked_op,
        &ctx.accounts.signer.key(),
        &marginfi_group.admin,
    )?;

    {
        let bank = ctx.accounts.bank.load()?;
        assert_bank_matches(&timelocked_op, &bank.mint)?;
    }

    require!(
        timelocked_op.data.value_u64_1 == new_risk_tier as u64,
        MarginfiError::InvalidConfig
    );

    let new_tier = parse_risk_tier(new_risk_tier)?;
    let mut bank = ctx.accounts.bank.load_mut()?;
    let old_tier = bank.config.risk_tier;

    let bank_config = BankConfigOpt {
        risk_tier: Some(new_tier),
        ..Default::default()
    };

    bank.configure(&bank_config)?;

    msg!(
        "Configured bank risk tier: {:?} → {:?}",
        old_tier,
        new_risk_tier
    );

    emit!(LendingPoolBankConfigureEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(ctx.accounts.signer.key())
        },
        bank: ctx.accounts.bank.key(),
        mint: bank.mint,
        config: bank_config,
    });

    drop(timelocked_op);
    drop(bank);

    let mut timelocked_op = ctx.accounts.timelocked_operation.load_mut()?;
    timelocked_op.executed = 1;

    drop(timelocked_op);
    close_timelocked_account(
        &ctx.accounts.timelocked_operation,
        &ctx.accounts.signer.to_account_info(),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolScheduleConfigureBankRiskTier<'info> {
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
            &[operation_type::CONFIGURE_BANK_RISK_TIER],
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    #[account(mut)]
    pub timelocked_admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LendingPoolExecuteTimelockedConfigureBankRiskTier<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        seeds = [
            TIMELOCKED_OPERATION_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            bank.key().as_ref(),
            &[operation_type::CONFIGURE_BANK_RISK_TIER],
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    pub signer: Signer<'info>,
}
