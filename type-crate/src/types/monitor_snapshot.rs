use super::ArchiveRecord;
#[cfg(feature = "anchor")]
use super::ArchiveHeader;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;

const MAX_SNAPSHOTS_PER_MINT_CAPACITY: usize = 168;

/// One hourly snapshot point for a mint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub snapshot_hour: u64,
    pub price: u64,
    pub native_apy: u64,
}

impl Snapshot {
    pub const LEN: usize = 24;
    pub const ZERO: Self = Self {
        snapshot_hour: 0,
        price: 0,
        native_apy: 0,
    };

    pub const fn is_zero(&self) -> bool {
        self.snapshot_hour == 0 && self.price == 0 && self.native_apy == 0
    }
}

/// Historical record for a mint, sorted descending by `snapshot_hour`.
///
/// This is indexed by mint address and uses the first 32 bytes of the
/// serialized representation as the archive key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintSnapshotRecords {
    pub mint: Pubkey,
    /// Physical index of the oldest snapshot currently stored.
    pub head: u16,
    /// Physical index of the newest snapshot currently stored.
    pub tail: u16,
    pub _pad: [u8; 4],
    pub snapshots: [Snapshot; MAX_SNAPSHOTS_PER_MINT_CAPACITY],
}

impl ArchiveRecord for MintSnapshotRecords {
    fn len(version: u8) -> Option<usize> {
        match version {
            Self::VERSION_V1 => Some(Self::LEN_V1),
            _ => None,
        }
    }

    fn version(&self) -> u8 {
        Self::VERSION_V1
    }

    fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::LEN_V1 || bytes[32] != Self::VERSION_V1 {
            return None;
        }

        let mint_bytes: [u8; 32] = bytes[0..32].try_into().ok()?;
        #[cfg(feature = "anchor")]
        let mint = Pubkey::new_from_array(mint_bytes);
        #[cfg(not(feature = "anchor"))]
        let mint = Pubkey::new(mint_bytes);

        let head = u16::from_le_bytes(bytes[33..35].try_into().ok()?);
        let tail = u16::from_le_bytes(bytes[35..37].try_into().ok()?);
        let cap_u16 = u16::try_from(Self::MAX_SNAPSHOTS_PER_MINT).ok()?;
        if head >= cap_u16 || tail >= cap_u16 {
            return None;
        }

        let mut snapshots = [Snapshot::ZERO; Self::MAX_SNAPSHOTS_PER_MINT];
        for (i, slot) in snapshots.iter_mut().enumerate() {
            let offset = 41 + (i * Snapshot::LEN);
            *slot = Snapshot {
                snapshot_hour: u64::from_le_bytes(bytes[offset..offset + 8].try_into().ok()?),
                price: u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().ok()?),
                native_apy: u64::from_le_bytes(bytes[offset + 16..offset + 24].try_into().ok()?),
            };
        }

        Some(Self {
            mint,
            head,
            tail,
            _pad: [0; 4],
            snapshots,
        })
    }

    fn to_bytes(&self, out: &mut [u8]) -> Option<()> {
        if out.len() != Self::LEN_V1 {
            return None;
        }

        out[0..32].copy_from_slice(self.mint.as_ref());
        out[32] = self.version();
        out[33..35].copy_from_slice(&self.head.to_le_bytes());
        out[35..37].copy_from_slice(&self.tail.to_le_bytes());
        out[37..41].copy_from_slice(&self._pad);
        for (i, snap) in self.snapshots.iter().enumerate() {
            let offset = 41 + (i * Snapshot::LEN);
            out[offset..offset + 8].copy_from_slice(&snap.snapshot_hour.to_le_bytes());
            out[offset + 8..offset + 16].copy_from_slice(&snap.price.to_le_bytes());
            out[offset + 16..offset + 24].copy_from_slice(&snap.native_apy.to_le_bytes());
        }
        Some(())
    }

    fn index(&self) -> [u8; 32] {
        self.mint.to_bytes()
    }
}

