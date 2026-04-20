#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use bytemuck::{Pod, Zeroable};

use crate::{assert_struct_align, assert_struct_size, constants::DAILY_RESET_INTERVAL};

assert_struct_size!(PanicStateCache, 24);
assert_struct_align!(PanicStateCache, 8);
/// Cached panic state information for fast checking during user operations
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Clone, Copy, Default, Zeroable, Pod, Debug, PartialEq, Eq)]
pub struct PanicStateCache {
    /// Whether the protocol is currently paused (1 = paused, 0 = not paused)
    pub pause_flags: u8,
    // Reserved for future use
    pub _reserved: [u8; 7],
    /// Timestamp when the current pause started (0 if not paused)
    pub pause_start_timestamp: i64,
    /// Timestamp when this cache was last updated
    pub last_cache_update: i64,
}

impl PanicStateCache {
    /// Only checks if the paused flag is set.
    /// * Note: if you want to see if a pause is currently active, see `is_expired` instead.
    pub fn is_paused_flag(&self) -> bool {
        (self.pause_flags & PanicState::FLAG_PAUSED) != 0
    }

    /// * If not paused at all, true (nothing to expire).
    /// * If paused and the pause duration has elapsed, true.
    /// * If paused and still within the pause duration, false.
    /// * If paused and current_timestamp invalid, false.
    pub fn is_expired(&self, current_timestamp: i64) -> bool {
        if !self.is_paused_flag() {
            return true;
        }

        // Catch edge cases around malformed timestamps to avoid a panic on sub
        if current_timestamp < self.pause_start_timestamp {
            return false;
        }

        (current_timestamp - self.pause_start_timestamp) >= PanicState::PAUSE_DURATION_SECONDS
    }

    pub fn update_from_panic_state(&mut self, panic_state: &PanicState, current_timestamp: i64) {
        self.pause_flags = panic_state.pause_flags;
        self.pause_start_timestamp = panic_state.pause_start_timestamp;
        self.last_cache_update = current_timestamp;
    }
}

/// Panic state for emergency protocol pausing
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Clone, Copy, Default, Zeroable, Pod, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct PanicState {
    /// Whether the protocol is currently paused (1 = paused, 0 = not paused)
    pub pause_flags: u8,
    /// Number of times paused today (resets every 24 hours)
    pub daily_pause_count: u8,
    /// Number of consecutive pauses (resets when unpause happens)
    pub consecutive_pause_count: u8,
    _reserved: [u8; 5],

    /// Timestamp when the current pause started (0 if not paused)
    /// * When a pause is extended before expiring, this could be in the future.
    pub pause_start_timestamp: i64,
    /// Timestamp of the last daily reset (for tracking daily pause count)
    pub last_daily_reset_timestamp: i64,
    /// Reserved for future use (making total struct 32 bytes)
    _reserved_space: [u8; 8],
}

impl PanicState {
    pub const FLAG_PAUSED: u8 = 1 << 0;
    pub const PAUSE_DURATION_SECONDS: i64 = 7 * 24 * 60 * 60; // 7 days
    pub const MAX_CONSECUTIVE_PAUSES: u8 = 10;
    pub const MAX_DAILY_PAUSES: u8 = 3;

    /// Only checks if the paused flag is set.
    /// * Note: if you want to see if a pause is currently active, see `is_expired` instead.
    pub fn is_paused_flag(&self) -> bool {
        (self.pause_flags & Self::FLAG_PAUSED) != 0
    }

    /// Validates the pauses-per-day and consecutive-pauses limits have not been exceeded
    pub fn can_pause(&self, current_timestamp: i64) -> bool {
        // Reset daily count if 24 hours have passed
        let needs_daily_reset =
            current_timestamp - self.last_daily_reset_timestamp >= DAILY_RESET_INTERVAL;

        let daily_count = if needs_daily_reset {
            0
        } else {
            self.daily_pause_count
        };

        // Check limits
        self.consecutive_pause_count < Self::MAX_CONSECUTIVE_PAUSES
            && daily_count < Self::MAX_DAILY_PAUSES
    }

    /// * If not paused at all, true (nothing to expire).
    /// * If paused and the pause duration has elapsed, true.
    /// * If paused and still within the pause duration, false.
    /// * If paused and current_timestamp invalid, false.
    pub fn is_expired(&self, current_timestamp: i64) -> bool {
        if !self.is_paused_flag() {
            return true;
        }

        // Catch edge cases around malformed timestamps to avoid a panic on sub
        if current_timestamp < self.pause_start_timestamp {
            return false;
        }

        (current_timestamp - self.pause_start_timestamp) >= Self::PAUSE_DURATION_SECONDS
    }
}
