use anchor_lang::prelude::*;
use marginfi_type_crate::types::MintSnapshotsArchive;

use crate::MarginfiResult;

/// Solana maximum account data size (10 MiB).
const MAX_ACCOUNT_DATA_LEN: usize = 10_485_760;
pub const MONITOR_INDEX_MAP_LEN: usize = 300;

pub fn monitor_archive_initialize(
    ctx: Context<MonitorArchiveInitialize>,
    snapshot_manager: Pubkey,
) -> MarginfiResult {
    MintSnapshotsArchive::initialize(&ctx.accounts.archive, snapshot_manager).ok_or(crate::MarginfiError::InvalidConfig)?;
    Ok(())
}

#[derive(Accounts)]
pub struct MonitorArchiveInitialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Raw archive account initialized to max Solana size.
    #[account(
        init,
        payer = payer,
        space = MAX_ACCOUNT_DATA_LEN,
    )]
    pub archive: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}
