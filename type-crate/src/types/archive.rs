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
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)
)]
pub struct ArchiveHeader {
    pub version: u8,
    pub _pad0: [u8; 7],
    pub record_count: u64,
    pub authority: Pubkey,
}

pub trait ArchiveRecord: Sized {
    fn len(version: u8) -> Option<usize>;
    fn version(&self) -> u8;
    fn parse(bytes: &[u8]) -> Option<Self>;
    fn to_bytes(&self, out: &mut [u8]) -> Option<()>;
    fn index(&self) -> [u8; 32];
}

impl ArchiveHeader {
    pub const LEN: usize = 48;
    pub const DISCRIMINATOR_LEN: usize = 8;
    pub const PAYLOAD_OFFSET: usize = Self::DISCRIMINATOR_LEN + Self::LEN;
    pub const HEADER_VERSION_OFFSET: usize = 8;
    pub const HEADER_RECORD_COUNT_OFFSET: usize = 16;
    pub const HEADER_AUTHORITY_OFFSET: usize = 24;
    pub const INDEX_LEN: usize = 32;
    pub const VERSION_LEN: usize = 1;
    pub const SLOT_META_LEN: usize = Self::INDEX_LEN + Self::VERSION_LEN;

    /// Read archive header fields directly from account bytes.
    ///
    /// Layout is `[8-byte discriminator][ArchiveHeader][payload]`.
    pub fn read_from_account_data(account_data: &[u8]) -> Option<Self> {
        if account_data.len() < Self::PAYLOAD_OFFSET {
            return None;
        }
        let version = *account_data.get(Self::HEADER_VERSION_OFFSET)?;
        let record_count = u64::from_le_bytes(
            account_data
                .get(Self::HEADER_RECORD_COUNT_OFFSET..Self::HEADER_RECORD_COUNT_OFFSET + 8)?
                .try_into()
                .ok()?,
        );
        let authority_bytes: [u8; 32] = account_data
            .get(Self::HEADER_AUTHORITY_OFFSET..Self::HEADER_AUTHORITY_OFFSET + 32)?
            .try_into()
            .ok()?;

        #[cfg(feature = "anchor")]
        let authority = Pubkey::new_from_array(authority_bytes);
        #[cfg(not(feature = "anchor"))]
        let authority = Pubkey::new(authority_bytes);

        Some(Self {
            version,
            _pad0: [0; 7],
            record_count,
            authority,
        })
    }

    /// Persist header fields back into account bytes without touching payload.
    pub fn write_to_account_data(&self, account_data: &mut [u8]) -> Option<()> {
        if account_data.len() < Self::PAYLOAD_OFFSET {
            return None;
        }
        account_data[Self::HEADER_VERSION_OFFSET] = self.version;
        account_data[Self::HEADER_RECORD_COUNT_OFFSET..Self::HEADER_RECORD_COUNT_OFFSET + 8]
            .copy_from_slice(&self.record_count.to_le_bytes());
        account_data[Self::HEADER_AUTHORITY_OFFSET..Self::HEADER_AUTHORITY_OFFSET + 32]
            .copy_from_slice(self.authority.as_ref());
        Some(())
    }

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

    fn used_payload_len<T: ArchiveRecord>(&self, data: &[u8]) -> Option<usize> {
        let max_records = usize::try_from(self.record_count).ok()?;
        let mut offset = 0usize;
        for _ in 0..max_records {
            let meta_end = offset.checked_add(Self::SLOT_META_LEN)?;
            if meta_end > data.len() {
                return None;
            }
            let version = data[offset + Self::INDEX_LEN];
            let slot_len = T::len(version)?;
            if slot_len < Self::SLOT_META_LEN {
                return None;
            }
            offset = offset.checked_add(slot_len)?;
            if offset > data.len() {
                return None;
            }
        }
        Some(offset)
    }

