use crate::{
    check, check_eq,
    constants::DEFAULT_ACCOUNT_TRANSFER_FEE_LAMPORTS,
    events::{AccountEventHeader, MarginfiAccountTransferToNewAccount},
    ix_utils::is_allowed_cpi_for_third_party_id,
    prelude::*,
    state::{
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
};
use anchor_lang::prelude::*;
use bytemuck::Zeroable;
use marginfi_type_crate::{
    constants::{FEE_STATE_SEED, MARGINFI_ACCOUNT_SEED},
    types::{
        FeeState, LendingAccount, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED,
        ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_ORDER_EXECUTION, ACCOUNT_IN_RECEIVERSHIP,
    },
};

fn initialize_migrated_account(
    new_account: &mut MarginfiAccount,
    old_account: &MarginfiAccount,
    new_authority: Pubkey,
    current_timestamp: u64,
    old_account_key: Pubkey,
) {
    new_account.initialize(old_account.group, new_authority, current_timestamp);
    new_account.lending_account = old_account.lending_account;
    new_account.emissions_destination_account = old_account.emissions_destination_account;
    new_account.account_flags = old_account.account_flags;
    new_account.migrated_from = old_account_key;
    new_account.indexer_flags = old_account.indexer_flags;
    new_account.sync_indexer_flags();
}

fn finalize_migrated_old_account(
    old_account: &mut MarginfiAccount,
    new_account_key: Pubkey,
    current_timestamp: u64,
) {
    old_account.migrated_to = new_account_key;
    old_account.last_update = current_timestamp;
    old_account.lending_account = LendingAccount::zeroed();
    old_account.set_flag(ACCOUNT_DISABLED, true);
    old_account.sync_indexer_flags();
}

pub fn transfer_to_new_account(ctx: Context<TransferToNewAccount>) -> MarginfiResult {
    // Validate the global fee wallet and claim a nominal fee
    let group = ctx.accounts.group.load()?;
    check_eq!(
        ctx.accounts.global_fee_wallet.key(),
        group.fee_state_cache.global_fee_wallet,
        MarginfiError::InvalidFeeAta
    );
    // Fee amount (in lamports) is configured on the FeeState (0 => use the default, preserving the
    // legacy fee).
    let fee_lamports = match ctx.accounts.fee_state.load()?.account_transfer_fee {
        0 => DEFAULT_ACCOUNT_TRANSFER_FEE_LAMPORTS,
        fee => fee,
    } as u64;
    anchor_lang::system_program::transfer(ctx.accounts.transfer_fee(), fee_lamports)?;

    let mut old_account = ctx.accounts.old_marginfi_account.load_mut()?;

    check!(
        !old_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );

    check!(
        !old_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
        MarginfiError::ForbiddenIx
    );

    check!(
        !old_account.get_flag(ACCOUNT_IN_ORDER_EXECUTION),
        MarginfiError::ForbiddenIx
    );

    check!(
        old_account.active_orders == 0,
        MarginfiError::IllegalAction,
        "Close all active orders before transfer"
    );

    // Prevent multiple migrations from the same account
    check_eq!(
        old_account.migrated_to,
        Pubkey::default(),
        MarginfiError::AccountAlreadyMigrated
    );

    let mut new_account = ctx.accounts.new_marginfi_account.load_init()?;
    let current_timestamp = Clock::get()?.unix_timestamp as u64;
    initialize_migrated_account(
        &mut new_account,
        &old_account,
        ctx.accounts.new_authority.key(),
        current_timestamp,
        ctx.accounts.old_marginfi_account.key(),
    );

    finalize_migrated_old_account(
        &mut old_account,
        ctx.accounts.new_marginfi_account.key(),
        current_timestamp,
    );

    emit!(MarginfiAccountTransferToNewAccount {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.new_marginfi_account.key(),
            marginfi_account_authority: ctx.accounts.new_authority.key(),
            marginfi_group: ctx.accounts.group.key(),
        },
        old_account: ctx.accounts.old_marginfi_account.key(),
        old_account_authority: ctx.accounts.authority.key(),
        new_account_authority: ctx.accounts.new_authority.key(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct TransferToNewAccount<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
        ) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let a = old_marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = old_marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub old_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        init,
        payer = fee_payer,
        space = 8 + std::mem::size_of::<MarginfiAccount>()
    )]
    pub new_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// CHECK: WARN: New authority is completely unchecked
    pub new_authority: UncheckedAccount<'info>,

    /// CHECK: Validated against group fee state cache
    #[account(mut)]
    pub global_fee_wallet: UncheckedAccount<'info>,

    // Note: there is just one FeeState per program. Read here for the configurable transfer fee.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    pub system_program: Program<'info, System>,
}

