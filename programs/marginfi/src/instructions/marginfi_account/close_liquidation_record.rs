use anchor_lang::prelude::*;
use marginfi_type_crate::types::{
    LiquidationRecord, MarginfiAccount, ACCOUNT_IN_RECEIVERSHIP, ACCOUNT_IN_DELEVERAGE,
};

use crate::{check, state::marginfi_account::MarginfiAccountImpl, MarginfiError, MarginfiResult};

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
pub fn close_liquidation_record(ctx: Context<CloseLiquidationRecord>) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    check!(
        !marginfi_account.get_flag(ACCOUNT_IN_RECEIVERSHIP)
            && !marginfi_account.get_flag(ACCOUNT_IN_DELEVERAGE),
        MarginfiError::IllegalAction,
        "Cannot close liquidation record while account is in receivership or deleverage"
    );

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