impl MintSnapshotRecords {
    /// Fixed retention target: 24 hourly snapshots * 7 days.
    pub const MAX_SNAPSHOTS_PER_MINT: usize = MAX_SNAPSHOTS_PER_MINT_CAPACITY;
    pub const VERSION_V1: u8 = 1;
    pub const LEN_V1: usize = 41 + (Self::MAX_SNAPSHOTS_PER_MINT * Snapshot::LEN);
    pub const VERSION_OFFSET: usize = 32;
    pub const HEAD_OFFSET: usize = 33;
    pub const TAIL_OFFSET: usize = 35;
    pub const SNAPSHOTS_OFFSET: usize = 41;

    /// Create a new mint snapshot record with an empty ring buffer.
    pub fn new(mint: Pubkey) -> Self {
        Self {
            mint,
            head: 0,
            tail: 0,
            _pad: [0; 4],
            snapshots: [Snapshot::ZERO; Self::MAX_SNAPSHOTS_PER_MINT],
        }
    }

    /// Append a newer snapshot into the mint ring buffer.
    ///
    /// Guardrails:
    /// - Rejects zeroed snapshot payloads (`snapshot.is_zero()`).
    /// - Rejects non-increasing timestamps relative to current latest.
    ///
    /// Behavior:
    /// - Bootstrap empty state is encoded as `head=0, tail=0, snapshots[0]=ZERO`.
    /// - If not full, advances `tail` and writes the new snapshot there.
    /// - If full, overwrites `head` (oldest), then advances `head`.
    pub fn push_latest_snapshot(&mut self, snapshot: Snapshot) -> Option<()> {
        if snapshot.is_zero() {
            return None;
        }

        let cap_u16 = u16::try_from(Self::MAX_SNAPSHOTS_PER_MINT).ok()?;
        // Specialized sentinel for this record type:
        // both cursors at zero + zero-value slot0 means "empty".
        let is_bootstrap_empty = self.head == 0 && self.tail == 0 && self.snapshots[0].is_zero();
        if is_bootstrap_empty {
            self.snapshots[0] = snapshot;
            return Some(());
        }

        let latest = self.latest_snapshot()?;
        if snapshot.snapshot_hour <= latest.snapshot_hour {
            return None;
        }

        let wrapped_tail = if self.tail + 1 == cap_u16 {
            0
        } else {
            self.tail + 1
        };
        let (next_tail, next_head) = if wrapped_tail == self.head {
            let bumped_head = if self.head + 1 == cap_u16 {
                0
            } else {
                self.head + 1
            };
            (self.head, bumped_head)
        } else {
            (wrapped_tail, self.head)
        };

        self.tail = next_tail;
        self.head = next_head;
        self.snapshots[usize::from(self.tail)] = snapshot;
        Some(())
    }

    /// Return the newest stored snapshot.
    pub fn latest_snapshot(&self) -> Option<Snapshot> {
        if self.head == 0 && self.tail == 0 && self.snapshots[0].is_zero() {
            return None;
        }
        Some(self.snapshots[usize::from(self.tail)])
    }

