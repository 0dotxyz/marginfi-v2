use crate::{prelude::MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{DAILY_RESET_INTERVAL, HOURLY_RESET_DURATION},
    types::{
        BankRateLimiter, GroupRateLimiter, RateLimitWindow, ACCOUNT_IN_DELEVERAGE,
        ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_RECEIVERSHIP,
    },
};
/// Implementation trait for the sliding window rate limiter.
pub trait RateLimitWindowImpl {
    /// Checks if rate limiting is enabled (max_outflow > 0).
    fn is_enabled(&self) -> bool;

    /// Initialize the window with a limit and duration.
    fn initialize(&mut self, max_outflow: u64, window_duration: u64, current_timestamp: i64);

    /// Advance the window if needed based on current timestamp.
    /// This is called automatically by other methods.
    fn maybe_advance_window(&mut self, current_timestamp: i64);

    /// Calculate the remaining outflow capacity using weighted blend of windows.
    /// Returns i64::MAX if rate limiting is disabled.
    fn remaining_capacity(&self, current_timestamp: i64) -> i64;

    /// Calculate remaining outflow capacity at a timestamp without mutating state.
    /// This simulates window advancement before computing capacity.
    fn effective_remaining_capacity(&self, current_timestamp: i64) -> i64;

    /// Record an outflow (withdraw/borrow). Returns error if limit exceeded.
    fn try_record_outflow(&mut self, amount: u64, current_timestamp: i64) -> MarginfiResult<()>;

    /// Record an inflow (deposit/repay). This reduces window usage.
    fn record_inflow(&mut self, amount: u64, current_timestamp: i64);
}

impl RateLimitWindowImpl for RateLimitWindow {
    fn is_enabled(&self) -> bool {
        self.max_outflow > 0
    }

    fn initialize(&mut self, max_outflow: u64, window_duration: u64, current_timestamp: i64) {
        self.max_outflow = max_outflow;
        self.window_duration = window_duration;
        self.window_start = current_timestamp;
        self.prev_window_outflow = 0;
        self.cur_window_outflow = 0;
    }

    fn maybe_advance_window(&mut self, current_timestamp: i64) {
        if !self.is_enabled() || self.window_duration == 0 {
            return;
        }

        let elapsed = current_timestamp.saturating_sub(self.window_start);
        if elapsed < 0 {
            return;
        }

        let elapsed = elapsed as u64;

        if elapsed >= self.window_duration * 2 {
            // More than 2 windows have passed, reset completely
            self.prev_window_outflow = 0;
            self.cur_window_outflow = 0;
            self.window_start = current_timestamp;
        } else if elapsed >= self.window_duration {
            // One window has passed, shift current to previous
            self.prev_window_outflow = self.cur_window_outflow;
            self.cur_window_outflow = 0;
            // Advance window_start by one duration (not to current_timestamp)
            // This keeps the window boundaries aligned
            self.window_start = self
                .window_start
                .saturating_add(self.window_duration as i64);
        }
        // Otherwise, still within current window, no changes needed
    }

    fn remaining_capacity(&self, current_timestamp: i64) -> i64 {
        if !self.is_enabled() {
            return i64::MAX;
        }
        remaining_capacity_from_state(
            self.max_outflow,
            self.window_duration,
            self.window_start,
            self.prev_window_outflow,
            self.cur_window_outflow,
            current_timestamp,
        )
    }

    fn effective_remaining_capacity(&self, current_timestamp: i64) -> i64 {
        if !self.is_enabled() {
            return i64::MAX;
        }

        let (window_start, prev_window_outflow, cur_window_outflow) =
            effective_window_state(self, current_timestamp);

        remaining_capacity_from_state(
            self.max_outflow,
            self.window_duration,
            window_start,
            prev_window_outflow,
            cur_window_outflow,
            current_timestamp,
        )
    }

    fn try_record_outflow(&mut self, amount: u64, current_timestamp: i64) -> MarginfiResult<()> {
        self.maybe_advance_window(current_timestamp);

        if !self.is_enabled() {
            return Ok(());
        }

        let remaining = self.remaining_capacity(current_timestamp);
        if amount as i64 > remaining {
            return Err(MarginfiError::InternalLogicError.into());
        }

        self.cur_window_outflow = self.cur_window_outflow.saturating_add(amount as i64);

        Ok(())
    }