    pub fn find_slot<'a, T: ArchiveRecord>(
        &self,
        data: &'a [u8],
        index: [u8; 32],
    ) -> Option<(usize, &'a [u8])> {
        let max_records = usize::try_from(self.record_count).ok()?;
        let mut offset = 0usize;
        for _ in 0..max_records {
            let meta_end = offset.checked_add(Self::SLOT_META_LEN)?;
            if meta_end > data.len() {
                return None;
            }
            let version = data[offset + Self::INDEX_LEN];
            let slot_len = T::len(version)?;
            let end = offset.checked_add(slot_len)?;
            if end > data.len() {
                return None;
            }

            if data[offset..offset + Self::INDEX_LEN] == index {
                return Some((offset, &data[offset..end]));
            }
            offset = end;
        }
        None
    }

    pub fn find_slot_mut<'a, T: ArchiveRecord>(
        &self,
        data: &'a mut [u8],
        index: [u8; 32],
    ) -> Option<(usize, &'a mut [u8])> {
        let max_records = usize::try_from(self.record_count).ok()?;
        let mut offset = 0usize;
        for _ in 0..max_records {
            let meta_end = offset.checked_add(Self::SLOT_META_LEN)?;
            if meta_end > data.len() {
                return None;
            }
            let version = data[offset + Self::INDEX_LEN];
            let slot_len = T::len(version)?;
            let end = offset.checked_add(slot_len)?;
            if end > data.len() {
                return None;
            }

            if data[offset..offset + Self::INDEX_LEN] == index {
                return Some((offset, &mut data[offset..end]));
            }
            offset = end;
        }
        None
    }

    pub fn find_slot_in_account_mut<'a, T: ArchiveRecord>(
        &self,
        account_data: &'a mut [u8],
        index: [u8; 32],
    ) -> Option<(usize, &'a mut [u8])> {
        // Slot offsets are relative to payload, so first slice off the
        // discriminator + fixed header region.
        let data = self.payload_region_mut(account_data)?;
        self.find_slot_mut::<T>(data, index)
    }

    pub fn update_or_insert<T: ArchiveRecord>(
        &mut self,
        account_data: &mut [u8],
        record: &T,
    ) -> Option<()> {
        let record_len = T::len(record.version())?;
        if record_len < Self::SLOT_META_LEN {
            return None;
        }

        let data = self.payload_region_mut(account_data)?;
        let index = record.index();
        if let Some((pos, slot)) = self.find_slot_mut::<T>(data, index) {
            let existing_version = slot[Self::INDEX_LEN];
            let existing_len = T::len(existing_version)?;
            if existing_version == record.version() && existing_len == record_len {
                record.to_bytes(slot)?;
                return Some(());
            }

            // Version/len changed for same index: compact out old slot and append
            // new record at tail if capacity permits.
            let used_end = self.used_payload_len::<T>(data)?;
            let remove_end = pos.checked_add(existing_len)?;
            if remove_end > used_end {
                return None;
            }
            let tail_len = used_end.checked_sub(remove_end)?;
            let new_used_end = used_end
                .checked_sub(existing_len)?
                .checked_add(record_len)?;
            if new_used_end > data.len() {
                return None;
            }
            if tail_len > 0 {
                data.copy_within(remove_end..used_end, pos);
            }
            record.to_bytes(&mut data[new_used_end - record_len..new_used_end])?;
            if new_used_end < used_end {
                for b in &mut data[new_used_end..used_end] {
                    *b = 0;
                }
            }
            return Some(());
        }

        let offset = self.used_payload_len::<T>(data)?;
        let end = offset.checked_add(record_len)?;
        if end > data.len() {
            return None;
        }

        record.to_bytes(&mut data[offset..end])?;
        self.record_count = self.record_count.checked_add(1)?;
        Some(())
    }

    pub fn get_record<T: ArchiveRecord>(
        &self,
        account_data: &[u8],
        index: [u8; 32],
    ) -> Option<(usize, T)> {
        let data = self.payload_region(account_data)?;
        let (pos, slot) = self.find_slot::<T>(data, index)?;
        Some((pos, T::parse(slot)?))
    }

    pub fn update<T: ArchiveRecord>(
        &self,
        account_data: &mut [u8],
        position: usize,
        record: &T,
    ) -> Option<()> {
        let record_len = T::len(record.version())?;
        if record_len < Self::SLOT_META_LEN {
            return None;
        }
        let data = self.payload_region_mut(account_data)?;
        let used_end = self.used_payload_len::<T>(data)?;
        let end = position.checked_add(record_len)?;
        if end > used_end {
            return None;
        }
        let slot = &mut data[position..end];
        if slot[0..Self::INDEX_LEN] != record.index() {
            return None;
        }
        if slot[Self::INDEX_LEN] != record.version() {
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
        fn len(version: u8) -> Option<usize> {
            match version {
                1 => Some(41),
                _ => None,
            }
        }

        fn version(&self) -> u8 {
            1
        }

        fn parse(bytes: &[u8]) -> Option<Self> {
            if bytes.len() != Self::len(1)? || bytes[32] != 1 {
                return None;
            }
            let key: [u8; 32] = bytes[0..32].try_into().ok()?;
            let value = u64::from_le_bytes(bytes[33..41].try_into().ok()?);
            Some(Self { key, value })
        }

        fn to_bytes(&self, out: &mut [u8]) -> Option<()> {
            if out.len() != Self::len(self.version())? {
                return None;
            }
            out[0..32].copy_from_slice(&self.key);
            out[32] = self.version();
            out[33..41].copy_from_slice(&self.value.to_le_bytes());
            Some(())
        }

        fn index(&self) -> [u8; 32] {
            self.key
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TestRecordV2 {
        key: [u8; 32],
        value: u64,
        extra: u64,
    }

    impl ArchiveRecord for TestRecordV2 {
        fn len(version: u8) -> Option<usize> {
            match version {
                1 => Some(41),
                2 => Some(49),
                _ => None,
            }
        }

        fn version(&self) -> u8 {
            2
        }

        fn parse(bytes: &[u8]) -> Option<Self> {
            if bytes.len() != Self::len(2)? || bytes[32] != 2 {
                return None;
            }
            let key: [u8; 32] = bytes[0..32].try_into().ok()?;
            let value = u64::from_le_bytes(bytes[33..41].try_into().ok()?);
            let extra = u64::from_le_bytes(bytes[41..49].try_into().ok()?);
            Some(Self { key, value, extra })
        }

        fn to_bytes(&self, out: &mut [u8]) -> Option<()> {
            if out.len() != Self::len(self.version())? {
                return None;
            }
            out[0..32].copy_from_slice(&self.key);
            out[32] = self.version();
            out[33..41].copy_from_slice(&self.value.to_le_bytes());
            out[41..49].copy_from_slice(&self.extra.to_le_bytes());
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
        vec![0u8; ArchiveHeader::PAYLOAD_OFFSET + (slots * TestRecord::len(1).unwrap())]
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

    #[test]
    fn update_existing_index_with_new_len_relocates_to_tail() {
        let mut header = new_header();
        let mut data = vec![0u8; ArchiveHeader::PAYLOAD_OFFSET + 3 * TestRecordV2::len(2).unwrap()];

        let key_a = [7u8; 32];
        let key_b = [8u8; 32];

        let rec_a_v1 = TestRecord {
            key: key_a,
            value: 11,
        };
        let rec_b_v1 = TestRecord {
            key: key_b,
            value: 22,
        };
        assert_eq!(
            header.update_or_insert::<TestRecord>(&mut data, &rec_a_v1),
            Some(())
        );
        assert_eq!(
            header.update_or_insert::<TestRecord>(&mut data, &rec_b_v1),
            Some(())
        );

        let rec_a_v2 = TestRecordV2 {
            key: key_a,
            value: 33,
            extra: 44,
        };
        assert_eq!(
            header.update_or_insert::<TestRecordV2>(&mut data, &rec_a_v2),
            Some(())
        );
        assert_eq!(header.record_count, 2);

        let (pos_b, got_b) = header.get_record::<TestRecord>(&data, key_b).unwrap();
        let (pos_a, got_a) = header.get_record::<TestRecordV2>(&data, key_a).unwrap();
        assert_eq!(got_b, rec_b_v1);
        assert_eq!(got_a, rec_a_v2);
        assert!(pos_b < pos_a);
    }
}
