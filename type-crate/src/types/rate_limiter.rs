#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use bytemuck::{Pod, Zeroable};

use crate::{assert_struct_align, assert_struct_size};

assert_struct_size!(RateLimitWindow, 40);
assert_struct_align!(RateLimitWindow, 8);
/// A sliding window rate limiter that tracks net outflow over a time window.
/// Uses weighted blend of previous and current windows for smooth transitions.
///
/// Net outflow = (withdraws + borrows) - (deposits + repays).
/// A negative net outflow increases remaining capacity for subsequent outflows.
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Clone, Copy, Default, Zeroable, Pod, Debug, PartialEq, Eq)]
pub struct RateLimitWindow {
    /// Maximum net outflow allowed per window (0 = disabled).
    /// For bank-level: denominated in native tokens.
    /// For group-level: denominated in USD.
    pub max_outflow: u64,

    /// Window duration in seconds (e.g., 3600 for hourly, 86400 for daily).
    pub window_duration: u64,

    /// Unix timestamp when the current window started.
    pub window_start: i64,

    /// Net outflow accumulated in the previous window.
    /// Signed to allow tracking when inflows exceed outflows.
    pub prev_window_outflow: i64,

    /// Net outflow accumulated in the current window.
    /// Signed to allow tracking when inflows exceed outflows.
    pub cur_window_outflow: i64,
}

assert_struct_size!(BankRateLimiter, 80);
assert_struct_align!(BankRateLimiter, 8);
/// Per-bank rate limiting configuration and state.
/// Tracks net outflow in native tokens.
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Clone, Copy, Default, Zeroable, Pod, Debug, PartialEq, Eq)]
pub struct BankRateLimiter {
    /// Hourly window rate limiter (native tokens).
    pub hourly: RateLimitWindow,

    /// Daily window rate limiter (native tokens).
    pub daily: RateLimitWindow,
}

assert_struct_size!(GroupRateLimiter, 80);
assert_struct_align!(GroupRateLimiter, 8);
/// Per-group rate limiting configuration and state.
/// Tracks aggregate net outflow in USD.
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Clone, Copy, Default, Zeroable, Pod, Debug, PartialEq, Eq)]
pub struct GroupRateLimiter {
    /// Hourly window rate limiter (USD).
    pub hourly: RateLimitWindow,

    /// Daily window rate limiter (USD).
    pub daily: RateLimitWindow,
}
