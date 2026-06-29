use bytemuck::{Pod, Zeroable};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

#[cfg(not(feature = "anchor"))]
use super::Pubkey;

use crate::{assert_struct_size, constants::discriminators};

assert_struct_size!(TimelockedOperation, 312);
/// Represents a pending timelocked operation.
/// PDAs prevent misuse and enable deterministic account derivation.
#[repr(C)]
#[cfg_attr(feature = "anchor", account(zero_copy))]
#[cfg_attr(not(feature = "anchor"), derive(Pod, Zeroable, Copy, Clone))]
#[derive(Default, Debug, PartialEq, Eq)]
pub struct TimelockedOperation {
    /// The marginfi group this operation belongs to
    pub group: Pubkey,
    /// Unix timestamp when this operation was created
    pub created_at: i64,
    /// Unix timestamp when this operation can be executed (created_at + delay)
    pub execution_available_at: i64,
    /// The admin that scheduled this operation.
    /// Only this admin or the current group admin can execute or cancel.
    pub admin: Pubkey,
    /// Operation type discriminator
    pub operation_type: u8,
    /// Executed flag (1 = executed, 0 = pending) - prevents replays
    pub executed: u8,
    /// Validated flag (1 = step 2 done, 0 = not yet)
    pub validated: u8,
    /// PDA bump for this operation account
    pub bump: u8,
    pub _pad0: [u8; 4],
    /// The mint of the bank being operated on (if applicable)
    pub bank_mint: Pubkey,
    /// Operation-specific data (layout depends on operation_type)
    pub data: TimelockedOperationData,
}

#[repr(C)]
#[derive(Default, Debug, PartialEq, Eq, Pod, Zeroable, Copy, Clone)]
pub struct TimelockedOperationData {
    /// u64 slots for operation-specific data. Meaning depends on operation_type.
    pub value_u64_1: u64,
    pub value_u64_2: u64,
    pub value_u64_3: u64,
    pub value_u64_4: u64,
    /// Pubkey slot for storing auxiliary keys
    pub pubkey_1: Pubkey,
    pub pubkey_2: Pubkey,
    pub pubkey_3: Pubkey,
    /// Additional bytes for operation data (128 bits for WrappedI80F48 values)
    pub extra: [u8; 32],
    /// Extended storage for additional operation data
    pub extra_extended: [u8; 32],
}

/// Operation type discriminators
pub mod operation_type {
    pub const ADD_BANK: u8 = 0;
    pub const CONFIGURE_ORACLE: u8 = 1;
    pub const SET_FIXED_ORACLE_PRICE: u8 = 2;
    pub const ACTIVATE_BANK: u8 = 3; // Transition from ReduceOnly/Paused to Operational
    pub const CONFIGURE_BANK_RISK_TIER: u8 = 4; // Risk tier change (Isolated ↔ Collateral)
}

impl TimelockedOperation {
    pub const LEN: usize = std::mem::size_of::<TimelockedOperation>();
    pub const DISCRIMINATOR: [u8; 8] = discriminators::TIMELOCKED_OPERATION;
}
