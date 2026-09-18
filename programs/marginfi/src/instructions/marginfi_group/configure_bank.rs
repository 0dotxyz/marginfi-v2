use crate::constants::MIN_EMISSIONS_SHARE_SUPPLY;
use crate::events::{
    GroupEventHeader, LendingPoolBankConfigureEvent, LendingPoolBankConfigureFrozenEvent,
};
use crate::ix_utils;
use crate::prelude::MarginfiError;
use crate::state::bank::BankImpl;
use crate::state::emode::EmodeSettingsImpl;
use crate::state::marginfi_group::MarginfiGroupImpl;
use crate::MarginfiResult;
use crate::{check, math_error, utils};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::{transfer_checked, TransferChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{CIRCUIT_BREAKER_ENABLED, FREEZE_SETTINGS},
    types::{
        is_marginfi_asset_tag, Bank, BankConfigFast, BankConfigGov, BankConfigOpt,
        BankOperationalState, MarginfiGroup,
    },
};

pub fn lending_pool_configure_bank(
    ctx: Context<LendingPoolConfigureBank>,
    bank_config: BankConfigFast,
) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    check!(
        matches!(
            bank_config.operational_state,
            None | Some(
                BankOperationalState::Paused
                    | BankOperationalState::ReduceOnly
                    | BankOperationalState::ReduceOnlyWithBorrowingPower
            )
        ),
        MarginfiError::InvalidFastBankOperationalState
    );

    let group = ctx.accounts.group.load()?;
    let mut bank = ctx.accounts.bank.load_mut()?;
    configure_bank(
        &mut bank,
        &group,
        bank_config.into(),
        ctx.accounts.group.key(),
        ctx.accounts.bank.key(),
        ctx.accounts.admin.key(),
    )
}

/// Configure governance-controlled bank parameters with the slow, timelocked governance admin.
pub fn lending_pool_configure_bank_gov(
    ctx: Context<LendingPoolConfigureBankGov>,
    bank_config: BankConfigGov,
) -> MarginfiResult {
    ix_utils::check_no_durable_nonce(&ctx.accounts.instruction_sysvar)?;

    check!(
        matches!(
            bank_config.operational_state,
            None | Some(BankOperationalState::Operational)
        ),
        MarginfiError::InvalidGovernanceBankOperationalState
    );

    let group = ctx.accounts.group.load()?;
    let mut bank = ctx.accounts.bank.load_mut()?;
    configure_bank(
        &mut bank,
        &group,
        bank_config.into(),
        ctx.accounts.group.key(),
        ctx.accounts.bank.key(),
        ctx.accounts.governance_admin.key(),
    )
}

fn configure_bank(
    bank: &mut Bank,
    group: &MarginfiGroup,
    bank_config: BankConfigOpt,
    group_key: Pubkey,
    bank_key: Pubkey,
    signer: Pubkey,
) -> MarginfiResult {
    // If settings are frozen, you can only update the deposit and borrow limits, everything else is ignored.
    if bank.get_flag(FREEZE_SETTINGS) {
        bank.configure_unfrozen_fields_only(&bank_config)?;

        msg!("WARN: Only deposit+borrow limits updated. Other settings IGNORED for frozen banks!");

        emit!(LendingPoolBankConfigureFrozenEvent {
            header: GroupEventHeader {
                marginfi_group: group_key,
                signer: Some(signer)
            },
            bank: bank_key,
            mint: bank.mint,
            deposit_limit: bank.config.deposit_limit,
            borrow_limit: bank.config.borrow_limit,
        });
    } else {
        // Consume the interest freeze before the disable: accrual excludes the halted time up to
        // now and restarts from here.
        if bank_config.circuit_breaker_enabled == Some(false)
            && bank.get_flag(CIRCUIT_BREAKER_ENABLED)
        {
            bank.accrue_interest(
                Clock::get()?.unix_timestamp,
                group,
                #[cfg(not(feature = "client"))]
                bank_key,
            )?;
        }

        // Settings are not frozen, everything updates
        bank.configure(&bank_config)?;
        msg!("Bank configured!");

        bank.emode.validate_entries_with_liability_weights(
            &bank.config,
            group.emode_max_init_leverage,
            group.emode_max_maint_leverage,
        )?;

        emit!(LendingPoolBankConfigureEvent {
            header: GroupEventHeader {
                marginfi_group: group_key,
                signer: Some(signer)
            },
            bank: bank_key,
            mint: bank.mint,
            config: bank_config,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBank<'info> {
    #[account(has_one = admin @ MarginfiError::Unauthorized)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBankGov<'info> {
    #[account(has_one = governance_admin @ MarginfiError::Unauthorized)]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub governance_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: instruction sysvar
    #[account(address = solana_instructions_sysvar::id())]
    pub instruction_sysvar: UncheckedAccount<'info>,
}

/// Permissionlessly deposit same-mint emissions directly into the bank liquidity vault,
/// increasing depositor value through asset share value.
pub fn lending_pool_emissions_deposit(
    ctx: Context<LendingPoolEmissionsDeposit>,
    amount: u64,
) -> MarginfiResult {
    if amount == 0 {
        return Ok(());
    }

    let clock = Clock::get()?;
    let mut bank = ctx.accounts.bank.load_mut()?;
    let group = ctx.accounts.group.load()?;

    utils::validate_bank_state(
        &bank,
        utils::InstructionKind::FailsIfPausedOrReduceState,
        true,
    )?;

    // Reject mints with non-zero transfer fees or active transfer hooks.
    let mint_ai = ctx.accounts.mint.to_account_info();
    check!(
        !utils::nonzero_fee(mint_ai.clone(), clock.epoch)?,
        MarginfiError::InvalidTransfer
    );
    check!(
        !utils::has_transfer_hook(mint_ai)?,
        MarginfiError::InvalidTransfer
    );

    // Emissions raise the share value by `amount / total_asset_shares`, so a floor on the supply
    // bounds how far any caller can move it. See `MIN_EMISSIONS_SHARE_SUPPLY`.
    let total_asset_shares = I80F48::from(bank.total_asset_shares);
    check!(
        total_asset_shares >= MIN_EMISSIONS_SHARE_SUPPLY,
        MarginfiError::EmissionsUpdateError,
        "Bank share supply is too small to accept emissions"
    );

    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        ctx.accounts.bank.key(),
    )?;

    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.emissions_funding_account.to_account_info(),
                to: ctx.accounts.liquidity_vault.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
                mint: ctx.accounts.mint.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.mint.decimals,
    )?;

    let total_assets = bank.get_asset_amount(total_asset_shares)?;
    let updated_total_assets = total_assets
        .checked_add(I80F48::from_num(amount))
        .ok_or_else(math_error!())?;

    bank.asset_share_value = updated_total_assets
        .checked_div(total_asset_shares)
        .ok_or_else(math_error!())?
        .into();

    bank.update_bank_cache(&group)?;

    msg!(
        "Deposited {} same-bank emissions into liquidity vault",
        amount
    );

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolEmissionsDeposit<'info> {
    #[account(
        constraint = (
            !group.load()?.is_protocol_paused()
        ) @ MarginfiError::ProtocolPaused
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = mint @ MarginfiError::InvalidEmissionsMint,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub mint: InterfaceAccount<'info, Mint>,

    /// NOTE: This is a TokenAccount, spl transfer will validate it.
    ///
    /// CHECK: Account provided only for funding rewards
    #[account(mut)]
    pub emissions_funding_account: UncheckedAccount<'info>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(mut)]
    pub liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}
