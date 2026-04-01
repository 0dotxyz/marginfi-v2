use crate::{MarginfiError, MarginfiResult};
use anchor_lang::prelude::*;
use marginfi_type_crate::{constants::DAILY_RESET_INTERVAL, types::PanicState};

pub trait PanicStateImpl {
    fn pause(&mut self, current_timestamp: i64) -> MarginfiResult;
    fn unpause(&mut self);
    fn unpause_if_expired(&mut self, current_timestamp: i64);
}

impl PanicStateImpl for PanicState {
    fn pause(&mut self, current_timestamp: i64) -> MarginfiResult<()> {
        // Clear existing pause if expired
        self.unpause_if_expired(current_timestamp);

        // Reset daily count if needed
        if current_timestamp.saturating_sub(self.last_daily_reset_timestamp) >= DAILY_RESET_INTERVAL
        {
            self.daily_pause_count = 0;
            self.last_daily_reset_timestamp = current_timestamp;
        }

        require!(
            self.can_pause(current_timestamp),
            MarginfiError::PauseLimitExceeded
        );

        // If already paused and not expired, treats this as an "extend" operation.
        if self.is_paused_flag() && !self.is_expired(current_timestamp) {
            self.pause_start_timestamp = self
                .pause_start_timestamp
                .saturating_add(Self::PAUSE_DURATION_SECONDS);
        } else {
            // Otherwise, we just start a new pause here
            self.pause_start_timestamp = current_timestamp;
        }

        self.pause_flags |= Self::FLAG_PAUSED;
        self.daily_pause_count = self.daily_pause_count.saturating_add(1);
        self.consecutive_pause_count = self.consecutive_pause_count.saturating_add(1);

        Ok(())
    }

    fn unpause(&mut self) {
        self.pause_flags &= !Self::FLAG_PAUSED;
        self.pause_start_timestamp = 0;
        self.consecutive_pause_count = 0;
    }

    /// No-op if not paused, or paused but time has not yet expired.
    fn unpause_if_expired(&mut self, current_timestamp: i64) {
        if self.is_paused_flag() && self.is_expired(current_timestamp) {
            self.unpause();
        }
    }
}

