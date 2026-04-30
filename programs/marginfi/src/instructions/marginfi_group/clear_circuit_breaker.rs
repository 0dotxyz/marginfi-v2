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
/// * `reseed_reference` - When true, also zero the EMA reference so the next pulse reseeds from
///   live oracle data. Use when the new price level is considered valid; otherwise the pre-halt
///   reference will likely cause an immediate re-halt.
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

    let mut bank = ctx.accounts.bank.load_mut()?;
    let prior_tier = bank.cb_tier;
    bank.reset_cb_runtime_state();
    if reseed_reference {
        bank.cache.cb_reference_price = WrappedI80F48::from(I80F48::ZERO);
    }

    emit!(CircuitBreakerClearedEvent {
        prior_tier,
        reason: CB_CLEAR_REASON_ADMIN,
        current_timestamp: Clock::get()?.unix_timestamp,
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
