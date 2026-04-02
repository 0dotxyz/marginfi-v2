use crate::{
    check,
    events::{GroupEventHeader, LendingPoolSuperAdminDepositEvent},
    math_error,
    prelude::{MarginfiError, MarginfiResult},
    state::bank::BankImpl,
    utils,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::ZERO_AMOUNT_THRESHOLD,
    types::{Bank, MarginfiGroup},
};

/// Group admin only.
/// Transfers tokens from `admin_token_account` into the bank liquidity vault and raises
/// `asset_share_value` so that existing depositor shares are increased proportionally.
pub fn super_admin_deposit<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, SuperAdminDeposit<'info>>,
    amount: u64,
) -> MarginfiResult {
    if amount == 0 {
        return Ok(());
    }

    let clock = Clock::get()?;
    let SuperAdminDeposit {
        group: group_loader,
        bank: bank_loader,
        admin,
        admin_token_account,
        liquidity_vault,
        token_program,
    } = &ctx.accounts;

    let maybe_bank_mint = {
        let bank = bank_loader.load()?;
        utils::maybe_take_bank_mint(&mut ctx.remaining_accounts, &bank, token_program.key)?
    };

    let group = group_loader.load()?;
    let mut bank = bank_loader.load_mut()?;
    bank.accrue_interest(
        clock.unix_timestamp,
        &group,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let total_asset_shares: I80F48 = bank.total_asset_shares.into();
    check!(total_asset_shares > ZERO_AMOUNT_THRESHOLD, MarginfiError::NoAssetFound);

    let assets_before = bank.get_asset_amount(total_asset_shares)?;
    let amount_post_fee = maybe_bank_mint
        .as_ref()
        .map(|mint| {
            utils::calculate_post_fee_spl_deposit_amount(
                mint.to_account_info(),
                amount,
                clock.epoch,
            )
        })
        .transpose()?
        .unwrap_or(amount);

    let assets_after = assets_before
        .checked_add(I80F48::from_num(amount_post_fee))
        .ok_or_else(math_error!())?;
    bank.asset_share_value = assets_after
        .checked_div(total_asset_shares)
        .ok_or_else(math_error!())?
        .into();

    bank.deposit_spl_transfer(
        amount,
        admin_token_account.to_account_info(),
        liquidity_vault.to_account_info(),
        admin.to_account_info(),
        maybe_bank_mint.as_ref(),
        token_program.to_account_info(),
        ctx.remaining_accounts,
    )?;

    bank.update_bank_cache(&group)?;

    emit!(LendingPoolSuperAdminDepositEvent {
        header: GroupEventHeader {
            signer: Some(admin.key()),
            marginfi_group: group_loader.key(),
        },
        bank: bank_loader.key(),
        mint: bank.mint,
        transfer_amount: amount,
        vault_inflow_amount: amount_post_fee,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SuperAdminDeposit<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: token mint / authority validated by SPL transfer call.
    #[account(mut)]
    pub admin_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
