use fixed::types::I80F48;

use crate::constants::{
    LIQUIDATION_TAG_DELAY_SECS, LIQUIDATION_TAG_FULL_PREMIUM_SECS, LIQUIDATION_TAG_MAX_PREMIUM,
    LIQUIDATION_TAG_RESET_DEFICIT_FRACTION, LIQUIDATION_TAG_RESET_REPAID_FRACTION,
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

/// The account's `tagged_at` after a completed liquidation: cleared once the account is healthy,
/// restarted at `now` when the liquidation erased at least `LIQUIDATION_TAG_RESET_DEFICIT_FRACTION`
/// of the health deficit or repaid at least `LIQUIDATION_TAG_RESET_REPAID_FRACTION` of the
/// liabilities, otherwise unchanged. The two healths share one weighting and price set, as do
/// `pre_liabs` and `repaid`.
pub fn tag_after_liquidation(
    tagged_at: i64,
    pre_health: I80F48,
    post_health: I80F48,
    pre_liabs: I80F48,
    repaid: I80F48,
    now: i64,
) -> i64 {
    if tagged_at == 0 {
        return 0;
    }
    let pre_deficit = I80F48::max(I80F48::ZERO, -pre_health);
    let post_deficit = I80F48::max(I80F48::ZERO, -post_health);
    if post_deficit == I80F48::ZERO {
        return 0;
    }
    let deficit_erased =
        pre_deficit - post_deficit >= pre_deficit * LIQUIDATION_TAG_RESET_DEFICIT_FRACTION;
    let debt_repaid =
        pre_liabs > I80F48::ZERO && repaid >= pre_liabs * LIQUIDATION_TAG_RESET_REPAID_FRACTION;
    if deficit_erased || debt_repaid {
        return now;
    }
    tagged_at
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::LIQUIDATION_BONUS_FEE_MINIMUM;
    use fixed_macro::types::I80F48;

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

    const NOW: i64 = TAGGED_AT + 50_000;
    const LIABS: I80F48 = I80F48!(1000);

    #[test]
    fn untagged_record_is_never_tagged_by_a_liquidation() {
        assert_eq!(
            tag_after_liquidation(0, I80F48!(-100), I80F48!(-10), LIABS, I80F48!(500), NOW),
            0
        );
    }

    #[test]
    fn healthy_after_liquidation_clears_tag() {
        assert_eq!(
            tag_after_liquidation(
                TAGGED_AT,
                I80F48!(-100),
                I80F48::ZERO,
                LIABS,
                I80F48!(1),
                NOW
            ),
            0
        );
        assert_eq!(
            tag_after_liquidation(TAGGED_AT, I80F48!(-100), I80F48!(5), LIABS, I80F48!(1), NOW),
            0
        );
    }

    #[test]
    fn deficit_reduction_at_threshold_restarts_clock() {
        // Exactly 25% of a 100 deficit erased
        assert_eq!(
            tag_after_liquidation(
                TAGGED_AT,
                I80F48!(-100),
                I80F48!(-75),
                LIABS,
                I80F48!(1),
                NOW
            ),
            NOW
        );
    }

    #[test]
    fn deficit_reduction_below_threshold_leaves_tag() {
        assert_eq!(
            tag_after_liquidation(
                TAGGED_AT,
                I80F48!(-100),
                I80F48!(-75.01),
                LIABS,
                I80F48!(1),
                NOW
            ),
            TAGGED_AT
        );
        // Dust repayment on a large deficit: the growth clock keeps running
        assert_eq!(
            tag_after_liquidation(
                TAGGED_AT,
                I80F48!(-1000),
                I80F48!(-999.99),
                LIABS,
                I80F48!(0.01),
                NOW
            ),
            TAGGED_AT
        );
    }

    #[test]
    fn repaying_threshold_share_of_debt_restarts_clock_even_as_deficit_grows() {
        // A full-premium liquidation: exactly 25% of the debt repaid while the deficit deepens
        assert_eq!(
            tag_after_liquidation(
                TAGGED_AT,
                I80F48!(-100),
                I80F48!(-120),
                LIABS,
                I80F48!(250),
                NOW
            ),
            NOW
        );
    }

    #[test]
    fn repaying_below_threshold_share_leaves_tag() {
        assert_eq!(
            tag_after_liquidation(
                TAGGED_AT,
                I80F48!(-100),
                I80F48!(-90),
                LIABS,
                I80F48!(249.99),
                NOW
            ),
            TAGGED_AT
        );
    }
}