    /// Append a snapshot by mutating an already-serialized slot in place.
    ///
    /// This avoids full record parse/serialize in hot paths (CU optimization).
    pub fn push_latest_snapshot_bytes(slot: &mut [u8], snapshot: Snapshot) -> Option<()> {
        if snapshot.is_zero() || slot.len() != Self::LEN_V1 {
            return None;
        }
        if *slot.get(Self::VERSION_OFFSET)? != Self::VERSION_V1 {
            return None;
        }

        let mut head = u16::from_le_bytes(
            slot[Self::HEAD_OFFSET..Self::HEAD_OFFSET + 2]
                .try_into()
                .ok()?,
        );
        let mut tail = u16::from_le_bytes(
            slot[Self::TAIL_OFFSET..Self::TAIL_OFFSET + 2]
                .try_into()
                .ok()?,
        );
        let cap_u16 = u16::try_from(Self::MAX_SNAPSHOTS_PER_MINT).ok()?;
        if head >= cap_u16 || tail >= cap_u16 {
            return None;
        }

        let is_bootstrap_empty =
            head == 0 && tail == 0 && Self::read_snapshot_at(slot, 0)?.is_zero();
        if is_bootstrap_empty {
            Self::write_snapshot_at(slot, 0, snapshot)?;
            return Some(());
        }

        {
            let latest = Self::read_snapshot_at(slot, usize::from(tail))?;
            if snapshot.snapshot_hour <= latest.snapshot_hour {
                return None;
            }
        }

        let wrapped_tail = if tail + 1 == cap_u16 { 0 } else { tail + 1 };
        let (next_tail, next_head, write_idx) = if wrapped_tail == head {
            let bumped_head = if head + 1 == cap_u16 { 0 } else { head + 1 };
            (head, bumped_head, usize::from(head))
        } else {
            (wrapped_tail, head, usize::from(wrapped_tail))
        };

        tail = next_tail;
        head = next_head;
        Self::write_snapshot_at(slot, write_idx, snapshot)?;
        slot[Self::HEAD_OFFSET..Self::HEAD_OFFSET + 2].copy_from_slice(&head.to_le_bytes());
        slot[Self::TAIL_OFFSET..Self::TAIL_OFFSET + 2].copy_from_slice(&tail.to_le_bytes());
        Some(())
    }

    fn read_snapshot_at(slot: &[u8], idx: usize) -> Option<Snapshot> {
        let base = Self::SNAPSHOTS_OFFSET.checked_add(idx.checked_mul(Snapshot::LEN)?)?;
        Some(Snapshot {
            snapshot_hour: u64::from_le_bytes(slot.get(base..base + 8)?.try_into().ok()?),
            price: u64::from_le_bytes(slot.get(base + 8..base + 16)?.try_into().ok()?),
            native_apy: u64::from_le_bytes(slot.get(base + 16..base + 24)?.try_into().ok()?),
        })
    }

    fn write_snapshot_at(slot: &mut [u8], idx: usize, snapshot: Snapshot) -> Option<()> {
        let base = Self::SNAPSHOTS_OFFSET.checked_add(idx.checked_mul(Snapshot::LEN)?)?;
        slot.get_mut(base..base + 8)?
            .copy_from_slice(&snapshot.snapshot_hour.to_le_bytes());
        slot.get_mut(base + 8..base + 16)?
            .copy_from_slice(&snapshot.price.to_le_bytes());
        slot.get_mut(base + 16..base + 24)?
            .copy_from_slice(&snapshot.native_apy.to_le_bytes());
        Some(())
    }

