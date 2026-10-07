// Permissionless ix to cache a Kamino lending market's emergency flag on a bank in that market.
use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use kamino_mocks::state::{MinimalLendingMarket, MinimalReserve};
use marginfi_type_crate::{
    constants::{ASSET_TAG_KAMINO, KAMINO_MARKET_EMERGENCY},
    types::Bank,
};

/// Kamino halts its own instructions when a market is in emergency mode, but it keeps serving
/// `refresh_reserve`, so nothing upstream stops users borrowing against collateral parked there.
/// The health path never sees the market account, so the flag is cached on the bank instead.
pub fn propagate_kamino_market_emergency(
    ctx: Context<PropagateKaminoMarketEmergency>,
) -> MarginfiResult {
    let emergency = ctx.accounts.lending_market.load()?.is_emergency_mode();
    let mut bank = ctx.accounts.bank.load_mut()?;
    // Not part of `GROUP_FLAGS` (Kamino owns this bit, not the group admin), so it is set directly
    // rather than through `update_flag`.
    if emergency {
        bank.flags |= KAMINO_MARKET_EMERGENCY;
    } else {
        bank.flags &= !KAMINO_MARKET_EMERGENCY;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct PropagateKaminoMarketEmergency<'info> {
    /// The reserve `bank` deposits into. Read only to reach its market.
    pub reserve: AccountLoader<'info, MinimalReserve>,

    /// The market that owns `reserve`, and the source of the propagated flag.
    #[account(
        constraint = reserve.load()?.lending_market == lending_market.key()
            @ MarginfiError::KaminoReserveValidationFailed
    )]
    pub lending_market: AccountLoader<'info, MinimalLendingMarket>,

    // Validated against `oracle_keys[1]`, the same key the pricing path loads the reserve from.
    #[account(
        mut,
        constraint = {
            let b = bank.load()?;
            b.config.asset_tag == ASSET_TAG_KAMINO && b.config.oracle_keys[1] == reserve.key()
        } @ MarginfiError::KaminoReserveValidationFailed
    )]
    pub bank: AccountLoader<'info, Bank>,
}
