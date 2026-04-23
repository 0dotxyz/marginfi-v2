use crate::{
    check,
    ix_utils::{get_discrim_hash, Hashable},
    state::marginfi_account::MarginfiAccountImpl,
    MarginfiError, MarginfiResult,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{
    LiquidationRecord, MarginfiAccount, ACCOUNT_IN_DELEVERAGE, ACCOUNT_IN_RECEIVERSHIP,
};

/// 60 days in seconds
const INACTIVITY_PERIOD_SECS: i64 = 60 * 24 * 60 * 60;

/// Close a liquidation record PDA and return rent to the original payer.
///
/// This is permissionless — anyone can call it, but rent always goes back to
/// `record_payer` (the wallet that paid to create the record). This allows
/// liquidators to reclaim rent from records they created, and also allows
/// cleanup bots to help reduce on-chain state bloat.
///
/// Conditions:
/// - The marginfi account must NOT be in receivership or deleverage
///   (no active liquidation in progress)
/// - The record must match the account's `liquidation_record` field
/// - The record must be inactive for at least 60 days (derived from the most
///   recent `LiquidationEntry.timestamp`), OR never have been liquidated at all
///   (all entry timestamps are zero).
pub fn close_liquidation_record(ctx: Context<CloseLiquidationRecord>) -> MarginfiResult {
    let record = ctx.accounts.liquidation_record.load()?;

    let last_activity = record
        .entries
        .iter()
        .map(|e| e.timestamp)
        .max()
        .unwrap_or(0);

    // Records that were never used (all timestamps zero) can be closed immediately.
    // Otherwise, require 60 days of inactivity.
    if last_activity > 0 {
        let now = Clock::get()?.unix_timestamp;
        check!(
            now.saturating_sub(last_activity) >= INACTIVITY_PERIOD_SECS,
            MarginfiError::IllegalAction,
            "Liquidation record must be inactive for at least 60 days"
        );
    }

    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    // Reset the account's liquidation_record reference
    marginfi_account.liquidation_record = Pubkey::default();

    Ok(())
}

#[derive(Accounts)]
pub struct CloseLiquidationRecord<'info> {
    #[account(
        mut,
        has_one = liquidation_record @ MarginfiError::InvalidLiquidationRecord,
        constraint = {
            let acc = marginfi_account.load()?;
            !acc.get_flag(ACCOUNT_IN_RECEIVERSHIP)
                && !acc.get_flag(ACCOUNT_IN_DELEVERAGE)
        } @ MarginfiError::IllegalAction
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        mut,
        close = record_payer,
        constraint = {
            let record = liquidation_record.load()?;
            record.liquidation_receiver == Pubkey::default()
        } @ MarginfiError::IllegalAction
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    /// The wallet that originally paid to create this record.
    /// Rent is returned here via Anchor's `close` constraint.
    /// CHECK: validated by the liquidation_record's record_payer field
    #[account(
        mut,
        constraint = {
            let record = liquidation_record.load()?;
            record.record_payer == record_payer.key()
        } @ MarginfiError::Unauthorized
    )]
    pub record_payer: AccountInfo<'info>,
}

impl Hashable for CloseLiquidationRecord<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_close_liq_record")
    }
}
