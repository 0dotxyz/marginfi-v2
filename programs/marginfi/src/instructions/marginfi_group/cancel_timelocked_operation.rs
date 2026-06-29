use super::timelocked_utils::*;
use crate::prelude::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::TIMELOCKED_OPERATION_SEED,
    types::{operation_type, MarginfiGroup, TimelockedOperation},
};

/// Cancel a scheduled timelocked operation.
/// Auth: original scheduler OR current group admin.
pub fn cancel_timelocked_operation(
    ctx: Context<CancelTimelockedOperation>,
    op_type: u8,
) -> MarginfiResult {
    let marginfi_group = ctx.accounts.marginfi_group.load()?;
    let timelocked_op = ctx.accounts.timelocked_operation.load()?;

    require!(
        timelocked_op.group == ctx.accounts.marginfi_group.key(),
        MarginfiError::InvalidConfig
    );
    require!(
        timelocked_op.operation_type == op_type,
        MarginfiError::InvalidConfig
    );

    let expected_pda = if op_type == operation_type::ADD_BANK {
        Pubkey::create_program_address(
            &[
                TIMELOCKED_OPERATION_SEED.as_bytes(),
                ctx.accounts.marginfi_group.key().as_ref(),
                ctx.accounts.bank_or_mint.key().as_ref(),
                &[timelocked_op.bump],
            ],
            &crate::ID,
        )
        .map_err(|_| MarginfiError::InvalidConfig)?
    } else {
        Pubkey::create_program_address(
            &[
                TIMELOCKED_OPERATION_SEED.as_bytes(),
                ctx.accounts.marginfi_group.key().as_ref(),
                ctx.accounts.bank_or_mint.key().as_ref(),
                &[op_type],
                &[timelocked_op.bump],
            ],
            &crate::ID,
        )
        .map_err(|_| MarginfiError::InvalidConfig)?
    };

    require!(
        expected_pda == ctx.accounts.timelocked_operation.key(),
        MarginfiError::InvalidConfig
    );

    assert_signer_authorized(
        &timelocked_op,
        &ctx.accounts.signer.key(),
        &marginfi_group.admin,
    )?;
    require!(timelocked_op.executed == 0, MarginfiError::InvalidConfig);

    msg!(
        "Canceling timelocked operation type: {} for group: {:?}",
        timelocked_op.operation_type,
        ctx.accounts.marginfi_group.key()
    );

    drop(timelocked_op);

    close_timelocked_account(
        &ctx.accounts.timelocked_operation,
        &ctx.accounts.signer.to_account_info(),
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(op_type: u8)]
pub struct CancelTimelockedOperation<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    /// CHECK: For ADD_BANK: bank_mint. For other ops: bank pubkey. Used only to derive the PDA seed.
    pub bank_or_mint: AccountInfo<'info>,

    #[account(mut)]
    pub timelocked_operation: AccountLoader<'info, TimelockedOperation>,

    pub signer: Signer<'info>,

    pub system_program: Program<'info, System>,
}
