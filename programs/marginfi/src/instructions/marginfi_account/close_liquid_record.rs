use crate::{
    ix_utils::{get_discrim_hash, Hashable},
    prelude::*,
    state::marginfi_account::MarginfiAccountImpl,
};
use anchor_lang::prelude::*;
use marginfi_type_crate::types::{
    LiquidationRecord, MarginfiAccount, MarginfiGroup, ACCOUNT_IN_DELEVERAGE,
    ACCOUNT_IN_RECEIVERSHIP,
};

pub fn close_liquidation_record(ctx: Context<CloseLiquidationRecord>) -> MarginfiResult {
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;

    // The has_one constraint on `marginfi_account` already proves the closed record was the one
    // linked here. Clear the link so callers/liquidators see this account as having no record.
    marginfi_account.liquidation_record = Pubkey::default();

    Ok(())
}

/// (risk_admin only) Close a `LiquidationRecord` and refund rent to the original `record_payer`.
///
/// The account must not currently be undergoing receivership liquidation or risk-admin deleverage,
/// because the record is read by `start_liquidation` / `end_liquidation` / `start_deleverage` /
/// `end_deleverage`. A new record can be created at any time via `marginfi_account_init_liq_record`.
#[derive(Accounts)]
pub struct CloseLiquidationRecord<'info> {
    #[account(
        has_one = risk_admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    pub risk_admin: Signer<'info>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
        has_one = liquidation_record @ MarginfiError::InvalidLiquidationRecord,
        constraint = {
            let acc = marginfi_account.load()?;
            !acc.get_flag(ACCOUNT_IN_RECEIVERSHIP) && !acc.get_flag(ACCOUNT_IN_DELEVERAGE)
        } @ MarginfiError::UnexpectedLiquidationState,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    /// The record being closed. Rent is refunded to its original `record_payer`.
    #[account(
        mut,
        has_one = marginfi_account @ MarginfiError::InvalidLiquidationRecord,
        has_one = record_payer @ MarginfiError::InvalidLiquidationRecord,
        close = record_payer,
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    /// CHECK: validated by `has_one = record_payer` on `liquidation_record`. Receives the rent
    /// refund. Does not need to sign because the risk_admin authorizes the close.
    #[account(mut)]
    pub record_payer: AccountInfo<'info>,
}

impl Hashable for CloseLiquidationRecord<'_> {
    fn get_hash() -> [u8; 8] {
        get_discrim_hash("global", "marginfi_account_close_liq_record")
    }
}
