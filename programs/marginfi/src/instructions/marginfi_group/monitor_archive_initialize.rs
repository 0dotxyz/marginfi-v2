use anchor_lang::prelude::*;
use marginfi_type_crate::types::ArchiveHeader;

use crate::MarginfiResult;

/// Solana maximum account data size (10 MiB).
const MAX_ACCOUNT_DATA_LEN: usize = 10_485_760;

pub fn monitor_archive_initialize(
    ctx: Context<MonitorArchiveInitialize>,
    snapshot_manager: Pubkey,
) -> MarginfiResult {
    let mut header = ctx.accounts.archive.load_init()?;
    header.version = 1;
    header._pad0 = [0; 7];
    header.record_count = 0;
    header.authority = snapshot_manager;
    Ok(())
}

#[derive(Accounts)]
pub struct MonitorArchiveInitialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Initialized as a zero-copy account header plus extra payload bytes.
    #[account(
        init,
        payer = payer,
        space = MAX_ACCOUNT_DATA_LEN,
    )]
    pub archive: AccountLoader<'info, ArchiveHeader>,

    pub system_program: Program<'info, System>,
}
