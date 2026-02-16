use anchor_lang::{prelude::*, Accounts, ToAccountInfo};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};
use marginfi_type_crate::{
    constants::{EMISSIONS_AUTH_SEED, EMISSIONS_TOKEN_ACCOUNT_SEED},
    types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_FROZEN},
};

use crate::{
    check, debug,
    ix_utils::{get_discrim_hash, Hashable},
    prelude::{MarginfiError, MarginfiResult},
    state::{
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, BankAccountWrapper,
            MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::is_marginfi_asset_tag,
};

pub fn lending_account_withdraw_emissions<'info>(
    ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdrawEmissions<'info>>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );

    let mut bank = ctx.accounts.bank.load_mut()?;

    let mut balance = BankAccountWrapper::find(
        ctx.accounts.bank.to_account_info().key,
        &mut bank,
        &mut marginfi_account.lending_account,
    )?;

    // Settle emissions
    let emissions_settle_amount = balance.settle_emissions_and_get_transfer_amount()?;

    if emissions_settle_amount > 0 {
        debug!("Transferring {} emissions to user", emissions_settle_amount);
        marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;

        let signer_seeds: &[&[&[u8]]] = &[&[
            EMISSIONS_AUTH_SEED.as_bytes(),
            &ctx.accounts.bank.key().to_bytes(),
            &ctx.accounts.emissions_mint.key().to_bytes(),
            &[ctx.bumps.emissions_auth],
        ]];

        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.emissions_vault.to_account_info(),
                    to: ctx.accounts.destination_account.to_account_info(),
                    authority: ctx.accounts.emissions_auth.to_account_info(),
                    mint: ctx.accounts.emissions_mint.to_account_info(),
                },
                signer_seeds,
            ),
            emissions_settle_amount,
            ctx.accounts.emissions_mint.decimals,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountWithdrawEmissions<'info> {
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
            let a = marginfi_account.load()?;
            account_not_frozen_for_authority(&a, authority.key())
        } @ MarginfiError::AccountFrozen,
        constraint = {
            let a = marginfi_account.load()?;
            let g = group.load()?;
            is_signer_authorized(&a, g.admin, authority.key(), false, false)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = emissions_mint @ MarginfiError::InvalidEmissionsMint,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: PDA seeds validated
    #[account(
        seeds = [
            EMISSIONS_AUTH_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump
    )]
    pub emissions_auth: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump,
    )]
    pub emissions_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub destination_account: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
}

impl Hashable for LendingAccountWithdrawEmissions<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "lending_account_withdraw_emissions")
    }
}

/// Permissionlessly settle unclaimed emissions to a users account.
pub fn lending_account_settle_emissions(
    ctx: Context<LendingAccountSettleEmissions>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let mut bank = ctx.accounts.bank.load_mut()?;

    let mut balance = BankAccountWrapper::find(
        ctx.accounts.bank.to_account_info().key,
        &mut bank,
        &mut marginfi_account.lending_account,
    )?;

    let current_timestamp = Clock::get()?.unix_timestamp as u64;
    balance.claim_emissions(current_timestamp)?;
    marginfi_account.last_update = current_timestamp;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountSettleEmissions<'info> {
    #[account(
        mut,
        constraint = marginfi_account.load()?.group == bank.load()?.group,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        mut,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,
}

impl Hashable for LendingAccountSettleEmissions<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "lending_account_settle_emissions")
    }
}

/// emissions rewards will be withdrawn to the emissions_destination_account
pub fn marginfi_account_update_emissions_destination_account<'info>(
    ctx: Context<'_, '_, 'info, 'info, MarginfiAccountUpdateEmissionsDestinationAccount<'info>>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );

    check!(
        !marginfi_account.get_flag(ACCOUNT_FROZEN),
        MarginfiError::AccountFrozen
    );
    marginfi_account.emissions_destination_account = ctx.accounts.destination_account.key();
    marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;

    Ok(())
}

#[derive(Accounts)]
pub struct MarginfiAccountUpdateEmissionsDestinationAccount<'info> {
    #[account(
        mut,
        has_one = authority @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    /// User's earned emissions will be sent to the canonical ATA of this wallet.
    ///
    /// CHECK: Completely unchecked, user picks a destination without restrictions
    pub destination_account: AccountInfo<'info>,
}

/// Permissionlessly withdraw emissions to user emissions_destination_account
pub fn lending_account_withdraw_emissions_permissionless<'info>(
    ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdrawEmissionsPermissionless<'info>>,
) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );
    check!(
        !marginfi_account.get_flag(ACCOUNT_FROZEN),
        MarginfiError::AccountFrozen
    );

    let emissions_dest_wallet = &marginfi_account.emissions_destination_account;
    let emissions_mint = &ctx.accounts.emissions_mint.key();
    let emissions_token_program = &ctx.accounts.token_program.key();

    // Ensure that the emissions_destination_account was previously set by the user
    check!(
        !emissions_dest_wallet.eq(&Pubkey::default()),
        MarginfiError::InvalidEmissionsDestinationAccount
    );

    // Ensure the destination is the canonical ATA of the user-specified wallet
    let ata_expected = get_associated_token_address_with_program_id(
        emissions_dest_wallet,
        emissions_mint,
        emissions_token_program,
    );
    check!(
        ata_expected == ctx.accounts.destination_account.key(),
        MarginfiError::InvalidEmissionsDestinationAccount
    );

    let mut bank = ctx.accounts.bank.load_mut()?;

    let mut bank_account = BankAccountWrapper::find(
        ctx.accounts.bank.to_account_info().key,
        &mut bank,
        &mut marginfi_account.lending_account,
    )?;

    // Settle emissions
    let emissions_settle_amount = bank_account.settle_emissions_and_get_transfer_amount()?;

    if emissions_settle_amount > 0 {
        debug!("Transferring {} emissions to user", emissions_settle_amount);
        marginfi_account.last_update = Clock::get()?.unix_timestamp as u64;

        let signer_seeds: &[&[&[u8]]] = &[&[
            EMISSIONS_AUTH_SEED.as_bytes(),
            &ctx.accounts.bank.key().to_bytes(),
            &ctx.accounts.emissions_mint.key().to_bytes(),
            &[ctx.bumps.emissions_auth],
        ]];

        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.emissions_vault.to_account_info(),
                    to: ctx.accounts.destination_account.to_account_info(),
                    authority: ctx.accounts.emissions_auth.to_account_info(),
                    mint: ctx.accounts.emissions_mint.to_account_info(),
                },
                signer_seeds,
            ),
            emissions_settle_amount,
            ctx.accounts.emissions_mint.decimals,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountWithdrawEmissionsPermissionless<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
        ) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = emissions_mint @ MarginfiError::InvalidEmissionsMint,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [
            EMISSIONS_AUTH_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump
    )]
    /// CHECK: Asserted by PDA
    pub emissions_auth: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump,
    )]
    pub emissions_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Handler will validate this is a canonical ATA of the `emissions_destination_account`
    /// registered on `marginfi_account`
    #[account(mut)]
    pub destination_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl Hashable for LendingAccountWithdrawEmissionsPermissionless<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash(
            "global",
            "lending_account_withdraw_emissions_permissionless",
        )
    }
}
