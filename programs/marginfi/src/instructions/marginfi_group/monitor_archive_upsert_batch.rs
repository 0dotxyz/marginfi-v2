use anchor_lang::prelude::*;
use marginfi_type_crate::types::{MintSnapshotRecords, MintSnapshotsArchive, Snapshot};

use crate::{check, MarginfiError, MarginfiResult};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotUpdateInput {
    pub snapshot_hour: u64,
    pub price: u64,
    pub native_apy: u64,
}

/// Upsert many monitor snapshot points in one instruction.
///
/// Mints are not passed as ix args; update `i` maps to `remaining_accounts[i]`
/// so clients can use LUT-backed account metas with compact ix data.
pub fn monitor_archive_upsert_batch(
    ctx: Context<MonitorArchiveUpsertBatch>,
    updates: Vec<SnapshotUpdateInput>,
) -> MarginfiResult {
    let manager = ctx.accounts.snapshot_manager.key();

    let mut archive = MintSnapshotsArchive::from_account_info(&ctx.accounts.archive)
        .ok_or(MarginfiError::InvalidConfig)?;

    check!(
        archive.meta().authority == manager,
        MarginfiError::Unauthorized
    );

    check!(
        updates.len() <= ctx.remaining_accounts.len(),
        MarginfiError::InvalidConfig
    );

    for (i, update) in updates.into_iter().enumerate() {
        let mint_info = ctx
            .remaining_accounts
            .get(i)
            .ok_or(MarginfiError::InvalidConfig)?;

        let mint = *mint_info.key;
        let snapshot = Snapshot {
            snapshot_hour: update.snapshot_hour,
            price: update.price,
            native_apy: update.native_apy,
        };

        // Records are mutated as raw bytes. A `MintSnapshotRecords` value is wider than the
        // SBF 4 KiB stack frame, so materializing one here aborts the program on entry.
        if let Some(position) = archive.position(mint.to_bytes()) {
            let mut slot = archive
                .slot_mut_bytes_at(position)
                .ok_or(MarginfiError::InvalidConfig)?;

            MintSnapshotRecords::push_latest_snapshot_bytes(&mut slot, snapshot)
                .ok_or(MarginfiError::InvalidConfig)?;
        } else {
            let (_, mut slot) = archive
                .append_slot(MintSnapshotRecords::VERSION_V1)
                .ok_or(MarginfiError::InvalidConfig)?;

            MintSnapshotRecords::init_bytes(&mut slot, mint).ok_or(MarginfiError::InvalidConfig)?;
            MintSnapshotRecords::push_latest_snapshot_bytes(&mut slot, snapshot)
                .ok_or(MarginfiError::InvalidConfig)?;
        }
    }

    Ok(())
}

#[derive(Accounts)]
pub struct MonitorArchiveUpsertBatch<'info> {
    /// Dedicated signer for monitor snapshot archive writes. Must match
    /// `ArchiveMeta.authority`.
    pub snapshot_manager: Signer<'info>,

    /// CHECK: Program-owned archive account with raw bytes layout:
    /// [8-byte discriminator][ArchiveMeta][index_map][payload].
    #[account(mut, owner = crate::ID)]
    pub archive: UncheckedAccount<'info>,
}
