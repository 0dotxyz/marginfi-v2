use anchor_lang::prelude::*;

pub mod constant;
pub mod errors;
pub mod events;
pub mod invokes;
pub mod module;
pub mod state;
pub mod utils;

use crate::module::*;
use crate::state::context::*;
use library::structs::AddressBool;

#[cfg(feature = "staging")]
declare_id!("7tjE28izRUjzmxC1QNXnNwcc4N82CNYCexf3k8mw67s3");

#[cfg(not(feature = "staging"))]
declare_id!("jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9");

#[program]
pub mod lending {
    use super::*;

    /***********************************|
    |           Admin Module             |
    |__________________________________*/

    pub fn init_lending_admin(
        ctx: Context<InitLendingAdmin>,
        liquidity_program: Pubkey,
        rebalancer: Pubkey,
        authority: Pubkey,
    ) -> Result<()> {
        admin::init_lending_admin(ctx, liquidity_program, rebalancer, authority)
    }

    pub fn update_authority(
        ctx: Context<UpdateAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        admin::update_authority(ctx, new_authority)
    }

    pub fn init_lending(
        ctx: Context<InitLending>,
        symbol: String,
        liquidity_program: Pubkey,
    ) -> Result<()> {
        admin::init_lending(ctx, symbol, liquidity_program)
    }

    pub fn set_rewards_rate_model(ctx: Context<SetRewardsRateModel>, mint: Pubkey) -> Result<()> {
        admin::set_rewards_rate_model(ctx, mint)
    }

    pub fn update_rebalancer(ctx: Context<UpdateRebalancer>, new_rebalancer: Pubkey) -> Result<()> {
        admin::update_rebalancer(ctx, new_rebalancer)
    }

    pub fn update_rate(ctx: Context<UpdateRate>) -> Result<()> {
        admin::update_rate(ctx)
    }

    pub fn rebalance(ctx: Context<Rebalance>) -> Result<()> {
        admin::rebalance(ctx)
    }

    pub fn update_auths(ctx: Context<UpdateAuths>, auth_status: Vec<AddressBool>) -> Result<()> {
        admin::update_auths(ctx, auth_status)
    }

    /***********************************|
    |           User Module             |
    |__________________________________*/

    pub fn deposit(ctx: Context<Deposit>, assets: u64) -> Result<u64> {
        user::deposit(ctx, assets)
    }

    pub fn deposit_with_min_amount_out(
        ctx: Context<Deposit>,
        assets: u64,
        min_amount_out: u64,
    ) -> Result<()> {
        user::deposit_with_min_amount_out(ctx, assets, min_amount_out)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<u64> {
        user::withdraw(ctx, amount)
    }

    pub fn withdraw_with_max_shares_burn(
        ctx: Context<Withdraw>,
        amount: u64,
        max_shares_burn: u64,
    ) -> Result<u64> {
        user::withdraw_with_max_shares_burn(ctx, amount, max_shares_burn)
    }

    pub fn redeem(ctx: Context<Withdraw>, shares: u64) -> Result<u64> {
        user::redeem(ctx, shares)
    }

    pub fn redeem_with_min_amount_out(
        ctx: Context<Withdraw>,
        shares: u64,
        min_amount_out: u64,
    ) -> Result<()> {
        user::redeem_with_min_amount_out(ctx, shares, min_amount_out)
    }

    pub fn mint(ctx: Context<Deposit>, shares: u64) -> Result<u64> {
        user::mint(ctx, shares)
    }

    pub fn mint_with_max_assets(
        ctx: Context<Deposit>,
        shares: u64,
        max_assets: u64,
    ) -> Result<u64> {
        user::mint_with_max_assets(ctx, shares, max_assets)
    }
}
