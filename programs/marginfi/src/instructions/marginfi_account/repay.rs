use crate::{
    check,
    events::{AccountEventHeader, LendingAccountRepayEvent},
    ix_utils::{get_discrim_hash, Hashable},
    prelude::{MarginfiError, MarginfiResult},
    state::{
        bank::BankImpl,
        marginfi_account::{
            account_not_frozen_for_authority, is_signer_authorized, BankAccountWrapper,
            LendingAccountImpl, MarginfiAccountImpl,
        },
        marginfi_group::MarginfiGroupImpl,
    },
    utils::{self, is_marginfi_asset_tag, validate_bank_state, InstructionKind},
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use marginfi_type_crate::{
    constants::{
        TOKENLESS_REPAYMENTS_ALLOWED, TOKENLESS_REPAYMENTS_COMPLETE, ZERO_AMOUNT_THRESHOLD,
    },
    types::{Bank, MarginfiAccount, MarginfiGroup, ACCOUNT_DISABLED},
};

/// 1. Accrue interest
/// 2. Find the user's existing bank account for the asset repaid
/// 3. Record liability decrease in the bank account
/// 4. Transfer funds from the signer's token account to the bank's liquidity vault
///
/// Will error if there is no existing liability <=> depositing is not allowed.
pub fn lending_account_repay<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingAccountRepay<'info>>,
    amount: u64,
    repay_all: Option<bool>,
) -> MarginfiResult {
    let LendingAccountRepay {
        marginfi_account: marginfi_account_loader,
        authority,
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

    let repay_all = repay_all.unwrap_or(false);
    let mut bank = bank_loader.load_mut()?;
    let mut marginfi_account = marginfi_account_loader.load_mut()?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_DISABLED),
        MarginfiError::AccountDisabled
    );
    validate_bank_state(&bank, InstructionKind::FailsInPausedState)?;

    let group = &marginfi_group_loader.load()?;
    bank.accrue_interest(
        clock.unix_timestamp,
        group,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let lending_account = &mut marginfi_account.lending_account;
    let mut bank_account =
        BankAccountWrapper::find(&bank_loader.key(), &mut bank, lending_account)?;

    let repay_amount_post_fee = if repay_all {
        bank_account.repay_all()?
    } else {
        bank_account.repay(I80F48::from_num(amount))?;

        amount
    };
    marginfi_account.last_update = clock.unix_timestamp as u64;

    if authority.key() == group.risk_admin
        && bank.get_flag(TOKENLESS_REPAYMENTS_ALLOWED)
        && repay_all
    {
        // In some rare cases (e.g. super illiquid token sunset) we allow risk admin
        // to "repay" the debt with nothing. Hence we skip the actual transfer here.

        // repay_all must be enabled: this enables the risk admin to voluntarily pay when it wants,
        // but in general, once the risk admin is prepared to use this feature, there's no point in
        // not repaying the entire balance!

        // Note: Doing this means there will not be enough funds left for lenders to withdraw! This
        // state is irrecoverable. Lenders will be paid out on a first-come-first-served basis as
        // they withdraw. Remaining lenders will either absorb the loss - or more likely - be repaid
        // through some OTC claims portal using assets seized from borrowers
    } else {
        let repay_amount_pre_fee = maybe_bank_mint
            .as_ref()
            .map(|mint| {
                utils::calculate_pre_fee_spl_deposit_amount(
                    mint.to_account_info(),
                    repay_amount_post_fee,
                    clock.epoch,
                )
            })
            .transpose()?
            .unwrap_or(repay_amount_post_fee);

        bank.deposit_spl_transfer(
            repay_amount_pre_fee,
            signer_token_account.to_account_info(),
            bank_liquidity_vault.to_account_info(),
            authority.to_account_info(),
            maybe_bank_mint.as_ref(),
            token_program.to_account_info(),
            ctx.remaining_accounts,
        )?;
    }

    // During deleverage, once the last repayment is complete, and the bank's debts have been fully
    // discharged, the risk admin becomes empowered to purge the balances of lenders
    let liabs: I80F48 = bank.total_liability_shares.into();
    if bank.get_flag(TOKENLESS_REPAYMENTS_ALLOWED)
        && liabs.abs() < ZERO_AMOUNT_THRESHOLD * I80F48!(10)
    {
        bank.update_flag(true, TOKENLESS_REPAYMENTS_COMPLETE);
    }

    bank.update_bank_cache(group)?;
    emit!(LendingAccountRepayEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            marginfi_account: marginfi_account_loader.key(),
            marginfi_account_authority: marginfi_account.authority,
            marginfi_group: marginfi_account.group,
        },
        bank: bank_loader.key(),
        mint: bank.mint,
        amount: repay_amount_post_fee,
        close_balance: repay_all,
    });

    marginfi_account.lending_account.sort_balances();

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountRepay<'info> {
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
            is_signer_authorized(&a, g.admin, authority.key(), true, true)
        } @ MarginfiError::Unauthorized
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// Must be marginfi_account's authority, unless in liquidation/deleverage receivership or order execution
    ///
    /// Note: during receivership and order execution, there are no signer checks whatsoever: any key can repay as
    /// long as the invariants checked at the end of execution are met.
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: Token mint/authority are checked at transfer
    #[account(mut)]
    pub signer_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl Hashable for LendingAccountRepay<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "lending_account_repay")
    }
}
