use crate::{assert_struct_align, assert_struct_size};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;
use bytemuck::Zeroable;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;

assert_struct_size!(BankMetadata, 496);
assert_struct_align!(BankMetadata, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy), derive(PartialEq, Eq))]
#[cfg_attr(not(feature = "anchor"), derive(Zeroable))]
#[derive(Debug)]
pub struct BankMetadata {
    /// Bank this metadata corresponds to
    pub bank: Pubkey,
    // Reserved for future use
    pub placeholder: u64,
    /// The token's ticker name, e.g. USDC
    /// * utf-8
    pub ticker: [u8; 64],
    /// The token's plain english description, e.g US Dollar Coin
    /// * utf-8
    pub description: [u8; 128],
    /// Reserved for future use. Room for a very small icon or something else cool
    pub data_blob: [u8; 256],

    /// The last data byte in description (padding follows)
    pub end_description_byte: u16,
    /// The last data byte in data_blob (padding follows)
    pub end_data_blob: u16,
    /// The last data byte in ticker (padding follows)
    pub end_ticker_byte: u8,
    pub bump: u8,
    pub _pad0: [u8; 2],
}

impl Default for BankMetadata {
    fn default() -> Self {
        BankMetadata::zeroed()
    }
}

impl BankMetadata {
    pub const LEN: usize = std::mem::size_of::<BankMetadata>();
    // TODO
    // pub const DISCRIMINATOR: [u8; 8] = discriminators::BankMetadata;
}
