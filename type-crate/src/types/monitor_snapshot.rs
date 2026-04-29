use super::HistoryRecord;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;

/// Snapshot reference key.
///
/// - `Mint(pubkey)` corresponds to `ref_type = "mint"` and `ref_key = mint address`
/// - `Bank(pubkey)` corresponds to `ref_type = "bank"` and `ref_key = bank address`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ref {
    Mint(Pubkey),
    Bank(Pubkey),
}

/// On-chain monitor snapshot record stored in HistoryArchive.
///
/// This maps to the off-chain `hourly_snapshots` row shape, with timestamps stored
/// as unix seconds and metric values stored as `u64` to avoid floating-point usage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorSnapshotRecord {
    /// Start-of-hour unix timestamp.
    pub snapshot_hour: u64,
    /// Reference dimension (`mint` or `bank`) and its address.
    pub reference: Ref,
    pub supply: u64,
    pub borrow: u64,
    pub price: u64,
    pub supply_apy: u64,
    pub borrow_apy: u64,
    pub native_apy: u64,
    /// Record creation unix timestamp.
    pub created_at: u64,
    /// Record update unix timestamp.
    pub updated_at: u64,
}

impl MonitorSnapshotRecord {
    /// Serialized tag for `Ref::Mint`.
    pub const REF_MINT: u8 = 0;
    /// Serialized tag for `Ref::Bank`.
    pub const REF_BANK: u8 = 1;
}

impl HistoryRecord for MonitorSnapshotRecord {
    // 8 snapshot_hour
    // 1 ref_kind + 7 pad + 32 ref_pubkey
    // 6 * 8 floats
    // 8 created_at + 8 updated_at
    const LEN: usize = 112;

    fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::LEN {
            return None;
        }

        let snapshot_hour = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let ref_kind = bytes[8];
        let ref_pubkey_bytes: [u8; 32] = bytes[16..48].try_into().ok()?;
        #[cfg(feature = "anchor")]
        let ref_pubkey = Pubkey::new_from_array(ref_pubkey_bytes);
        #[cfg(not(feature = "anchor"))]
        let ref_pubkey = Pubkey::new(ref_pubkey_bytes);
        let reference = match ref_kind {
            Self::REF_MINT => Ref::Mint(ref_pubkey),
            Self::REF_BANK => Ref::Bank(ref_pubkey),
            _ => return None,
        };

        let supply = u64::from_le_bytes(bytes[48..56].try_into().ok()?);
        let borrow = u64::from_le_bytes(bytes[56..64].try_into().ok()?);
        let price = u64::from_le_bytes(bytes[64..72].try_into().ok()?);
        let supply_apy = u64::from_le_bytes(bytes[72..80].try_into().ok()?);
        let borrow_apy = u64::from_le_bytes(bytes[80..88].try_into().ok()?);
        let native_apy = u64::from_le_bytes(bytes[88..96].try_into().ok()?);
        let created_at = u64::from_le_bytes(bytes[96..104].try_into().ok()?);
        let updated_at = u64::from_le_bytes(bytes[104..112].try_into().ok()?);

