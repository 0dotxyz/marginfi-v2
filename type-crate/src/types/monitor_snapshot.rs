use super::ArchiveRecord;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;

/// One hourly snapshot point for a bank.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub snapshot_hour: u64,
    pub supply: u64,
    pub borrow: u64,
    pub price: u64,
    pub supply_apy: u64,
    pub borrow_apy: u64,
    pub native_apy: u64,
}

/// Historical snapshots for one bank, sorted descending by `snapshot_hour`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankSnapshotRecords<const MAX_SNAPSHOTS: usize> {
    pub bank: Pubkey,
    pub snapshots: [Snapshot; MAX_SNAPSHOTS],
}

/// Historical mint record containing all associated banks.
///
/// This is indexed by mint address and uses first 32 bytes of serialized
/// representation as the index key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MintSnapshotRecords<const MAX_BANKS: usize, const MAX_SNAPSHOTS: usize> {
    pub mint: Pubkey,
    pub banks: [BankSnapshotRecords<MAX_SNAPSHOTS>; MAX_BANKS],
}

impl Snapshot {
    pub const LEN: usize = 56;
    pub const ZERO: Self = Self {
        snapshot_hour: 0,
        supply: 0,
        borrow: 0,
        price: 0,
        supply_apy: 0,
        borrow_apy: 0,
        native_apy: 0,
    };

    pub const fn is_zero(&self) -> bool {
        self.snapshot_hour == 0
            && self.supply == 0
            && self.borrow == 0
            && self.price == 0
            && self.supply_apy == 0
            && self.borrow_apy == 0
            && self.native_apy == 0
    }
}

#[cfg(feature = "anchor")]
const ZERO_PUBKEY: Pubkey = Pubkey::new_from_array([0; 32]);
#[cfg(not(feature = "anchor"))]
const ZERO_PUBKEY: Pubkey = Pubkey::new([0; 32]);

impl<const MAX_SNAPSHOTS: usize> BankSnapshotRecords<MAX_SNAPSHOTS> {
    pub const LEN: usize = 32 + (MAX_SNAPSHOTS * Snapshot::LEN);

    pub const ZERO: Self = Self {
        bank: ZERO_PUBKEY,
        snapshots: [Snapshot::ZERO; MAX_SNAPSHOTS],
    };

    fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::LEN {
            return None;
        }

        let bank_bytes: [u8; 32] = bytes[0..32].try_into().ok()?;
        #[cfg(feature = "anchor")]
        let bank = Pubkey::new_from_array(bank_bytes);
        #[cfg(not(feature = "anchor"))]
        let bank = Pubkey::new(bank_bytes);

        let mut snapshots = [Snapshot::ZERO; MAX_SNAPSHOTS];
        for (i, slot) in snapshots.iter_mut().enumerate() {
            let offset = 32 + (i * Snapshot::LEN);
            *slot = Snapshot {
                snapshot_hour: u64::from_le_bytes(bytes[offset..offset + 8].try_into().ok()?),
                supply: u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().ok()?),
                borrow: u64::from_le_bytes(bytes[offset + 16..offset + 24].try_into().ok()?),
                price: u64::from_le_bytes(bytes[offset + 24..offset + 32].try_into().ok()?),
                supply_apy: u64::from_le_bytes(bytes[offset + 32..offset + 40].try_into().ok()?),
                borrow_apy: u64::from_le_bytes(bytes[offset + 40..offset + 48].try_into().ok()?),
                native_apy: u64::from_le_bytes(bytes[offset + 48..offset + 56].try_into().ok()?),
            };
        }

        Some(Self { bank, snapshots })
    }

    fn to_bytes(&self, out: &mut [u8]) -> Option<()> {
        if out.len() != Self::LEN {
            return None;
        }
        out[0..32].copy_from_slice(self.bank.as_ref());
        for (i, snap) in self.snapshots.iter().enumerate() {
            let offset = 32 + (i * Snapshot::LEN);
            out[offset..offset + 8].copy_from_slice(&snap.snapshot_hour.to_le_bytes());
            out[offset + 8..offset + 16].copy_from_slice(&snap.supply.to_le_bytes());
            out[offset + 16..offset + 24].copy_from_slice(&snap.borrow.to_le_bytes());
            out[offset + 24..offset + 32].copy_from_slice(&snap.price.to_le_bytes());
            out[offset + 32..offset + 40].copy_from_slice(&snap.supply_apy.to_le_bytes());
            out[offset + 40..offset + 48].copy_from_slice(&snap.borrow_apy.to_le_bytes());
            out[offset + 48..offset + 56].copy_from_slice(&snap.native_apy.to_le_bytes());
        }
        Some(())
    }

    /// Insert latest snapshot at index 0 and shift older snapshots right by one.
    ///
    /// For `MAX_SNAPSHOTS = 3`:
    /// `[A, _, _] -> [B, A, _] -> [C, B, A] -> [D, C, B]`
    ///
    /// Guardrails:
    /// - Rejects zeroed snapshot payloads (`snapshot.is_zero()`).
    /// - Rejects non-increasing timestamps when a latest snapshot exists.
    pub fn push_latest_snapshot(&mut self, snapshot: Snapshot) -> Option<()> {
        if MAX_SNAPSHOTS == 0 {
            return None;
        }
        if snapshot.is_zero() {
            return None;
        }
        let latest = self.snapshots[0];
        if !latest.is_zero() && snapshot.snapshot_hour <= latest.snapshot_hour {
            return None;
        }
        for i in (1..MAX_SNAPSHOTS).rev() {
            self.snapshots[i] = self.snapshots[i - 1];
        }
        self.snapshots[0] = snapshot;
        Some(())
    }

    /// Return true when this bank slot is unused.
    pub fn is_empty_slot(&self) -> bool {
        self.bank == ZERO_PUBKEY
    }
}

