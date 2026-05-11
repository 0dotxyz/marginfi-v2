use anchor_lang::prelude::*;
use marginfi_type_crate::types::{ArchiveHeader, MintSnapshotRecords, Snapshot};

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

    let archive_ai = &ctx.accounts.archive;
    check!(archive_ai.owner == &crate::ID, MarginfiError::InvalidConfig);

    let mut data = archive_ai.try_borrow_mut_data()?;
    check!(
        data.len() >= ArchiveHeader::PAYLOAD_OFFSET,
        MarginfiError::InvalidConfig
    );

    let mut header = ArchiveHeader::read_from_account_data(&data).ok_or(MarginfiError::InvalidConfig)?;
    check!(header.authority == manager, MarginfiError::Unauthorized);

    check!(
        updates.len() <= ctx.remaining_accounts.len(),
        MarginfiError::InvalidConfig
    );

    for (i, update) in updates.into_iter().enumerate() {
        let mint_ai = ctx
            .remaining_accounts
            .get(i)
            .ok_or(MarginfiError::InvalidConfig)?;

        let mint = *mint_ai.key;
        let snapshot = Snapshot {
            snapshot_hour: update.snapshot_hour,
            price: update.price,
            native_apy: update.native_apy,
        };

        if let Some((_, slot)) = header.find_slot_in_account_mut::<
            MintSnapshotRecords<{ ArchiveHeader::MAX_SNAPSHOTS_PER_MINT }>,
        >(&mut data, mint.to_bytes())
        {
            MintSnapshotRecords::<{ ArchiveHeader::MAX_SNAPSHOTS_PER_MINT }>::push_latest_snapshot_bytes(slot, snapshot)
                .ok_or(MarginfiError::InvalidConfig)?;
        } else {
            let mut record = MintSnapshotRecords::<{ ArchiveHeader::MAX_SNAPSHOTS_PER_MINT }>::new(mint);
            record.push_latest_snapshot(snapshot).ok_or(MarginfiError::InvalidConfig)?;
            header
                .update_or_insert::<MintSnapshotRecords<{ ArchiveHeader::MAX_SNAPSHOTS_PER_MINT }>>(
                    &mut data, &record,
                )
                .ok_or(MarginfiError::InvalidConfig)?;
        }
    }

    header
        .write_to_account_data(&mut data)
        .ok_or(MarginfiError::InvalidConfig)?;
    Ok(())
}

#[derive(Accounts)]
pub struct MonitorArchiveUpsertBatch<'info> {
    /// Dedicated signer for monitor snapshot archive writes. Must match
    /// `ArchiveHeader.authority`.
    pub snapshot_manager: Signer<'info>,

    /// CHECK: Program-owned archive account with raw bytes layout:
    /// [8-byte discriminator][ArchiveHeader][payload].
    #[account(mut, owner = crate::ID)]
    pub archive: AccountInfo<'info>,
}
