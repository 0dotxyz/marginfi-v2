use super::ArchiveRecord;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;

/// One hourly snapshot point for a mint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub snapshot_hour: u64,
    pub price: u64,
    pub native_apy: u64,
}

/// Historical record for a mint, sorted descending by `snapshot_hour`.
///
/// This is indexed by mint address and uses the first 32 bytes of the
/// serialized representation as the archive key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintSnapshotRecords<const MAX_SNAPSHOTS: usize> {
    pub mint: Pubkey,
    /// Physical index of the oldest snapshot currently stored.
    pub head: u16,
    /// Number of valid snapshots currently stored (always `<= MAX_SNAPSHOTS`).
    pub len: u16,
    pub _pad: [u8; 4],
    pub snapshots: [Snapshot; MAX_SNAPSHOTS],
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

impl<const MAX_SNAPSHOTS: usize> ArchiveRecord for MintSnapshotRecords<MAX_SNAPSHOTS> {
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
        let len = u16::from_le_bytes(bytes[35..37].try_into().ok()?);
        if usize::from(len) > MAX_SNAPSHOTS {
            return None;
        }

        let mut snapshots = [Snapshot::ZERO; MAX_SNAPSHOTS];
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
            len,
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
        out[35..37].copy_from_slice(&self.len.to_le_bytes());
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

impl<const MAX_SNAPSHOTS: usize> MintSnapshotRecords<MAX_SNAPSHOTS> {
    pub const VERSION_V1: u8 = 1;
    pub const LEN_V1: usize = 41 + (MAX_SNAPSHOTS * Snapshot::LEN);

    /// Create a new mint snapshot record with an empty ring buffer.
    pub fn new(mint: Pubkey) -> Self {
        Self {
            mint,
            head: 0,
            len: 0,
            _pad: [0; 4],
            snapshots: [Snapshot::ZERO; MAX_SNAPSHOTS],
        }
    }

    /// Append a newer snapshot into the mint ring buffer.
    ///
    /// Guardrails:
    /// - Rejects zeroed snapshot payloads (`snapshot.is_zero()`).
    /// - Rejects non-increasing timestamps relative to current latest.
    ///
    /// Behavior:
    /// - If not full, writes to tail and increments `len`.
    /// - If full, overwrites `head` (oldest) and advances `head` by one.
    pub fn push_latest_snapshot(&mut self, snapshot: Snapshot) -> Option<()> {
        if MAX_SNAPSHOTS == 0 {
            return None;
        }
        if snapshot.is_zero() {
            return None;
        }

        if let Some(latest) = self.latest_snapshot() {
            if snapshot.snapshot_hour <= latest.snapshot_hour {
                return None;
            }
        }

        let cap_u16 = u16::try_from(MAX_SNAPSHOTS).ok()?;
        if self.len < cap_u16 {
            let tail = (usize::from(self.head) + usize::from(self.len)) % MAX_SNAPSHOTS;
            self.snapshots[tail] = snapshot;
            self.len = self.len.checked_add(1)?;
            return Some(());
        }

        let head_idx = usize::from(self.head);
        if head_idx >= MAX_SNAPSHOTS {
            return None;
        }
        self.snapshots[head_idx] = snapshot;
        self.head = if self.head + 1 == cap_u16 {
            0
        } else {
            self.head + 1
        };
        Some(())
    }

    /// Return the newest stored snapshot.
    pub fn latest_snapshot(&self) -> Option<Snapshot> {
        if self.len == 0 || MAX_SNAPSHOTS == 0 {
            return None;
        }
        let latest_idx =
            (usize::from(self.head) + usize::from(self.len).checked_sub(1)?) % MAX_SNAPSHOTS;
        Some(self.snapshots[latest_idx])
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
        Pubkey::from([seed; 32])
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
        type Rec = MintSnapshotRecords<3>;
        let mut rec = Rec::new(pk(9));
        assert_eq!(rec.push_latest_snapshot(snap(9, 2)), Some(()));
        assert_eq!(rec.push_latest_snapshot(snap(10, 1)), Some(()));

        let mut out = vec![0u8; Rec::LEN_V1];
        assert_eq!(rec.to_bytes(&mut out), Some(()));
        assert_eq!(Rec::parse(&out), Some(rec));
        assert_eq!(out[0..32], rec.index());
    }

    #[test]
    fn archive_update_or_insert_by_mint_index() {
        type Rec = MintSnapshotRecords<2>;
        let mut header = new_header();
        let mut account_data = vec![0u8; ArchiveHeader::PAYLOAD_OFFSET + (2 * Rec::LEN_V1)];

        let mut rec_a = Rec::new(pk(11));
        assert_eq!(rec_a.push_latest_snapshot(snap(99, 2)), Some(()));
        assert_eq!(rec_a.push_latest_snapshot(snap(100, 1)), Some(()));
        let mut rec_a_updated = rec_a;
        assert_eq!(rec_a_updated.push_latest_snapshot(snap(101, 5)), Some(()));

        let mut rec_b = Rec::new(pk(12));
        assert_eq!(rec_b.push_latest_snapshot(snap(100, 7)), Some(()));

        assert_eq!(
            header.update_or_insert::<Rec>(&mut account_data, &rec_a),
            Some(())
        );
        assert_eq!(
            header.update_or_insert::<Rec>(&mut account_data, &rec_b),
            Some(())
        );
        assert_eq!(header.record_count, 2);

        assert_eq!(
            header.update_or_insert::<Rec>(&mut account_data, &rec_a_updated),
            Some(())
        );
        assert_eq!(header.record_count, 2);
        assert_eq!(
            header.get_record::<Rec>(&account_data, rec_a.index()),
            Some((0, rec_a_updated))
        );
    }

    #[test]
    fn push_latest_snapshot_ring_overwrites_oldest() {
        type Rec = MintSnapshotRecords<3>;
        let mut rec = Rec::new(pk(42));
        let a = snap(1, 10);
        let b = snap(2, 20);
        let c = snap(3, 30);
        let d = snap(4, 40);

        assert_eq!(rec.push_latest_snapshot(a), Some(()));
        assert_eq!(rec.head, 0);
        assert_eq!(rec.len, 1);
        assert_eq!(rec.latest_snapshot(), Some(a));

        assert_eq!(rec.push_latest_snapshot(b), Some(()));
        assert_eq!(rec.head, 0);
        assert_eq!(rec.len, 2);
        assert_eq!(rec.latest_snapshot(), Some(b));

        assert_eq!(rec.push_latest_snapshot(c), Some(()));
        assert_eq!(rec.head, 0);
        assert_eq!(rec.len, 3);
        assert_eq!(rec.latest_snapshot(), Some(c));

        assert_eq!(rec.push_latest_snapshot(d), Some(()));
        assert_eq!(rec.head, 1);
        assert_eq!(rec.len, 3);
        assert_eq!(rec.latest_snapshot(), Some(d));
    }

    #[test]
    fn push_latest_snapshot_rejects_zero_and_non_increasing() {
        type Rec = MintSnapshotRecords<2>;
        let mut rec = Rec::new(pk(7));

        assert_eq!(rec.push_latest_snapshot(Snapshot::ZERO), None);
        assert_eq!(rec.push_latest_snapshot(snap(10, 1)), Some(()));
        assert_eq!(rec.push_latest_snapshot(snap(10, 2)), None);
        assert_eq!(rec.push_latest_snapshot(snap(9, 3)), None);
    }
}