impl<'info> TransferToNewAccount<'info> {
    fn transfer_fee(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, anchor_lang::system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.key(),
            anchor_lang::system_program::Transfer {
                from: self.fee_payer.to_account_info(),
                to: self.global_fee_wallet.to_account_info(),
            },
        )
    }
}

pub fn transfer_to_new_account_pda(
    ctx: Context<TransferToNewAccountPda>,
    account_index: u16,
    third_party_id: Option<u16>,
) -> MarginfiResult {
    // Validate the global fee wallet and claim a nominal fee
    let group = ctx.accounts.group.load()?;
    check_eq!(
        ctx.accounts.global_fee_wallet.key(),
        group.fee_state_cache.global_fee_wallet,
        MarginfiError::InvalidFeeAta
    );
    // Fee amount (in lamports) is configured on the FeeState (0 => use the default, preserving the
    // legacy fee).
    let fee_lamports = match ctx.accounts.fee_state.load()?.account_transfer_fee {
        0 => DEFAULT_ACCOUNT_TRANSFER_FEE_LAMPORTS,
        fee => fee,
    } as u64;
    anchor_lang::system_program::transfer(ctx.accounts.transfer_fee(), fee_lamports)?;

    let mut old_account = ctx.accounts.old_marginfi_account.load_mut()?;

    check!(
        !old_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        MarginfiError::AccountInFlashloan
    );

    check!(
        !old_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
        MarginfiError::ForbiddenIx
    );

    check!(
        !old_account.get_flag(ACCOUNT_IN_ORDER_EXECUTION),
        MarginfiError::ForbiddenIx
    );

    check!(
        old_account.active_orders == 0,
        MarginfiError::IllegalAction,
        "Close all active orders before transfer"
    );

    // Prevent multiple migrations from the same account
    check_eq!(
        old_account.migrated_to,
        Pubkey::default(),
        MarginfiError::AccountAlreadyMigrated
    );

    // Validate third-party id restriction if provided
    if let Some(id) = third_party_id {
        if !is_allowed_cpi_for_third_party_id(&ctx.accounts.instructions_sysvar, id)? {
            return err!(MarginfiError::Unauthorized);
        }
    }

    let mut new_account = ctx.accounts.new_marginfi_account.load_init()?;
    let current_timestamp = Clock::get()?.unix_timestamp as u64;
    initialize_migrated_account(
        &mut new_account,
        &old_account,
        ctx.accounts.new_authority.key(),
        current_timestamp,
        ctx.accounts.old_marginfi_account.key(),
    );
    new_account.account_index = account_index;
    new_account.third_party_index = third_party_id.unwrap_or(0);
    new_account.bump = ctx.bumps.new_marginfi_account;

    finalize_migrated_old_account(
        &mut old_account,
        ctx.accounts.new_marginfi_account.key(),
        current_timestamp,
    );

    emit!(MarginfiAccountTransferToNewAccount {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: ctx.accounts.new_marginfi_account.key(),
            marginfi_account_authority: ctx.accounts.new_authority.key(),
            marginfi_group: ctx.accounts.group.key(),
        },
        old_account: ctx.accounts.old_marginfi_account.key(),
        old_account_authority: ctx.accounts.authority.key(),
        new_account_authority: ctx.accounts.new_authority.key(),
    });

    Ok(())
}

#[derive(Accounts)]
#[instruction(account_index: u16, third_party_id: Option<u16>)]
pub struct TransferToNewAccountPda<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
        ) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        constraint = {
            let a = old_marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = old_marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub old_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        init,
        payer = fee_payer,
        space = 8 + std::mem::size_of::<MarginfiAccount>(),
        seeds = [
            MARGINFI_ACCOUNT_SEED.as_bytes(),
            group.key().as_ref(),
            new_authority.key().as_ref(),
            &account_index.to_le_bytes(),
            &third_party_id.unwrap_or(0).to_le_bytes(),
        ],
        bump
    )]
    pub new_marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// CHECK: WARN: New authority is completely unchecked
    pub new_authority: UncheckedAccount<'info>,

    /// CHECK: Validated against group fee state cache
    #[account(mut)]
    pub global_fee_wallet: UncheckedAccount<'info>,

    // Note: there is just one FeeState per program. Read here for the configurable transfer fee.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    /// Instructions sysvar for CPI validation
    /// CHECK: Standard sysvar account
    #[account(address = solana_instructions_sysvar::id())]
    pub instructions_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl<'info> TransferToNewAccountPda<'info> {
    fn transfer_fee(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, anchor_lang::system_program::Transfer<'info>> {
        CpiContext::new(
            self.system_program.key(),
            anchor_lang::system_program::Transfer {
                from: self.fee_payer.to_account_info(),
                to: self.global_fee_wallet.to_account_info(),
            },
        )
    }
}
