use crate::events::{GroupEventHeader, LendingPoolPremiumFeesCollectedEvent};
use crate::state::bank::{BankImpl, BankVaultType};
use crate::state::marginfi_group::MarginfiGroupImpl;
use crate::{bank_signer, math_error, MarginfiResult};
use crate::{check, utils, MarginfiError};
use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::{FEE_STATE_V2_SEED, LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED},
    types::{Bank, FeeStateV2, MarginfiGroup},
};
use std::cmp::min;

/// (Permissionless) Sweep realized variable-borrow premium from the bank's liquidity vault to
/// the canonical ATA of `FeeStateV2.premium_wallet` for the bank's mint.
///
/// `collected_premium_outstanding` only ever counts premium that arrived as real repayment
/// tokens, so the sweep never takes lenders' liquidity; the `min` with the vault balance is
/// belt-and-braces for transfer-fee mints.
pub fn lending_pool_collect_bank_premium_fees<'info>(
    mut ctx: Context<'info, LendingPoolCollectBankPremiumFees<'info>>,
) -> MarginfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    // Validate the premium ata is correct
    {
        let premium_wallet = &ctx.accounts.fee_state_v2.load()?.premium_wallet;
        check!(
            premium_wallet != &Pubkey::default(),
            MarginfiError::PremiumWalletNotSet
        );
        let ata_expected = get_associated_token_address_with_program_id(
            premium_wallet,
            &bank.mint,
            &ctx.accounts.token_program.key(),
        );
        check!(
            ctx.accounts.premium_ata.key().eq(&ata_expected),
            MarginfiError::InvalidPremiumAta
        );
    }

    let LendingPoolCollectBankPremiumFees {
        liquidity_vault_authority,
        liquidity_vault,
        premium_ata,
        token_program,
        ..
    } = ctx.accounts;

    let maybe_bank_mint =
        utils::maybe_take_bank_mint(&mut ctx.remaining_accounts, &bank, token_program.key)?;

    let available_liquidity = I80F48::from_num(liquidity_vault.amount);
    let outstanding = I80F48::from(bank.collected_premium_outstanding);
    let transfer_amount = min(outstanding, available_liquidity).int();
    let new_outstanding = outstanding
        .checked_sub(transfer_amount)
        .ok_or_else(math_error!())?;

    bank.collected_premium_outstanding = new_outstanding.into();

    bank.withdraw_spl_transfer(
        transfer_amount.checked_to_num().ok_or_else(math_error!())?,
        liquidity_vault.to_account_info(),
        premium_ata.to_account_info(),
        liquidity_vault_authority.to_account_info(),
        maybe_bank_mint.as_ref(),
        token_program.to_account_info(),
        bank_signer!(
            BankVaultType::Liquidity,
            ctx.accounts.bank.key(),
            bank.liquidity_vault_authority_bump
        ),
        ctx.remaining_accounts,
    )?;

    emit!(LendingPoolPremiumFeesCollectedEvent {
        header: GroupEventHeader {
            marginfi_group: ctx.accounts.group.key(),
            signer: None
        },
        bank: ctx.accounts.bank.key(),
        mint: liquidity_vault.mint,
        premium_collected: transfer_amount.to_num::<f64>(),
        premium_outstanding: new_outstanding.to_num::<f64>(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolCollectBankPremiumFees<'info> {
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
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: validated by seeds
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_bump
    )]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    // Note: there is just one FeeStateV2 per program, so no further check is required.
    #[account(
        seeds = [FEE_STATE_V2_SEED.as_bytes()],
        bump,
    )]
    pub fee_state_v2: AccountLoader<'info, FeeStateV2>,

    /// Canonical ATA of the `FeeStateV2.premium_wallet` for the mint used by this bank
    /// (validated in handler). Must already exist.
    #[account(mut)]
    pub premium_ata: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