        Some(Self {
            snapshot_hour,
            reference,
            supply,
            borrow,
            price,
            supply_apy,
            borrow_apy,
            native_apy,
            created_at,
            updated_at,
        })
    }

    fn to_bytes(&self, out: &mut [u8]) -> Option<()> {
        if out.len() != Self::LEN {
            return None;
        }

        out[0..8].copy_from_slice(&self.snapshot_hour.to_le_bytes());
        let (ref_kind, ref_pubkey) = match self.reference {
            Ref::Mint(pk) => (Self::REF_MINT, pk),
            Ref::Bank(pk) => (Self::REF_BANK, pk),
        };
        out[8] = ref_kind;
        out[9..16].fill(0);
        out[16..48].copy_from_slice(ref_pubkey.as_ref());

        out[48..56].copy_from_slice(&self.supply.to_le_bytes());
        out[56..64].copy_from_slice(&self.borrow.to_le_bytes());
        out[64..72].copy_from_slice(&self.price.to_le_bytes());
        out[72..80].copy_from_slice(&self.supply_apy.to_le_bytes());
        out[80..88].copy_from_slice(&self.borrow_apy.to_le_bytes());
        out[88..96].copy_from_slice(&self.native_apy.to_le_bytes());
        out[96..104].copy_from_slice(&self.created_at.to_le_bytes());
        out[104..112].copy_from_slice(&self.updated_at.to_le_bytes());

        Some(())
    }

    fn timestamp(&self) -> u64 {
        self.snapshot_hour
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HistoryHeader;

    fn new_header(capacity: u32) -> HistoryHeader {
        HistoryHeader {
            version: 1,
            authority: Pubkey::default(),
            _pad0: [0; 3],
            head: 0,
            len: 0,
            capacity,
            _pad1: [0; 8],
            head_ts: 0,
            latest_ts: 0,
            _pad: [0; 5],
        }
    }

    fn new_account_data(capacity: u32) -> Vec<u8> {
        vec![0u8; HistoryHeader::PAYLOAD_OFFSET + (capacity as usize * MonitorSnapshotRecord::LEN)]
    }

    fn rec(ts: u64, value_seed: u64, reference: Ref) -> MonitorSnapshotRecord {
        MonitorSnapshotRecord {
            snapshot_hour: ts,
            reference,
            supply: value_seed,
            borrow: value_seed + 1,
            price: value_seed + 2,
            supply_apy: value_seed + 3,
            borrow_apy: value_seed + 4,
            native_apy: value_seed + 5,
            created_at: ts + 10,
            updated_at: ts + 20,
        }
    }

    fn pk(seed: u8) -> Pubkey {
        Pubkey::new([seed; 32])
    }

    #[test]
    fn parse_round_trip_mint_and_bank() {
        let mint = rec(1_700_000_000, 100, Ref::Mint(pk(1)));
        let bank = rec(1_700_000_001, 200, Ref::Bank(pk(2)));

        let mut out = [0u8; MonitorSnapshotRecord::LEN];
        assert_eq!(mint.to_bytes(&mut out), Some(()));
        assert_eq!(MonitorSnapshotRecord::parse(&out), Some(mint));

        assert_eq!(bank.to_bytes(&mut out), Some(()));
        assert_eq!(MonitorSnapshotRecord::parse(&out), Some(bank));
    }

    #[test]
    fn append_and_get_latest() {
        let mut header = new_header(4);
        let mut data = new_account_data(4);

        let r1 = rec(1000, 10, Ref::Mint(pk(3)));
        let r2 = rec(1001, 20, Ref::Bank(pk(4)));

        assert_eq!(header.append(&mut data, &r1), Some(()));
        assert_eq!(header.append(&mut data, &r2), Some(()));

        assert_eq!(header.get_latest::<MonitorSnapshotRecord>(&data), Some(r2));
        assert_eq!(header.get::<MonitorSnapshotRecord>(&data, 1000), Some(r1));
        assert_eq!(header.get::<MonitorSnapshotRecord>(&data, 1001), Some(r2));
    }

    #[test]
    fn upsert_latest_on_equal_timestamp() {
        let mut header = new_header(4);
        let mut data = new_account_data(4);

        let r1 = rec(2000, 30, Ref::Mint(pk(5)));
        let mut r2 = r1;
        r2.price = 999;
        r2.updated_at = 999_999;

        assert_eq!(header.append(&mut data, &r1), Some(()));
        assert_eq!(header.append(&mut data, &r2), Some(()));

        assert_eq!(header.len, 1);
        assert_eq!(header.get_latest::<MonitorSnapshotRecord>(&data), Some(r2));
        assert_eq!(header.get::<MonitorSnapshotRecord>(&data, 2000), Some(r2));
    }

    #[test]
    fn stale_timestamp_is_ignored() {
        let mut header = new_header(4);
        let mut data = new_account_data(4);

        let newer = rec(3001, 40, Ref::Bank(pk(6)));
        let older = rec(3000, 50, Ref::Bank(pk(7)));

        assert_eq!(header.append(&mut data, &newer), Some(()));
        assert_eq!(header.append(&mut data, &older), Some(()));

        assert_eq!(header.len, 1);
        assert_eq!(
            header.get_latest::<MonitorSnapshotRecord>(&data),
            Some(newer)
        );
        assert_eq!(header.get::<MonitorSnapshotRecord>(&data, 3000), None);
    }

    #[test]
    fn overwrite_when_capacity_full_updates_head_and_window() {
        let mut header = new_header(3);
        let mut data = new_account_data(3);

        let r1 = rec(4001, 1, Ref::Mint(pk(8)));
        let r2 = rec(4002, 2, Ref::Mint(pk(9)));
        let r3 = rec(4003, 3, Ref::Mint(pk(10)));
        let r4 = rec(4004, 4, Ref::Mint(pk(11)));

        assert_eq!(header.append(&mut data, &r1), Some(()));
        assert_eq!(header.append(&mut data, &r2), Some(()));
        assert_eq!(header.append(&mut data, &r3), Some(()));
        assert_eq!(header.append(&mut data, &r4), Some(()));

        assert_eq!(header.len, 3);
        assert_eq!(header.head_ts, 4002);
        assert_eq!(header.latest_ts, 4004);
        assert_eq!(header.get::<MonitorSnapshotRecord>(&data, 4001), None);
        assert_eq!(header.get::<MonitorSnapshotRecord>(&data, 4002), Some(r2));
        assert_eq!(header.get::<MonitorSnapshotRecord>(&data, 4004), Some(r4));
        assert_eq!(header.get_latest::<MonitorSnapshotRecord>(&data), Some(r4));
    }
}
