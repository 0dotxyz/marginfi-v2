use crate::{assert_struct_align, assert_struct_size, constants::discriminators};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

use super::WrappedI80F48;

// Asset shares one account sent toward a wallet that has not accepted them yet. The shares stay in
// the bank's totals, so they keep earning interest while held here.
// PDA: [POSITION_ESCROW_SEED, bank, receiver_authority, source_account, memo].
assert_struct_size!(PositionEscrow, 256);
assert_struct_align!(PositionEscrow, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Debug)]
pub struct PositionEscrow {
    pub bank: Pubkey,
    /// The sending account. Returned shares go back here.
    pub source_account: Pubkey,
    /// The account the opening send named as the one the shares are for. Kept for accounting, not
    /// checked.
    pub receiver_account: Pubkey,
    /// Accepts into any of its accounts, or declines.
    pub receiver_authority: Pubkey,
    /// Paid the rent and gets it back when the escrow closes.
    pub rent_payer: Pubkey,
    pub asset_shares: WrappedI80F48,
    /// The sender's tag, such as a distribution ID. Part of the PDA, so each memo has its own
    /// escrow.
    pub memo: [u8; 16],
    /// Unix timestamp after which anyone may return the shares to the sender.
    pub expires_at: i64,
    pub bump: u8,
    pub _pad0: [u8; 7],
    pub _reserved0: [u8; 48],
}

impl PositionEscrow {
    pub const LEN: usize = core::mem::size_of::<PositionEscrow>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::POSITION_ESCROW;
}
