use crate::{assert_struct_align, assert_struct_size};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

assert_struct_size!(HistoryHeader, 112);
assert_struct_align!(HistoryHeader, 8);
/// Fixed-size header stored at the beginning of a history archive account.
///
/// Layout on chain:
/// - `[0..8)` account discriminator (handled by Anchor/account allocation path)
/// - `[8..8+HistoryHeader::LEN)` this header
/// - `[HistoryHeader::PAYLOAD_OFFSET..)` ring-buffer payload area containing fixed-size records
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)
)]
pub struct HistoryHeader {
    /// Header schema version. Useful for forward-compatible upgrades.
    pub version: u8,
    /// Authority allowed to manage writes to this archive account.
    pub authority: Pubkey,
    /// Explicit padding so the header has deterministic byte layout.
    pub _pad0: [u8; 3],
    /// Physical ring index of the oldest stored record.
    pub head: u32,
    /// Number of currently stored records (always `<= capacity`).
    pub len: u32,
    /// Total number of fixed-size records that fit in the payload area.
    pub capacity: u32,
    /// Explicit alignment padding before 64-bit timestamps.
    pub _pad1: [u8; 8],
    /// Timestamp of the record currently stored at `head` (oldest retained).
    pub head_ts: u64,
    /// Timestamp of the newest retained record.
    pub latest_ts: u64,
    /// Reserved/padding words for future header fields without changing layout.
    pub _pad: [u64; 5],
}

/// Trait implemented by each concrete history record type.
///
/// The archive stores records as fixed-size slots, so every record type must
/// define an exact serialized size and provide conversion both directions.
pub trait HistoryRecord: Sized {
    /// Exact serialized record size in bytes.
    const LEN: usize;

    /// Parse one serialized record from bytes.
    ///
    /// `bytes` must contain exactly one encoded record for this type.
    fn parse(bytes: &[u8]) -> Option<Self>;
    /// Serialize this record into `out`.
    ///
    /// `out` must be exactly `Self::LEN` bytes.
    fn to_bytes(&self, out: &mut [u8]) -> Option<()>;
    /// Timestamp key used for lookup via `HistoryHeader::get`.
    fn timestamp(&self) -> u64;
}

impl HistoryHeader {
    /// Byte size of the in-account `HistoryHeader` region (excluding discriminator).
    pub const LEN: usize = 112;
    /// Anchor account discriminator size in bytes.
    pub const DISCRIMINATOR_LEN: usize = 8;
    /// Absolute byte offset where payload records begin in account data.
    pub const PAYLOAD_OFFSET: usize = Self::DISCRIMINATOR_LEN + Self::LEN;

