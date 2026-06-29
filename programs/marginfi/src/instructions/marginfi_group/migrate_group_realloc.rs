use crate::prelude::MarginfiError;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use marginfi_type_crate::types::MarginfiGroup;

/// Reallocate MarginfiGroup to accommodate timelocked admin fields. Admin only.
pub fn migrate_group_realloc(ctx: Context<MigrateGroupRealloc>) -> MarginfiResult {
    let account_info = ctx.accounts.marginfi_group.to_account_info();
    let current_size = account_info.data_len();
    let required_size = 8 + std::mem::size_of::<MarginfiGroup>();

    if current_size >= required_size {
        msg!(
            "MarginfiGroup already {} bytes (required {}). No realloc needed.",
            current_size,
            required_size
        );
        return Ok(());
    }

    msg!(
        "Reallocating MarginfiGroup from {} to {} bytes",
        current_size,
        required_size
    );

    let rent = Rent::get()?;
    let current_lamports = account_info.lamports();
    let required_lamports = rent.minimum_balance(required_size);
    let lamports_needed = required_lamports.saturating_sub(current_lamports);

    if lamports_needed > 0 {
        msg!("Transferring {} lamports for rent", lamports_needed);

        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.fee_payer.to_account_info(),
                    to: account_info.clone(),
                },
            ),
            lamports_needed,
        )?;
    }

    account_info.realloc(required_size, true)?;

    // Verify fields initialized correctly
    {
        let group = ctx.accounts.marginfi_group.load()?;
        require!(
            group.timelocked_admin == Pubkey::default(),
            MarginfiError::InvalidConfig
        );
        require!(
            group.timelocked_operation_delay_seconds == 0,
            MarginfiError::InvalidConfig
        );
    }

    msg!(
        "Successfully reallocated MarginfiGroup to {} bytes.",
        required_size
    );

    Ok(())
}

#[derive(Accounts)]
pub struct MigrateGroupRealloc<'info> {
    #[account(
        mut,
        has_one = admin @ MarginfiError::Unauthorized
    )]
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}
