use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
};

use crate::ID;

// Clock is not needed for these instructions - verified against solend-sdk

// Instruction discriminators from Solend
pub const INIT_OBLIGATION_DISCRIMINATOR: u8 = 6;
pub const DEPOSIT_DISCRIMINATOR: u8 = 14;
pub const WITHDRAW_DISCRIMINATOR: u8 = 15;

pub mod accounts {
    pub use super::DepositReserveLiquidityAndObligationCollateral;
    pub use super::InitObligation;
    pub use super::WithdrawObligationCollateralAndRedeemReserveCollateral;
}

// Account structs
#[derive(Accounts)]
pub struct InitObligation<'info> {
    #[account(mut)]
    pub obligation_info: UncheckedAccount<'info>,
    pub lending_market_info: UncheckedAccount<'info>,
    #[account(signer)]
    pub obligation_owner_info: UncheckedAccount<'info>,
    pub rent_info: UncheckedAccount<'info>,
    pub token_program_info: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct DepositReserveLiquidityAndObligationCollateral<'info> {
    #[account(mut)]
    pub source_liquidity_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub user_collateral_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub reserve_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub reserve_liquidity_supply_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub reserve_collateral_mint_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub lending_market_info: UncheckedAccount<'info>,
    pub lending_market_authority_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub destination_deposit_collateral_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub obligation_info: UncheckedAccount<'info>,
    #[account(signer)]
    pub obligation_owner_info: UncheckedAccount<'info>,
    pub pyth_price_info: UncheckedAccount<'info>,
    pub switchboard_feed_info: UncheckedAccount<'info>,
    #[account(signer)]
    pub user_transfer_authority_info: UncheckedAccount<'info>,
    pub token_program_info: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct WithdrawObligationCollateralAndRedeemReserveCollateral<'info> {
    #[account(mut)]
    pub source_collateral_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub destination_collateral_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub reserve_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub obligation_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub lending_market_info: UncheckedAccount<'info>,
    pub lending_market_authority_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub destination_liquidity_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub reserve_collateral_mint_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub reserve_liquidity_supply_info: UncheckedAccount<'info>,
    #[account(signer)]
    pub obligation_owner_info: UncheckedAccount<'info>,
    #[account(signer)]
    pub user_transfer_authority_info: UncheckedAccount<'info>,
    pub token_program_info: UncheckedAccount<'info>,
    #[account(mut)]
    pub deposit_reserve_info: UncheckedAccount<'info>,
}

// CPI functions
// Note: Could reduce duplication by implementing ToAccountMetas trait on account structs
pub fn init_obligation<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, InitObligation<'info>>,
) -> Result<()> {
    let ix = Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new(*ctx.accounts.obligation_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.lending_market_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.obligation_owner_info.key, true),
            AccountMeta::new_readonly(*ctx.accounts.rent_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.token_program_info.key, false),
        ],
        data: vec![INIT_OBLIGATION_DISCRIMINATOR],
    };

    invoke_signed(
        &ix,
        &[
            ctx.accounts.obligation_info.to_account_info(),
            ctx.accounts.lending_market_info.to_account_info(),
            ctx.accounts.obligation_owner_info.to_account_info(),
            ctx.accounts.rent_info.to_account_info(),
            ctx.accounts.token_program_info.to_account_info(),
        ],
        ctx.signer_seeds,
    )?;

    Ok(())
}