    /// Return immutable view of payload bytes (everything after header).
    ///
    /// # Parameters
    /// - `account_data`: entire account data buffer including discriminator and header.
    ///
    /// # Errors
    /// Returns `AccountTooSmall` if the buffer cannot contain the payload start.
    fn payload_region<'a>(&self, account_data: &'a [u8]) -> Option<&'a [u8]> {
        if account_data.len() < Self::PAYLOAD_OFFSET {
            return None;
        }
        Some(&account_data[Self::PAYLOAD_OFFSET..])
    }

    /// Return mutable view of payload bytes (everything after header).
    ///
    /// # Parameters
    /// - `account_data`: entire mutable account data buffer including discriminator and header.
    ///
    /// # Errors
    /// Returns `AccountTooSmall` if the buffer cannot contain the payload start.
    fn payload_region_mut<'a>(&self, account_data: &'a mut [u8]) -> Option<&'a mut [u8]> {
        if account_data.len() < Self::PAYLOAD_OFFSET {
            return None;
        }
        Some(&mut account_data[Self::PAYLOAD_OFFSET..])
    }

    /// Resolve the physical index of the newest record currently stored.
    ///
    /// Returns `None` when archive is empty or invalid (`capacity == 0`).
    fn latest_physical_index(&self) -> Option<u32> {
        if self.len == 0 || self.capacity == 0 || self.len > self.capacity {
            return None;
        }
        Some((self.head + self.len - 1) % self.capacity)
    }

    /// Resolve a logical index (`0` oldest, `len - 1` newest) into a physical slot.
    fn physical_index(&self, logical_index: u32) -> Option<u32> {
        if self.capacity == 0 || self.len > self.capacity || logical_index >= self.len {
            return None;
        }
        Some((self.head + logical_index) % self.capacity)
    }

    /// Parse record `T` from a physical slot in the payload ring.
    ///
    /// # Parameters
    /// - `payload`: payload slice (`[PAYLOAD_OFFSET..]`) only.
    /// - `physical_index`: ring slot index inside payload.
    fn parse_physical<T: HistoryRecord>(&self, payload: &[u8], physical_index: u32) -> Option<T> {
        let idx = usize::try_from(physical_index).ok()?;
        let start = idx.checked_mul(T::LEN)?;
        let end = start.checked_add(T::LEN)?;
        if end > payload.len() {
            return None;
        }
        T::parse(&payload[start..end])
    }

    /// Serialize and write a record into the newest physical slot.
    ///
    /// Caller must ensure the archive is non-empty and `payload` points to the payload region.
    fn write_latest_slot<T: HistoryRecord>(&self, payload: &mut [u8], record: &T) -> Option<()> {
        let latest_physical = self.latest_physical_index()? as usize;

        let start = latest_physical.checked_mul(T::LEN)?;
        let end = start.checked_add(T::LEN)?;

        if end > payload.len() {
            return None;
        }

        record.to_bytes(&mut payload[start..end])
    }

    /// Append one typed record into the archive ring buffer.
    ///
    /// Behavior:
    /// - If `record.timestamp() < latest_ts`, write is ignored (non-monotonic historical update).
    /// - If `record.timestamp() == latest_ts`, newest record is replaced (latest-only upsert).
    /// - If `record.timestamp() > latest_ts`, record is appended as newest.
    /// - If full, oldest slot is overwritten and `head` advances by one.
    ///
    /// # Parameters
    /// - `account_data`: full mutable account data (discriminator + header + payload).
    /// - `record`: record value to serialize and write.
    ///
    pub fn append<T: HistoryRecord>(&mut self, account_data: &mut [u8], record: &T) -> Option<()> {
        if T::LEN == 0 {
            return None;
        }
        if self.capacity == 0 || self.len > self.capacity || (self.len == 0 && self.head > 0) {
            return None;
        }

        let payload = self.payload_region_mut(account_data)?;
        let expected_capacity = payload.len() / T::LEN;
        let expected_capacity_u32 = u32::try_from(expected_capacity).ok()?;
        if expected_capacity_u32 != self.capacity {
            return None;
        }

        let ts = record.timestamp();
        if ts < self.latest_ts {
            // Non-monotonic timestamp update; ignore write to preserve historical integrity.
            return Some(());
        }

        let len = self.len;
        let is_empty = len == 0;

        // Check if upsert possible, avoid writing duplicate timestamp if record is not newer than current latest.
        // upsert latest record when timestamps are equal, keep head/len the same since we're not adding a new record
        if !is_empty && ts == self.latest_ts {
            self.write_latest_slot(payload, record)?;
            return Some(());
        }

        let write_idx = if len < self.capacity {
            (self.head + len) % self.capacity
        } else {
            self.head
        } as usize;

        let start = write_idx.checked_mul(T::LEN)?;
        let end = start.checked_add(T::LEN)?;
        if end > payload.len() {
            return None;
        }

        record.to_bytes(&mut payload[start..end])?;

        self.latest_ts = ts;

        if is_empty {
            self.head_ts = ts;
            self.len = 1;
        } else if len < self.capacity {
            self.len = std::cmp::min(len + 1, self.capacity);
        } else {
            self.head = (self.head + 1) % self.capacity;
            let head_record = self.parse_physical::<T>(payload, self.head)?;
            self.head_ts = head_record.timestamp();
        }

        Some(())
    }

    /// Fetch the newest record currently stored in the archive.
    ///
    /// # Parameters
    /// - `account_data`: full account data (discriminator + header + payload).
    ///
    /// # Returns
    /// - `Some(T)` if a newest record exists and parses successfully.
    /// - `None` if archive is empty, out-of-bounds, or parsing fails.
    pub fn get_latest<T: HistoryRecord>(&self, account_data: &[u8]) -> Option<T> {
        let payload = self.payload_region(account_data)?;
        let idx = self.latest_physical_index()?;
        self.parse_physical::<T>(payload, idx)
    }

    /// Fetch the newest record whose `timestamp()` equals `timestamp`.
    ///
    /// Search order is newest -> oldest so duplicate timestamps return the most recent match.
    ///
    /// # Parameters
    /// - `account_data`: full account data (discriminator + header + payload).
    /// - `timestamp`: exact timestamp to match against `T::timestamp()`.
    ///
    /// # Returns
    /// - `Some(T)` when a matching record is found and parsed.
    /// - `None` when no match exists or if any bound/parse check fails.
    pub fn get<T: HistoryRecord>(&self, account_data: &[u8], timestamp: u64) -> Option<T> {
        if self.capacity == 0 || self.len == 0 || self.len > self.capacity {
            return None;
        }
        if timestamp < self.head_ts || timestamp > self.latest_ts {
            return None;
        }

        let payload = self.payload_region(account_data)?;
        for i in (0..self.len).rev() {
            let physical = self.physical_index(i)?;
            let rec = self.parse_physical::<T>(payload, physical)?;
            if rec.timestamp() == timestamp {
                return Some(rec);
            }
        }
        None
    }
}

