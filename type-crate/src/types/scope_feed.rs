//! Pricing decoded Scope entries without depending on an oracle SDK or `AccountInfo`.
//!
//! Callers must authenticate the account and select the bank's configured feed and entry before
//! constructing these values. Use `check_age` before pricing live data. Integration exchange rates,
//! their freshness, and borrow-power restrictions are the caller's responsibility.
//!
//! ```
//! use marginfi_type_crate::types::ScopeFeed;
//!
//! // After authenticating the account and selecting the configured entry:
//! let scope = ScopeFeed::from_entry(12_345, 2, 1_000).unwrap();
//! scope.check_age(1_030, 60).unwrap();
//! let scope_price = scope.price_and_confidence().unwrap();
//! assert_eq!(scope_price.source_time, 1_000);
//! ```

use fixed::types::I80F48;

use super::OraclePriceWithConfidence;
use crate::constants::{EXP_10_I80F48, MAX_EXP_10_I80F48};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeFeedError {
    InvalidEntry,
    Stale,
    MathError,
}

/// A decoded Scope entry, converted to marginfi's price representation.
#[derive(Clone, Copy, Debug)]
pub struct ScopeFeed {
    pub price: I80F48,
    pub last_updated_timestamp: u64,
}

impl ScopeFeed {
    /// Checks the entry's value and exponent, but does not authenticate its account or check age.
    pub fn from_entry(value: u64, exponent: u64, timestamp: u64) -> Result<Self, ScopeFeedError> {
        if value == 0 || timestamp == 0 || exponent >= MAX_EXP_10_I80F48 as u64 {
            return Err(ScopeFeedError::InvalidEntry);
        }
        let price = I80F48::from_num(value)
            .checked_div(EXP_10_I80F48[exponent as usize])
            .ok_or(ScopeFeedError::MathError)?;
        Ok(Self {
            price,
            last_updated_timestamp: timestamp,
        })
    }

    /// Mirrors the program's live Scope freshness checks, including rejecting future timestamps.
    pub fn check_age(&self, now: i64, max_age: u64) -> Result<(), ScopeFeedError> {
        let timestamp =
            i64::try_from(self.last_updated_timestamp).map_err(|_| ScopeFeedError::InvalidEntry)?;
        if timestamp > now {
            return Err(ScopeFeedError::InvalidEntry);
        }
        if now.saturating_sub(timestamp) > max_age as i64 {
            return Err(ScopeFeedError::Stale);
        }
        Ok(())
    }

    /// Scope does not supply a confidence interval. Call `check_age` for live prices first.
    pub fn price_and_confidence(&self) -> Result<OraclePriceWithConfidence, ScopeFeedError> {
        let source_time =
            i64::try_from(self.last_updated_timestamp).map_err(|_| ScopeFeedError::InvalidEntry)?;
        Ok(OraclePriceWithConfidence {
            price: self.price,
            confidence: I80F48::ZERO,
            source_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_entry_and_live_checks() {
        let feed = ScopeFeed::from_entry(12_345, 2, 1_000).unwrap();
        let price = feed.price_and_confidence().unwrap();
        assert_eq!(
            price.price,
            I80F48::from_num(12_345) / I80F48::from_num(100)
        );
        assert_eq!(price.confidence, I80F48::ZERO);
        assert_eq!(price.source_time, 1_000);
        assert_eq!(feed.check_age(1_060, 60), Ok(()));
        assert_eq!(feed.check_age(1_061, 60), Err(ScopeFeedError::Stale));
        assert_eq!(feed.check_age(999, 60), Err(ScopeFeedError::InvalidEntry));
        assert!(matches!(
            ScopeFeed::from_entry(0, 2, 1),
            Err(ScopeFeedError::InvalidEntry)
        ));
        assert!(matches!(
            ScopeFeed::from_entry(1, 2, 0),
            Err(ScopeFeedError::InvalidEntry)
        ));
        assert!(matches!(
            ScopeFeed::from_entry(1, MAX_EXP_10_I80F48 as u64, 1),
            Err(ScopeFeedError::InvalidEntry)
        ));
        let out_of_range = ScopeFeed::from_entry(1, 0, u64::MAX).unwrap();
        assert_eq!(
            out_of_range.check_age(1_000, 60),
            Err(ScopeFeedError::InvalidEntry)
        );
        assert!(matches!(
            out_of_range.price_and_confidence(),
            Err(ScopeFeedError::InvalidEntry)
        ));
    }

    #[test]
    fn scope_price_matches_program_entry_fixture() {
        // Same input as the program's Scope adapter test, without depending on the program crate.
        let feed = ScopeFeed::from_entry(10_344_510_800, 8, 1_000).unwrap();
        let observation = feed.price_and_confidence().unwrap();
        assert!((observation.price - I80F48::from_num(103.445108)).abs() < I80F48::from_num(1e-9));
        assert_eq!(observation.confidence, I80F48::ZERO);
        assert_eq!(observation.source_time, 1_000);
        assert_eq!(feed.check_age(1_060, 90), Ok(()));
        assert_eq!(feed.check_age(1_061, 60), Err(ScopeFeedError::Stale));
        assert_eq!(
            ScopeFeed::from_entry(123, 0, 1).unwrap().price,
            I80F48::from_num(123)
        );
        assert!(ScopeFeed::from_entry(1, (MAX_EXP_10_I80F48 - 1) as u64, 1).is_ok());
    }
}
