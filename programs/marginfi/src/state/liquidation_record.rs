use fixed::types::I80F48;

use crate::constants::{
    LIQUIDATION_TAG_DELAY_SECS, LIQUIDATION_TAG_FULL_PREMIUM_SECS, LIQUIDATION_TAG_MAX_PREMIUM,
};

/// Maximum premium a liquidator may earn, accounting for the record's tag: `base_premium` until
/// `LIQUIDATION_TAG_DELAY_SECS` after the tag, then linear growth to `LIQUIDATION_TAG_MAX_PREMIUM`
/// at `LIQUIDATION_TAG_FULL_PREMIUM_SECS`.
pub fn tag_adjusted_premium(base_premium: I80F48, tagged_at: i64, now: i64) -> I80F48 {
    if tagged_at == 0 || base_premium >= LIQUIDATION_TAG_MAX_PREMIUM {
        return base_premium;
    }
    let elapsed = now.saturating_sub(tagged_at);
    if elapsed <= LIQUIDATION_TAG_DELAY_SECS {
        return base_premium;
    }
    let growth_secs = elapsed.min(LIQUIDATION_TAG_FULL_PREMIUM_SECS) - LIQUIDATION_TAG_DELAY_SECS;
    let growth_window = LIQUIDATION_TAG_FULL_PREMIUM_SECS - LIQUIDATION_TAG_DELAY_SECS;
    let progress = I80F48::from_num(growth_secs) / I80F48::from_num(growth_window);
    base_premium + (LIQUIDATION_TAG_MAX_PREMIUM - base_premium) * progress
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::LIQUIDATION_BONUS_FEE_MINIMUM;

    const BASE: I80F48 = LIQUIDATION_BONUS_FEE_MINIMUM;
    const TAGGED_AT: i64 = 1_000_000;

    #[test]
    fn untagged_returns_base() {
        assert_eq!(tag_adjusted_premium(BASE, 0, TAGGED_AT), BASE);
    }

    #[test]
    fn within_delay_returns_base() {
        assert_eq!(tag_adjusted_premium(BASE, TAGGED_AT, TAGGED_AT), BASE);
        assert_eq!(
            tag_adjusted_premium(BASE, TAGGED_AT, TAGGED_AT + LIQUIDATION_TAG_DELAY_SECS),
            BASE
        );
    }

    #[test]
    fn clock_behind_tag_returns_base() {
        assert_eq!(tag_adjusted_premium(BASE, TAGGED_AT, TAGGED_AT - 100), BASE);
    }

    #[test]
    fn grows_linearly_after_delay() {
        let growth_window = LIQUIDATION_TAG_FULL_PREMIUM_SECS - LIQUIDATION_TAG_DELAY_SECS;
        let halfway = TAGGED_AT + LIQUIDATION_TAG_DELAY_SECS + growth_window / 2;
        let expected = BASE + (LIQUIDATION_TAG_MAX_PREMIUM - BASE) / 2;
        assert_eq!(tag_adjusted_premium(BASE, TAGGED_AT, halfway), expected);
    }

    #[test]
    fn caps_at_max_premium() {
        let at_full = TAGGED_AT + LIQUIDATION_TAG_FULL_PREMIUM_SECS;
        assert_eq!(
            tag_adjusted_premium(BASE, TAGGED_AT, at_full),
            LIQUIDATION_TAG_MAX_PREMIUM
        );
        assert_eq!(
            tag_adjusted_premium(BASE, TAGGED_AT, at_full + 1_000_000),
            LIQUIDATION_TAG_MAX_PREMIUM
        );
    }

    #[test]
    fn base_above_max_is_unchanged() {
        let base = LIQUIDATION_TAG_MAX_PREMIUM + I80F48::ONE;
        assert_eq!(
            tag_adjusted_premium(
                base,
                TAGGED_AT,
                TAGGED_AT + LIQUIDATION_TAG_FULL_PREMIUM_SECS
            ),
            base
        );
    }
}
