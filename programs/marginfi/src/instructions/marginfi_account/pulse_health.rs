use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;
use bytemuck::Zeroable;
use fixed::types::I80F48;
use marginfi_type_crate::types::{HealthCache, HealthPriceMode, MarginfiAccount, MarginfiGroup};

use crate::{
    constants::PROGRAM_VERSION,
    events::HealthPulseEvent,
    state::marginfi_account::{
        check_account_bankrupt, check_account_init_health,
        check_pre_liquidation_condition_and_get_account_health,
        compute_has_isolated_liability_flag,
    },
    state::premium::{update_premium_snapshots, PremiumScratch},
    MarginfiError, MarginfiResult,
};

const TRIVIAL_BALANCE_THRESHOLD: I80F48 = I80F48::ONE;

/// Marks accounts whose last pulse saw net equity greater than $0 and less than $1. This is
/// intended for indexer pruning of dust accounts, so underwater accounts are excluded even if
/// their gross assets are below the trivial threshold.
fn has_trivial_balance(equity_assets: I80F48, equity_liabs: I80F48) -> bool {
    let Some(net_equity) = equity_assets.checked_sub(equity_liabs) else {
        return false;
    };
    net_equity > I80F48::ZERO && net_equity < TRIVIAL_BALANCE_THRESHOLD
}

pub fn lending_account_pulse_health<'info>(
    ctx: Context<'info, PulseHealth<'info>>,
) -> MarginfiResult {
    let clock = Clock::get()?;
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;
    health_cache.program_version = PROGRAM_VERSION;

    // Check account init health using heap reuse optimization. Also collects the premium
    // scratch: this permissionless ix doubles as the premium crank, materializing dormant
    // accounts' pending premium and refreshing stale rate snapshots.
    let mut premium_scratch = PremiumScratch::default();
    let engine_result = check_account_init_health(
        &marginfi_account,
        ctx.remaining_accounts,
        &mut Some(&mut health_cache),
        &mut Some(&mut premium_scratch),
    );

    // Claim at old rates + rewrite snapshots. Self-gated: a partial health pass (e.g. an oracle
    // failure mid-loop) leaves the scratch incomplete and this is a no-op, so a failed pulse can
    // never write garbage rates.
    {
        let group = ctx.accounts.group.load()?;
        update_premium_snapshots(
            &mut marginfi_account,
            &group,
            &premium_scratch,
            clock.unix_timestamp as u64,
        )?;
    }

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
    let mut liquidatable_flag_update: Option<u8> = None;
    if let Err(err) = liq_result {
        match err {
            // Note: in the vastly majority of cases, this will be "HealthyAccount"
            Error::AnchorError(anchor_error) => {
                let err_code = anchor_error.error_code_number;
                health_cache.internal_liq_err = err_code;
                let mfi_err: MarginfiError = err_code.into();
                if matches!(mfi_err, MarginfiError::HealthyAccount) {
                    liquidatable_flag_update = Some(0);
                }
            }
            Error::ProgramError(_) => {
                msg!("generic program error, this should never happen.")
            }
        }
    } else {
        liquidatable_flag_update = Some(1);
    }

    // Check bankruptcy condition using heap reuse optimization
    let bankruptcy_result = check_account_bankrupt(
        &marginfi_account,
        ctx.remaining_accounts,
        &mut Some(&mut health_cache),
    );
    let mut equity_flags_decisive = false;
    if let Err(err) = bankruptcy_result {
        match err {
            // Note: in the vastly majority of cases, this will be "AccountNotBankrupt"
            Error::AnchorError(anchor_error) => {
                let err_code = anchor_error.error_code_number;
                health_cache.internal_bankruptcy_err = err_code;
                let mfi_err: MarginfiError = err_code.into();
                if matches!(mfi_err, MarginfiError::AccountNotBankrupt) {
                    equity_flags_decisive = true;
                }
            }
            Error::ProgramError(_) => {
                msg!("generic program error, this should never happen.")
            }
        }
    } else {
        equity_flags_decisive = true;
    }

    let equity_assets: I80F48 = health_cache.asset_value_equity.into();
    let equity_liabs: I80F48 = health_cache.liability_value_equity.into();
    let elapsed = clock
        .unix_timestamp
        .saturating_sub(marginfi_account.last_update as i64);
    let has_isolated_update =
        compute_has_isolated_liability_flag(&marginfi_account, ctx.remaining_accounts).ok();

    // Reborrow through a single DerefMut so the borrow checker can split indexer_flags
    // (mut) from lending_account.balances (shared).
    let account = &mut *marginfi_account;
    account
        .indexer_flags
        .sync_balance_derived(&account.lending_account.balances);
    account.indexer_flags.sync_activity_flags(elapsed);
    if let Some(has_isolated) = has_isolated_update {
        account.indexer_flags.has_isolated = has_isolated;
    }
    if let Some(was_liquidatable) = liquidatable_flag_update {
        account.indexer_flags.was_liquidatable = was_liquidatable;
    }
    if equity_flags_decisive {
        account.indexer_flags.was_underwater = (equity_assets < equity_liabs) as u8;
        account.indexer_flags.has_trivial_balance =
            has_trivial_balance(equity_assets, equity_liabs) as u8;
    }
    account.health_cache = health_cache;

    emit!(HealthPulseEvent {
        account: ctx.accounts.marginfi_account.key(),
        health_cache
    });

    Ok(())
}

#[derive(Accounts)]
pub struct PulseHealth<'info> {
    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// Needed to read the premium matrix for snapshot recompute
    pub group: AccountLoader<'info, MarginfiGroup>,
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
