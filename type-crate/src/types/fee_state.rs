#[cfg(not(feature = "anchor"))]
use {
    super::Pubkey,
    bytemuck::{Pod, Zeroable},
};

use crate::{assert_struct_align, assert_struct_size, constants::discriminators};

use super::{PanicState, WrappedI80F48};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

assert_struct_size!(FeeState, 256);
assert_struct_align!(FeeState, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(
    not(feature = "anchor"),
    derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone)
)]
/// Unique per-program. The Program Owner uses this account to administrate fees collected by the protocol
pub struct FeeState {
    /// The fee state's own key. A PDA derived from just `b"feestate"`
    pub key: Pubkey,
    /// Can modify fees
    pub global_fee_admin: Pubkey,
    /// The base wallet for all protocol fees. All SOL fees go to this wallet. All non-SOL fees go
    /// to the cannonical ATA of this wallet for that asset.
    pub global_fee_wallet: Pubkey,
    // Reserved for future use, forces 8-byte alignment
    pub placeholder0: u64,
    /// Flat fee assessed when a new bank is initialized, in lamports.
    /// * In SOL, in native decimals.
    pub bank_init_flat_sol_fee: u32,
    pub bump_seed: u8,
    // Pad to next 8-byte multiple
    _padding0: [u8; 3],
    /// Liquidators can claim at this premium, when liquidating an asset in receivership
    /// liquidation, e.g. (1 + this) * amount repaid >= asset seized
    /// * A percentage
    pub liquidation_max_fee: WrappedI80F48,
    /// Fee collected by the program owner from all groups
    /// * A percentage
    pub program_fee_fixed: WrappedI80F48,
    /// Fee collected by the program owner from all groups
    /// * A percentage
    pub program_fee_rate: WrappedI80F48,
    /// When the global admin pauses the protocol in the event of an emergency, information about
    /// the pause duration will be stored here and propagated to groups.
    pub panic_state: PanicState,
    // Reserved for future use, forces 8-byte alignment
    pub placeholder1: u64,
    /// Flat fee assessed for insurance/program use when a liquidation is executed
    /// * In SOL, in native decimals.
    pub liquidation_flat_sol_fee: u32,
    /// Flat fee assessed for preventing spam use when creating an order
    /// * In SOL, in native decimals.
    pub order_init_flat_sol_fee: u32,
    /// Orders can be executed at this premium
    /// e.g. (1 + this) * amount repaid >= asset seized
    /// * A percentage    
    pub order_execution_max_fee: WrappedI80F48,
    _reserved1: [u8; 32],
}

impl FeeState {
    pub const LEN: usize = std::mem::size_of::<FeeState>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::FEE_STATE;

    pub fn from_bytes(v: &[u8]) -> &Self {
        bytemuck::from_bytes(v)
    }

    pub fn from_bytes_mut(v: &mut [u8]) -> &mut Self {
        bytemuck::from_bytes_mut(v)
    }
}
