use crate::{
    bank_signer, check,
    events::AdminCrossBankSocializeLossEvent,
    math_error,
    prelude::MarginfiError,
    state::bank::{BankImpl, BankVaultType},
    utils::{self, is_marginfi_asset_tag},
    MarginfiResult,
};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use anchor_spl::{
    token::accessor,
    token_interface::{TokenAccount, TokenInterface},
};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::LIQUIDITY_VAULT_AUTHORITY_SEED,
    types::{Bank, BankOperationalState, MarginfiGroup},
};

/// (risk admin only) Socialize a loss across a bank's depositors and transfer tokens out of the
/// bank's vault. Used to spread bad debt from a compromised integration across multiple banks
/// proportionally. No pause checks — must work during incident response.
pub fn lending_pool_admin_cross_bank_socialize<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, AdminCrossBankSocialize<'info>>,
    amount: u64,
) -> MarginfiResult {
    check!(amount > 0, MarginfiError::InvalidTransfer);

    let AdminCrossBankSocialize {
        destination_token_account,
        liquidity_vault: bank_liquidity_vault,
        token_program,
        bank_liquidity_vault_authority,
        bank: bank_loader,
        group: marginfi_group_loader,
        ..
    } = ctx.accounts;

    let clock = Clock::get()?;

    let maybe_bank_mint = {
        let bank = bank_loader.load()?;
        utils::maybe_take_bank_mint(&mut ctx.remaining_accounts, &bank, token_program.key)?
    };

    let mut bank = bank_loader.load_mut()?;
    let group = marginfi_group_loader.load()?;

    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let liquidity_vault_authority_bump = bank.liquidity_vault_authority_bump;
    let loss_amount = I80F48::from_num(amount);

    // Compute total deposit value to cap the socialized amount
    let total_asset_shares: I80F48 = bank.total_asset_shares.into();
    let asset_share_value: I80F48 = bank.asset_share_value.into();
    let total_deposit_value = total_asset_shares
        .checked_mul(asset_share_value)
        .ok_or_else(math_error!())?;

    // Cap at total deposits — can't socialize more than what exists
    let effective_loss = I80F48::min(loss_amount, total_deposit_value);
    let transfer_amount: u64 = effective_loss
        .checked_floor()
        .ok_or_else(math_error!())?
        .checked_to_num()
        .ok_or_else(math_error!())?;

    // Cap at actual vault balance
    let vault_balance = accessor::amount(&bank_liquidity_vault.to_account_info())?;
    let transfer_amount = u64::min(transfer_amount, vault_balance);

    let kill_bank = bank.socialize_loss(effective_loss)?;
    if kill_bank {
        msg!("socialized loss exceeded total deposits, bank killed");
        bank.config.operational_state = BankOperationalState::KilledByBankruptcy;
    }

    if transfer_amount > 0 {
        bank.withdraw_spl_transfer(
            transfer_amount,
            bank_liquidity_vault.to_account_info(),
            destination_token_account.to_account_info(),
            bank_liquidity_vault_authority.to_account_info(),
            maybe_bank_mint.as_ref(),
            token_program.to_account_info(),
            bank_signer!(
                BankVaultType::Liquidity,
                bank_loader.key(),
                liquidity_vault_authority_bump
            ),
            ctx.remaining_accounts,
        )?;
    }

    bank.update_bank_cache(&group)?;

    emit!(AdminCrossBankSocializeLossEvent {
        group: marginfi_group_loader.key(),
        bank: bank_loader.key(),
        mint: bank.mint,
        amount: transfer_amount,
        kill_bank,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct AdminCrossBankSocialize<'info> {
    #[account(
        has_one = risk_admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub risk_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = is_marginfi_asset_tag(bank.load()?.config.asset_tag)
            @ MarginfiError::WrongAssetTagForStandardInstructions,
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Seed constraint check
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump,
    )]
    pub bank_liquidity_vault_authority: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
