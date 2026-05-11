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

    let mut header = read_archive_header(&data)?;
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

        let mut record = if let Some((_, existing)) = header.get_record::<MintSnapshotRecords<
            { ArchiveHeader::MAX_SNAPSHOTS_PER_MINT },
        >>(&data, mint.to_bytes())
        {
            existing
        } else {
            MintSnapshotRecords::<{ ArchiveHeader::MAX_SNAPSHOTS_PER_MINT }>::new(mint)
        };

        record
            .push_latest_snapshot(snapshot)
            .ok_or(MarginfiError::InvalidConfig)?;

        header
            .update_or_insert::<MintSnapshotRecords<{ ArchiveHeader::MAX_SNAPSHOTS_PER_MINT }>>(
                &mut data, &record,
            )
            .ok_or(MarginfiError::InvalidConfig)?;
    }

    write_archive_header(&mut data, &header);
    Ok(())
}

fn read_archive_header(data: &[u8]) -> Result<ArchiveHeader> {
    let version = *data
        .get(ArchiveHeader::HEADER_VERSION_OFFSET)
        .ok_or(MarginfiError::InvalidConfig)?;
    let record_count = u64::from_le_bytes(
        data.get(
            ArchiveHeader::HEADER_RECORD_COUNT_OFFSET
                ..ArchiveHeader::HEADER_RECORD_COUNT_OFFSET + 8,
        )
        .ok_or(MarginfiError::InvalidConfig)?
        .try_into()
        .map_err(|_| MarginfiError::InvalidConfig)?,
    );
    let authority_bytes: [u8; 32] = data
        .get(ArchiveHeader::HEADER_AUTHORITY_OFFSET..ArchiveHeader::HEADER_AUTHORITY_OFFSET + 32)
        .ok_or(MarginfiError::InvalidConfig)?
        .try_into()
        .map_err(|_| MarginfiError::InvalidConfig)?;

    Ok(ArchiveHeader {
        version,
        _pad0: [0; 7],
        record_count,
        authority: Pubkey::new_from_array(authority_bytes),
    })
}

fn write_archive_header(data: &mut [u8], header: &ArchiveHeader) {
    data[ArchiveHeader::HEADER_VERSION_OFFSET] = header.version;
    data[ArchiveHeader::HEADER_RECORD_COUNT_OFFSET..ArchiveHeader::HEADER_RECORD_COUNT_OFFSET + 8]
        .copy_from_slice(&header.record_count.to_le_bytes());
    data[ArchiveHeader::HEADER_AUTHORITY_OFFSET..ArchiveHeader::HEADER_AUTHORITY_OFFSET + 32]
        .copy_from_slice(header.authority.as_ref());
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
