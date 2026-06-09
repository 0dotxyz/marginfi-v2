use crate::ix_utils::is_allowed_cpi_for_third_party_id;
use crate::state::marginfi_account::MarginfiAccountImpl;
use crate::{
    events::{AccountEventHeader, MarginfiAccountCreateEvent},
    prelude::*,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::MARGINFI_ACCOUNT_SEED,
    types::{MarginfiAccount, MarginfiGroup},
};

pub fn initialize_account(ctx: Context<MarginfiAccountInitialize>) -> MarginfiResult {
    let MarginfiAccountInitialize {
        authority,
        marginfi_group,
        marginfi_account: marginfi_account_loader,
        ..
    } = ctx.accounts;

    let mut marginfi_account = marginfi_account_loader.load_init()?;

    marginfi_account.initialize(
        marginfi_group.key(),
        authority.key(),
        Clock::get()?.unix_timestamp as u64,
    );

    emit!(MarginfiAccountCreateEvent {
        header: AccountEventHeader {
            signer: Some(authority.key()),
            marginfi_account: marginfi_account_loader.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        }
    });

    Ok(())
}

#[derive(Accounts)]
pub struct MarginfiAccountInitialize<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        init,
        payer = fee_payer,
        space = 8 + std::mem::size_of::<MarginfiAccount>()
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_account_pda(
    ctx: Context<MarginfiAccountInitializePda>,
    account_index: u16,
    third_party_id: Option<u16>,
) -> MarginfiResult {
    let MarginfiAccountInitializePda {
        authority,
        marginfi_group,
        marginfi_account: marginfi_account_loader,
        ..
    } = ctx.accounts;

    if let Some(id) = third_party_id {
        if !is_allowed_cpi_for_third_party_id(&ctx.accounts.instructions_sysvar, id)? {
            return err!(MarginfiError::Unauthorized);
        }
    }

    let mut marginfi_account = marginfi_account_loader.load_init()?;

    marginfi_account.initialize(
        marginfi_group.key(),
        authority.key(),
        Clock::get()?.unix_timestamp as u64,
    );
    marginfi_account.account_index = account_index;
    marginfi_account.third_party_index = third_party_id.unwrap_or(0);
    marginfi_account.bump = ctx.bumps.marginfi_account;

    emit!(MarginfiAccountCreateEvent {
        header: AccountEventHeader {
            signer: Some(authority.key()),
            marginfi_account: marginfi_account_loader.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        }
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(account_index: u16, third_party_id: Option<u16>)]
pub struct MarginfiAccountInitializePda<'info> {
    pub marginfi_group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        init,
        payer = fee_payer,
        space = 8 + std::mem::size_of::<MarginfiAccount>(),
        seeds = [
            MARGINFI_ACCOUNT_SEED.as_bytes(),
            marginfi_group.key().as_ref(),
            authority.key().as_ref(),
            &account_index.to_le_bytes(),
            &third_party_id.unwrap_or(0).to_le_bytes(),
        ],
        bump
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// Instructions sysvar for CPI validation
    ///
    /// CHECK: Standard sysvar account
    #[account(address = solana_instructions_sysvar::id())]
    pub instructions_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
