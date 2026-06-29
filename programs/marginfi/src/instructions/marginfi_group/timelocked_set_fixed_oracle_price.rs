use super::timelocked_utils::*;
use crate::check;
use crate::events::{GroupEventHeader, LendingPoolBankSetFixedOraclePriceEvent};
use crate::prelude::MarginfiError;
use crate::state::bank::BankImpl;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::constants::{
    ASSET_TAG_DRIFT, ASSET_TAG_JUPLEND, ASSET_TAG_KAMINO, ASSET_TAG_STAKED, FREEZE_SETTINGS,
    TIMELOCKED_OPERATION_SEED,
};
use marginfi_type_crate::types::{
    operation_type, Bank, MarginfiGroup, OracleSetup, TimelockedOperation, WrappedI80F48,
};

/// Schedule fixed oracle price operation.
pub fn lending_pool_schedule_set_fixed_oracle_price(
    ctx: Context<LendingPoolScheduleSetFixedOraclePrice>,
    price: WrappedI80F48,
) -> MarginfiResult {
    let marginfi_group = ctx.accounts.marginfi_group.load()?;

    assert_timelocked_admin_authorized(&marginfi_group, &ctx.accounts.timelocked_admin.key())?;

    let bank = ctx.accounts.bank.load()?;
    require!(
        bank.group == ctx.accounts.marginfi_group.key(),
        MarginfiError::InvalidGroup
    );

    require!(!bank.get_flag(FREEZE_SETTINGS), MarginfiError::Unauthorized);

    check!(
        I80F48::from_le_bytes(price.value) >= I80F48::ZERO,
        MarginfiError::FixedOraclePriceNegative
    );

    let clock = Clock::get()?;
    let mut timelocked_op = ctx.accounts.timelocked_operation.load_init()?;
    init_timelocked_operation(
        &mut timelocked_op,
        ctx.accounts.marginfi_group.key(),
        ctx.accounts.timelocked_admin.key(),
        operation_type::SET_FIXED_ORACLE_PRICE,
        bank.mint,
        ctx.bumps.timelocked_operation,
        marginfi_group.timelocked_operation_delay_seconds,
        clock.unix_timestamp,
    )?;

    timelocked_op.data.extra[..16].copy_from_slice(&price.value);

    msg!(
        "Scheduled set fixed oracle price for bank: {:?}, available at timestamp: {}",
        ctx.accounts.bank.key(),
        timelocked_op.execution_available_at
    );

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolScheduleSetFixedOraclePrice<'info> {
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
            &[operation_type::SET_FIXED_ORACLE_PRICE],
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    #[account(mut)]
    pub timelocked_admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Execute scheduled fixed price operation.
pub fn lending_pool_execute_timelocked_set_fixed_oracle_price(
    ctx: Context<LendingPoolExecuteTimelockedSetFixedOraclePrice>,
    price: WrappedI80F48,
) -> MarginfiResult {
    let marginfi_group = ctx.accounts.marginfi_group.load()?;
    let clock = Clock::get()?;

    {
        let timelocked_op = ctx.accounts.timelocked_operation.load()?;

        assert_ready_for_execution(
            &timelocked_op,
            &ctx.accounts.marginfi_group.key(),
            operation_type::SET_FIXED_ORACLE_PRICE,
            clock.unix_timestamp,
        )?;

        assert_signer_authorized(
            &timelocked_op,
            &ctx.accounts.signer.key(),
            &marginfi_group.admin,
        )?;

        require!(
            timelocked_op.data.extra[..16] == price.value[..],
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

    if bank.config.asset_tag == ASSET_TAG_STAKED {
        msg!("Staked banks cannot set a fixed price");
        return err!(MarginfiError::Unauthorized);
    }

    bank.config.oracle_setup = if bank.config.asset_tag == ASSET_TAG_KAMINO {
        OracleSetup::FixedKamino
    } else if bank.config.asset_tag == ASSET_TAG_DRIFT {
        OracleSetup::FixedDrift
    } else if bank.config.asset_tag == ASSET_TAG_JUPLEND {
        OracleSetup::FixedJuplend
    } else {
        OracleSetup::Fixed
    };

    bank.config.oracle_keys[0] = Pubkey::default();
    bank.config.fixed_price = price;

    msg!(
        "Executing set fixed oracle price for bank: {:?} after timelock",
        ctx.accounts.bank.key()
    );

    emit!(LendingPoolBankSetFixedOraclePriceEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.marginfi_group.key(),
            signer: Some(ctx.accounts.signer.key()),
        },
        bank: ctx.accounts.bank.key(),
        price,
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
pub struct LendingPoolExecuteTimelockedSetFixedOraclePrice<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        seeds = [
            TIMELOCKED_OPERATION_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            bank.key().as_ref(),
            &[operation_type::SET_FIXED_ORACLE_PRICE],
        ],
        bump
    )]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    pub signer: Signer<'info>,
}
