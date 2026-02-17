use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token_interface::TokenAccount;

use library::math::tick::TickMath;

use crate::{
    errors::ErrorCodes,
    state::{
        Admin, ClosePosition, InitPosition, Liquidate, Operate, Rebalance, UpdateExchangePrices,
        UpdateOracle, COLD_TICK,
    },
};

pub fn verify_admin_context(ctx: &Context<Admin>, vault_id: u16) -> Result<()> {
    let vault_config = ctx.accounts.vault_config.load()?;
    if vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let vault_state = &ctx.accounts.vault_state.load()?;
    if vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let supply_reserves = ctx.accounts.supply_token_reserves_liquidity.load()?;
    if supply_reserves.mint != vault_config.supply_token {
        return Err(error!(ErrorCodes::VaultInvalidSupplyMint));
    }

    let borrow_reserves = ctx.accounts.borrow_token_reserves_liquidity.load()?;
    if borrow_reserves.mint != vault_config.borrow_token {
        return Err(error!(ErrorCodes::VaultInvalidBorrowMint));
    }

    Ok(())
}

pub fn verify_update_exchange_prices_context(
    ctx: &Context<UpdateExchangePrices>,
    vault_id: u16,
) -> Result<()> {
    let vault_config = ctx.accounts.vault_config.load()?;
    if vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let vault_state = &ctx.accounts.vault_state.load()?;
    if vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let supply_reserves = ctx.accounts.supply_token_reserves_liquidity.load()?;
    if supply_reserves.mint != vault_config.supply_token {
        return Err(error!(ErrorCodes::VaultInvalidSupplyMint));
    }

    let borrow_reserves = ctx.accounts.borrow_token_reserves_liquidity.load()?;
    if borrow_reserves.mint != vault_config.borrow_token {
        return Err(error!(ErrorCodes::VaultInvalidBorrowMint));
    }

    Ok(())
}

pub fn verify_update_oracle_context(ctx: &Context<UpdateOracle>, vault_id: u16) -> Result<()> {
    let vault_config = ctx.accounts.vault_config.load()?;
    if vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let vault_state = &ctx.accounts.vault_state.load()?;
    if vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let supply_reserves = ctx.accounts.supply_token_reserves_liquidity.load()?;
    if supply_reserves.mint != vault_config.supply_token {
        return Err(error!(ErrorCodes::VaultInvalidSupplyMint));
    }

    let borrow_reserves = ctx.accounts.borrow_token_reserves_liquidity.load()?;
    if borrow_reserves.mint != vault_config.borrow_token {
        return Err(error!(ErrorCodes::VaultInvalidBorrowMint));
    }

    let new_oracle = &ctx.accounts.new_oracle;
    // if new oracle is the same as the current oracle, return error
    if new_oracle.key() == vault_config.oracle {
        return Err(error!(ErrorCodes::VaultInvalidOracle));
    }

    Ok(())
}

pub fn validate_owner(expected_owner: &Pubkey, owner_account_info: &AccountInfo) -> Result<()> {
    if expected_owner != owner_account_info.key || !owner_account_info.is_signer {
        return Err(ErrorCodes::VaultInvalidPositionAuthority.into());
    }

    Ok(())
}

pub fn verify_position_authority_interface(
    // position_token_account is owned by either TokenProgram or Token2022Program
    position_token_account: &InterfaceAccount<'_, TokenAccount>,
    position_authority: &Signer<'_>,
) -> Result<()> {
    // Check token authority using validate_owner method...
    match position_token_account.delegate {
        COption::Some(ref delegate) if position_authority.key == delegate => {
            validate_owner(delegate, &position_authority.to_account_info())?;
            if position_token_account.delegated_amount != 1 {
                return Err(error!(ErrorCodes::VaultInvalidPositionTokenAmount));
            }
        }
        _ => validate_owner(
            &position_token_account.owner,
            &position_authority.to_account_info(),
        )?,
    };
    Ok(())
}

pub fn verify_init_position<'info>(
    ctx: &Context<InitPosition>,
    vault_id: u16,
    next_position_id: u32,
) -> Result<()> {
    let vault_state = ctx.accounts.vault_state.load()?;

    if vault_state.vault_id != vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    if vault_state.next_position_id != next_position_id {
        return Err(error!(ErrorCodes::VaultInvalidNextPositionId));
    }

    Ok(())
}

pub fn verify_close_position<'info>(
    ctx: &Context<ClosePosition>,
    vault_id: u16,
    position_id: u32,
) -> Result<()> {
    let vault_state = ctx.accounts.vault_state.load()?;
    let vault_config = ctx.accounts.vault_config.load()?;

    if vault_state.vault_id != vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    if vault_config.vault_id != vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    let position = ctx.accounts.position.load()?;

    // Verify position belongs to the correct vault
    if position.vault_id != vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    // Verify position ID matches
    if position.nft_id != position_id {
        return Err(error!(ErrorCodes::VaultInvalidPositionId));
    }

    // Verify position mint matches
    if position.position_mint != ctx.accounts.position_mint.key() {
        return Err(error!(ErrorCodes::VaultInvalidPositionMint));
    }

    // Verify position token account has exactly 1 token
    let position_token_account = &ctx.accounts.position_token_account;
    if position_token_account.mint != position.position_mint || position_token_account.amount != 1 {
        return Err(error!(ErrorCodes::VaultInvalidPositionMint));
    }

    // Verify the signer owns the position token account (or is authorized delegate)
    verify_position_authority_interface(
        &ctx.accounts.position_token_account,
        &ctx.accounts.signer,
    )?;

    // Verify position has no active debt or collateral (should be empty position to close)
    if !position.is_supply_only_position() || position.get_supply_amount()? > 0 {
        return Err(error!(ErrorCodes::VaultPositionNotEmpty));
    }

    Ok(())
}