pub fn deposit_reserve_liquidity_and_obligation_collateral<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, DepositReserveLiquidityAndObligationCollateral<'info>>,
    liquidity_amount: u64,
) -> Result<()> {
    let mut data = vec![DEPOSIT_DISCRIMINATOR];
    data.extend_from_slice(&liquidity_amount.to_le_bytes());

    let ix = Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new(*ctx.accounts.source_liquidity_info.key, false),
            AccountMeta::new(*ctx.accounts.user_collateral_info.key, false),
            AccountMeta::new(*ctx.accounts.reserve_info.key, false),
            AccountMeta::new(*ctx.accounts.reserve_liquidity_supply_info.key, false),
            AccountMeta::new(*ctx.accounts.reserve_collateral_mint_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.lending_market_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.lending_market_authority_info.key, false),
            AccountMeta::new(*ctx.accounts.destination_deposit_collateral_info.key, false),
            AccountMeta::new(*ctx.accounts.obligation_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.obligation_owner_info.key, true),
            AccountMeta::new_readonly(*ctx.accounts.pyth_price_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.switchboard_feed_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.user_transfer_authority_info.key, true),
            AccountMeta::new_readonly(*ctx.accounts.token_program_info.key, false),
        ],
        data,
    };

    invoke_signed(
        &ix,
        &[
            ctx.accounts.source_liquidity_info.to_account_info(),
            ctx.accounts.user_collateral_info.to_account_info(),
            ctx.accounts.reserve_info.to_account_info(),
            ctx.accounts.reserve_liquidity_supply_info.to_account_info(),
            ctx.accounts.reserve_collateral_mint_info.to_account_info(),
            ctx.accounts.lending_market_info.to_account_info(),
            ctx.accounts.lending_market_authority_info.to_account_info(),
            ctx.accounts.destination_deposit_collateral_info.to_account_info(),
            ctx.accounts.obligation_info.to_account_info(),
            ctx.accounts.obligation_owner_info.to_account_info(),
            ctx.accounts.pyth_price_info.to_account_info(),
            ctx.accounts.switchboard_feed_info.to_account_info(),
            ctx.accounts.user_transfer_authority_info.to_account_info(),
            ctx.accounts.token_program_info.to_account_info(),
        ],
        ctx.signer_seeds,
    )?;

    Ok(())
}

pub fn withdraw_obligation_collateral_and_redeem_reserve_collateral<'info>(
    ctx: CpiContext<
        '_,
        '_,
        '_,
        'info,
        WithdrawObligationCollateralAndRedeemReserveCollateral<'info>,
    >,
    collateral_amount: u64,
) -> Result<()> {
    let mut data = vec![WITHDRAW_DISCRIMINATOR];
    data.extend_from_slice(&collateral_amount.to_le_bytes());

    let ix = Instruction {
        program_id: ID,
        accounts: vec![
            AccountMeta::new(*ctx.accounts.source_collateral_info.key, false),
            AccountMeta::new(*ctx.accounts.destination_collateral_info.key, false),
            AccountMeta::new(*ctx.accounts.reserve_info.key, false),
            AccountMeta::new(*ctx.accounts.obligation_info.key, false),
            AccountMeta::new(*ctx.accounts.lending_market_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.lending_market_authority_info.key, false),
            AccountMeta::new(*ctx.accounts.destination_liquidity_info.key, false),
            AccountMeta::new(*ctx.accounts.reserve_collateral_mint_info.key, false),
            AccountMeta::new(*ctx.accounts.reserve_liquidity_supply_info.key, false),
            AccountMeta::new_readonly(*ctx.accounts.obligation_owner_info.key, true),
            AccountMeta::new_readonly(*ctx.accounts.user_transfer_authority_info.key, true),
            AccountMeta::new_readonly(*ctx.accounts.token_program_info.key, false),
            AccountMeta::new(*ctx.accounts.deposit_reserve_info.key, false),
        ],
        data,
    };

    invoke_signed(
        &ix,
        &[
            ctx.accounts.source_collateral_info.to_account_info(),
            ctx.accounts.destination_collateral_info.to_account_info(),
            ctx.accounts.reserve_info.to_account_info(),
            ctx.accounts.obligation_info.to_account_info(),
            ctx.accounts.lending_market_info.to_account_info(),
            ctx.accounts.lending_market_authority_info.to_account_info(),
            ctx.accounts.destination_liquidity_info.to_account_info(),
            ctx.accounts.reserve_collateral_mint_info.to_account_info(),
            ctx.accounts.reserve_liquidity_supply_info.to_account_info(),
            ctx.accounts.obligation_owner_info.to_account_info(),
            ctx.accounts.user_transfer_authority_info.to_account_info(),
            ctx.accounts.token_program_info.to_account_info(),
            ctx.accounts.deposit_reserve_info.to_account_info(),
        ],
        ctx.signer_seeds,
    )?;

    Ok(())
}

// Helper to derive lending market authority
pub fn derive_lending_market_authority(lending_market: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&lending_market.to_bytes()[..32]], &ID)
}
