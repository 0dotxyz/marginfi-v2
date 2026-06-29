use super::timelocked_utils::*;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use anchor_spl::token_interface::*;
use marginfi_type_crate::{
    constants::TIMELOCKED_OPERATION_SEED,
    types::{operation_type, BankConfigCompact, MarginfiGroup, TimelockedOperation},
};

/// Schedule add bank. Config stored for later verification.
pub fn lending_pool_schedule_add_bank(
    ctx: Context<LendingPoolScheduleAddBank>,
    bank_config: BankConfigCompact,
) -> MarginfiResult {
    let marginfi_group = ctx.accounts.marginfi_group.load()?;

    assert_timelocked_admin_authorized(&marginfi_group, &ctx.accounts.timelocked_admin.key())?;

    let clock = Clock::get()?;
    let mut timelocked_op = ctx.accounts.timelocked_operation.load_init()?;
    init_timelocked_operation(
        &mut timelocked_op,
        ctx.accounts.marginfi_group.key(),
        ctx.accounts.timelocked_admin.key(),
        operation_type::ADD_BANK,
        ctx.accounts.bank_mint.key(),
        ctx.bumps.timelocked_operation,
        marginfi_group.timelocked_operation_delay_seconds,
        clock.unix_timestamp,
    )?;

    // Store critical config for verification at execution
    timelocked_op.data.value_u64_1 = bank_config.deposit_limit;
    timelocked_op.data.value_u64_2 = bank_config.borrow_limit;
    timelocked_op.data.value_u64_3 =
        (bank_config.risk_tier as u64) | ((bank_config.asset_tag as u64) << 8);
    timelocked_op.data.value_u64_4 = bank_config.total_asset_value_init_limit;

    // Store collateral/liability weights (most critical for protocol safety)
    timelocked_op.data.extra[0..16].copy_from_slice(&bank_config.asset_weight_init.value);
    timelocked_op.data.extra[16..32].copy_from_slice(&bank_config.asset_weight_maint.value);
    timelocked_op.data.extra_extended[0..16]
        .copy_from_slice(&bank_config.liability_weight_init.value);
    timelocked_op.data.extra_extended[16..32]
        .copy_from_slice(&bank_config.liability_weight_maint.value);

    msg!(
        "Scheduled add bank for mint: {:?}, available at timestamp: {}",
        ctx.accounts.bank_mint.key(),
        timelocked_op.execution_available_at
    );

    Ok(())
}

#[derive(Accounts)]
#[instruction(bank_config: BankConfigCompact)]
pub struct LendingPoolScheduleAddBank<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        init,
        space = 8 + std::mem::size_of::<TimelockedOperation>(),
        payer = timelocked_admin,
        seeds = [
            TIMELOCKED_OPERATION_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            bank_mint.key().as_ref(),
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    #[account(mut)]
    pub timelocked_admin: Signer<'info>,

    pub bank_mint: Box<InterfaceAccount<'info, Mint>>,

    pub system_program: Program<'info, System>,
}

/// Step 2 of 3: Validate config after timelock. Locks config for finalization.
pub fn lending_pool_validate_timelocked_add_bank(
    ctx: Context<LendingPoolValidateTimelockedAddBank>,
    bank_config: BankConfigCompact,
) -> MarginfiResult {
    let timelocked_op = ctx.accounts.timelocked_operation.load()?;
    let marginfi_group = ctx.accounts.marginfi_group.load()?;

    assert_ready_for_execution(
        &timelocked_op,
        &ctx.accounts.marginfi_group.key(),
        operation_type::ADD_BANK,
        Clock::get()?.unix_timestamp,
    )?;

    assert_signer_authorized(
        &timelocked_op,
        &ctx.accounts.signer.key(),
        &marginfi_group.admin,
    )?;

    assert_bank_matches(&timelocked_op, &ctx.accounts.bank_mint.key())?;

    assert_bank_config_matches_op(
        bank_config.deposit_limit,
        bank_config.borrow_limit,
        bank_config.risk_tier,
        bank_config.asset_tag,
        bank_config.total_asset_value_init_limit,
        &bank_config.asset_weight_init,
        &bank_config.asset_weight_maint,
        &bank_config.liability_weight_init,
        &bank_config.liability_weight_maint,
        &timelocked_op,
    )?;

    msg!(
        "Validated timelocked add bank for mint: {:?}. Config locked.",
        ctx.accounts.bank_mint.key()
    );

    drop(timelocked_op);
    let mut timelocked_op = ctx.accounts.timelocked_operation.load_mut()?;
    timelocked_op.validated = 1;

    Ok(())
}

#[derive(Accounts)]
#[instruction(bank_config: BankConfigCompact)]
pub struct LendingPoolValidateTimelockedAddBank<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub bank_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        seeds = [
            TIMELOCKED_OPERATION_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            bank_mint.key().as_ref(),
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    pub signer: Signer<'info>,
}
