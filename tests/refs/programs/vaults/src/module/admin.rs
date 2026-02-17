use anchor_lang::prelude::*;
use std::collections::HashMap;

use crate::{
    constants::{
        EXCHANGE_PRICES_PRECISION, FOUR_DECIMALS, GOVERNANCE_MS, MAX_AUTH_COUNT,
        MAX_LIQUIDATION_PENALTY, THREE_DECIMALS, X10,
    },
    errors::ErrorCodes,
    events::*,
    state::*,
    utils::{
        verify_admin_context, verify_update_exchange_prices_context, verify_update_oracle_context,
    },
};

use library::math::{casting::*, safe_math::*};
use library::structs::AddressBool;
use liquidity::ID as LIQUIDITY_PROGRAM_ID;
use oracle::ID as ORACLE_PROGRAM_ID;

// Helper function to check liquidation max limit and penalty
fn check_liquidation_max_limit_and_penalty(
    liquidation_max_limit: u16,
    liquidation_penalty: u16,
) -> Result<()> {
    // liquidation max limit with penalty should not go above 99.7%
    if (liquidation_max_limit.safe_add(liquidation_penalty)?) > MAX_LIQUIDATION_PENALTY {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    Ok(())
}

pub fn init_vault_admin(
    ctx: Context<InitVaultAdmin>,
    liquidity: Pubkey,
    authority: Pubkey,
) -> Result<()> {
    let vault_admin = &mut ctx.accounts.vault_admin;

    // Ensure that the liquidity program is the same as the one in the liquidity program at compile time
    if liquidity != LIQUIDITY_PROGRAM_ID {
        return Err(error!(ErrorCodes::VaultAdminLiquidityProgramMismatch));
    }

    if authority == Pubkey::default() {
        return Err(error!(ErrorCodes::VaultAdminAddressZeroNotAllowed));
    }

    vault_admin.authority = authority;
    vault_admin.liquidity_program = liquidity;

    vault_admin.auths.push(authority);
    vault_admin.bump = ctx.bumps.vault_admin;

    // @dev vault_id starts from 1
    vault_admin.next_vault_id = 1;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn init_vault_config(
    ctx: Context<InitVaultConfig>,
    vault_id: u16,
    params: InitVaultConfigParams,
) -> Result<()> {
    let vault_admin = &mut ctx.accounts.vault_admin;

    if vault_id != vault_admin.next_vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    if vault_admin.liquidity_program != params.liquidity_program {
        return Err(error!(ErrorCodes::VaultAdminLiquidityProgramMismatch));
    }

    if params.oracle_program != ORACLE_PROGRAM_ID {
        return Err(error!(ErrorCodes::VaultAdminOracleProgramMismatch));
    }

    // Verify non-zero addresses
    if params.rebalancer == Pubkey::default() {
        return Err(error!(ErrorCodes::VaultAdminAddressZeroNotAllowed));
    }

    // Validate parameters
    check_liquidation_max_limit_and_penalty(
        params.liquidation_max_limit,
        params.liquidation_penalty,
    )?;

    // Convert from raw input format
    let collateral_factor = params.collateral_factor.safe_div(10)?;
    let liquidation_threshold = params.liquidation_threshold.safe_div(10)?;
    let liquidation_max_limit = params.liquidation_max_limit.safe_div(10)?;
    let withdraw_gap = params.withdraw_gap.safe_div(10)?;

    // Check all limits
    if params.supply_rate_magnifier > FOUR_DECIMALS.cast()?
        || params.supply_rate_magnifier < -FOUR_DECIMALS.cast()?
        || params.borrow_rate_magnifier > FOUR_DECIMALS.cast()?
        || params.borrow_rate_magnifier < -FOUR_DECIMALS.cast()?
        || collateral_factor >= liquidation_threshold
        || liquidation_threshold >= liquidation_max_limit
        || withdraw_gap > THREE_DECIMALS.cast()?
        || params.liquidation_penalty > X10.cast()?
        || params.borrow_fee > X10.cast()?
    {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    // Initialize the vault config
    let vault_config = &mut ctx.accounts.vault_config.load_init()?;

    vault_config.supply_rate_magnifier = params.supply_rate_magnifier;
    vault_config.borrow_rate_magnifier = params.borrow_rate_magnifier;

    vault_config.collateral_factor = collateral_factor;
    vault_config.liquidation_threshold = liquidation_threshold;
    vault_config.liquidation_max_limit = liquidation_max_limit;
    vault_config.withdraw_gap = withdraw_gap;

    vault_config.liquidation_penalty = params.liquidation_penalty;
    vault_config.borrow_fee = params.borrow_fee;
    vault_config.oracle = ctx.accounts.oracle.key();
    vault_config.oracle_program = params.oracle_program;
    vault_config.rebalancer = params.rebalancer;
    vault_config.liquidity_program = params.liquidity_program;
    vault_config.bump = ctx.bumps.vault_config;
    vault_config.supply_token = ctx.accounts.supply_token.key();
    vault_config.borrow_token = ctx.accounts.borrow_token.key();
    vault_config.vault_id = vault_admin.next_vault_id;

    // increment the next vault id by 1
    vault_admin.next_vault_id = vault_admin.next_vault_id.safe_add(1)?;

    // Initialize the vault metadata
    let vault_metadata = &mut ctx.accounts.vault_metadata;
    vault_metadata.vault_id = vault_config.vault_id;
    vault_metadata.supply_mint_decimals = ctx.accounts.supply_token.decimals;
    vault_metadata.borrow_mint_decimals = ctx.accounts.borrow_token.decimals;

    Ok(emit!(LogInitVaultConfig {
        vault_config: ctx.accounts.vault_config.key(),
    }))
}

pub fn init_vault_state(ctx: Context<InitVaultState>, vault_id: u16) -> Result<()> {
    let vault_config = &ctx.accounts.vault_config.load()?;

    if vault_id != vault_config.vault_id {
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

    let vault_state = &mut ctx.accounts.vault_state.load_init()?;

    vault_state.current_branch_id = 1; // Start with branch 1
    vault_state.total_branch_id = 1;
    vault_state.topmost_tick = COLD_TICK;
    vault_state.vault_id = vault_id;

    vault_state.liquidity_supply_exchange_price =
        supply_reserves.calculate_exchange_prices()?.0.cast()?;
    vault_state.liquidity_borrow_exchange_price =
        borrow_reserves.calculate_exchange_prices()?.1.cast()?;

    vault_state.last_update_timestamp = Clock::get()?.unix_timestamp.cast()?;

    let default_exchange_price: u64 = EXCHANGE_PRICES_PRECISION.cast()?;

    if vault_state.liquidity_supply_exchange_price < default_exchange_price
        || vault_state.liquidity_borrow_exchange_price < default_exchange_price
    {
        return Err(error!(ErrorCodes::VaultTokenNotInitialized));
    }

    vault_state.vault_supply_exchange_price = default_exchange_price;
    vault_state.vault_borrow_exchange_price = default_exchange_price;

    // position_id starts from 1
    vault_state.next_position_id = 1;

    Ok(emit!(LogInitVaultState {
        vault_state: ctx.accounts.vault_state.key(),
    }))
}

pub fn init_branch(ctx: Context<InitBranch>, vault_id: u16, branch_id: u32) -> Result<()> {
    let vault_config = &ctx.accounts.vault_config.load()?;
    if vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let mut branch = ctx.accounts.branch.load_init()?;

    branch.vault_id = vault_id;
    branch.branch_id = branch_id;
    branch.minima_tick = COLD_TICK;
    branch.connected_minima_tick = COLD_TICK;

    Ok(emit!(LogInitBranch {
        branch: ctx.accounts.branch.key(),
        branch_id: branch_id,
    }))
}

pub fn init_tick_has_debt_array(
    ctx: Context<InitTickHasDebtArray>,
    vault_id: u16,
    index: u8,
) -> Result<()> {
    let vault_config = &ctx.accounts.vault_config.load()?;
    if vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let mut tick_has_debt_array = ctx.accounts.tick_has_debt_array.load_init()?;

    tick_has_debt_array.index = index;
    tick_has_debt_array.vault_id = vault_id;

    Ok(emit!(LogInitTickHasDebtArray {
        tick_has_debt_array: ctx.accounts.tick_has_debt_array.key(),
    }))
}

pub fn init_tick(ctx: Context<InitTick>, vault_id: u16, tick: i32) -> Result<()> {
    let vault_config = &ctx.accounts.vault_config.load()?;
    if vault_id != vault_config.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    let tick_data = &mut ctx.accounts.tick_data.load_init()?;
    tick_data.tick = tick;
    tick_data.vault_id = vault_id;

    Ok(emit!(LogInitTick {
        tick: ctx.accounts.tick_data.key(),
    }))
}

pub fn init_tick_id_liquidation(
    ctx: Context<InitTickIdLiquidation>,
    vault_id: u16,
    tick: i32,
    total_ids: u32,
) -> Result<()> {
    let tick_data = &ctx.accounts.tick_data.load()?;
    if vault_id != tick_data.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    if tick_data.tick != tick {
        return Err(error!(ErrorCodes::VaultAdminTickMismatch));
    }

    let mut tick_id_liquidation = ctx.accounts.tick_id_liquidation.load_init()?;

    tick_id_liquidation.tick = tick;
    tick_id_liquidation.vault_id = vault_id;
    tick_id_liquidation.tick_map = total_ids.safe_add(2)?.safe_div(3)?;

    Ok(emit!(LogInitTickIdLiquidation {
        tick_id_liquidation: ctx.accounts.tick_id_liquidation.key(),
        tick: tick,
    }))
}

pub fn update_auths(context: Context<UpdateAuths>, auth_status: Vec<AddressBool>) -> Result<()> {
    let vault_admin = &mut context.accounts.vault_admin;

    let mut auth_map: HashMap<Pubkey, bool> = vault_admin
        .auths
        .iter()
        .map(|addr: &Pubkey| (*addr, true))
        .collect();

    let default_pubkey: Pubkey = Pubkey::default();

    for auth in auth_status.iter() {
        if auth.addr == default_pubkey || (auth.addr == vault_admin.authority && !auth.value) {
            return Err(ErrorCodes::VaultAdminInvalidParams.into());
        }

        auth_map.insert(auth.addr, auth.value);
    }

    vault_admin.auths = auth_map
        .into_iter()
        .filter(|(_, value)| *value)
        .map(|(addr, _)| addr)
        .collect();

    if vault_admin.auths.len() > MAX_AUTH_COUNT {
        return Err(ErrorCodes::VaultAdminMaxAuthCountReached.into());
    }

    emit!(LogUpdateAuths {
        auth_status: auth_status.clone(),
    });

    Ok(())
}

pub fn update_authority(context: Context<UpdateAuthority>, new_authority: Pubkey) -> Result<()> {
    if context.accounts.signer.key() != context.accounts.vault_admin.authority {
        // second check on top of context.rs to be extra sure
        return Err(ErrorCodes::VaultAdminOnlyAuthority.into());
    }

    if new_authority != GOVERNANCE_MS {
        return Err(ErrorCodes::VaultAdminInvalidParams.into());
    }

    let old_authority = context.accounts.vault_admin.authority.clone();

    context.accounts.vault_admin.authority = new_authority;

    let mut auth_map: HashMap<Pubkey, bool> = context
        .accounts
        .vault_admin
        .auths
        .iter()
        .map(|addr: &Pubkey| (*addr, true))
        .collect();

    auth_map.remove(&old_authority);
    auth_map.insert(new_authority, true);

    context.accounts.vault_admin.auths = auth_map
        .into_iter()
        .filter(|(_, value)| *value)
        .map(|(addr, _)| addr)
        .collect();

    emit!(LogUpdateAuthority {
        new_authority: new_authority,
    });

    Ok(())
}

pub fn update_lookup_table(
    ctx: Context<UpdateLookupTable>,
    vault_id: u16,
    lookup_table: Pubkey,
) -> Result<()> {
    let vault_metadata = &mut ctx.accounts.vault_metadata;

    if vault_id != vault_metadata.vault_id {
        return Err(error!(ErrorCodes::VaultAdminVaultIdMismatch));
    }

    if lookup_table == Pubkey::default() || lookup_table == vault_metadata.lookup_table {
        return Err(error!(ErrorCodes::VaultAdminInvalidParams));
    }

    vault_metadata.lookup_table = lookup_table;

    emit!(LogUpdateLookupTable {
        lookup_table: lookup_table,
    });

    Ok(())
}

// @notice updates the supply rate magnifier to `supply_rate_magnifier`. Input in 1e2 (1% = 100, 100% = 10_000).
pub fn update_supply_rate_magnifier(
    ctx: Context<Admin>,
    vault_id: u16,
    supply_rate_magnifier: i16,
) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    if supply_rate_magnifier > FOUR_DECIMALS.cast()?
        || supply_rate_magnifier < -FOUR_DECIMALS.cast()?
    {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    vault_config.supply_rate_magnifier = supply_rate_magnifier;

    Ok(emit!(LogUpdateSupplyRateMagnifier {
        supply_rate_magnifier: vault_config.supply_rate_magnifier
    }))
}

// @notice updates the borrow rate magnifier to `borrow_rate_magnifier`. Input in 1e2 (1% = 100, 100% = 10_000).
pub fn update_borrow_rate_magnifier(
    ctx: Context<Admin>,
    vault_id: u16,
    borrow_rate_magnifier: i16,
) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    if borrow_rate_magnifier > FOUR_DECIMALS.cast()?
        || borrow_rate_magnifier < -FOUR_DECIMALS.cast()?
    {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    vault_config.borrow_rate_magnifier = borrow_rate_magnifier;

    Ok(emit!(LogUpdateBorrowRateMagnifier {
        borrow_rate_magnifier: vault_config.borrow_rate_magnifier
    }))
}

// @notice updates the collateral factor to `collateral_factor`. Input in 1e2 (1% = 100, 100% = 10_000).
pub fn update_collateral_factor(
    ctx: Context<Admin>,
    vault_id: u16,
    collateral_factor: u16,
) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;
    // Convert to proper format (divide by 10)
    let collateral_factor: u16 = collateral_factor.safe_div(10)?;

    // Check liquidation threshold
    if collateral_factor >= vault_config.liquidation_threshold {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    vault_config.collateral_factor = collateral_factor;

    Ok(emit!(LogUpdateCollateralFactor {
        collateral_factor: collateral_factor.safe_mul(10)? // Convert back for event
    }))
}

// @notice updates the liquidation threshold to `liquidation_threshold`. Input in 1e2 (1% = 100, 100% = 10_000).
pub fn update_liquidation_threshold(
    ctx: Context<Admin>,
    vault_id: u16,
    liquidation_threshold: u16,
) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;
    // Convert to proper format (divide by 10)
    let liquidation_threshold = liquidation_threshold.safe_div(10)?;

    if vault_config.collateral_factor >= liquidation_threshold
        || liquidation_threshold >= vault_config.liquidation_max_limit
    {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    vault_config.liquidation_threshold = liquidation_threshold;

    Ok(emit!(LogUpdateLiquidationThreshold {
        liquidation_threshold: liquidation_threshold.safe_mul(10)? // Convert back for event
    }))
}

// @notice updates the liquidation max limit to `liquidation_max_limit`. Input in 1e2 (1% = 100, 100% = 10_000).
pub fn update_liquidation_max_limit(
    ctx: Context<Admin>,
    vault_id: u16,
    liquidation_max_limit: u16,
) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    // Check that liquidation max limit and penalty combined do not exceed 99.7%
    // both are in 1e2 decimals (1e2 = 1%)
    check_liquidation_max_limit_and_penalty(
        liquidation_max_limit,
        vault_config.liquidation_penalty,
    )?;

    // Convert to proper format (divide by 10)
    let liquidation_max_limit = liquidation_max_limit.safe_div(10)?;

    // Check limits
    if vault_config.liquidation_threshold >= liquidation_max_limit {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    vault_config.liquidation_max_limit = liquidation_max_limit;

    Ok(emit!(LogUpdateLiquidationMaxLimit {
        liquidation_max_limit: liquidation_max_limit.safe_mul(10)? // Convert back for event
    }))
}

/// @notice updates the withdrawal gap to `withdraw_gap`. Input in 1e2 (1% = 100, 100% = 10_000).
pub fn update_withdraw_gap(ctx: Context<Admin>, vault_id: u16, withdraw_gap: u16) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    // Convert to proper format (divide by 10)
    let withdraw_gap = withdraw_gap.safe_div(10)?;

    if withdraw_gap > THREE_DECIMALS.cast()? {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    vault_config.withdraw_gap = withdraw_gap;

    Ok(emit!(LogUpdateWithdrawGap {
        withdraw_gap: withdraw_gap.safe_mul(10)? // Convert back for event
    }))
}

// @notice updates the liquidation penalty to `liquidationPenalty_`. Input in 1e2 (1% = 100, 100% = 10_000).
pub fn update_liquidation_penalty(
    ctx: Context<Admin>,
    vault_id: u16,
    liquidation_penalty: u16,
) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    // Check that liquidation max limit and penalty combined do not exceed 99.7%
    check_liquidation_max_limit_and_penalty(
        vault_config.liquidation_max_limit.safe_mul(10)?,
        liquidation_penalty,
    )?;

    if liquidation_penalty > X10.cast()? {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    vault_config.liquidation_penalty = liquidation_penalty;

    Ok(emit!(LogUpdateLiquidationPenalty {
        liquidation_penalty
    }))
}

// @notice updates the borrow fee to `borrowFee_`. Input in 1e2 (1% = 100, 100% = 10_000).
pub fn update_borrow_fee(ctx: Context<Admin>, vault_id: u16, borrow_fee: u16) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    // Check limits
    if borrow_fee > X10.cast()? {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    // Update vault config
    vault_config.borrow_fee = borrow_fee;

    Ok(emit!(LogUpdateBorrowFee { borrow_fee }))
}

/// @notice updates the all Vault core settings according to input params.
/// All input values are expected in 1e2 (1% = 100, 100% = 10_000).
pub fn update_core_settings(
    ctx: Context<Admin>,
    vault_id: u16,
    params: UpdateCoreSettingsParams,
) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    // Check that liquidation max limit and penalty combined do not exceed 99.7%
    check_liquidation_max_limit_and_penalty(
        params.liquidation_max_limit,
        params.liquidation_penalty,
    )?;

    // Convert to proper format (divide by 10)
    let collateral_factor = params.collateral_factor.safe_div(10)?;
    let liquidation_threshold = params.liquidation_threshold.safe_div(10)?;
    let liquidation_max_limit = params.liquidation_max_limit.safe_div(10)?;
    let withdraw_gap = params.withdraw_gap.safe_div(10)?;

    // Check all limits
    if params.supply_rate_magnifier > FOUR_DECIMALS.cast()?
        || params.supply_rate_magnifier < -FOUR_DECIMALS.cast()?
        || params.borrow_rate_magnifier > FOUR_DECIMALS.cast()?
        || params.borrow_rate_magnifier < -FOUR_DECIMALS.cast()?
        || collateral_factor >= liquidation_threshold
        || liquidation_threshold >= liquidation_max_limit
        || withdraw_gap > THREE_DECIMALS.cast()?
        || params.liquidation_penalty > X10.cast()?
        || params.borrow_fee > X10.cast()?
    {
        return Err(error!(ErrorCodes::VaultAdminValueAboveLimit));
    }

    // Update vault config
    vault_config.supply_rate_magnifier = params.supply_rate_magnifier;
    vault_config.borrow_rate_magnifier = params.borrow_rate_magnifier;
    vault_config.collateral_factor = collateral_factor;
    vault_config.liquidation_threshold = liquidation_threshold;
    vault_config.liquidation_max_limit = liquidation_max_limit;
    vault_config.withdraw_gap = withdraw_gap;
    vault_config.liquidation_penalty = params.liquidation_penalty;
    vault_config.borrow_fee = params.borrow_fee;

    Ok(emit!(LogUpdateCoreSettings {
        supply_rate_magnifier: params.supply_rate_magnifier,
        borrow_rate_magnifier: params.borrow_rate_magnifier,
        collateral_factor: params.collateral_factor,
        liquidation_threshold: params.liquidation_threshold,
        liquidation_max_limit: params.liquidation_max_limit,
        withdraw_gap: params.withdraw_gap,
        liquidation_penalty: params.liquidation_penalty,
        borrow_fee: params.borrow_fee,
    }))
}

/// @notice updates the Vault oracle to `newOracle_`. Must implement the FluidOracle interface.
pub fn update_oracle(ctx: Context<UpdateOracle>, vault_id: u16) -> Result<()> {
    verify_update_oracle_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    if ctx.accounts.new_oracle.key() == Pubkey::default() {
        return Err(error!(ErrorCodes::VaultAdminAddressZeroNotAllowed));
    }

    vault_config.oracle = ctx.accounts.new_oracle.key();

    Ok(emit!(LogUpdateOracle {
        new_oracle: ctx.accounts.new_oracle.key(),
    }))
}

/// @notice updates the allowed rebalancer to `new_rebalancer`.
pub fn update_rebalancer(ctx: Context<Admin>, vault_id: u16, new_rebalancer: Pubkey) -> Result<()> {
    verify_admin_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let mut vault_config = ctx.accounts.vault_config.load_mut()?;

    // Update exchange price first
    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    if new_rebalancer == Pubkey::default() {
        return Err(error!(ErrorCodes::VaultAdminAddressZeroNotAllowed));
    }

    // Update vault config
    vault_config.rebalancer = new_rebalancer;

    Ok(emit!(LogUpdateRebalancer { new_rebalancer }))
}

pub fn update_exchange_prices(ctx: Context<UpdateExchangePrices>, vault_id: u16) -> Result<()> {
    verify_update_exchange_prices_context(&ctx, vault_id)?;

    let mut vault_state = ctx.accounts.vault_state.load_mut()?;
    let vault_config = ctx.accounts.vault_config.load()?;

    vault_state.update_exchange_prices(
        &vault_config,
        &ctx.accounts.supply_token_reserves_liquidity,
        &ctx.accounts.borrow_token_reserves_liquidity,
    )?;

    Ok(())
}
