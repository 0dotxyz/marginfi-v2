use super::timelocked_utils::*;
use crate::events::{GroupEventHeader, LendingPoolBankConfigureOracleEvent};
use crate::prelude::MarginfiError;
use crate::state::bank::BankImpl;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{FREEZE_SETTINGS, TIMELOCKED_OPERATION_SEED},
    types::{operation_type, Bank, MarginfiGroup, OracleSetup, TimelockedOperation},
};

/// Schedule oracle configuration.
pub fn lending_pool_schedule_configure_bank_oracle(
    ctx: Context<LendingPoolScheduleConfigureBankOracle>,
    setup: u8,
    oracle: Pubkey,
) -> MarginfiResult {
    let marginfi_group = ctx.accounts.marginfi_group.load()?;

    assert_timelocked_admin_authorized(&marginfi_group, &ctx.accounts.timelocked_admin.key())?;

    let bank = ctx.accounts.bank.load()?;
    require!(
        bank.group == ctx.accounts.marginfi_group.key(),
        MarginfiError::InvalidGroup
    );

    require!(!bank.get_flag(FREEZE_SETTINGS), MarginfiError::Unauthorized);

    let setup_type = OracleSetup::from_u8(setup).ok_or(MarginfiError::InvalidConfig)?;

    require!(
        setup_type != OracleSetup::Fixed,
        MarginfiError::InvalidConfig
    );

    let clock = Clock::get()?;
    let mut timelocked_op = ctx.accounts.timelocked_operation.load_init()?;
    init_timelocked_operation(
        &mut timelocked_op,
        ctx.accounts.marginfi_group.key(),
        ctx.accounts.timelocked_admin.key(),
        operation_type::CONFIGURE_ORACLE,
        bank.mint,
        ctx.bumps.timelocked_operation,
        marginfi_group.timelocked_operation_delay_seconds,
        clock.unix_timestamp,
    )?;

    timelocked_op.data.value_u64_1 = setup as u64;
    timelocked_op.data.pubkey_1 = oracle;

    msg!(
        "Scheduled configure oracle for bank: {:?}, available at timestamp: {}",
        ctx.accounts.bank.key(),
        timelocked_op.execution_available_at
    );

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolScheduleConfigureBankOracle<'info> {
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
            &[operation_type::CONFIGURE_ORACLE],
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    #[account(mut)]
    pub timelocked_admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Execute scheduled oracle configuration.
pub fn lending_pool_execute_timelocked_configure_bank_oracle(
    ctx: Context<LendingPoolExecuteTimelockedConfigureBankOracle>,
    setup: u8,
    oracle: Pubkey,
) -> MarginfiResult {
    let marginfi_group = ctx.accounts.marginfi_group.load()?;
    let clock = Clock::get()?;

    {
        let timelocked_op = ctx.accounts.timelocked_operation.load()?;

        assert_ready_for_execution(
            &timelocked_op,
            &ctx.accounts.marginfi_group.key(),
            operation_type::CONFIGURE_ORACLE,
            clock.unix_timestamp,
        )?;

        assert_signer_authorized(
            &timelocked_op,
            &ctx.accounts.signer.key(),
            &marginfi_group.admin,
        )?;

        require!(
            timelocked_op.data.value_u64_1 == setup as u64,
            MarginfiError::InvalidConfig
        );
        require!(
            timelocked_op.data.pubkey_1 == oracle,
            MarginfiError::InvalidConfig
        );

        {
            let bank = ctx.accounts.bank.load()?;
            assert_bank_matches(&timelocked_op, &bank.mint)?;
            require!(
                bank.group == ctx.accounts.marginfi_group.key(),
                MarginfiError::InvalidGroup
            );
        }
    }

    let mut bank = ctx.accounts.bank.load_mut()?;

    require!(!bank.get_flag(FREEZE_SETTINGS), MarginfiError::Unauthorized);

    let setup_type = OracleSetup::from_u8(setup).ok_or(MarginfiError::InvalidConfig)?;
    if setup_type == OracleSetup::Fixed {
        msg!("Use set_fixed_oracle_price instead");
        return err!(MarginfiError::InvalidConfig);
    }

    bank.config.oracle_setup = setup_type;
    bank.config.oracle_keys[0] = oracle;

    msg!(
        "Executing oracle configuration for bank: {:?} after timelock",
        ctx.accounts.bank.key()
    );

    emit!(LendingPoolBankConfigureOracleEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(ctx.accounts.signer.key())
        },
        bank: ctx.accounts.bank.key(),
        oracle_setup: setup,
        oracle
    });

    drop(marginfi_group);
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
pub struct LendingPoolExecuteTimelockedConfigureBankOracle<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        seeds = [
            TIMELOCKED_OPERATION_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            bank.key().as_ref(),
            &[operation_type::CONFIGURE_ORACLE],
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    pub signer: Signer<'info>,
}
