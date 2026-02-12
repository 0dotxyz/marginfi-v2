use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{LiquidationRecord, MarginfiAccount, MarginfiGroup},
};

use crate::prelude::*;

/// (risk_admin only) Close an idle LiquidationRecord and reclaim rent.
/// Clears the MarginfiAccount.liquidation_record field to prevent stale references.
pub fn risk_admin_close_liquidation_record(
    ctx: Context<RiskAdminCloseLiquidationRecord>,
) -> MarginfiResult {
    let liq_record = ctx.accounts.liquidation_record.load()?;

    // Safety: do not close a record that is actively in use by an ongoing liquidation/deleverage.
    // When a liquidation is in progress, `liquidation_receiver` is set to the liquidator's key.
    // It is reset to Pubkey::default() when the liquidation ends.
    require!(
        liq_record.liquidation_receiver == Pubkey::default(),
        MarginfiError::IllegalAction,
    );

    // Verify the record belongs to the provided marginfi_account
    require!(
        liq_record.marginfi_account == ctx.accounts.marginfi_account.key(),
        MarginfiError::InvalidLiquidationRecord,
    );

    drop(liq_record);

    // Only clear the pointer if it actually points to this record (or is already default).
    // This prevents accidentally zeroing a pointer that was already reassigned.
    let mut marginfi_account = ctx.accounts.marginfi_account.load_mut()?;
    let record_key = ctx.accounts.liquidation_record.key();
    require!(
        marginfi_account.liquidation_record == record_key
            || marginfi_account.liquidation_record == Pubkey::default(),
        MarginfiError::InvalidLiquidationRecord,
    );
    marginfi_account.liquidation_record = Pubkey::default();

    // The `close = receiver` constraint on the account handles lamport transfer and zeroing.
    Ok(())
}

#[derive(Accounts)]
pub struct RiskAdminCloseLiquidationRecord<'info> {
    #[account(
        has_one = risk_admin @ MarginfiError::Unauthorized,
    )]
    pub group: AccountLoader<'info, MarginfiGroup>,

    #[account(
        mut,
        has_one = group @ MarginfiError::InvalidGroup,
    )]
    pub marginfi_account: AccountLoader<'info, MarginfiAccount>,

    #[account(
        mut,
        seeds = [LIQUIDATION_RECORD_SEED.as_bytes(), marginfi_account.key().as_ref()],
        bump,
        close = receiver,
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    pub risk_admin: Signer<'info>,

    /// CHECK: Receiver of the closed account's lamports. Can be any account.
    #[account(mut)]
    pub receiver: UncheckedAccount<'info>,
}