impl<const MAX_BANKS: usize, const MAX_SNAPSHOTS: usize> ArchiveRecord
    for MintSnapshotRecords<MAX_BANKS, MAX_SNAPSHOTS>
{
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

        let mut banks = [BankSnapshotRecords::<MAX_SNAPSHOTS>::ZERO; MAX_BANKS];
        for (i, slot) in banks.iter_mut().enumerate() {
            let offset = 33 + (i * BankSnapshotRecords::<MAX_SNAPSHOTS>::LEN);
            *slot = BankSnapshotRecords::<MAX_SNAPSHOTS>::parse(
                &bytes[offset..offset + BankSnapshotRecords::<MAX_SNAPSHOTS>::LEN],
            )?;
        }

        Some(Self { mint, banks })
    }

    fn to_bytes(&self, out: &mut [u8]) -> Option<()> {
        if out.len() != Self::LEN_V1 {
            return None;
        }

        out[0..32].copy_from_slice(self.mint.as_ref());
        out[32] = self.version();
        for (i, bank_rec) in self.banks.iter().enumerate() {
            let offset = 33 + (i * BankSnapshotRecords::<MAX_SNAPSHOTS>::LEN);
            bank_rec
                .to_bytes(&mut out[offset..offset + BankSnapshotRecords::<MAX_SNAPSHOTS>::LEN])?;
        }
        Some(())
    }

    fn index(&self) -> [u8; 32] {
        self.mint.to_bytes()
    }
}