pub fn verify_operate<'info>(ctx: &Context<'_, '_, 'info, 'info, Operate<'info>>) -> Result<()> {
    let vault_state = ctx.accounts.vault_state.load()?;
    let vault_config = ctx.accounts.vault_config.load()?;

    if vault_state.vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    if vault_config.supply_token != ctx.accounts.supply_token.key() {
        return Err(error!(ErrorCodes::VaultInvalidSupplyMint));
    }

    if vault_config.borrow_token != ctx.accounts.borrow_token.key() {
        return Err(error!(ErrorCodes::VaultInvalidBorrowMint));
    }

    if ctx.accounts.liquidity_program.key() != vault_config.liquidity_program {
        return Err(error!(ErrorCodes::VaultInvalidLiquidityProgram));
    }

    let oracle = &ctx.accounts.oracle;
    if oracle.key() != vault_config.oracle {
        return Err(error!(ErrorCodes::VaultInvalidOracle));
    }

    if ctx.accounts.oracle_program.key() != vault_config.oracle_program {
        return Err(error!(ErrorCodes::VaultOracleNotValid));
    }

    let position = &ctx.accounts.position.load()?;
    if position.vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    let position_token_account = &ctx.accounts.position_token_account;
    if position_token_account.mint != position.position_mint || position_token_account.amount != 1 {
        return Err(error!(ErrorCodes::VaultInvalidPositionMint));
    }

    let current_position_tick = &ctx.accounts.current_position_tick.load()?;
    if current_position_tick.vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    let mut tick_to_verify = position.tick;

    // case for first init position
    if tick_to_verify == COLD_TICK {
        tick_to_verify = TickMath::MIN_TICK;
    }

    if tick_to_verify != current_position_tick.tick {
        return Err(error!(ErrorCodes::VaultInvalidTick));
    }

    let new_position_tick = &ctx.accounts.final_position_tick.load()?;
    if new_position_tick.vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    let current_position_tick_id = &ctx.accounts.current_position_tick_id.load()?;
    if current_position_tick_id.vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    let final_position_tick_id = &ctx.accounts.final_position_tick_id.load()?;
    if final_position_tick_id.vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    let new_branch = &ctx.accounts.new_branch.load()?;
    if new_branch.vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    let supply_token_reserves_liquidity = ctx.accounts.supply_token_reserves_liquidity.load()?;
    if supply_token_reserves_liquidity.mint != vault_config.supply_token {
        return Err(error!(ErrorCodes::VaultInvalidSupplyMint));
    }

    let borrow_token_reserves_liquidity = ctx.accounts.borrow_token_reserves_liquidity.load()?;
    if borrow_token_reserves_liquidity.mint != vault_config.borrow_token {
        return Err(error!(ErrorCodes::VaultInvalidBorrowMint));
    }

    Ok(())
}

pub fn verify_liquidate<'info>(
    ctx: &Context<'_, '_, 'info, 'info, Liquidate<'info>>,
) -> Result<()> {
    let vault_state = ctx.accounts.vault_state.load()?;
    let vault_config = ctx.accounts.vault_config.load()?;

    if vault_state.vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    if vault_config.supply_token != ctx.accounts.supply_token.key() {
        return Err(error!(ErrorCodes::VaultInvalidSupplyMint));
    }

    if vault_config.borrow_token != ctx.accounts.borrow_token.key() {
        return Err(error!(ErrorCodes::VaultInvalidBorrowMint));
    }

    if ctx.accounts.liquidity_program.key() != vault_config.liquidity_program {
        return Err(error!(ErrorCodes::VaultInvalidLiquidityProgram));
    }

    let oracle = &ctx.accounts.oracle;
    if oracle.key() != vault_config.oracle {
        return Err(error!(ErrorCodes::VaultInvalidOracle));
    }

    if ctx.accounts.oracle_program.key() != vault_config.oracle_program {
        return Err(error!(ErrorCodes::VaultOracleNotValid));
    }

    let new_branch = &ctx.accounts.new_branch.load()?;
    if new_branch.vault_id != vault_state.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    let supply_token_reserves_liquidity = ctx.accounts.supply_token_reserves_liquidity.load()?;
    if supply_token_reserves_liquidity.mint != vault_config.supply_token {
        return Err(error!(ErrorCodes::VaultInvalidSupplyMint));
    }

    let borrow_token_reserves_liquidity = ctx.accounts.borrow_token_reserves_liquidity.load()?;
    if borrow_token_reserves_liquidity.mint != vault_config.borrow_token {
        return Err(error!(ErrorCodes::VaultInvalidBorrowMint));
    }

    Ok(())
}

pub fn verify_rebalance<'info>(
    ctx: &Context<'_, '_, 'info, 'info, Rebalance<'info>>,
) -> Result<()> {
    let vault_state = ctx.accounts.vault_state.load()?;
    let vault_config = ctx.accounts.vault_config.load()?;

    if vault_state.vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultInvalidVaultId));
    }

    if ctx.accounts.liquidity_program.key() != vault_config.liquidity_program {
        return Err(error!(ErrorCodes::VaultInvalidLiquidityProgram));
    }

    Ok(())
}
