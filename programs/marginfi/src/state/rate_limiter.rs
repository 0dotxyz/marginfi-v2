use crate::{prelude::MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{
    constants::{DAILY_WINDOW_DURATION, HOURLY_WINDOW_DURATION},
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

    /// Attempt to record a flow. Positive = outflow (withdraw/borrow), negative = inflow (deposit/repay).
    /// Returns error if outflow would exceed the limit.
    fn record_flow(&mut self, amount: i64, current_timestamp: i64) -> MarginfiResult<()>;

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

        // Calculate elapsed time in current window
        let elapsed = current_timestamp.saturating_sub(self.window_start);
        if elapsed < 0 {
            return 0;
        }
        let elapsed = elapsed as u64;

        if elapsed >= self.window_duration {
            // We're past the window, only cur_window matters (it would become prev)
            // and it would be reset, so full capacity available
            return self.max_outflow as i64;
        }

        // Weight the previous window by remaining time fraction
        // remaining_time = window_duration - elapsed
        // weight = remaining_time / window_duration
        let remaining_time = self.window_duration.saturating_sub(elapsed);

        // Calculate weighted previous window contribution
        // Use saturating operations to avoid overflow
        let prev_abs = self.prev_window_outflow.unsigned_abs();
        let weighted_prev_abs = prev_abs
            .saturating_mul(remaining_time)
            .checked_div(self.window_duration)
            .unwrap_or(0);

        // Apply the sign back
        let weighted_prev = if self.prev_window_outflow >= 0 {
            weighted_prev_abs as i64
        } else {
            -(weighted_prev_abs as i64)
        };

        // Total net outflow = weighted_prev + cur_window_outflow
        let total_net_outflow = weighted_prev.saturating_add(self.cur_window_outflow);

        // Remaining capacity = max_outflow - total_net_outflow
        // If total_net_outflow is negative (more inflows), we have extra capacity
        (self.max_outflow as i64).saturating_sub(total_net_outflow)
    }

    fn record_flow(&mut self, amount: i64, current_timestamp: i64) -> MarginfiResult<()> {
        self.maybe_advance_window(current_timestamp);

        if !self.is_enabled() {
            // Rate limiting disabled, allow everything
            return Ok(());
        }

        // For outflows (positive amount), check if we have capacity
        if amount > 0 {
            let remaining = self.remaining_capacity(current_timestamp);
            if amount > remaining {
                // Would exceed limit
                return Err(MarginfiError::InternalLogicError.into());
            }
        }

        // Record the flow
        self.cur_window_outflow = self.cur_window_outflow.saturating_add(amount);

        Ok(())
    }

    fn try_record_outflow(&mut self, amount: u64, current_timestamp: i64) -> MarginfiResult<()> {
        self.record_flow(amount as i64, current_timestamp)
    }

    fn record_inflow(&mut self, amount: u64, current_timestamp: i64) {
        self.maybe_advance_window(current_timestamp);

        if !self.is_enabled() {
            return;
        }

        // Inflow is negative flow (reduces net outflow)
        self.cur_window_outflow = self.cur_window_outflow.saturating_sub(amount as i64);
    }
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

impl BankRateLimiterImpl for BankRateLimiter {
    fn is_enabled(&self) -> bool {
        self.hourly.is_enabled() || self.daily.is_enabled()
    }

    fn configure_hourly(&mut self, max_outflow: u64, current_timestamp: i64) {
        self.hourly
            .initialize(max_outflow, HOURLY_WINDOW_DURATION, current_timestamp);
    }

    fn configure_daily(&mut self, max_outflow: u64, current_timestamp: i64) {
        self.daily
            .initialize(max_outflow, DAILY_WINDOW_DURATION, current_timestamp);
    }

    fn try_record_outflow(&mut self, amount: u64, current_timestamp: i64) -> MarginfiResult<()> {
        // Advance windows before computing remaining capacity to avoid boundary gaps.
        self.hourly.maybe_advance_window(current_timestamp);
        self.daily.maybe_advance_window(current_timestamp);

        // Check hourly limit first
        if self.hourly.is_enabled() {
            let remaining = self.hourly.remaining_capacity(current_timestamp);
            if (amount as i64) > remaining {
                msg!(
                    "Bank hourly rate limit exceeded: amount={}, remaining={}",
                    amount,
                    remaining
                );
                return err!(MarginfiError::BankHourlyRateLimitExceeded);
            }
        }

        // Check daily limit
        if self.daily.is_enabled() {
            let remaining = self.daily.remaining_capacity(current_timestamp);
            if (amount as i64) > remaining {
                msg!(
                    "Bank daily rate limit exceeded: amount={}, remaining={}",
                    amount,
                    remaining
                );
                return err!(MarginfiError::BankDailyRateLimitExceeded);
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

/// Implementation trait for group-level rate limiting (USD).
pub trait GroupRateLimiterImpl {
    /// Check if any rate limiting is enabled.
    fn is_enabled(&self) -> bool;

    /// Configure the hourly rate limit.
    fn configure_hourly(&mut self, max_outflow_usd: u64, current_timestamp: i64);

    /// Configure the daily rate limit.
    fn configure_daily(&mut self, max_outflow_usd: u64, current_timestamp: i64);

    /// Attempt to record an outflow (in USD). Returns specific error if limit exceeded.
    fn try_record_outflow(&mut self, amount_usd: u64, current_timestamp: i64)
        -> MarginfiResult<()>;

    /// Record an inflow (in USD). This reduces window usage.
    fn record_inflow(&mut self, amount_usd: u64, current_timestamp: i64);
}

impl GroupRateLimiterImpl for GroupRateLimiter {
    fn is_enabled(&self) -> bool {
        self.hourly.is_enabled() || self.daily.is_enabled()
    }

    fn configure_hourly(&mut self, max_outflow_usd: u64, current_timestamp: i64) {
        self.hourly
            .initialize(max_outflow_usd, HOURLY_WINDOW_DURATION, current_timestamp);
    }

    fn configure_daily(&mut self, max_outflow_usd: u64, current_timestamp: i64) {
        self.daily
            .initialize(max_outflow_usd, DAILY_WINDOW_DURATION, current_timestamp);
    }

    fn try_record_outflow(
        &mut self,
        amount_usd: u64,
        current_timestamp: i64,
    ) -> MarginfiResult<()> {
        // Advance windows before computing remaining capacity to avoid boundary gaps.
        self.hourly.maybe_advance_window(current_timestamp);
        self.daily.maybe_advance_window(current_timestamp);

        // Check hourly limit first
        if self.hourly.is_enabled() {
            let remaining = self.hourly.remaining_capacity(current_timestamp);
            if (amount_usd as i64) > remaining {
                msg!(
                    "Group hourly rate limit exceeded: amount_usd={}, remaining={}",
                    amount_usd,
                    remaining
                );
                return err!(MarginfiError::GroupHourlyRateLimitExceeded);
            }
        }

        // Check daily limit
        if self.daily.is_enabled() {
            let remaining = self.daily.remaining_capacity(current_timestamp);
            if (amount_usd as i64) > remaining {
                msg!(
                    "Group daily rate limit exceeded: amount_usd={}, remaining={}",
                    amount_usd,
                    remaining
                );
                return err!(MarginfiError::GroupDailyRateLimitExceeded);
            }
        }

        // Both checks passed, record the outflow
        if self.hourly.is_enabled() {
            self.hourly
                .try_record_outflow(amount_usd, current_timestamp)?;
        }
        if self.daily.is_enabled() {
            self.daily
                .try_record_outflow(amount_usd, current_timestamp)?;
        }

        Ok(())
    }

    fn record_inflow(&mut self, amount_usd: u64, current_timestamp: i64) {
        if self.hourly.is_enabled() {
            self.hourly.record_inflow(amount_usd, current_timestamp);
        }
        if self.daily.is_enabled() {
            self.daily.record_inflow(amount_usd, current_timestamp);
        }
    }
}

/// Checks if rate limiting should be skipped based on account flags.
/// Returns true for flashloan, liquidation, and deleverage operations.
pub fn should_skip_rate_limit(account_flags: u64) -> bool {
    (account_flags & ACCOUNT_IN_FLASHLOAN) != 0
        || (account_flags & ACCOUNT_IN_RECEIVERSHIP) != 0
        || (account_flags & ACCOUNT_IN_DELEVERAGE) != 0
}
