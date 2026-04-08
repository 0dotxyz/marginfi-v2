use anchor_lang::prelude::*;
use anchor_lang::solana_program::{clock::Clock, sysvar::Sysvar};
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::types::{HealthCache, MarginfiAccount};

use crate::{
    constants::PROGRAM_VERSION,
    events::HealthPulseEvent,
    state::marginfi_account::{
        check_account_bankrupt, check_account_init_health,
        check_pre_liquidation_condition_and_get_account_health, HealthPriceMode,
        MarginfiAccountImpl,
    },
    MarginfiError, MarginfiResult,
};

const SECONDS_PER_DAY: i64 = 86_400;
const TRIVIAL_BALANCE_THRESHOLD: I80F48 = I80F48::ONE;

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
        HealthPriceMode::Live { liq_cache: None },
        false,
    );
    let is_liquidatable = liq_result.is_ok();
    if let Err(err) = liq_result {
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
    if let Err(err) = bankruptcy_result {
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

    // Update indexer flags
    let equity_assets: I80F48 = health_cache.asset_value_equity.into();
    let equity_liabs: I80F48 = health_cache.liability_value_equity.into();
    let elapsed = clock
        .unix_timestamp
        .saturating_sub(marginfi_account.last_update as i64);

    marginfi_account.indexer_flags.was_liquidatable = is_liquidatable as u8;
    marginfi_account.indexer_flags.was_underwater = (equity_assets < equity_liabs) as u8;
    marginfi_account.indexer_flags.was_active_30d = (elapsed <= 30 * SECONDS_PER_DAY) as u8;
    marginfi_account.indexer_flags.was_active_90d = (elapsed <= 90 * SECONDS_PER_DAY) as u8;
    marginfi_account.indexer_flags.was_active_1y = (elapsed <= 365 * SECONDS_PER_DAY) as u8;
    marginfi_account.indexer_flags.has_trivial_balance =
        (equity_assets < TRIVIAL_BALANCE_THRESHOLD) as u8;

    // Sync balance-derived flags first so is_empty is fresh
    marginfi_account.sync_indexer_flags();

    let is_empty = marginfi_account.indexer_flags.is_empty == 1;
    marginfi_account.indexer_flags.pending_closure =
        (is_empty && elapsed >= 30 * SECONDS_PER_DAY) as u8;
    marginfi_account.indexer_flags.closeable = (is_empty && elapsed >= 60 * SECONDS_PER_DAY) as u8;

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
