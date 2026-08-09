use anchor_lang::prelude::*;
use marginfi_type_crate::types::MintSnapshotsArchive;

use crate::MarginfiResult;

/// Solana maximum account data size (10 MiB). Clients allocate the archive at this
/// size with a top-level `system_instruction::create_account`; it cannot be allocated
/// from inside the program because CPI account growth is capped at
/// `MAX_PERMITTED_DATA_INCREASE` (10 KiB).
pub const MAX_ACCOUNT_DATA_LEN: usize = 10_485_760;
pub const MONITOR_INDEX_MAP_LEN: usize = 300;

pub fn monitor_archive_initialize(
    ctx: Context<MonitorArchiveInitialize>,
    snapshot_manager: Pubkey,
) -> MarginfiResult {
    MintSnapshotsArchive::initialize(&ctx.accounts.archive, snapshot_manager)
        .ok_or(crate::MarginfiError::InvalidConfig)?;
    Ok(())
}

#[derive(Accounts)]
pub struct MonitorArchiveInitialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Program-owned archive account, already allocated by the client at
    /// `MAX_ACCOUNT_DATA_LEN`. `MintSnapshotsArchive::initialize` rejects it unless it is
    /// large enough and its discriminator is still zero.
    #[account(mut, owner = crate::ID)]
    pub archive: UncheckedAccount<'info>,
}
