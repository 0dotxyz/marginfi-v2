#[cfg(feature = "anchor")]
use std::cell::{Ref, RefMut};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

pub trait ArchiveRecord: Sized {
    const TYPE_DISCRIMINATOR: [u8; 8];
    fn len(version: u8) -> Option<usize>;
    fn version(&self) -> u8;
    fn self_len(&self) -> Option<usize> {
        Self::len(self.version())
    }
    fn parse(bytes: &[u8]) -> Option<Self>;
    fn to_bytes(&self, out: &mut [u8]) -> Option<()>;
    fn index(&self) -> [u8; 32];
}
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)
)]
pub struct ArchiveMeta {
    pub version: u8,
    pub _pad0: [u8; 7],
    pub record_count: u64,
    pub authority: Pubkey,
}

impl ArchiveMeta {
    pub const LEN: usize = 48;

    pub fn is_initialized(account_data: &[u8]) -> Option<bool> {
        if account_data.len() < Self::LEN {
            return Some(false);
        }
        let authority_bytes: [u8; 32] = account_data.get(16..48)?.try_into().ok()?;
        Some(authority_bytes != [0; 32])
    }

    pub fn read(account_data: &[u8]) -> Option<Self> {
        if account_data.len() < Self::LEN {
            return None;
        }
        let version = *account_data.get(0)?;
        let record_count = u64::from_le_bytes(account_data.get(8..16)?.try_into().ok()?);
        let authority_bytes: [u8; 32] = account_data.get(16..48)?.try_into().ok()?;

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
    pub fn write(&self, account_data: &mut [u8]) -> Option<()> {
        if account_data.len() < Self::LEN {
            return None;
        }
        account_data[0] = self.version;
        account_data[8..16].copy_from_slice(&self.record_count.to_le_bytes());
        account_data[16..48].copy_from_slice(self.authority.as_ref());
        Some(())
    }
}

#[cfg(feature = "anchor")]
pub struct Archive<'a, 'info, const INDEX_MAP_LEN: usize, T: ArchiveRecord>(
    ArchiveMeta,
    &'a AccountInfo<'info>,
    core::marker::PhantomData<T>,
);
#[cfg(feature = "anchor")]
impl<'a, 'info, const INDEX_MAP_LEN: usize, T: ArchiveRecord> Archive<'a, 'info, INDEX_MAP_LEN, T>
where
    'info: 'a,
{
    pub const DISCRIMINATOR_BYTES: usize = 8;
    pub const INDEX_MAP_BYTES: usize = INDEX_MAP_LEN * 64;

    pub fn initialize(account_info: &'a AccountInfo<'info>, authority: Pubkey) -> Option<Self> {
        let mut data = account_info.try_borrow_mut_data().ok()?;

        let discriminator = &data[0..Self::DISCRIMINATOR_BYTES];
        if discriminator != [0; 8] {
            return None;
        }

        if data.len() < Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN + Self::INDEX_MAP_BYTES {
            return None;
        }

        if ArchiveMeta::is_initialized(&data[Self::DISCRIMINATOR_BYTES..Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN])? {
            return None;
        }

        data[0..Self::DISCRIMINATOR_BYTES].copy_from_slice(&T::TYPE_DISCRIMINATOR);
        let meta = ArchiveMeta {
            version: 1,
            _pad0: [0; 7],
            record_count: 0,
            authority,
        };
        meta.write(&mut data[Self::DISCRIMINATOR_BYTES..Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN])?;
        Some(Self(meta, account_info, core::marker::PhantomData))
    }

    pub fn from_account_info(account_info: &'a AccountInfo<'info>) -> Option<Self> {
        if account_info.data_len()
            < Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN + Self::INDEX_MAP_BYTES
        {
            return None;
        }

        let data = match account_info.try_borrow_data() {
            Ok(data) => data,
            Err(_) => return None,
        };

        let discriminator = &data[0..Self::DISCRIMINATOR_BYTES];
        if discriminator != T::TYPE_DISCRIMINATOR {
            return None;
        }

        let meta = ArchiveMeta::read(&data[Self::DISCRIMINATOR_BYTES..Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN])?;
        
        Some(Self(meta, account_info, core::marker::PhantomData))
    }

    pub fn meta(&self) -> &ArchiveMeta {
        &self.0
    }

    pub fn persist_meta(&self) -> Option<()> {
        let mut data = match self.1.try_borrow_mut_data() {
            Ok(data) => data,
            Err(_) => return None,
        };
        self.0.write(&mut data[Self::DISCRIMINATOR_BYTES..Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN])
    }

    pub fn index_map_bytes(&self) -> Option<Ref<'_, [u8]>> {
        let data = match self.1.try_borrow_data() {
            Ok(data) => data,
            Err(_) => return None,
        };

        let offset = Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN;
        if data.len() < offset + Self::INDEX_MAP_BYTES {
            return None;
        }

        Some(Ref::map(data, |bytes| {
            &bytes[offset..offset + Self::INDEX_MAP_BYTES]
        }))
    }