const _: () = assert!(core::mem::size_of::<HistoryHeader>() == HistoryHeader::LEN);

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TestRecord {
        ts: u64,
        value: u64,
    }

    impl HistoryRecord for TestRecord {
        const LEN: usize = 16;

        fn parse(bytes: &[u8]) -> Option<Self> {
            if bytes.len() != Self::LEN {
                return None;
            }
            let ts = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
            let value = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
            Some(Self { ts, value })
        }

        fn to_bytes(&self, out: &mut [u8]) -> Option<()> {
            if out.len() != Self::LEN {
                return None;
            }
            out[0..8].copy_from_slice(&self.ts.to_le_bytes());
            out[8..16].copy_from_slice(&self.value.to_le_bytes());
            Some(())
        }

        fn timestamp(&self) -> u64 {
            self.ts
        }
    }

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
        vec![0u8; HistoryHeader::PAYLOAD_OFFSET + (capacity as usize * TestRecord::LEN)]
    }

    #[test]
    fn append_and_get_latest() {
        let mut header = new_header(4);
        let mut data = new_account_data(4);

        let r1 = TestRecord { ts: 10, value: 111 };
        let r2 = TestRecord { ts: 11, value: 222 };

        assert_eq!(header.append(&mut data, &r1), Some(()));
        assert_eq!(header.append(&mut data, &r2), Some(()));

        assert_eq!(header.get_latest::<TestRecord>(&data), Some(r2));
    }

    #[test]
    fn upsert_latest_when_timestamp_equal() {
        let mut header = new_header(4);
        let mut data = new_account_data(4);

        let old = TestRecord { ts: 42, value: 1 };
        let new = TestRecord { ts: 42, value: 999 };

        assert_eq!(header.append(&mut data, &old), Some(()));
        assert_eq!(header.append(&mut data, &new), Some(()));

        assert_eq!(header.len, 1);
        assert_eq!(header.get_latest::<TestRecord>(&data), Some(new));
        assert_eq!(header.get::<TestRecord>(&data, 42), Some(new));
    }

    #[test]
    fn ignore_older_timestamp_than_latest() {
        let mut header = new_header(4);
        let mut data = new_account_data(4);

        let newer = TestRecord { ts: 100, value: 7 };
        let older = TestRecord { ts: 99, value: 9 };

        assert_eq!(header.append(&mut data, &newer), Some(()));
        assert_eq!(header.append(&mut data, &older), Some(()));

        assert_eq!(header.len, 1);
        assert_eq!(header.get_latest::<TestRecord>(&data), Some(newer));
        assert_eq!(header.get::<TestRecord>(&data, 99), None);
    }

    #[test]
    fn overwrite_oldest_when_full() {
        let mut header = new_header(3);
        let mut data = new_account_data(3);

        let r1 = TestRecord { ts: 1, value: 10 };
        let r2 = TestRecord { ts: 2, value: 20 };
        let r3 = TestRecord { ts: 3, value: 30 };
        let r4 = TestRecord { ts: 4, value: 40 };

        assert_eq!(header.append(&mut data, &r1), Some(()));
        assert_eq!(header.append(&mut data, &r2), Some(()));
        assert_eq!(header.append(&mut data, &r3), Some(()));
        assert_eq!(header.append(&mut data, &r4), Some(()));

        assert_eq!(header.len, 3);
        assert_eq!(header.head_ts, 2);
        assert_eq!(header.latest_ts, 4);
        assert_eq!(header.get::<TestRecord>(&data, 1), None);
        assert_eq!(header.get::<TestRecord>(&data, 2), Some(r2));
        assert_eq!(header.get::<TestRecord>(&data, 4), Some(r4));
    }
}
