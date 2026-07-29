use crate::{
    events::{CircuitBreakerClearedEvent, CB_CLEAR_REASON_ADMIN},
    state::bank::BankImpl,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::types::{Bank, MarginfiGroup, WrappedI80F48};

/// (admin or risk_admin) Clear a circuit-breaker halt on a bank.
///
/// * `reseed_reference` - When true, also zero the EMA and long-window references so the next pulse
///   reseeds from live oracle data. Use when the new price level is considered valid; otherwise the
///   pre-halt reference will likely cause an immediate re-halt.
pub fn lending_pool_clear_circuit_breaker(
    ctx: Context<LendingPoolClearCircuitBreaker>,
    reseed_reference: bool,
) -> MarginfiResult {
    {
        let group = ctx.accounts.group.load()?;
        let signer = ctx.accounts.authority.key();
        require!(
            signer == group.admin || signer == group.risk_admin,
            MarginfiError::Unauthorized
        );
    }

    let now = Clock::get()?.unix_timestamp;
    let mut bank = ctx.accounts.bank.load_mut()?;
    // Consume the interest freeze while the halt span is still recorded: accruing here
    // excludes the halted time (up to now) and restarts accrual from the clear.
    bank.accrue_interest(
        now,
        &*ctx.accounts.group.load()?,
        #[cfg(not(feature = "client"))]
        ctx.accounts.bank.key(),
    )?;
    let prior_tier = bank.cb_tier;
    bank.reset_cb_runtime_state();
    if reseed_reference {
        bank.cb_reference_price = WrappedI80F48::from(I80F48::ZERO);
        bank.cb_window_reference_price = WrappedI80F48::from(I80F48::ZERO);
        bank.cb_window_started_at = 0;
    }

    emit!(CircuitBreakerClearedEvent {
        prior_tier,
        reason: CB_CLEAR_REASON_ADMIN,
        current_timestamp: now,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolClearCircuitBreaker<'info> {
    pub group: AccountLoader<'info, MarginfiGroup>,

    /// Either `group.admin` or `group.risk_admin`. Validated in the handler.
    pub authority: Signer<'info>,

    #[account(mut, has_one = group @ MarginfiError::InvalidGroup)]
    pub bank: AccountLoader<'info, Bank>,
}