    fn record_inflow(&mut self, amount: u64, current_timestamp: i64) {
        self.maybe_advance_window(current_timestamp);

        if !self.is_enabled() {
            return;
        }

        // Inflow reduces net outflow
        self.cur_window_outflow = self.cur_window_outflow.saturating_sub(amount as i64);
    }
}

fn effective_window_state(window: &RateLimitWindow, current_timestamp: i64) -> (i64, i64, i64) {
    if !window.is_enabled() || window.window_duration == 0 {
        return (
            window.window_start,
            window.prev_window_outflow,
            window.cur_window_outflow,
        );
    }

    let elapsed = current_timestamp.saturating_sub(window.window_start);
    if elapsed < 0 {
        return (
            window.window_start,
            window.prev_window_outflow,
            window.cur_window_outflow,
        );
    }
    let elapsed = elapsed as u64;

    if elapsed >= window.window_duration.saturating_mul(2) {
        (current_timestamp, 0, 0)
    } else if elapsed >= window.window_duration {
        (
            window
                .window_start
                .saturating_add(window.window_duration as i64),
            window.cur_window_outflow,
            0,
        )
    } else {
        (
            window.window_start,
            window.prev_window_outflow,
            window.cur_window_outflow,
        )
    }
}

fn remaining_capacity_from_state(
    max_outflow: u64,
    window_duration: u64,
    window_start: i64,
    prev_window_outflow: i64,
    cur_window_outflow: i64,
    current_timestamp: i64,
) -> i64 {
    if window_duration == 0 {
        return max_outflow as i64;
    }

    // Calculate elapsed time in current window
    let elapsed = current_timestamp.saturating_sub(window_start);
    if elapsed < 0 {
        return 0;
    }
    let elapsed = elapsed as u64;

    if elapsed >= window_duration {
        // We're past the window, only cur_window matters (it would become prev)
        // and it would be reset, so full capacity available
        return max_outflow as i64;
    }

    // Weight the previous window by remaining time fraction
    // remaining_time = window_duration - elapsed
    // weight = remaining_time / window_duration
    let remaining_time = window_duration.saturating_sub(elapsed);

    // Calculate weighted previous window contribution
    let prev_abs = prev_window_outflow.unsigned_abs();
    let weighted_prev_abs = prev_abs
        .saturating_mul(remaining_time)
        .checked_div(window_duration)
        .unwrap_or(0);

    // Apply the sign back
    let weighted_prev = if prev_window_outflow >= 0 {
        weighted_prev_abs as i64
    } else {
        -(weighted_prev_abs as i64)
    };

    // Total net outflow = weighted_prev + cur_window_outflow
    let total_net_outflow = weighted_prev.saturating_add(cur_window_outflow);

    // Remaining capacity = max_outflow - total_net_outflow
    // If total_net_outflow is negative (more inflows), we have extra capacity
    (max_outflow as i64).saturating_sub(total_net_outflow)
}

