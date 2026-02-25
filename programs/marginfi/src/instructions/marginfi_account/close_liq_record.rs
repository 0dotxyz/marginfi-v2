use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::LIQUIDATION_RECORD_SEED,
    types::{LiquidationRecord, MarginfiAccount, MarginfiGroup},
};

use crate::prelude::*;

const SECS_60_DAYS: i64 = 60 * 24 * 60 * 60;
const SECS_90_DAYS: i64 = 90 * 24 * 60 * 60;

fn check_liquidation_record_expiry(
    last_activity_ts: i64,
    required_inactivity_window: i64,
    now_ts: i64,
) -> MarginfiResult {
    if last_activity_ts == 0 {
        return Ok(());
    }

    require!(
        now_ts.saturating_sub(last_activity_ts) >= required_inactivity_window,
        MarginfiError::LiquidationRecordNotExpired,
    );

    Ok(())
}

/// (record_payer only) Close an idle LiquidationRecord after 60 days of inactivity.
/// Clears the MarginfiAccount.liquidation_record field to prevent stale references.
pub fn close_liquidation_record_by_creator(
    ctx: Context<CreatorCloseLiquidationRecord>,
) -> MarginfiResult {
    let now_ts = Clock::get()?.unix_timestamp;
    let liq_record = ctx.accounts.liquidation_record.load()?;

    // Safety: do not close a record that is actively in use by an ongoing liquidation/deleverage.
    require!(
        liq_record.liquidation_receiver == Pubkey::default(),
        MarginfiError::IllegalAction,
    );

    // Verify the record belongs to the provided marginfi_account.
    require!(
        liq_record.marginfi_account == ctx.accounts.marginfi_account.key(),
        MarginfiError::InvalidLiquidationRecord,
    );

    // Only the original record payer can close through this path.
    require!(
        liq_record.record_payer == ctx.accounts.creator.key(),
        MarginfiError::Unauthorized,
    );

    check_liquidation_record_expiry(liq_record.last_activity_ts, SECS_60_DAYS, now_ts)?;

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

    // The `close = creator` constraint on the account handles lamport transfer and zeroing.
    Ok(())
}

/// (risk_admin only) Close an idle LiquidationRecord and reclaim rent.
/// Clears the MarginfiAccount.liquidation_record field to prevent stale references.
pub fn risk_admin_close_liquidation_record(
    ctx: Context<RiskAdminCloseLiquidationRecord>,
) -> MarginfiResult {
    let now_ts = Clock::get()?.unix_timestamp;
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

    // Rent recipient must always match the original record payer.
    require!(
        liq_record.record_payer == ctx.accounts.record_payer.key(),
        MarginfiError::Unauthorized,
    );

    check_liquidation_record_expiry(liq_record.last_activity_ts, SECS_90_DAYS, now_ts)?;

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

    // The `close = record_payer` constraint on the account handles lamport transfer and zeroing.
    Ok(())
}

#[derive(Accounts)]
pub struct CreatorCloseLiquidationRecord<'info> {
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
        close = creator,
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    pub creator: Signer<'info>,
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
        close = record_payer,
    )]
    pub liquidation_record: AccountLoader<'info, LiquidationRecord>,

    pub risk_admin: Signer<'info>,

    /// CHECK: Lamports are always refunded to the record creator.
    #[account(mut)]
    pub record_payer: UncheckedAccount<'info>,
}
