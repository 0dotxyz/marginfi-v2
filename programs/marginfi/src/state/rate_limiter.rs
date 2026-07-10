use crate::{prelude::MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{DAILY_RESET_INTERVAL, HOURLY_RESET_DURATION},
    types::{
        BankRateLimiter, GroupRateLimiter, RateLimitWindow, ACCOUNT_IN_DELEVERAGE,
        ACCOUNT_IN_FLASHLOAN, ACCOUNT_IN_RECEIVERSHIP,
    },
};
/// Converts a `u64` amount into the signed counter representation, returning
/// `None` when it exceeds `i64::MAX` and cannot be tracked.
fn amount_as_i64(amount: u64) -> Option<i64> {
    i64::try_from(amount).ok()
}

/// Rate limiter state uses signed counters, so any amount or configured cap
/// above `i64::MAX` cannot be represented and must be rejected by callers.
pub(crate) fn is_valid_rate_limit_amount(amount: u64) -> bool {
    amount_as_i64(amount).is_some()
}

fn clamp_i128_to_i64(value: i128) -> i64 {
    value.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

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

        let amount = amount_as_i64(amount).ok_or(MarginfiError::InternalLogicError)?;
        let remaining = self.remaining_capacity(current_timestamp);
        if amount > remaining {
            return Err(MarginfiError::InternalLogicError.into());
        }

        self.cur_window_outflow = self.cur_window_outflow.saturating_add(amount);

        Ok(())
    }

    fn record_inflow(&mut self, amount: u64, current_timestamp: i64) {
        self.maybe_advance_window(current_timestamp);

        if !self.is_enabled() {
            return;
        }

        // Inflow reduces net outflow. Unlike an oversized outflow (rejected),
        // an oversized inflow is clamped to the max representable credit so a
        // legitimate large deposit is not trapped behind the outflow cap.
        let inflow = amount_as_i64(amount).unwrap_or(i64::MAX);
        self.cur_window_outflow = self.cur_window_outflow.saturating_sub(inflow);
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
    let Some(max_outflow) = amount_as_i64(max_outflow) else {
        return 0;
    };

    if window_duration == 0 {
        return max_outflow;
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
        return max_outflow;
    }

    // Weight the previous window by remaining time fraction
    // remaining_time = window_duration - elapsed
    // weight = remaining_time / window_duration
    let remaining_time = window_duration.saturating_sub(elapsed);

    // Use signed i128 arithmetic so the full i64 state space, including
    // i64::MIN, remains representable during weighting.
    let weighted_prev = (prev_window_outflow as i128)
        .saturating_mul(remaining_time as i128)
        .checked_div(window_duration as i128)
        .unwrap_or(0);

    // Total net outflow = weighted_prev + cur_window_outflow
    let total_net_outflow = weighted_prev.saturating_add(cur_window_outflow as i128);

    // Remaining capacity = max_outflow - total_net_outflow
    // If total_net_outflow is negative (more inflows), we have extra capacity
    clamp_i128_to_i64((max_outflow as i128).saturating_sub(total_net_outflow))
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

                // An amount that does not fit in i64 cannot be represented and
                // is treated as exceeding every window.
                let amount_i64 = amount_as_i64(amount);
                let exceeds = |remaining: i64| match amount_i64 {
                    Some(a) => a > remaining,
                    None => true,
                };

                if self.hourly.is_enabled() {
                    let remaining = self.hourly.remaining_capacity(current_timestamp);
                    if exceeds(remaining) {
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

                if self.daily.is_enabled() {
                    let remaining = self.daily.remaining_capacity(current_timestamp);
                    if exceeds(remaining) {
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

                // Both checks passed, record the outflow.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// An outflow above `i64::MAX` must fail the limiter closed instead of
    /// wrapping into a negative `i64` and slipping past the signed comparison.
    #[test]
    fn oversized_outflow_does_not_fail_limiter_open() {
        let mut window = RateLimitWindow::default();
        window.initialize(i64::MAX as u64, HOURLY_RESET_DURATION, 0);

        // `i64::MAX as u64 + 1` previously aliased to `i64::MIN` or `i64::MAX`,
        // depending on the code path, and could slip through comparisons.
        let oversized = i64::MAX as u64 + 1;
        assert!(window.try_record_outflow(oversized, 1).is_err());

        // The rejected outflow must not have corrupted the counter, and the
        // exact signed boundary is still permitted.
        assert_eq!(window.cur_window_outflow, 0);
        assert!(window.try_record_outflow(i64::MAX as u64, 1).is_ok());
        assert_eq!(window.cur_window_outflow, i64::MAX);
    }

    /// Inflows and outflows are deliberately asymmetric at the `i64::MAX`
    /// boundary: an oversized outflow is rejected, but an oversized inflow is
    /// clamped to the maximum representable credit so a legitimate large
    /// deposit is not trapped behind the outflow cap. The clamp must not wrap
    /// negative or leave the counter in an un-representable state.
    #[test]
    fn oversized_inflow_clamps_to_max_credit() {
        let mut window = RateLimitWindow::default();
        window.initialize(100, HOURLY_RESET_DURATION, 0);

        window.record_inflow(i64::MAX as u64 + 1, 1);
        assert_eq!(window.cur_window_outflow, -i64::MAX);
        assert_eq!(window.remaining_capacity(1), i64::MAX);

        // Further inflow saturates at `i64::MIN`; the i128 capacity math keeps
        // that state representable and the limiter reads as fully open.
        window.record_inflow(1, 1);
        assert_eq!(window.cur_window_outflow, i64::MIN);
        assert_eq!(window.remaining_capacity(1), i64::MAX);
    }

    #[test]
    fn invalid_max_outflow_fails_closed() {
        let mut window = RateLimitWindow::default();
        window.initialize(i64::MAX as u64 + 1, HOURLY_RESET_DURATION, 0);

        assert_eq!(window.remaining_capacity(1), 0);
        assert_eq!(window.effective_remaining_capacity(1), 0);
        assert!(window.try_record_outflow(1, 1).is_err());
    }

    #[test]
    fn invalid_amounts_are_rejected_by_invariant() {
        assert!(is_valid_rate_limit_amount(i64::MAX as u64));
        assert!(!is_valid_rate_limit_amount(i64::MAX as u64 + 1));
    }

    /// The dual-window limiter is the production entry point. An outflow above
    /// `i64::MAX` must be rejected with the window-specific error (not wrap
    /// negative and slip past the check) and must not corrupt either counter.
    #[test]
    fn bank_limiter_rejects_oversized_outflow() {
        let mut limiter = BankRateLimiter::default();
        limiter.configure_hourly(100, 0);

        let err = limiter
            .try_record_outflow(i64::MAX as u64 + 1, 1)
            .unwrap_err();
        assert_eq!(err, MarginfiError::BankHourlyRateLimitExceeded.into());

        // The rejected outflow left the counter untouched, so a later outflow
        // above the 100-unit cap is still rejected.
        assert_eq!(limiter.hourly.cur_window_outflow, 0);
        assert!(limiter.try_record_outflow(101, 1).is_err());
    }

    /// When only the daily window is enabled, an oversized outflow must still
    /// be rejected — via the daily error rather than the hourly one.
    #[test]
    fn bank_limiter_rejects_oversized_outflow_daily_only() {
        let mut limiter = BankRateLimiter::default();
        limiter.configure_daily(100, 0);

        let err = limiter
            .try_record_outflow(i64::MAX as u64 + 1, 1)
            .unwrap_err();
        assert_eq!(err, MarginfiError::BankDailyRateLimitExceeded.into());
        assert_eq!(limiter.daily.cur_window_outflow, 0);
    }

    /// With no window enabled there is nothing to rate limit, so even an
    /// oversized amount is accepted and no counter is touched.
    #[test]
    fn bank_limiter_allows_oversized_outflow_when_disabled() {
        let mut limiter = BankRateLimiter::default();

        assert!(limiter.try_record_outflow(i64::MAX as u64 + 1, 1).is_ok());
        assert_eq!(limiter.hourly.cur_window_outflow, 0);
        assert_eq!(limiter.daily.cur_window_outflow, 0);
    }
}