    fn index_map_mut_bytes(&self) -> Option<RefMut<'_, [u8]>> {
        let data = match self.1.try_borrow_mut_data() {
            Ok(data) => data,
            Err(_) => return None,
        };

        let offset = Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN;
        if data.len() < offset + Self::INDEX_MAP_BYTES {
            return None;
        }

        Some(RefMut::map(data, |bytes| {
            &mut bytes[offset..offset + Self::INDEX_MAP_BYTES]
        }))
    }

    pub fn data_bytes(&self) -> Option<Ref<'_, [u8]>> {
        let data = match self.1.try_borrow_data() {
            Ok(data) => data,
            Err(_) => return None,
        };

        let offset = Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN + Self::INDEX_MAP_BYTES;
        if data.len() < offset {
            return None;
        }

        Some(Ref::map(data, |bytes| &bytes[offset..]))
    }

    fn data_mut_bytes(&self) -> Option<RefMut<'_, [u8]>> {
        let data = match self.1.try_borrow_mut_data() {
            Ok(data) => data,
            Err(_) => return None,
        };

        let offset = Self::DISCRIMINATOR_BYTES + ArchiveMeta::LEN + Self::INDEX_MAP_BYTES;
        if data.len() < offset {
            return None;
        }

        Some(RefMut::map(data, |bytes| &mut bytes[offset..]))
    }

    pub fn find_index_with(&self, secondary: [u8; 32]) -> Option<[u8; 32]> {
        if secondary == [0; 32] {
            return None;
        }

        let index_map = self.index_map_bytes()?;

        for slot in index_map.chunks_exact(64) {
            let sec: [u8; 32] = slot[0..32].try_into().ok()?;
            if sec == secondary {
                return Some(slot[32..64].try_into().ok()?);
            }
        }

        None
    }

    pub fn upsert_index(&self, secondary: [u8; 32], primary: [u8; 32]) -> Option<()> {
        let mut index_map = self.index_map_mut_bytes()?;

        if secondary == [0; 32] || primary == [0; 32] {
            return None;
        }

        let mut next_unused_space: Option<usize> = None;
        for (i, slot) in index_map.chunks_exact_mut(64).enumerate() {
            let sec: [u8; 32] = slot[0..32].try_into().ok()?;
            if sec == secondary {
                slot[32..64].copy_from_slice(&primary);
                return Some(());
            }
            if next_unused_space.is_none() && sec == [0u8; 32] {
                next_unused_space = Some(i * 64);
            }
        }

        let offset = next_unused_space?;
        index_map[offset..offset + 32].copy_from_slice(&secondary);
        index_map[offset + 32..offset + 64].copy_from_slice(&primary);
        Some(())
    }

    fn position(&self, index: [u8; 32]) -> Option<usize> {
        let data = self.data_bytes()?;

        let mut position = 0usize;
        while position < data.len() {
            let slot_index = data.get(position..position + 32)?;
            if slot_index == index {
                return Some(position);
            }

            let version = *data.get(position + 32)?;
            let slot_len = T::len(version)?;
            if slot_len < 33 {
                return None;
            }
            position = position.checked_add(slot_len)?;
        }
        None
    }

    fn next_empty_position(&self) -> Option<usize> {
        let data = self.data_bytes()?;

        let mut position = 0usize;
        while position < data.len() {
            let slot_index = data.get(position..position + 32)?;
            if slot_index == [0; 32] {
                return Some(position);
            }

            let version = *data.get(position + 32)?;
            let slot_len = T::len(version)?;
            if slot_len < 33 {
                return None;
            }
            position = position.checked_add(slot_len)?;
        }
        None
    }

    fn append(&self, record: &T) -> Option<usize> {
        if record.self_len()? < 33 {
            return None;
        }

        if let Some(position) = self.next_empty_position() {
            let mut data_bytes = self.data_mut_bytes()?;

            let record_len = T::len(record.version())?;

            let end = position.checked_add(record_len)?;
            if end > data_bytes.len() {
                return None;
            }

            record.to_bytes(&mut data_bytes[position..end])?;

            return Some(position);
        }
        None
    }

    fn update(&self, position: usize, record: &T) -> Option<()> {
        let mut data_bytes = self.data_mut_bytes()?;
        let version = *data_bytes.get(position + 32)?;

        if version != record.version() {
            // remove this record and upgrade to new version and append to end
            return None;
        }

        let len = T::len(version)?;
        let end = position.checked_add(len)?;

        if end > data_bytes.len() {
            return None;
        }

        record.to_bytes(&mut data_bytes[position..end])?;
        Some(())
    }

    pub fn upsert(&mut self, record: &T) -> Option<usize> {
        if let Some(position) = self.position(record.index()) {
            if self.update(position, record).is_some() {
                Some(position)
            } else {
                // [TODO]: remove this version
                self.append(record)
            }
        } else {
            let position = self.append(record);
            if position.is_some() {
                self.0.record_count = self.0.record_count.checked_add(1)?;
                self.persist_meta()?;
            }
            position
        }
    }

    pub fn get(&self, index: [u8; 32]) -> Option<(usize, T)> {
        let data = self.data_bytes()?;
        let position = self.position(index)?;
        let len = T::len(*data.get(position + 32)?)?;
        let end = position.checked_add(len)?;
        if end > data.len() {
            return None;
        }
        T::parse(data.get(position..end)?).map(|x| (position, x))
    }
}