impl<const MAX_BANKS: usize, const MAX_SNAPSHOTS: usize>
    MintSnapshotRecords<MAX_BANKS, MAX_SNAPSHOTS>
{
    pub const VERSION_V1: u8 = 1;
    pub const LEN_V1: usize = 33 + (MAX_BANKS * BankSnapshotRecords::<MAX_SNAPSHOTS>::LEN);

    /// Find bank slot index by bank pubkey.
    pub fn find_bank_index(&self, bank: &Pubkey) -> Option<usize> {
        self.banks.iter().position(|b| b.bank == *bank)
    }

    /// Find an existing bank slot or allocate an empty one.
    pub fn find_or_create_bank_slot(&mut self, bank: Pubkey) -> Option<usize> {
        if let Some(idx) = self.find_bank_index(&bank) {
            return Some(idx);
        }
        let idx = self.banks.iter().position(|b| b.is_empty_slot())?;
        self.banks[idx].bank = bank;
        Some(idx)
    }

    /// Apply one `{bank, snapshot}` update to this mint record.
    ///
    /// Behavior:
    /// - Reuses existing bank slot when present.
    /// - Otherwise allocates first empty bank slot.
    /// - Pushes latest snapshot to index 0 and shifts older snapshots right.
    pub fn apply_bank_snapshot(&mut self, bank: Pubkey, snapshot: Snapshot) -> Option<()> {
        let idx = self.find_or_create_bank_slot(bank)?;
        self.banks[idx].push_latest_snapshot(snapshot)
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
        Pubkey::new([seed; 32])
    }

    fn snap(hour: u64, seed: u64) -> Snapshot {
        Snapshot {
            snapshot_hour: hour,
            supply: seed,
            borrow: seed + 1,
            price: seed + 2,
            supply_apy: seed + 3,
            borrow_apy: seed + 4,
            native_apy: seed + 5,
        }
    }

    #[test]
    fn mint_record_round_trip() {
        type Rec = MintSnapshotRecords<2, 3>;
        let rec = Rec {
            mint: pk(9),
            banks: [
                BankSnapshotRecords {
                    bank: pk(1),
                    snapshots: [snap(10, 1), snap(9, 2), Snapshot::ZERO],
                },
                BankSnapshotRecords {
                    bank: pk(2),
                    snapshots: [snap(10, 3), Snapshot::ZERO, Snapshot::ZERO],
                },
            ],
        };

        let mut out = vec![0u8; Rec::LEN_V1];
        assert_eq!(rec.to_bytes(&mut out), Some(()));
        assert_eq!(Rec::parse(&out), Some(rec));
        assert_eq!(out[0..32], rec.index());
    }

    #[test]
    fn archive_update_or_insert_by_mint_index() {
        type Rec = MintSnapshotRecords<1, 2>;
        let mut header = new_header();
        let mut account_data = vec![0u8; ArchiveHeader::PAYLOAD_OFFSET + (2 * Rec::LEN_V1)];

        let rec_a = Rec {
            mint: pk(11),
            banks: [BankSnapshotRecords {
                bank: pk(1),
                snapshots: [snap(100, 1), snap(99, 2)],
            }],
        };
        let mut rec_a_updated = rec_a;
        rec_a_updated.banks[0].snapshots[0] = snap(101, 5);

        let rec_b = Rec {
            mint: pk(12),
            banks: [BankSnapshotRecords {
                bank: pk(2),
                snapshots: [snap(100, 7), Snapshot::ZERO],
            }],
        };

        assert_eq!(
            header.update_or_insert::<Rec>(&mut account_data, &rec_a),
            Some(())
        );
        assert_eq!(header.record_count, 1);
        assert_eq!(
            header.get_record::<Rec>(&account_data, rec_a.index()),
            Some((0, rec_a))
        );

        assert_eq!(
            header.update_or_insert::<Rec>(&mut account_data, &rec_a_updated),
            Some(())
        );
        assert_eq!(header.record_count, 1);
        assert_eq!(
            header.get_record::<Rec>(&account_data, rec_a.index()),
            Some((0, rec_a_updated))
        );

        assert_eq!(
            header.update_or_insert::<Rec>(&mut account_data, &rec_b),
            Some(())
        );
        assert_eq!(header.record_count, 2);
        assert_eq!(
            header.get_record::<Rec>(&account_data, rec_b.index()),
            Some((Rec::LEN_V1, rec_b))
        );
    }

    #[test]
    fn bank_snapshot_push_latest_shifts_right() {
        type BankRec = BankSnapshotRecords<3>;
        let mut bank_rec = BankRec {
            bank: pk(55),
            snapshots: [Snapshot::ZERO, Snapshot::ZERO, Snapshot::ZERO],
        };

        let a = snap(100, 1);
        let b = snap(101, 2);
        let c = snap(102, 3);
        let d = snap(103, 4);

        assert_eq!(bank_rec.push_latest_snapshot(a), Some(()));
        assert_eq!(bank_rec.snapshots, [a, Snapshot::ZERO, Snapshot::ZERO]);

        assert_eq!(bank_rec.push_latest_snapshot(b), Some(()));
        assert_eq!(bank_rec.snapshots, [b, a, Snapshot::ZERO]);

        assert_eq!(bank_rec.push_latest_snapshot(c), Some(()));
        assert_eq!(bank_rec.snapshots, [c, b, a]);

        assert_eq!(bank_rec.push_latest_snapshot(d), Some(()));
        assert_eq!(bank_rec.snapshots, [d, c, b]);
    }

    #[test]
    fn apply_bank_snapshot_finds_or_creates_slot() {
        type Rec = MintSnapshotRecords<2, 3>;
        let mut rec = Rec {
            mint: pk(99),
            banks: [BankSnapshotRecords::ZERO, BankSnapshotRecords::ZERO],
        };

        let bank_a = pk(1);
        let bank_b = pk(2);

        let a1 = snap(10, 10);
        let a2 = snap(11, 11);
        let b1 = snap(12, 12);

        assert_eq!(rec.apply_bank_snapshot(bank_a, a1), Some(()));
        assert_eq!(rec.banks[0].bank, bank_a);
        assert_eq!(rec.banks[0].snapshots[0], a1);

        assert_eq!(rec.apply_bank_snapshot(bank_a, a2), Some(()));
        assert_eq!(rec.banks[0].snapshots[0], a2);
        assert_eq!(rec.banks[0].snapshots[1], a1);

        assert_eq!(rec.apply_bank_snapshot(bank_b, b1), Some(()));
        assert_eq!(rec.banks[1].bank, bank_b);
        assert_eq!(rec.banks[1].snapshots[0], b1);
    }

    #[test]
    fn push_latest_snapshot_rejects_zero_or_non_increasing() {
        type BankRec = BankSnapshotRecords<3>;
        let mut bank_rec = BankRec {
            bank: pk(77),
            snapshots: [Snapshot::ZERO, Snapshot::ZERO, Snapshot::ZERO],
        };

        assert_eq!(bank_rec.push_latest_snapshot(Snapshot::ZERO), None);
        assert_eq!(
            bank_rec.snapshots,
            [Snapshot::ZERO, Snapshot::ZERO, Snapshot::ZERO]
        );

        let a = snap(200, 1);
        let same_ts = snap(200, 2);
        let lower_ts = snap(199, 3);

        assert_eq!(bank_rec.push_latest_snapshot(a), Some(()));
        assert_eq!(bank_rec.push_latest_snapshot(same_ts), None);
        assert_eq!(bank_rec.push_latest_snapshot(lower_ts), None);
        assert_eq!(bank_rec.snapshots[0], a);
    }
}
