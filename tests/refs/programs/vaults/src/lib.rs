use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod events;
pub mod invokes;
pub mod module;
pub mod state;
pub mod utils;

use crate::state::*;
use library::structs::AddressBool;
use liquidity::state::TransferType;

#[cfg(feature = "staging")]
declare_id!("Ho32sUQ4NzuAQgkPkHuNDG3G18rgHmYtXFA8EBmqQrAu");

#[cfg(not(feature = "staging"))]
declare_id!("jupr81YtYssSyPt8jbnGuiWon5f6x9TcDEFxYe3Bdzi");

#[program]
pub mod vaults {
    use super::*;

    /***********************************|
    |           Admin Module             |
    |__________________________________*/

    pub fn init_vault_admin(
        ctx: Context<InitVaultAdmin>,
        liquidity: Pubkey,
        authority: Pubkey,
    ) -> Result<()> {
        module::admin::init_vault_admin(ctx, liquidity, authority)
    }

    pub fn init_vault_config(
        ctx: Context<InitVaultConfig>,
        vault_id: u16,
        params: InitVaultConfigParams,
    ) -> Result<()> {
        module::admin::init_vault_config(ctx, vault_id, params)
    }

    pub fn init_vault_state(ctx: Context<InitVaultState>, vault_id: u16) -> Result<()> {
        module::admin::init_vault_state(ctx, vault_id)
    }

    pub fn init_branch(ctx: Context<InitBranch>, vault_id: u16, branch_id: u32) -> Result<()> {
        module::admin::init_branch(ctx, vault_id, branch_id)
    }

    pub fn init_tick_has_debt_array(
        ctx: Context<InitTickHasDebtArray>,
        vault_id: u16,
        index: u8,
    ) -> Result<()> {
        module::admin::init_tick_has_debt_array(ctx, vault_id, index)
    }

    pub fn init_tick(ctx: Context<InitTick>, vault_id: u16, tick: i32) -> Result<()> {
        module::admin::init_tick(ctx, vault_id, tick)
    }

    pub fn init_tick_id_liquidation(
        ctx: Context<InitTickIdLiquidation>,
        vault_id: u16,
        tick: i32,
        total_ids: u32,
    ) -> Result<()> {
        module::admin::init_tick_id_liquidation(ctx, vault_id, tick, total_ids)
    }

    pub fn update_supply_rate_magnifier(
        ctx: Context<Admin>,
        vault_id: u16,
        supply_rate_magnifier: i16,
    ) -> Result<()> {
        module::admin::update_supply_rate_magnifier(ctx, vault_id, supply_rate_magnifier)
    }

    pub fn update_borrow_rate_magnifier(
        ctx: Context<Admin>,
        vault_id: u16,
        borrow_rate_magnifier: i16,
    ) -> Result<()> {
        module::admin::update_borrow_rate_magnifier(ctx, vault_id, borrow_rate_magnifier)
    }

    pub fn update_collateral_factor(
        ctx: Context<Admin>,
        vault_id: u16,
        collateral_factor: u16,
    ) -> Result<()> {
        module::admin::update_collateral_factor(ctx, vault_id, collateral_factor)
    }

    pub fn update_liquidation_threshold(
        ctx: Context<Admin>,
        vault_id: u16,
        liquidation_threshold: u16,
    ) -> Result<()> {
        module::admin::update_liquidation_threshold(ctx, vault_id, liquidation_threshold)
    }

    pub fn update_liquidation_max_limit(
        ctx: Context<Admin>,
        vault_id: u16,
        liquidation_max_limit: u16,
    ) -> Result<()> {
        module::admin::update_liquidation_max_limit(ctx, vault_id, liquidation_max_limit)
    }

    pub fn update_withdraw_gap(
        ctx: Context<Admin>,
        vault_id: u16,
        withdraw_gap: u16,
    ) -> Result<()> {
        module::admin::update_withdraw_gap(ctx, vault_id, withdraw_gap)
    }

    pub fn update_liquidation_penalty(
        ctx: Context<Admin>,
        vault_id: u16,
        liquidation_penalty: u16,
    ) -> Result<()> {
        module::admin::update_liquidation_penalty(ctx, vault_id, liquidation_penalty)
    }

    pub fn update_borrow_fee(ctx: Context<Admin>, vault_id: u16, borrow_fee: u16) -> Result<()> {
        module::admin::update_borrow_fee(ctx, vault_id, borrow_fee)
    }

    pub fn update_core_settings(
        ctx: Context<Admin>,
        vault_id: u16,
        params: UpdateCoreSettingsParams,
    ) -> Result<()> {
        module::admin::update_core_settings(ctx, vault_id, params)
    }

    pub fn update_oracle(ctx: Context<UpdateOracle>, vault_id: u16) -> Result<()> {
        module::admin::update_oracle(ctx, vault_id)
    }

    pub fn update_lookup_table(
        ctx: Context<UpdateLookupTable>,
        vault_id: u16,
        lookup_table: Pubkey,
    ) -> Result<()> {
        module::admin::update_lookup_table(ctx, vault_id, lookup_table)
    }

    pub fn update_rebalancer(
        ctx: Context<Admin>,
        vault_id: u16,
        new_rebalancer: Pubkey,
    ) -> Result<()> {
        module::admin::update_rebalancer(ctx, vault_id, new_rebalancer)
    }

    pub fn update_auths(ctx: Context<UpdateAuths>, auth_status: Vec<AddressBool>) -> Result<()> {
        module::admin::update_auths(ctx, auth_status)
    }

    pub fn update_authority(
        context: Context<UpdateAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        module::admin::update_authority(context, new_authority)
    }

    pub fn update_exchange_prices(ctx: Context<UpdateExchangePrices>, vault_id: u16) -> Result<()> {
        module::admin::update_exchange_prices(ctx, vault_id)
    }

    /***********************************|
    |           User Module             |
    |__________________________________*/

    pub fn init_position(
        ctx: Context<InitPosition>,
        vault_id: u16,
        next_position_id: u32,
    ) -> Result<()> {
        module::user::init_position(ctx, vault_id, next_position_id)
    }

    pub fn operate<'info>(
        ctx: Context<'_, '_, 'info, 'info, Operate<'info>>,
        new_col: i128,
        new_debt: i128,
        transfer_type: Option<TransferType>,
        remaining_accounts_indices: Vec<u8>, // first index is sources, second is branches, third is ticks has debt
    ) -> Result<(u32, i128, i128)> {
        module::user::operate(
            ctx,
            new_col,
            new_debt,
            transfer_type,
            remaining_accounts_indices,
        )
    }

    pub fn liquidate<'info>(
        ctx: Context<'_, '_, 'info, 'info, Liquidate<'info>>,
        debt_amt: u64,
        col_per_unit_debt: u128,
        absorb: bool,
        transfer_type: Option<TransferType>,
        remaining_accounts_indices: Vec<u8>, // first index is sources, second is branches, third is ticks, fourth is tick has debt
    ) -> Result<(u128, u128)> {
        module::user::liquidate(
            ctx,
            debt_amt,
            col_per_unit_debt,
            absorb,
            transfer_type,
            remaining_accounts_indices,
        )
    }

    pub fn rebalance<'info>(
        ctx: Context<'_, '_, 'info, 'info, Rebalance<'info>>,
    ) -> Result<(i128, i128)> {
        module::user::rebalance(ctx)
    }

    /***********************************|
    |           View Module             |
    |__________________________________*/

    pub fn get_exchange_prices(
        ctx: Context<GetExchangePrices>,
    ) -> Result<(u128, u128, u128, u128)> {
        module::view::get_exchange_prices(ctx)
    }
}