    /// Load a mint snapshot record directly from an archive account.
    ///
    /// This helper is intended for on-chain callers that already receive the
    /// archive account as an input account and want typed access without
    /// re-implementing byte parsing logic.
    #[cfg(feature = "anchor")]
    pub fn from_archive_account(archive: &AccountInfo, mint: Pubkey) -> Option<Self> {
        let data = archive.try_borrow_data().ok()?;
        let header = ArchiveHeader::read_from_account_data(&data)?;
        let (_, record) = header.get_record::<Self>(&data, mint.to_bytes())?;
        Some(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ArchiveHeader;

    fn new_header() -> ArchiveHeader {
        ArchiveHeader {
            version: 1,
            _pad0: [0; 7],
            record_count: 0,
            authority: Pubkey::default(),
        }
    }

    fn pk(seed: u8) -> Pubkey {
        #[cfg(feature = "anchor")]
        {
            Pubkey::from([seed; 32])
        }
        #[cfg(not(feature = "anchor"))]
        {
            Pubkey::new([seed; 32])
        }
    }

    fn snap(hour: u64, seed: u64) -> Snapshot {
        Snapshot {
            snapshot_hour: hour,
            price: seed,
            native_apy: seed + 1,
        }
    }

    #[test]
    fn mint_record_round_trip() {
        let mut rec = MintSnapshotRecords::new(pk(9));
        assert_eq!(rec.push_latest_snapshot(snap(9, 2)), Some(()));
        assert_eq!(rec.push_latest_snapshot(snap(10, 1)), Some(()));

        let mut out = vec![0u8; MintSnapshotRecords::LEN_V1];
        assert_eq!(rec.to_bytes(&mut out), Some(()));
        assert_eq!(MintSnapshotRecords::parse(&out), Some(rec));
        assert_eq!(out[0..32], rec.index());
    }

    #[test]
    fn archive_update_or_insert_by_mint_index() {
        let mut header = new_header();
        let mut account_data =
            vec![0u8; ArchiveHeader::PAYLOAD_OFFSET + (2 * MintSnapshotRecords::LEN_V1)];

        let mut rec_a = MintSnapshotRecords::new(pk(11));
        assert_eq!(rec_a.push_latest_snapshot(snap(99, 2)), Some(()));
        assert_eq!(rec_a.push_latest_snapshot(snap(100, 1)), Some(()));
        let mut rec_a_updated = rec_a;
        assert_eq!(rec_a_updated.push_latest_snapshot(snap(101, 5)), Some(()));

        let mut rec_b = MintSnapshotRecords::new(pk(12));
        assert_eq!(rec_b.push_latest_snapshot(snap(100, 7)), Some(()));

        assert_eq!(
            header.update_or_insert::<MintSnapshotRecords>(&mut account_data, &rec_a),
            Some(())
        );
        assert_eq!(
            header.update_or_insert::<MintSnapshotRecords>(&mut account_data, &rec_b),
            Some(())
        );
        assert_eq!(header.record_count, 2);

        assert_eq!(
            header.update_or_insert::<MintSnapshotRecords>(&mut account_data, &rec_a_updated),
            Some(())
        );
        assert_eq!(header.record_count, 2);
        assert_eq!(
            header.get_record::<MintSnapshotRecords>(&account_data, rec_a.index()),
            Some((0, rec_a_updated))
        );
    }

    #[test]
    fn push_latest_snapshot_ring_overwrites_oldest() {
        let mut rec = MintSnapshotRecords::new(pk(42));
        assert_eq!(rec.push_latest_snapshot(snap(1, 10)), Some(()));
        assert_eq!(rec.head, 0);
        assert_eq!(rec.tail, 0);
        assert_eq!(rec.latest_snapshot(), Some(snap(1, 10)));

        assert_eq!(rec.push_latest_snapshot(snap(2, 20)), Some(()));
        assert_eq!(rec.head, 0);
        assert_eq!(rec.tail, 1);
        assert_eq!(rec.latest_snapshot(), Some(snap(2, 20)));

        for i in 3..=MintSnapshotRecords::MAX_SNAPSHOTS_PER_MINT as u64 {
            assert_eq!(rec.push_latest_snapshot(snap(i, i * 10)), Some(()));
        }
        assert_eq!(rec.head, 0);
        assert_eq!(
            rec.tail,
            (MintSnapshotRecords::MAX_SNAPSHOTS_PER_MINT - 1) as u16
        );

        let latest_before_overwrite = snap(
            MintSnapshotRecords::MAX_SNAPSHOTS_PER_MINT as u64,
            MintSnapshotRecords::MAX_SNAPSHOTS_PER_MINT as u64 * 10,
        );
        assert_eq!(rec.latest_snapshot(), Some(latest_before_overwrite));

        let overwrite = snap(MintSnapshotRecords::MAX_SNAPSHOTS_PER_MINT as u64 + 1, 9_999);
        assert_eq!(rec.push_latest_snapshot(overwrite), Some(()));
        assert_eq!(rec.head, 1);
        assert_eq!(rec.tail, 0);
        assert_eq!(rec.latest_snapshot(), Some(overwrite));
    }

    #[test]
    fn push_latest_snapshot_rejects_zero_and_non_increasing() {
        let mut rec = MintSnapshotRecords::new(pk(7));

        assert_eq!(rec.push_latest_snapshot(Snapshot::ZERO), None);
        assert_eq!(rec.push_latest_snapshot(snap(10, 1)), Some(()));
        assert_eq!(rec.push_latest_snapshot(snap(10, 2)), None);
        assert_eq!(rec.push_latest_snapshot(snap(9, 3)), None);
    }
}
