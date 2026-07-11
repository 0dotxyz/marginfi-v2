use crate::check;
use crate::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::FEE_STATE_SEED,
    types::{FeeState, MarginfiGroup},
};

/// (permissionless) Resize a v1-sized group account to the current struct size. `payer` funds
/// the added rent; new bytes are zero-filled.
pub fn lending_pool_resize_group_account(
    ctx: Context<LendingPoolResizeGroupAccount>,
) -> MarginfiResult {
    let group_ai = &ctx.accounts.group;

    check!(group_ai.owner == &crate::ID, MarginfiError::InvalidGroup);
    {
        let data = group_ai.try_borrow_data()?;
        check!(
            data.len() >= 8 && data[..8] == MarginfiGroup::DISCRIMINATOR,
            MarginfiError::InvalidGroup
        );
    }

    grow_account(
        group_ai,
        8 + MarginfiGroup::LEN,
        &ctx.accounts.payer,
        &ctx.accounts.system_program,
    )
}

/// (permissionless) Resize the v1-sized fee-state account to the current struct size. `payer` funds the
/// added rent; new bytes are zero-filled.
pub fn resize_global_fee_state(ctx: Context<ResizeGlobalFeeState>) -> MarginfiResult {
    let fee_state_ai = &ctx.accounts.fee_state;

    check!(
        fee_state_ai.owner == &crate::ID,
        MarginfiError::InvalidGroup
    );
    {
        let data = fee_state_ai.try_borrow_data()?;
        check!(
            data.len() >= 8 && data[..8] == FeeState::DISCRIMINATOR,
            MarginfiError::InvalidGroup
        );
    }

    grow_account(
        fee_state_ai,
        8 + FeeState::LEN,
        &ctx.accounts.payer,
        &ctx.accounts.system_program,
    )
}

/// Grow `account_ai` to `new_len`, topping up rent from `payer` first (the runtime requires
/// the account to stay rent-exempt). Grow-only; new bytes are zero-filled by `resize`.
fn grow_account<'info>(
    account_ai: &UncheckedAccount<'info>,
    new_len: usize,
    payer: &Signer<'info>,
    system_program: &Program<'info, System>,
) -> MarginfiResult {
    let old_len = account_ai.data_len();
    check!(old_len < new_len, MarginfiError::InvalidResize);

    let required_lamports = Rent::get()?.minimum_balance(new_len);
    let current_lamports = account_ai.lamports();
    if required_lamports > current_lamports {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                system_program.key(),
                anchor_lang::system_program::Transfer {
                    from: payer.to_account_info(),
                    to: account_ai.to_account_info(),
                },
            ),
            required_lamports - current_lamports,
        )?;
    }

    account_ai.resize(new_len)?;

    msg!("account resized: {:?} -> {:?} bytes", old_len, new_len);

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolResizeGroupAccount<'info> {
    /// CHECK: owner + discriminator validated in the handler; not an AccountLoader so an
    /// undersized group can still be resized under the future (larger-struct) program.
    #[account(mut)]
    pub group: UncheckedAccount<'info>,

    /// Funds the rent for the added account space.
    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResizeGlobalFeeState<'info> {
    /// CHECK: PDA address pinned by seeds; owner + discriminator validated in the handler.
    /// Not an AccountLoader so an undersized fee state can still be resized under the future
    /// (larger-struct) program.
    #[account(
        mut,
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: UncheckedAccount<'info>,

    /// Funds the rent for the added account space.
    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}
