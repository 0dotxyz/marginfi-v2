use crate::{assert_struct_align, assert_struct_size};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

assert_struct_size!(ArchiveHeader, 48);
assert_struct_align!(ArchiveHeader, 8);
/// Fixed-size header stored at the beginning of an indexed archive account.
///
/// Layout on chain:
/// - `[0..8)` account discriminator
/// - `[8..8+ArchiveHeader::LEN)` header
/// - `[ArchiveHeader::PAYLOAD_OFFSET..)` fixed-size record slots
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)
)]
pub struct ArchiveHeader {
    /// Header schema version.
    pub version: u8,
    /// Explicit alignment padding before `record_count`.
    pub _pad0: [u8; 7],
    /// Number of populated records in payload.
    pub record_count: u64,
    /// Authority allowed to mutate this archive.
    pub authority: Pubkey,
}

/// Trait implemented by each concrete fixed-size archive record.
///
/// Invariant:
/// - First 32 bytes of serialized record are reserved for index key.
/// - `index()` must match `out[0..32]` produced by `to_bytes`.
pub trait ArchiveRecord: Sized {
    /// Exact serialized record size in bytes.
    const LEN: usize;

    /// Parse one record from bytes.
    fn parse(bytes: &[u8]) -> Option<Self>;
    /// Serialize one record into bytes.
    fn to_bytes(&self, out: &mut [u8]) -> Option<()>;
    /// Record index key (always 32 bytes).
    fn index(&self) -> [u8; 32];
}

impl ArchiveHeader {
    /// Header byte length (excluding discriminator).
    pub const LEN: usize = 48;
    /// Anchor account discriminator length.
    pub const DISCRIMINATOR_LEN: usize = 8;
    /// Payload start offset in account data.
    pub const PAYLOAD_OFFSET: usize = Self::DISCRIMINATOR_LEN + Self::LEN;

    fn payload_region<'a>(&self, account_data: &'a [u8]) -> Option<&'a [u8]> {
        if account_data.len() < Self::PAYLOAD_OFFSET {
            return None;
        }
        Some(&account_data[Self::PAYLOAD_OFFSET..])
    }

    fn payload_region_mut<'a>(&self, account_data: &'a mut [u8]) -> Option<&'a mut [u8]> {
        if account_data.len() < Self::PAYLOAD_OFFSET {
            return None;
        }
        Some(&mut account_data[Self::PAYLOAD_OFFSET..])
    }

    /// Find immutable slot bytes and byte position for given record index.
    pub fn find_slot<'a, T: ArchiveRecord>(
        &self,
        data: &'a [u8],
        index: [u8; 32],
    ) -> Option<(usize, &'a [u8])> {
        if T::LEN < 32 {
            return None;
        }
        let max_records = usize::try_from(self.record_count).ok()?;
        for (i, chunk) in data.chunks_exact(T::LEN).take(max_records).enumerate() {
            if chunk[0..32] == index {
                let pos = i.checked_mul(T::LEN)?;
                return Some((pos, chunk));
            }
        }
        None
    }

    /// Find mutable slot bytes and byte position for given record index.
    pub fn find_slot_mut<'a, T: ArchiveRecord>(
        &self,
        data: &'a mut [u8],
        index: [u8; 32],
    ) -> Option<(usize, &'a mut [u8])> {
        if T::LEN < 32 {
            return None;
        }
        let max_records = usize::try_from(self.record_count).ok()?;
        for (i, chunk) in data.chunks_exact_mut(T::LEN).take(max_records).enumerate() {
            if chunk[0..32] == index {
                let pos = i.checked_mul(T::LEN)?;
                return Some((pos, chunk));
            }
        }
        None
    }

    /// Update existing indexed record or append a new one if capacity permits.
    pub fn update_or_insert<T: ArchiveRecord>(
        &mut self,
        account_data: &mut [u8],
        record: &T,
    ) -> Option<()> {
        if T::LEN < 32 {
            return None;
        }

        let data = self.payload_region_mut(account_data)?;
        let index = record.index();

        if let Some((_, slot)) = self.find_slot_mut::<T>(data, index) {
            record.to_bytes(slot)?;
            return Some(());
        }

        let record_count = usize::try_from(self.record_count).ok()?;
        let offset = record_count.checked_mul(T::LEN)?;
        let end = offset.checked_add(T::LEN)?;
        if end > data.len() {
            return None;
        }

        record.to_bytes(&mut data[offset..end])?;
        self.record_count = self.record_count.checked_add(1)?;
        Some(())
    }

    /// Parse indexed record from archive and return `(position, record)`.
    pub fn get_record<T: ArchiveRecord>(
        &self,
        account_data: &[u8],
        index: [u8; 32],
    ) -> Option<(usize, T)> {
        let data = self.payload_region(account_data)?;
        let (pos, slot) = self.find_slot::<T>(data, index)?;
        Some((pos, T::parse(slot)?))
    }

    /// Update existing record at `position`.
    ///
    /// Safety check:
    /// - the index already present at `position` must match `record.index()`.
    pub fn update<T: ArchiveRecord>(
        &self,
        account_data: &mut [u8],
        position: usize,
        record: &T,
    ) -> Option<()> {
        if T::LEN < 32 {
            return None;
        }

        let data = self.payload_region_mut(account_data)?;
        let end = position.checked_add(T::LEN)?;
        if end > data.len() {
            return None;
        }
        let max_records = usize::try_from(self.record_count).ok()?;
        let max_end = max_records.checked_mul(T::LEN)?;
        if end > max_end {
            return None;
        }

        let slot = &mut data[position..end];
        if slot[0..32] != record.index() {
            return None;
        }
        record.to_bytes(slot)
    }
}