macro_rules! impl_dual_window_rate_limiter {
    (
        $impl_trait:ident for $type:ty,
        hourly_error: $hourly_err:ident,
        daily_error: $daily_err:ident,
        log_prefix: $prefix:literal
    ) => {
        impl $impl_trait for $type {
            fn is_enabled(&self) -> bool {
                self.hourly.is_enabled() || self.daily.is_enabled()
            }

            fn configure_hourly(&mut self, max_outflow: u64, current_timestamp: i64) {
                self.hourly
                    .initialize(max_outflow, HOURLY_RESET_DURATION, current_timestamp);
            }

            fn configure_daily(&mut self, max_outflow: u64, current_timestamp: i64) {
                self.daily
                    .initialize(max_outflow, DAILY_RESET_INTERVAL as u64, current_timestamp);
            }

            fn try_record_outflow(
                &mut self,
                amount: u64,
                current_timestamp: i64,
            ) -> MarginfiResult<()> {
                // Advance windows before computing remaining capacity to avoid boundary gaps.
                self.hourly.maybe_advance_window(current_timestamp);
                self.daily.maybe_advance_window(current_timestamp);

                // Check hourly limit first
                if self.hourly.is_enabled() {
                    let remaining = self.hourly.remaining_capacity(current_timestamp);
                    if (amount as i64) > remaining {
                        msg!(
                            concat!(
                                $prefix,
                                " hourly rate limit exceeded: amount={}, remaining={}"
                            ),
                            amount,
                            remaining
                        );
                        return err!(MarginfiError::$hourly_err);
                    }
                }

                // Check daily limit
                if self.daily.is_enabled() {
                    let remaining = self.daily.remaining_capacity(current_timestamp);
                    if (amount as i64) > remaining {
                        msg!(
                            concat!(
                                $prefix,
                                " daily rate limit exceeded: amount={}, remaining={}"
                            ),
                            amount,
                            remaining
                        );
                        return err!(MarginfiError::$daily_err);
                    }
                }

                // Both checks passed, record the outflow
                if self.hourly.is_enabled() {
                    self.hourly.try_record_outflow(amount, current_timestamp)?;
                }
                if self.daily.is_enabled() {
                    self.daily.try_record_outflow(amount, current_timestamp)?;
                }

                Ok(())
            }

            fn record_inflow(&mut self, amount: u64, current_timestamp: i64) {
                if self.hourly.is_enabled() {
                    self.hourly.record_inflow(amount, current_timestamp);
                }
                if self.daily.is_enabled() {
                    self.daily.record_inflow(amount, current_timestamp);
                }
            }
        }
    };
}

/// Implementation trait for bank-level rate limiting (native tokens).
pub trait BankRateLimiterImpl {
    /// Check if any rate limiting is enabled.
    fn is_enabled(&self) -> bool;

    /// Configure the hourly rate limit.
    fn configure_hourly(&mut self, max_outflow: u64, current_timestamp: i64);

    /// Configure the daily rate limit.
    fn configure_daily(&mut self, max_outflow: u64, current_timestamp: i64);

    /// Attempt to record an outflow (withdraw/borrow). Returns specific error if limit exceeded.
    fn try_record_outflow(&mut self, amount: u64, current_timestamp: i64) -> MarginfiResult<()>;

    /// Record an inflow (deposit/repay). This reduces window usage.
    fn record_inflow(&mut self, amount: u64, current_timestamp: i64);
}

impl_dual_window_rate_limiter!(
    BankRateLimiterImpl for BankRateLimiter,
    hourly_error: BankHourlyRateLimitExceeded,
    daily_error: BankDailyRateLimitExceeded,
    log_prefix: "Bank"
);

/// Implementation trait for group-level rate limiting (USD).
pub trait GroupRateLimiterImpl {
    /// Check if any rate limiting is enabled.
    fn is_enabled(&self) -> bool;

    /// Configure the hourly rate limit.
    fn configure_hourly(&mut self, max_outflow: u64, current_timestamp: i64);

    /// Configure the daily rate limit.
    fn configure_daily(&mut self, max_outflow: u64, current_timestamp: i64);

    /// Attempt to record an outflow (in USD). Returns specific error if limit exceeded.
    fn try_record_outflow(&mut self, amount: u64, current_timestamp: i64) -> MarginfiResult<()>;

    /// Record an inflow (in USD). This reduces window usage.
    fn record_inflow(&mut self, amount: u64, current_timestamp: i64);
}

impl_dual_window_rate_limiter!(
    GroupRateLimiterImpl for GroupRateLimiter,
    hourly_error: GroupHourlyRateLimitExceeded,
    daily_error: GroupDailyRateLimitExceeded,
    log_prefix: "Group"
);

/// Checks if rate limiting should be skipped based on account flags.
/// Returns true for flashloan, liquidation, and deleverage operations.
pub fn should_skip_rate_limit(account_flags: u64) -> bool {
    (account_flags & ACCOUNT_IN_FLASHLOAN) != 0
        || (account_flags & ACCOUNT_IN_RECEIVERSHIP) != 0
        || (account_flags & ACCOUNT_IN_DELEVERAGE) != 0
}
