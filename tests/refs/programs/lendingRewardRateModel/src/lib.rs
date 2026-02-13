use anchor_lang::prelude::*;
use std::collections::HashMap;

pub mod constants;
pub mod errors;
pub mod events;
pub mod invokes;
pub mod state;

use crate::constants::*;
use crate::errors::*;
use crate::events::*;
use crate::state::*;

use library::structs::AddressBool;
use library::token::check_for_token_extensions_only_mint;

#[cfg(feature = "staging")]
declare_id!("68LHLkpgjAvo6Lgd9FT6KYEX4FWn1911EohSXxHYMFjc");

#[cfg(not(feature = "staging"))]
declare_id!("jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar");

#[program]
pub mod lending_reward_rate_model {
    use super::*;

    pub fn init_lending_rewards_admin(
        ctx: Context<InitLendingRewardsAdmin>,
        authority: Pubkey,
        lending_program: Pubkey,
    ) -> Result<()> {
        let lending_rewards_admin = &mut ctx.accounts.lending_rewards_admin;

        if authority == Pubkey::default() || lending_program == Pubkey::default() {
            return Err(ErrorCodes::InvalidParams.into());
        }

        lending_rewards_admin.authority = authority;
        lending_rewards_admin.lending_program = lending_program;
        lending_rewards_admin.auths.push(authority);
        lending_rewards_admin.bump = ctx.bumps.lending_rewards_admin;

        Ok(())
    }

    pub fn update_authority(
        context: Context<UpdateAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        if context.accounts.authority.key() != context.accounts.lending_rewards_admin.authority {
            // second check on top of context.rs to be extra sure
            return Err(ErrorCodes::OnlyAuthority.into());
        }

        if new_authority != GOVERNANCE_MS {
            return Err(ErrorCodes::InvalidParams.into());
        }

        let old_authority = context.accounts.lending_rewards_admin.authority.clone();

        context.accounts.lending_rewards_admin.authority = new_authority;

        let mut auth_map: HashMap<Pubkey, bool> = context
            .accounts
            .lending_rewards_admin
            .auths
            .iter()
            .map(|addr: &Pubkey| (*addr, true))
            .collect();

        auth_map.remove(&old_authority);
        auth_map.insert(new_authority, true);

        context.accounts.lending_rewards_admin.auths = auth_map
            .into_iter()
            .filter(|(_, value)| *value)
            .map(|(addr, _)| addr)
            .collect();

        emit!(LogUpdateAuthority {
            new_authority: new_authority,
        });

        Ok(())
    }

    pub fn update_auths(
        context: Context<UpdateAuths>,
        auth_status: Vec<AddressBool>,
    ) -> Result<()> {
        let mut auth_map: HashMap<Pubkey, bool> = context
            .accounts
            .lending_rewards_admin
            .auths
            .iter()
            .map(|addr: &Pubkey| (*addr, true))
            .collect();

        let default_pubkey: Pubkey = Pubkey::default();
        let authority = context.accounts.lending_rewards_admin.authority;

        for auth in auth_status.iter() {
            if auth.addr == default_pubkey || (auth.addr == authority && !auth.value) {
                return Err(ErrorCodes::InvalidParams.into());
            }

            auth_map.insert(auth.addr, auth.value);
        }

        if auth_map.len() == 0 {
            return Err(ErrorCodes::InvalidParams.into());
        }

        context.accounts.lending_rewards_admin.auths = auth_map
            .into_iter()
            .filter(|(_, value)| *value)
            .map(|(addr, _)| addr)
            .collect();

        if context.accounts.lending_rewards_admin.auths.len() > MAX_AUTH_COUNT {
            return Err(ErrorCodes::MaxAuthCountReached.into());
        }

        emit!(LogUpdateAuths {
            auth_status: auth_status.clone(),
        });

        Ok(())
    }

    pub fn init_lending_rewards_rate_model(
        ctx: Context<InitLendingRewardsRateModel>,
    ) -> Result<()> {
        // @dev check for token extensions during init for new rewards rate model
        check_for_token_extensions_only_mint(&ctx.accounts.mint)?;

        let lending_rewards_rate_model = &mut ctx.accounts.lending_rewards_rate_model;

        lending_rewards_rate_model.mint = ctx.accounts.mint.key();
        lending_rewards_rate_model.bump = ctx.bumps.lending_rewards_rate_model;

        Ok(())
    }

    pub fn stop_rewards(ctx: Context<LendingRewards>) -> Result<()> {
        ctx.accounts.stop()
    }

    pub fn start_rewards(
        ctx: Context<LendingRewards>,
        reward_amount: u64,
        duration: u64,
        start_time: u64,
        start_tvl: u64,
    ) -> Result<()> {
        ctx.accounts
            .start(reward_amount, duration, start_time, start_tvl)
    }

    pub fn cancel_queued_rewards(ctx: Context<LendingRewards>) -> Result<()> {
        ctx.accounts.cancel()
    }

    pub fn queue_next_rewards(
        ctx: Context<LendingRewards>,
        reward_amount: u64,
        duration: u64,
    ) -> Result<()> {
        ctx.accounts.queue(reward_amount, duration)
    }

    pub fn transition_to_next_rewards(ctx: Context<TransitionToNextRewards>) -> Result<()> {
        ctx.accounts.next()
    }
}
