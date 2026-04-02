use crate::{
    bank_signer, check,
    events::{GroupEventHeader, LendingPoolSuperAdminWithdrawEvent},
    math_error,
    prelude::{MarginfiError, MarginfiResult},
    state::bank::{BankImpl, BankVaultType},
    utils::{self, is_marginfi_asset_tag},
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use fixed_macro::types::I80F48;
use marginfi_type_crate::{
    constants::{
        ASSET_TAG_DEFAULT, ASSET_TAG_SOL, LIQUIDITY_VAULT_AUTHORITY_SEED, ZERO_AMOUNT_THRESHOLD,
    },
    types::{Bank, MarginfiGroup},
};

/// Group admin only.
/// Transfers tokens directly from the bank liquidity vault to `destination_token_account` and
/// lowers `asset_share_value` so that existing depositor shares are decreased proportionally.
pub fn super_admin_withdraw<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, SuperAdminWithdraw<'info>>,
    amount: u64,
) -> MarginfiResult {
    if amount == 0 {
        return Ok(());
    }

    let clock = Clock::get()?;
    let SuperAdminWithdraw {
        group: group_loader,
        bank: bank_loader,
        destination_token_account,
        liquidity_vault,
        liquidity_vault_authority,
        token_program,
        admin,
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
    check!(
        total_asset_shares > ZERO_AMOUNT_THRESHOLD,
        MarginfiError::NoAssetFound
    );

    let assets_before = bank.get_asset_amount(total_asset_shares)?;
    let withdrawal_amount = I80F48::from_num(amount);
    check!(
        assets_before >= withdrawal_amount,
        MarginfiError::NoAssetFound
    );

    let assets_after = assets_before
        .checked_sub(withdrawal_amount)
        .ok_or_else(math_error!())?;
    bank.asset_share_value = assets_after
        .checked_div(total_asset_shares)
        .ok_or_else(math_error!())?
        .into();

    let share_value_after: I80F48 = bank.asset_share_value.into();
    if share_value_after <= I80F48!(0.8) {
        panic!("too low, sausage fingers!");
    }

    bank.withdraw_spl_transfer(
        amount,
        liquidity_vault.to_account_info(),
        destination_token_account.to_account_info(),
        liquidity_vault_authority.to_account_info(),
        maybe_bank_mint.as_ref(),
        token_program.to_account_info(),
        bank_signer!(
            BankVaultType::Liquidity,
            bank_loader.key(),
            bank.liquidity_vault_authority_bump
        ),
        ctx.remaining_accounts,
    )?;

    bank.update_bank_cache(&group)?;

    emit!(LendingPoolSuperAdminWithdrawEvent {
        header: GroupEventHeader {
            signer: Some(admin.key()),
            marginfi_group: group_loader.key(),
        },
        bank: bank_loader.key(),
        mint: bank.mint,
        vault_outflow_amount: amount,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SuperAdminWithdraw<'info> {
    #[account(
        has_one = admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidity_vault @ MarginfiError::InvalidLiquidityVault,
        constraint = {
            let b = bank.load()?;
            b.config.asset_tag == ASSET_TAG_DEFAULT || b.config.asset_tag == ASSET_TAG_SOL
        }
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(mut)]
    pub destination_token_account: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: Seed constraint check.
    #[account(
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_authority_bump
    )]
    pub liquidity_vault_authority: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