const _: () = assert!(core::mem::size_of::<ArchiveHeader>() == ArchiveHeader::LEN);

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TestRecord {
        key: [u8; 32],
        value: u64,
    }

    impl ArchiveRecord for TestRecord {
        const LEN: usize = 40;

        fn parse(bytes: &[u8]) -> Option<Self> {
            if bytes.len() != Self::LEN {
                return None;
            }
            let key: [u8; 32] = bytes[0..32].try_into().ok()?;
            let value = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
            Some(Self { key, value })
        }

        fn to_bytes(&self, out: &mut [u8]) -> Option<()> {
            if out.len() != Self::LEN {
                return None;
            }
            out[0..32].copy_from_slice(&self.key);
            out[32..40].copy_from_slice(&self.value.to_le_bytes());
            Some(())
        }

        fn index(&self) -> [u8; 32] {
            self.key
        }
    }

    fn new_header() -> ArchiveHeader {
        ArchiveHeader {
            version: 1,
            _pad0: [0; 7],
            record_count: 0,
            authority: Pubkey::default(),
        }
    }

    fn new_account_data(slots: usize) -> Vec<u8> {
        vec![0u8; ArchiveHeader::PAYLOAD_OFFSET + (slots * TestRecord::LEN)]
    }

    #[test]
    fn insert_then_get() {
        let mut header = new_header();
        let mut data = new_account_data(4);
        let rec = TestRecord {
            key: [7; 32],
            value: 99,
        };

        assert_eq!(
            header.update_or_insert::<TestRecord>(&mut data, &rec),
            Some(())
        );
        assert_eq!(header.record_count, 1);
        assert_eq!(
            header.get_record::<TestRecord>(&data, [7; 32]),
            Some((0, rec))
        );
    }

    #[test]
    fn update_existing_index() {
        let mut header = new_header();
        let mut data = new_account_data(4);
        let old = TestRecord {
            key: [9; 32],
            value: 1,
        };
        let new = TestRecord {
            key: [9; 32],
            value: 2,
        };

        assert_eq!(
            header.update_or_insert::<TestRecord>(&mut data, &old),
            Some(())
        );
        assert_eq!(
            header.update_or_insert::<TestRecord>(&mut data, &new),
            Some(())
        );
        assert_eq!(header.record_count, 1);
        assert_eq!(
            header.get_record::<TestRecord>(&data, [9; 32]),
            Some((0, new))
        );
    }

    #[test]
    fn insert_fails_when_full() {
        let mut header = new_header();
        let mut data = new_account_data(1);
        let a = TestRecord {
            key: [1; 32],
            value: 10,
        };
        let b = TestRecord {
            key: [2; 32],
            value: 20,
        };

        assert_eq!(
            header.update_or_insert::<TestRecord>(&mut data, &a),
            Some(())
        );
        assert_eq!(header.update_or_insert::<TestRecord>(&mut data, &b), None);
        assert_eq!(header.record_count, 1);
    }

    #[test]
    fn get_record_and_update_with_index_guard() {
        let mut header = new_header();
        let mut data = new_account_data(2);

        let a = TestRecord {
            key: [3; 32],
            value: 10,
        };
        let a_updated = TestRecord {
            key: [3; 32],
            value: 42,
        };
        let wrong_key = TestRecord {
            key: [4; 32],
            value: 99,
        };

        assert_eq!(
            header.update_or_insert::<TestRecord>(&mut data, &a),
            Some(())
        );
        let (pos, parsed) = header.get_record::<TestRecord>(&data, [3; 32]).unwrap();
        assert_eq!(pos, 0);
        assert_eq!(parsed, a);

        assert_eq!(
            header.update::<TestRecord>(&mut data, pos, &wrong_key),
            None
        );
        assert_eq!(
            header.update::<TestRecord>(&mut data, pos, &a_updated),
            Some(())
        );
        assert_eq!(
            header.get_record::<TestRecord>(&data, [3; 32]),
            Some((0, a_updated))
        );
    }
}