#[cfg(all(test, feature = "anchor"))]
mod tests {
    use super::*;
    use anchor_lang::solana_program::{account_info::AccountInfo, clock::Epoch};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct TestRecord {
        key: [u8; 32],
        value: u64,
    }

    impl ArchiveRecord for TestRecord {
        const TYPE_DISCRIMINATOR: [u8; 8] = *b"TESTREC1";

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
            if bytes.len() != Self::len(1)? || bytes.get(32).copied()? != 1 {
                return None;
            }
            let key: [u8; 32] = bytes.get(0..32)?.try_into().ok()?;
            let value = u64::from_le_bytes(bytes.get(33..41)?.try_into().ok()?);
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

    fn pk(seed: u8) -> Pubkey {
        Pubkey::new_from_array([seed; 32])
    }

    fn new_archive_account<const INDEX_MAP_LEN: usize>(payload_len: usize) -> AccountInfo<'static> {
        let key = Box::leak(Box::new(pk(200)));
        let owner = Box::leak(Box::new(crate::ID));
        let lamports = Box::leak(Box::new(1_000_000u64));
        let total_len = 8 + ArchiveMeta::LEN + (INDEX_MAP_LEN * 64) + payload_len;
        let data = Box::leak(vec![0u8; total_len].into_boxed_slice());

        let meta = ArchiveMeta {
            version: 1,
            _pad0: [0; 7],
            record_count: 0,
            authority: pk(7),
        };
        meta.write(&mut data[8..56]).unwrap();

        AccountInfo::new(
            key,
            false,
            true,
            lamports,
            data,
            owner,
            false,
            Epoch::default(),
        )
    }

    #[test]
    fn upsert_get_and_update_record() {
        let account = new_archive_account::<2>(TestRecord::len(1).unwrap() * 4);
        let mut archive = Archive::<2, TestRecord>::from_account_info(&account).unwrap();

        let rec = TestRecord {
            key: [1; 32],
            value: 10,
        };
        let pos0 = archive.upsert(&rec).unwrap();
        assert_eq!(pos0, 0);
        assert_eq!(archive.meta().record_count, 1);
        let (_, got) = archive.get(rec.key).unwrap();
        assert_eq!(got, rec);

        let updated = TestRecord {
            key: rec.key,
            value: 42,
        };
        let pos1 = archive.upsert(&updated).unwrap();
        assert_eq!(pos1, 0);
        assert_eq!(archive.meta().record_count, 1);
        let (_, got2) = archive.get(rec.key).unwrap();
        assert_eq!(got2, updated);
    }

    #[test]
    fn append_fails_when_payload_full() {
        let account = new_archive_account::<0>(TestRecord::len(1).unwrap());
        let mut archive = Archive::<0, TestRecord>::from_account_info(&account).unwrap();

        let a = TestRecord {
            key: [2; 32],
            value: 1,
        };
        let b = TestRecord {
            key: [3; 32],
            value: 2,
        };
        assert_eq!(archive.upsert(&a), Some(0));
        assert_eq!(archive.upsert(&b), None);
        assert_eq!(archive.meta().record_count, 1);
    }

    #[test]
    fn index_map_upsert_and_find() {
        let account = new_archive_account::<2>(0);
        let archive = Archive::<2, TestRecord>::from_account_info(&account).unwrap();

        let secondary = [9u8; 32];
        let primary = [8u8; 32];
        assert_eq!(archive.find_index_with(secondary), None);
        assert_eq!(archive.upsert_index(secondary, primary), Some(()));
        assert_eq!(archive.find_index_with(secondary), Some(primary));

        let primary2 = [7u8; 32];
        assert_eq!(archive.upsert_index(secondary, primary2), Some(()));
        assert_eq!(archive.find_index_with(secondary), Some(primary2));
    }

    #[test]
    fn index_map_rejects_zero_keys_and_full() {
        let account = new_archive_account::<1>(0);
        let archive = Archive::<1, TestRecord>::from_account_info(&account).unwrap();

        assert_eq!(archive.upsert_index([0; 32], [1; 32]), None);
        assert_eq!(archive.upsert_index([1; 32], [0; 32]), None);
        assert_eq!(archive.upsert_index([1; 32], [2; 32]), Some(()));
        assert_eq!(archive.upsert_index([3; 32], [4; 32]), None);
    }
}
