use crate::{assert_struct_align, assert_struct_size, constants::discriminators};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;
#[cfg(feature = "anchor")]
use bytemuck::Zeroable;

#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

pub const MAX_SAME_ASSET_EMODE_GROUPS: usize = 128;
pub const MAX_SAME_ASSET_EMODE_BANKS: usize = 512;

assert_struct_size!(SameAssetEmodeGroup, 96);
assert_struct_align!(SameAssetEmodeGroup, 1);
#[repr(C)]
#[cfg_attr(feature = "anchor", zero_copy)]
#[cfg_attr(not(feature = "anchor"), derive(PartialEq, Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct SameAssetEmodeGroup {
    /// Representative bank for this `(mint, oracle_key)` grouping.
    pub bank: Pubkey,
    pub mint: Pubkey,
    /// The canonical price source, matching `Bank.config.oracle_keys[0]`.
    pub oracle_key: Pubkey,
}

impl Default for SameAssetEmodeGroup {
    fn default() -> Self {
        Self::zeroed()
    }
}

assert_struct_size!(SameAssetEmodeBank, 40);
assert_struct_align!(SameAssetEmodeBank, 1);
#[repr(C)]
#[cfg_attr(feature = "anchor", zero_copy)]
#[cfg_attr(not(feature = "anchor"), derive(PartialEq, Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct SameAssetEmodeBank {
    pub bank: Pubkey,
    /// Index into `SameAssetEmodeRegistry.groups`.
    pub group_index: u8,
    pub _padding: [u8; 7],
}

impl Default for SameAssetEmodeBank {
    fn default() -> Self {
        Self::zeroed()
    }
}

assert_struct_size!(SameAssetEmodeRegistry, 32976);
assert_struct_align!(SameAssetEmodeRegistry, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(PartialEq, Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct SameAssetEmodeRegistry {
    pub _padding_0: u64,
    /// This registry's own key.
    pub key: Pubkey,
    /// Group for which this registry applies.
    pub group: Pubkey,
    pub bank_count: u16,
    pub group_count: u8,
    pub bump: u8,
    pub _padding_1: [u8; 4],
    pub groups: [SameAssetEmodeGroup; MAX_SAME_ASSET_EMODE_GROUPS],
    pub banks: [SameAssetEmodeBank; MAX_SAME_ASSET_EMODE_BANKS],
    pub _padding_2: [u8; 128],
}

impl SameAssetEmodeRegistry {
    pub const LEN: usize = std::mem::size_of::<SameAssetEmodeRegistry>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::SAME_ASSET_EMODE_REGISTRY;

    pub fn find_group_index(&self, mint: Pubkey, oracle_key: Pubkey) -> Option<usize> {
        self.groups[..self.group_count as usize]
            .iter()
            .position(|group| group.mint == mint && group.oracle_key == oracle_key)
    }

    pub fn find_bank_index(&self, bank: Pubkey) -> Option<usize> {
        self.banks[..self.bank_count as usize]
            .iter()
            .position(|entry| entry.bank == bank)
    }

    pub fn group_member_count(&self, group_index: u8) -> usize {
        self.banks[..self.bank_count as usize]
            .iter()
            .filter(|entry| entry.group_index == group_index)
            .count()
    }

    pub fn first_bank_for_group(&self, group_index: u8) -> Option<Pubkey> {
        self.banks[..self.bank_count as usize]
            .iter()
            .find(|entry| entry.group_index == group_index)
            .map(|entry| entry.bank)
    }
}
