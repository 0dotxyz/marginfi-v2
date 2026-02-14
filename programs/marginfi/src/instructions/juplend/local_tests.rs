#[cfg(test)]
mod tests {
    use juplend_mocks::state::EXCHANGE_PRICES_PRECISION;
    use marginfi_type_crate::types::price::{mul_div_i128, mul_div_i64, mul_div_u64};

    fn juplend_adjust_i64(raw: i64, token_exchange_price: u64) -> Option<i64> {
        mul_div_i64(raw, token_exchange_price as u128, EXCHANGE_PRICES_PRECISION)
    }

    fn juplend_adjust_u64(raw: u64, token_exchange_price: u64) -> Option<u64> {
        mul_div_u64(raw, token_exchange_price as u128, EXCHANGE_PRICES_PRECISION)
    }

    fn juplend_adjust_i128(raw: i128, token_exchange_price: u64) -> Option<i128> {
        mul_div_i128(raw, token_exchange_price as u128, EXCHANGE_PRICES_PRECISION)
    }

    /// Find the largest u64 raw value that won't overflow when adjusted.
    /// Formula: adjusted = raw * token_exchange_price / EXCHANGE_PRICES_PRECISION
    fn largest_safe_raw_for_u64_exact(token_exchange_price: u64) -> u64 {
        if token_exchange_price == 0 {
            return u64::MAX;
        }
        let max_product = (u64::MAX as u128)
            .checked_mul(EXCHANGE_PRICES_PRECISION)
            .unwrap_or(u128::MAX);
        let safe = max_product / token_exchange_price as u128;
        if safe > u64::MAX as u128 {
            u64::MAX
        } else {
            safe as u64
        }
    }

    fn overflow_raw_for_u64_exact(token_exchange_price: u64) -> u64 {
        largest_safe_raw_for_u64_exact(token_exchange_price).saturating_add(1)
    }

    /// Find the largest i64 raw value that won't overflow when adjusted.
    fn largest_safe_raw_for_i64_exact(token_exchange_price: u64) -> i64 {
        if token_exchange_price == 0 {
            return i64::MAX;
        }
        let max_product = (i64::MAX as u128)
            .checked_mul(EXCHANGE_PRICES_PRECISION)
            .unwrap_or(u128::MAX);
        let safe = max_product / token_exchange_price as u128;
        if safe > i64::MAX as u128 {
            i64::MAX
        } else {
            safe as i64
        }
    }

    fn overflow_raw_for_i64_exact(token_exchange_price: u64) -> i64 {
        largest_safe_raw_for_i64_exact(token_exchange_price).saturating_add(1)
    }

    #[test]
    fn adjust_oracle_price() {
        let price = 1_000_000i64;

        // At 1.0x: adjusted = price
        assert_eq!(juplend_adjust_i64(price, 1_000_000_000_000).unwrap(), price);

        // At 1.2x: adjusted = 1_200_000
        assert_eq!(
            juplend_adjust_i64(price, 1_200_000_000_000).unwrap(),
            1_200_000i64
        );
    }

    #[test]
    fn adjust_u64() {
        // 1.5x
        assert_eq!(
            juplend_adjust_u64(10_000, 1_500_000_000_000).unwrap(),
            15_000u64
        );
    }

    #[test]
    fn adjust_i128_for_switchboard_price() {
        let price: i128 = 1_000_000_000_000_000_000;
        assert_eq!(
            juplend_adjust_i128(price, 1_200_000_000_000).unwrap(),
            1_200_000_000_000_000_000i128
        );
    }

    #[test]
    fn adjust_negative_values_fails() {
        assert!(juplend_adjust_i64(-1, 1_000_000_000_000).is_none());
        assert!(juplend_adjust_i128(-1, 1_000_000_000_000).is_none());
    }

    #[test]
    fn adjust_u64_overflow_at_exact_boundary() {
        // 200x exchange price to trigger overflow
        let token_exchange_price = 200_000_000_000_000u64;

        let safe = largest_safe_raw_for_u64_exact(token_exchange_price);
        assert!(
            juplend_adjust_u64(safe, token_exchange_price).is_some(),
            "safe value {} should succeed",
            safe
        );

        let ovf = overflow_raw_for_u64_exact(token_exchange_price);
        assert!(
            juplend_adjust_u64(ovf, token_exchange_price).is_none(),
            "overflow value {} should fail",
            ovf
        );

        assert!(safe < u64::MAX);
    }

    #[test]
    fn adjust_i64_overflow_at_exact_boundary() {
        let token_exchange_price = 200_000_000_000_000u64;

        let safe = largest_safe_raw_for_i64_exact(token_exchange_price);
        assert!(
            juplend_adjust_i64(safe, token_exchange_price).is_some(),
            "safe value {} should succeed",
            safe
        );

        let ovf = overflow_raw_for_i64_exact(token_exchange_price);
        assert!(
            juplend_adjust_i64(ovf, token_exchange_price).is_none(),
            "overflow value {} should fail",
            ovf
        );

        assert!(safe < i64::MAX);
    }

    #[test]
    fn adjust_i128_overflow_detection() {
        assert!(juplend_adjust_i128(-1, 1_000_000_000_000).is_none());
        assert!(juplend_adjust_i128(i128::MIN, 1_000_000_000_000).is_none());

        // Large positive values overflow during multiply / conversion back to i128.
        assert!(juplend_adjust_i128(i128::MAX / 5, 100_000_000_000_000).is_none());

        // Normal Switchboard values work.
        assert!(juplend_adjust_i128(1_000_000_000_000_000_000i128, 1_000_000_000_000).is_some());
    }

    #[test]
    fn integer_division_floors_correctly() {
        // 1.2x
        let token_exchange_price = 1_200_000_000_000u64;

        assert_eq!(juplend_adjust_i64(5, token_exchange_price).unwrap(), 6); // 5 * 1.2 = 6
        assert_eq!(juplend_adjust_i64(1, token_exchange_price).unwrap(), 1); // 1 * 1.2 = 1.2 -> floor
        assert_eq!(juplend_adjust_i64(4, token_exchange_price).unwrap(), 4); // 4 * 1.2 = 4.8 -> floor
    }
}