#[cfg(test)]
mod panic_state_tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let panic_state = PanicState::default();
        assert!(!panic_state.is_paused_flag());
        assert_eq!(panic_state.daily_pause_count, 0);
        assert_eq!(panic_state.consecutive_pause_count, 0);
        assert_eq!(panic_state.pause_start_timestamp, 0);
        assert_eq!(panic_state.last_daily_reset_timestamp, 0);
    }

    #[test]
    fn test_can_pause_initially() {
        let panic_state = PanicState::default();
        assert!(panic_state.can_pause(1000));
    }

    #[test]
    fn test_pause_success() {
        let mut panic_state = PanicState::default();
        let timestamp = 100; // Use smaller timestamp to avoid daily reset

        panic_state.pause(timestamp).unwrap();

        assert!(panic_state.is_paused_flag());
        assert_eq!(panic_state.pause_start_timestamp, timestamp);
        assert_eq!(panic_state.daily_pause_count, 1);
        assert_eq!(panic_state.consecutive_pause_count, 1);
        // Since timestamp (100) < DAILY_RESET_INTERVAL (86400), no daily reset occurs
        assert_eq!(panic_state.last_daily_reset_timestamp, 0);
    }

    #[test]
    fn test_pause_already_paused_extends() {
        let mut panic_state = PanicState::default();

        // First pause starts at t=1000, ends at 1900
        panic_state.pause(1000).unwrap();
        let first_start = panic_state.pause_start_timestamp;
        let first_expiry = first_start + PanicState::PAUSE_DURATION_SECONDS;

        // Call pause again while still paused (in the middle of pause, before expiry)
        let result = panic_state.pause(1400);
        assert!(result.is_ok());

        // The pause_start_timestamp should have been extended forward by PAUSE_DURATION_SECONDS
        let second_start = panic_state.pause_start_timestamp;
        let second_expiry = second_start + PanicState::PAUSE_DURATION_SECONDS;

        assert_eq!(
            second_start,
            first_start + PanicState::PAUSE_DURATION_SECONDS
        );
        assert_eq!(
            second_expiry,
            first_expiry + PanicState::PAUSE_DURATION_SECONDS
        );

        // Daily/consecutive counts still trigger
        assert_eq!(panic_state.daily_pause_count, 2);
        assert_eq!(panic_state.consecutive_pause_count, 2);
    }

    #[test]
    fn test_consecutive_pause_limit() {
        let mut panic_state = PanicState::default();
        let base_timestamp = 1000;

        // First pause - should work
        panic_state.pause(base_timestamp).unwrap();
        assert_eq!(panic_state.consecutive_pause_count, 1);

        // Let it expire and update
        panic_state.unpause_if_expired(base_timestamp + PanicState::PAUSE_DURATION_SECONDS);
        assert!(!panic_state.is_paused_flag());
        assert_eq!(panic_state.consecutive_pause_count, 0); // Reset on auto-unpause

        // Second pause - should work
        panic_state
            .pause(base_timestamp + PanicState::PAUSE_DURATION_SECONDS + 1)
            .unwrap();
        assert_eq!(panic_state.consecutive_pause_count, 1);

        // Let it expire and update
        panic_state.unpause_if_expired(base_timestamp + 2 * PanicState::PAUSE_DURATION_SECONDS + 1);
        assert!(!panic_state.is_paused_flag());
        assert_eq!(panic_state.consecutive_pause_count, 0);
    }

    #[test]
    fn test_daily_pause_limit() {
        let mut panic_state = PanicState::default();
        let base_timestamp = 1000;

        // Exhaust daily limit
        for i in 0..PanicState::MAX_DAILY_PAUSES {
            panic_state.pause(base_timestamp + i as i64).unwrap();
            panic_state.unpause();
        }

        // Next pause should fail
        let result = panic_state.pause(base_timestamp + PanicState::MAX_DAILY_PAUSES as i64);
        assert!(result.is_err());
        assert!(!panic_state.can_pause(base_timestamp + PanicState::MAX_DAILY_PAUSES as i64));
    }

    #[test]
    fn test_daily_reset() {
        let mut panic_state = PanicState::default();
        let base_timestamp = 1000;

        // Exhaust daily limit
        for i in 0..PanicState::MAX_DAILY_PAUSES {
            panic_state.pause(base_timestamp + i as i64).unwrap();
            panic_state.unpause();
        }

        // Move forward 24+ hours
        let next_day = base_timestamp + DAILY_RESET_INTERVAL + 1;

        // Should be able to pause again
        assert!(panic_state.can_pause(next_day));
        panic_state.pause(next_day).unwrap();

        // Daily count should reset
        assert_eq!(panic_state.daily_pause_count, 1);
        assert_eq!(panic_state.last_daily_reset_timestamp, next_day);
    }

    #[test]
    fn test_is_expired() {
        let mut panic_state = PanicState::default();
        let start_time = 1000;

        panic_state.pause(start_time).unwrap();

        // Not expired within duration
        assert!(!panic_state.is_expired(start_time + PanicState::PAUSE_DURATION_SECONDS - 1));

        // Expired after duration
        assert!(panic_state.is_expired(start_time + PanicState::PAUSE_DURATION_SECONDS));
        assert!(panic_state.is_expired(start_time + PanicState::PAUSE_DURATION_SECONDS + 100));
    }

    #[test]
    fn test_expired_when_not_paused() {
        let panic_state = PanicState::default();
        assert!(panic_state.is_expired(1000));
    }

    #[test]
    fn test_update_if_expired() {
        let mut panic_state = PanicState::default();
        let start_time = 1000;

        panic_state.pause(start_time).unwrap();
        assert!(panic_state.is_paused_flag());

        // Update before expiration - should remain paused
        panic_state.unpause_if_expired(start_time + PanicState::PAUSE_DURATION_SECONDS - 1);
        assert!(panic_state.is_paused_flag());

        // Update after expiration - should auto-unpause
        panic_state.unpause_if_expired(start_time + PanicState::PAUSE_DURATION_SECONDS);
        assert!(!panic_state.is_paused_flag());
        assert_eq!(panic_state.consecutive_pause_count, 0);
        assert_eq!(panic_state.pause_start_timestamp, 0);
    }

    #[test]
    fn test_unpause() {
        let mut panic_state = PanicState::default();
        panic_state.pause(1000).unwrap();

        assert!(panic_state.is_paused_flag());
        assert_eq!(panic_state.consecutive_pause_count, 1);

        panic_state.unpause();

        assert!(!panic_state.is_paused_flag());
        assert_eq!(panic_state.consecutive_pause_count, 0);
        assert_eq!(panic_state.pause_start_timestamp, 0);
        // Daily count should remain unchanged
        assert_eq!(panic_state.daily_pause_count, 1);
    }

    #[test]
    fn test_pause_constants() {
        assert_eq!(PanicState::PAUSE_DURATION_SECONDS, 7 * 24 * 60 * 60);
        assert_eq!(PanicState::MAX_CONSECUTIVE_PAUSES, 10);
        assert_eq!(PanicState::MAX_DAILY_PAUSES, 3);
    }

    #[test]
    fn test_multiple_pause_unpause_cycles() {
        let mut panic_state = PanicState::default();
        let base_timestamp = 1000;

        // First cycle
        panic_state.pause(base_timestamp).unwrap();
        assert_eq!(panic_state.consecutive_pause_count, 1);
        assert_eq!(panic_state.daily_pause_count, 1);
        panic_state.unpause();

        // Second cycle
        panic_state.pause(base_timestamp + 100).unwrap();
        assert_eq!(panic_state.consecutive_pause_count, 1);
        assert_eq!(panic_state.daily_pause_count, 2);
        panic_state.unpause();

        // Third cycle
        panic_state.pause(base_timestamp + 200).unwrap();
        assert_eq!(panic_state.consecutive_pause_count, 1);
        assert_eq!(panic_state.daily_pause_count, 3);
        panic_state.unpause();

        // Fourth cycle should fail daily limit
        let result = panic_state.pause(base_timestamp + 300);
        assert!(result.is_err());
    }

    #[test]
    fn test_consecutive_limit_resets_after_unpause() {
        let mut panic_state = PanicState::default();
        let base_timestamp = 1000;

        // First pause
        panic_state.pause(base_timestamp).unwrap();
        panic_state.unpause();

        // Second pause
        panic_state.pause(base_timestamp + 100).unwrap();
        assert_eq!(panic_state.consecutive_pause_count, 1); // Reset after unpause
        panic_state.unpause();

        // Should be able to pause again since consecutive count reset
        panic_state.pause(base_timestamp + 200).unwrap();
        assert_eq!(panic_state.consecutive_pause_count, 1);
    }

    #[test]
    fn test_consecutive_limit_without_unpause() {
        let mut panic_state = PanicState::default();
        let base_timestamp = 1000;

        // First pause that expires automatically
        panic_state.pause(base_timestamp).unwrap();
        panic_state.unpause_if_expired(base_timestamp + PanicState::PAUSE_DURATION_SECONDS);
        assert_eq!(panic_state.consecutive_pause_count, 0); // Reset after expiration

        // Second pause that expires automatically
        panic_state
            .pause(base_timestamp + PanicState::PAUSE_DURATION_SECONDS + 1)
            .unwrap();
        panic_state.unpause_if_expired(base_timestamp + 2 * PanicState::PAUSE_DURATION_SECONDS + 1);
        assert_eq!(panic_state.consecutive_pause_count, 0); // Reset after expiration

        // Should still be able to pause since consecutive count resets on expiration
        panic_state
            .pause(base_timestamp + 2 * PanicState::PAUSE_DURATION_SECONDS + 2)
            .unwrap();
        assert_eq!(panic_state.consecutive_pause_count, 1);
    }

    #[test]
    fn test_pause_after_expired_without_manual_unpause() {
        let mut s = PanicState::default();
        let t = 1000;
        s.pause(t).unwrap();
        assert_eq!(s.consecutive_pause_count, 1);
        assert_eq!(s.daily_pause_count, 1);

        // Time jumps past expiry; but pause flag still set, we never bothered to unpause
        let t2 = t + PanicState::PAUSE_DURATION_SECONDS + 1;
        // Should "unpause" and then start a new "pause"
        assert!(s.pause(t2).is_ok());
        assert!(s.is_paused_flag());
        // Note: still 1, this doesn't count as consecutive because the previous one expired.
        assert_eq!(s.consecutive_pause_count, 1);
        // Daily count resets because t2 is 7+ days after t (past DAILY_RESET_INTERVAL)
        assert_eq!(s.daily_pause_count, 1);

        // Time jumps past second pause expiry and into a new day
        let t3 = t2 + PanicState::PAUSE_DURATION_SECONDS + DAILY_RESET_INTERVAL + 1;
        // Should "unpause" and then start a new "pause"
        assert!(s.pause(t3).is_ok());
        assert!(s.is_paused_flag());
        // Note: still 1, this doesn't count as consecutive because the previous one expired.
        assert_eq!(s.consecutive_pause_count, 1);
        // Daily count reset, this is another day!
        assert_eq!(s.daily_pause_count, 1);
    }

    #[test]
    fn test_unpause_if_expired_noop_when_unpaused() {
        let mut s = PanicState::default();
        s.unpause_if_expired(12345); // should not change anything / panic
        assert!(!s.is_paused_flag());
        assert_eq!(s.consecutive_pause_count, 0);
    }
}
