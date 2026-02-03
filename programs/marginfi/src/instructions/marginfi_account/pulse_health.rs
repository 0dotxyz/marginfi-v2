use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use bytemuck::Zeroable;
use marginfi_type_crate::types::{HealthCache, MarginfiAccount};

use crate::{
    constants::PROGRAM_VERSION,
    events::HealthPulseEvent,
    state::marginfi_account::{
        check_account_bankrupt, check_account_init_health,
        check_pre_liquidation_condition_and_get_account_health,
    },
    MarginfiError, MarginfiResult,
};

pub fn lending_account_pulse_health<'info>(
    ctx: Context<'_, '_, 'info, 'info, PulseHealth<'info>>,
) -> MarginfiResult {
    let clock = Clock::get()?;
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;
    health_cache.program_version = PROGRAM_VERSION;

    // Check account init health using heap reuse optimization
    let engine_result = check_account_init_health(
        &marginfi_account,
        ctx.remaining_accounts,
        &mut Some(&mut health_cache),
    );
    match engine_result {
        Ok(()) => {
            if health_cache.internal_err != 0 {
                health_cache.set_oracle_ok(false);
            } else {
                health_cache.set_oracle_ok(true);
            }
            health_cache.set_engine_ok(true);
        }
        Err(e) => match e {
            Error::AnchorError(a_e) => {
                let e_n = a_e.error_code_number;
                health_cache.mrgn_err = e_n;
                let mfi_err: MarginfiError = e_n.into();
                if mfi_err.is_risk_engine_rejection() {
                    // risk engine failure is ignored for engine_ok purposes
                    health_cache.set_engine_ok(true);
                } else {
                    health_cache.set_engine_ok(false);
                }
                if mfi_err.is_oracle_error() || health_cache.internal_err != 0 {
                    health_cache.set_oracle_ok(false);
                } else {
                    health_cache.set_oracle_ok(true);
                }
            }
            Error::ProgramError(_) => {
                health_cache.set_engine_ok(false);
            }
        },
    };

    // Check pre-liquidation condition using heap reuse optimization
    let liq_result = check_pre_liquidation_condition_and_get_account_health(
        &marginfi_account,
        ctx.remaining_accounts,
        None,
        &mut Some(&mut health_cache),
        false,
    );
    if liq_result.is_err() {
        let err = liq_result.unwrap_err();
        match err {
            // Note: in the vastly majority of cases, this will be "HealthyAccount"
            Error::AnchorError(anchor_error) => {
                health_cache.internal_liq_err = anchor_error.error_code_number;
            }
            Error::ProgramError(_) => {
                msg!("generic program error, this should never happen.")
            }
        }
    }

    // Check bankruptcy condition using heap reuse optimization
    let bankruptcy_result = check_account_bankrupt(
        &marginfi_account,
        ctx.remaining_accounts,
        &mut Some(&mut health_cache),
    );
    if bankruptcy_result.is_err() {
        let err = bankruptcy_result.unwrap_err();
        match err {
            // Note: in the vastly majority of cases, this will be "AccountNotBankrupt"
            Error::AnchorError(anchor_error) => {
                health_cache.internal_bankruptcy_err = anchor_error.error_code_number;
            }
            Error::ProgramError(_) => {
                msg!("generic program error, this should never happen.")
            }
        }
    }

    marginfi_account.health_cache = health_cache;

    emit!(HealthPulseEvent {
        account: ctx.accounts.marginfi_account.key(),
        health_cache
    });

    Ok(())
}

#[derive(Accounts)]
pub struct PulseHealth<'info> {
    #[account(mut)]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,
}
