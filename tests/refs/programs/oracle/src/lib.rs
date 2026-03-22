use anchor_lang::prelude::*;
use std::collections::HashMap;

pub mod constants;
pub mod errors;
pub mod events;
pub mod helper;
pub mod modules;
pub mod state;

use crate::constants::GOVERNANCE_MS;
use crate::constants::{
    JUP_LEND_ACCOUNTS_COUNT, MAX_AUTH_COUNT, MAX_SOURCES, SINGLE_POOL_ACCOUNTS_COUNT,
};
use crate::errors::ErrorCodes;
use crate::events::*;
use crate::helper::get_hops_exchange_rate;
use crate::modules::jup_lend::validate_jup_lend_sources;
use crate::modules::single_pool::validate_single_pool_sources;
use crate::state::*;

use library::structs::AddressBool;

#[cfg(feature = "staging")]
declare_id!("C59ahnV7xHuW1Q9E87yrYbCHMHaYnkqRdix8BfLp3vnk");

#[cfg(not(feature = "staging"))]
declare_id!("jupnw4B6Eqs7ft6rxpzYLJZYSnrpRgPcr589n5Kv4oc");

#[program]
pub mod oracle {
    use super::*;

    pub fn init_admin(ctx: Context<InitAdmin>, authority: Pubkey) -> Result<()> {
        let oracle_admin = &mut ctx.accounts.oracle_admin;

        if authority == Pubkey::default() {
            return err!(ErrorCodes::InvalidParams);
        }

        oracle_admin.authority = authority;
        oracle_admin.auths.push(authority);

        Ok(())
    }

    pub fn update_auths(
        context: Context<UpdateAuths>,
        auth_status: Vec<AddressBool>,
    ) -> Result<()> {
        let oracle_admin = &mut context.accounts.oracle_admin;

        let mut auth_map: HashMap<Pubkey, bool> = oracle_admin
            .auths
            .iter()
            .map(|addr: &Pubkey| (*addr, true))
            .collect();

        let default_pubkey: Pubkey = Pubkey::default();

        for auth in auth_status.iter() {
            if auth.addr == default_pubkey || (auth.addr == oracle_admin.authority && !auth.value) {
                return Err(ErrorCodes::OracleAdminInvalidParams.into());
            }

            auth_map.insert(auth.addr, auth.value);
        }

        oracle_admin.auths = auth_map
            .into_iter()
            .filter(|(_, value)| *value)
            .map(|(addr, _)| addr)
            .collect();

        if oracle_admin.auths.len() > MAX_AUTH_COUNT {
            return Err(ErrorCodes::OracleAdminMaxAuthCountReached.into());
        }

        emit!(LogUpdateAuths {
            auth_status: auth_status.clone(),
        });

        Ok(())
    }

    pub fn update_authority(
        context: Context<UpdateAuthority>,
        new_authority: Pubkey,
    ) -> Result<()> {
        if context.accounts.authority.key() != context.accounts.oracle_admin.authority {
            // second check on top of context.rs to be extra sure
            return Err(ErrorCodes::OracleAdminOnlyAuthority.into());
        }

        if new_authority != GOVERNANCE_MS {
            return Err(ErrorCodes::InvalidParams.into());
        }

        let old_authority = context.accounts.oracle_admin.authority.clone();

        context.accounts.oracle_admin.authority = new_authority;

        let mut auth_map: HashMap<Pubkey, bool> = context
            .accounts
            .oracle_admin
            .auths
            .iter()
            .map(|addr: &Pubkey| (*addr, true))
            .collect();

        auth_map.remove(&old_authority);
        auth_map.insert(new_authority, true);

        context.accounts.oracle_admin.auths = auth_map
            .into_iter()
            .filter(|(_, value)| *value)
            .map(|(addr, _)| addr)
            .collect();

        emit!(LogUpdateAuthority {
            new_authority: new_authority,
        });

        Ok(())
    }

    pub fn init_oracle_config(
        ctx: Context<InitOracleConfig>,
        sources: Vec<Sources>,
        nonce: u16,
    ) -> Result<()> {
        if sources.len() > MAX_SOURCES {
            return err!(ErrorCodes::InvalidSourcesLength);
        }

        let mut i = 0;

        while i < sources.len() {
            let source = &sources[i];

            if !source.is_valid() {
                return err!(ErrorCodes::InvalidParams);
            }

            // restrict pyth, stake pool, msol pool, and jup lend source to have 1 divisor and 1 multiplier
            if source.source_type == SourceType::Pyth
                || source.source_type == SourceType::StakePool
                || source.source_type == SourceType::MsolPool
                || source.source_type == SourceType::JupLend
            {
                if source.divisor != 1 || source.multiplier != 1 {
                    return err!(ErrorCodes::InvalidPythSourceMultiplierAndDivisor);
                }
            }

            // single pool source must have three consecutive accounts with single pool source type
            if source.is_single_pool_source() {
                validate_single_pool_sources(&sources[i..i + SINGLE_POOL_ACCOUNTS_COUNT])?;
                i += SINGLE_POOL_ACCOUNTS_COUNT;
            } else if source.is_jup_lend_source() {
                // JupLend source must have four consecutive accounts
                validate_jup_lend_sources(&sources[i..i + JUP_LEND_ACCOUNTS_COUNT])?;
                i += JUP_LEND_ACCOUNTS_COUNT;
            } else {
                i += 1;
            }
        }

        let oracle = &mut ctx.accounts.oracle;

        oracle.nonce = nonce;
        oracle.sources = sources;
        oracle.bump = ctx.bumps.oracle;

        Ok(())
    }

    pub fn get_exchange_rate<'info>(ctx: Context<GetExchangeRate>, _nonce: u16) -> Result<u128> {
        Ok(get_hops_exchange_rate(
            &ctx.accounts.oracle.sources,
            ctx.remaining_accounts,
            Some(false),
        )?)
    }

    // Returns both exchange rates for 1. result param liquidate and 2. result param operate
    pub fn get_both_exchange_rate<'info>(
        ctx: Context<GetExchangeRate>,
        _nonce: u16,
    ) -> Result<(u128, u128)> {
        Ok((
            get_hops_exchange_rate(
                &ctx.accounts.oracle.sources,
                ctx.remaining_accounts,
                Some(true),
            )?,
            get_hops_exchange_rate(
                &ctx.accounts.oracle.sources,
                ctx.remaining_accounts,
                Some(false),
            )?,
        ))
    }

    // @dev currently liquidate and operate uses same internal function, subject to change in future
    pub fn get_exchange_rate_liquidate<'info>(
        ctx: Context<GetExchangeRate>,
        _nonce: u16,
    ) -> Result<u128> {
        Ok(get_hops_exchange_rate(
            &ctx.accounts.oracle.sources,
            ctx.remaining_accounts,
            Some(true),
        )?)
    }

    pub fn get_exchange_rate_operate<'info>(
        ctx: Context<GetExchangeRate>,
        _nonce: u16,
    ) -> Result<u128> {
        Ok(get_hops_exchange_rate(
            &ctx.accounts.oracle.sources,
            ctx.remaining_accounts,
            Some(false),
        )?)
    }
}
