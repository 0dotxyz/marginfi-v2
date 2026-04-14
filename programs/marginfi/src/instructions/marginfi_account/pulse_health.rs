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
        check_pre_liquidation_condition_and_get_account_health, compute_risk_tier_snapshot,
        HealthPriceMode, MarginfiAccountImpl,
    },
    MarginfiError, MarginfiResult,
};

const SECONDS_PER_DAY: i64 = 86_400;
const TRIVIAL_BALANCE_THRESHOLD: I80F48 = I80F48::ONE;

/// Marks accounts whose last pulse saw net equity greater than $0 and less than $1. This is
/// intended for indexer pruning of dust accounts, so underwater accounts are excluded even if
/// their gross assets are below the trivial threshold.
fn has_trivial_balance(equity_assets: I80F48, equity_liabs: I80F48) -> bool {
    let net_equity = equity_assets - equity_liabs;
    net_equity > I80F48::ZERO && net_equity < TRIVIAL_BALANCE_THRESHOLD
}

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

    let equity_assets: I80F48 = health_cache.asset_value_equity.into();
    let equity_liabs: I80F48 = health_cache.liability_value_equity.into();
    let elapsed = clock
        .unix_timestamp
        .saturating_sub(marginfi_account.last_update as i64);

    marginfi_account.indexer_flags.was_liquidatable = is_liquidatable as u8;
    marginfi_account.indexer_flags.was_underwater = (equity_assets < equity_liabs) as u8;
    marginfi_account.indexer_flags.was_active_30d = (elapsed <= 30 * SECONDS_PER_DAY) as u8;
    marginfi_account.indexer_flags.was_active_60d = (elapsed <= 60 * SECONDS_PER_DAY) as u8;
    marginfi_account.indexer_flags.has_trivial_balance =
        has_trivial_balance(equity_assets, equity_liabs) as u8;

    marginfi_account.sync_indexer_flags();

    if let Ok(snapshot) = compute_risk_tier_snapshot(&marginfi_account, ctx.remaining_accounts) {
        marginfi_account.indexer_flags.has_isolated = snapshot.has_isolated_liability() as u8;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trivial_balance_uses_strictly_positive_net_equity() {
        assert!(has_trivial_balance(I80F48::from_num(0.5), I80F48::ZERO));
        assert!(!has_trivial_balance(I80F48::ZERO, I80F48::ZERO));
        assert!(!has_trivial_balance(
            I80F48::from_num(0.5),
            I80F48::from_num(2)
        ));
        assert!(!has_trivial_balance(
            I80F48::from_num(5),
            I80F48::from_num(2)
        ));
    }
}
