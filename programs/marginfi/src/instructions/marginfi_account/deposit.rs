use crate::{
    check,
    events::{AccountEventHeader, LendingAccountDepositEvent},
    prelude::*,
    state::{
        bank::BankImpl,
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::{
        self, is_marginfi_asset_tag, validate_asset_tags, validate_bank_state, InstructionKind,
    },
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use anchor_lang::solana_program::sysvar::Sysvar;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::TOKENLESS_REPAYMENTS_ALLOWED,
    types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED, ACCOUNT_IN_RECEIVERSHIP},
};

/// 1. Accrue interest
/// 2. Create the user's bank account for the asset deposited if it does not exist yet
/// 3. Record asset increase in the bank account
/// 4. Transfer funds from the signer's token account to the bank's liquidity vault
///
/// Will error if there is an existing liability <=> repaying is not allowed.
pub fn lending_account_deposit<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingAccountDeposit<'info>>,
    amount: u64,
    deposit_up_to_limit: Option<bool>,
) -> MarginfiResult {
    let LendingAccountDeposit {
        marginfi_account: marginfi_account_loader,
        authority: signer,
        signer_token_account,
        liquidity_vault: bank_liquidity_vault,
        token_program,
        bank: bank_loader,
        group: marginfi_group_loader,
        ..
    } = ctx.accounts;
    let clock = Clock::get()?;
    let maybe_bank_mint = utils::maybe_take_bank_mint(
        &mut ctx.remaining_accounts,
        &*bank_loader.load()?,
        token_program.key,
    )?;
    let deposit_up_to_limit = deposit_up_to_limit.unwrap_or(false);

    let mut bank = bank_loader.load_mut()?;
    let mut marginfi_account = marginfi_account_loader.load_mut()?;
    let group = &marginfi_group_loader.load()?;
    validate_asset_tags(&bank, &marginfi_account)?;
    validate_bank_state(&bank, InstructionKind::FailsIfPausedOrReduceState)?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED)
            // Sanity check: liquidation doesn't allow the deposit ix, but just in case
            && !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP),
        MarginfiError::AccountDisabled
    );

    let deposit_amount = if deposit_up_to_limit {
        amount.min(bank.get_remaining_deposit_capacity()?)
    } else {
        amount
    };

    if deposit_amount == 0 {
        return Ok(());
    }
    bank.accrue_interest(
        clock.unix_timestamp,
        group,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let mut bank_account = BankAccountWrapper::find_or_create(
        &bank_loader.key(),
        &mut bank,
        &mut marginfi_account.lending_account,
    )?;

    bank_account.deposit(I80F48::from_num(deposit_amount))?;
    marginfi_account.last_update = clock.unix_timestamp as u64;

    let amount_pre_fee = maybe_bank_mint
        .as_ref()
        .map(|mint| {
            utils::calculate_pre_fee_spl_deposit_amount(
                mint.to_account_info(),
                deposit_amount,
                clock.epoch,
            )
        })
        .transpose()?
        .unwrap_or(deposit_amount);

    bank.deposit_spl_transfer(
        amount_pre_fee,
        signer_token_account.to_account_info(),
        bank_liquidity_vault.to_account_info(),
        signer.to_account_info(),
        maybe_bank_mint.as_ref(),
        token_program.to_account_info(),
        ctx.remaining_accounts,
    )?;

    bank.update_bank_cache(group)?;
    emit!(LendingAccountDepositEvent {
        header: AccountEventHeader {
            signer: Some(signer.key()),
            marginfi_account: marginfi_account_loader.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank: bank_loader.key(),
        mint: bank.mint,
        amount: deposit_amount,
    });

    marginfi_account.lending_account.sort_balances();

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountDeposit<'info> {
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
            is_signer_authorized(&a, g.admin, authority.key(), false)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
        constraint = !(bank.load()?.get_flag(TOKENLESS_REPAYMENTS_ALLOWED))
            @ MarginfiError::BankReduceOnly
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: Token mint/authority are checked at transfer
    #[account(mut)]
    pub signer_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
