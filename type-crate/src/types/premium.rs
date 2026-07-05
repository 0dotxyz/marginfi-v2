use bytemuck::{Pod, Zeroable};

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use crate::{assert_struct_align, assert_struct_size};

/// Maximum pairwise premium entries storable in the group account without a resize. A future
/// group-account resize can raise the effective capacity (`PremiumSettings.entry_capacity`)
/// beyond this without changing lookup callers.
pub const MAX_PREMIUM_ENTRIES: usize = 64;
/// A `premium_tag` of 0 is untagged: it never matches any premium entry.
pub const PREMIUM_TAG_EMPTY: u16 = 0;

assert_struct_size!(PremiumSettings, 32);
assert_struct_align!(PremiumSettings, 8);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone, Eq)]
/// Header for the group's pairwise variable-borrow premium matrix.
/// * `entry_count > 0` is the single source of truth for whether the matrix is configured.
pub struct PremiumSettings {
    /// Unix timestamp from the system clock when the premium matrix was last updated.
    pub timestamp: i64,
    /// Number of live entries at the start of `premium_entries`. 0 = matrix off.
    pub entry_count: u16,
    /// Storage capacity for entries. `MAX_PREMIUM_ENTRIES` for groups at the current account
    /// size; a future group-account resize may raise this.
    pub entry_capacity: u16,
    // Pad to next 8-byte multiple
    pub _pad0: [u8; 4],
    /// Reserved for future use
    pub _reserved0: [u64; 2],
}

impl Default for PremiumSettings {
    fn default() -> Self {
        Self::zeroed()
    }
}

assert_struct_size!(PremiumEntry, 8);
assert_struct_align!(PremiumEntry, 4);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Pod, Zeroable, Copy, Clone, Eq)]
/// One pairwise variable-borrow premium rate: accounts lending collateral tagged
/// `collateral_tag` pay an extra `rate` APR (proportional to that collateral's share of their
/// total collateral) when borrowing from banks tagged `liability_tag`.
pub struct PremiumEntry {
    /// `premium_tag` of the collateral bank(s) this surcharge applies to. 0 = empty slot.
    pub collateral_tag: u16,
    /// `premium_tag` of the liability bank(s) this surcharge applies to. 0 = empty slot.
    pub liability_tag: u16,
    /// Premium APR for this pair, encoded like interest-curve points via `milli_to_u32`
    /// (0-1000%).
    pub rate: u32,
}

impl PremiumEntry {
    pub fn is_empty(&self) -> bool {
        self.collateral_tag == PREMIUM_TAG_EMPTY || self.liability_tag == PREMIUM_TAG_EMPTY
    }
}
