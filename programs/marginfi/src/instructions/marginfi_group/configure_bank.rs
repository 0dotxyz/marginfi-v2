use crate::check;
use crate::events::{
    GroupEventHeader, LendingPoolBankConfigureEvent, LendingPoolBankConfigureFrozenEvent,
};
use crate::prelude::MarginfiError;
use crate::state::bank::BankImpl;
use crate::state::emode::EmodeSettingsImpl;
use crate::MarginfiResult;
use anchor_lang::prelude::*;
use anchor_spl::token_2022::{transfer_checked, TransferChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{
        EMISSIONS_AUTH_SEED, EMISSIONS_TOKEN_ACCOUNT_SEED, EMISSION_FLAGS, FREEZE_SETTINGS,
    },
    types::{Bank, BankConfigOpt, MarginfiGroup},
};

pub fn lending_pool_configure_bank(
    ctx: Context<LendingPoolConfigureBank>,
    bank_config: BankConfigOpt,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    // If settings are frozen, you can only update the deposit and borrow limits, everything else is ignored.
    if bank.get_flag(FREEZE_SETTINGS) {
        bank.configure_unfrozen_fields_only(&bank_config)?;

        msg!("WARN: Only deposit+borrow limits updated. Other settings IGNORED for frozen banks!");

        emit!(LendingPoolBankConfigureFrozenEvent {
            header: GroupEventHeader {
                marginfi_group: ctx.accounts.group.key(),
                signer: Some(*ctx.accounts.admin.key)
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            deposit_limit: bank.config.deposit_limit,
            borrow_limit: bank.config.borrow_limit,
        });
    } else {
        // Settings are not frozen, everything updates
        bank.configure(&bank_config)?;
        msg!("Bank configured!");

        let group = ctx.accounts.group.load()?;
        bank.emode.validate_entries_with_liability_weights(
            &bank.config,
            group.emode_max_init_leverage,
            group.emode_max_maint_leverage,
        )?;

        emit!(LendingPoolBankConfigureEvent {
            header: GroupEventHeader {
                marginfi_group: ctx.accounts.group.key(),
                signer: Some(*ctx.accounts.admin.key)
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            config: bank_config,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBank<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,
}

/// (delegate_emissions_admin only) Reclaim all remaining tokens from the
/// emissions vault and disable emissions on the bank.
pub fn lending_pool_reclaim_emissions_vault(
    ctx: Context<LendingPoolReclaimEmissionsVault>,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(
        bank.emissions_mint.ne(&Pubkey::default()),
        MarginfiError::EmissionsUpdateError,
        "Emissions were never set up on this bank"
    );

    let vault_balance = ctx.accounts.emissions_vault.amount;

    if vault_balance > 0 {
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
            vault_balance,
            ctx.accounts.emissions_mint.decimals,
        )?;
    }

    bank.emissions_remaining = I80F48::ZERO.into();
    bank.emissions_rate = 0;
    bank.flags &= !EMISSION_FLAGS;

    msg!(
        "Reclaimed {} tokens from emissions vault for bank {}",
        vault_balance,
        ctx.accounts.bank.key()
    );

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolReclaimEmissionsVault<'info> {
    #[account(
        has_one = delegate_emissions_admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub delegate_emissions_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = emissions_mint @ MarginfiError::InvalidEmissionsMint,
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: Asserted by PDA constraints
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

    #[account(
        mut,
        constraint = destination_account.mint == emissions_mint.key()
            @ MarginfiError::InvalidEmissionsMint,
    )]
    pub destination_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}
