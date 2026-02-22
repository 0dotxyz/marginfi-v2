#[cfg(test)]
mod tests {
    use bytemuck::Zeroable;
    use juplend_mocks::state::{Lending, EXCHANGE_PRICES_PRECISION};
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

    fn lending_state(liquidity_exchange_price: u64, token_exchange_price: u64) -> Lending {
        let mut lending = Lending::zeroed();
        lending.liquidity_exchange_price = liquidity_exchange_price;
        lending.token_exchange_price = token_exchange_price;
        lending
    }

    /// Largest u64 assets before the resulting shares overflow u64.
    fn largest_safe_assets_for_shares(token_price: u64) -> u64 {
        let safe = (u64::MAX as u128) * token_price as u128 / EXCHANGE_PRICES_PRECISION;
        safe.min(u64::MAX as u128) as u64
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

    #[test]
    fn shares_for_deposit_matches_computed_values() {
        // Baseline: both prices at 1e12 -> assets == shares (no rounding)
        let l = lending_state(1_000_000_000_000, 1_000_000_000_000);
        assert_eq!(
            l.expected_shares_for_deposit(100_000_000).unwrap(),
            100_000_000
        );

        // Different prices: liq=1.2e12, token=1.5e12
        //   raw    = floor(100_000_000 * 1e12 / 1.2e12) = 83_333_333
        //   norm   = floor(83_333_333 * 1.2e12 / 1e12)  = 99_999_999
        //   shares = floor(99_999_999 * 1e12 / 1.5e12)  = 66_666_666
        let l = lending_state(1_200_000_000_000, 1_500_000_000_000);
        assert_eq!(
            l.expected_shares_for_deposit(100_000_000).unwrap(),
            66_666_666
        );

        let l = lending_state(1_000_000_000_000, 2_000_000_000_000);
        let shares = l.expected_shares_for_deposit(100_000_000).unwrap();
        assert_eq!(shares, 50_000_000);
        assert!(shares < 100_000_000, "protocol must not overmint");

        // Tiny deposit: floor divisions can eat the entire value
        let l = lending_state(1_200_000_000_000, 1_500_000_000_000);
        assert_eq!(l.expected_shares_for_deposit(1).unwrap(), 0);
    }

    #[test]
    fn zero_prices_return_no_shares() {
        let l = lending_state(0, 1_000_000_000_000);
        assert!(l.expected_shares_for_deposit(100).is_none());

        let l = lending_state(1_000_000_000_000, 0);
        assert!(l.expected_shares_for_deposit(100).is_none());
    }

    #[test]
    fn shares_for_deposit_overflow_at_exact_boundary() {
        // token_price < 1e12 amplifies the output (more shares than assets),
        // which is what makes overflow reachable at a meaningful boundary.
        let liq_price = 1_000_000_000_000u64;
        let token_price = 500_000_000_000u64; // 0.5x -> 2x amplification
        let l = lending_state(liq_price, token_price);

        let safe = largest_safe_assets_for_shares(token_price);
        assert!(
            l.expected_shares_for_deposit(safe).is_some(),
            "safe value {} should succeed",
            safe
        );

        // One above the safe boundary must fail
        let ovf = safe.saturating_add(1);
        if ovf > safe {
            assert!(
                l.expected_shares_for_deposit(ovf).is_none(),
                "overflow value {} should fail",
                ovf
            );
        }

        assert!(safe < u64::MAX, "boundary should be below u64::MAX");
    }

    #[test]
    fn shares_for_deposit_overflow_with_extreme_prices() {
        // Realistic prices (>= 1e12): overflow is unreachable, u64::MAX inputs succeed
        let l = lending_state(1_000_000_000_000, 1_000_000_000_000);
        assert!(l.expected_shares_for_deposit(u64::MAX).is_some());

        let l = lending_state(1_500_000_000_000, 2_000_000_000_000);
        assert!(l.expected_shares_for_deposit(u64::MAX).is_some());

        // Very high token price (100x yield): output shrinks, never overflows
        let l = lending_state(1_000_000_000_000, 100_000_000_000_000);
        let shares = l.expected_shares_for_deposit(u64::MAX).unwrap();
        assert!(shares < u64::MAX, "100x price must shrink output");

        // Unrealistic but defensive: tiny token price (0.001x) -> 1000x amplification
        // Overflow boundary is well below u64::MAX
        let token_price = 1_000_000_000u64; // 0.001x
        let l = lending_state(1_000_000_000_000, token_price);
        let safe = largest_safe_assets_for_shares(token_price);
        assert!(safe < u64::MAX / 500, "1000x amplification should have a low boundary");
        assert!(l.expected_shares_for_deposit(safe).is_some());
        assert!(l.expected_shares_for_deposit(safe + 1).is_none());

        // Both prices equal but below 1e12: intermediate rounding still keeps output safe
        let l = lending_state(500_000_000_000, 500_000_000_000);
        let safe = largest_safe_assets_for_shares(500_000_000_000);
        assert!(l.expected_shares_for_deposit(safe).is_some());
        assert!(l.expected_shares_for_deposit(safe + 1).is_none());
    }


    #[test]
    fn expected_shares_to_burn_for_withdrawal_matches_computed_values() {
        // Baseline: price at 1e12 -> shares == assets (no rounding)
        let l = lending_state(0, 1_000_000_000_000);
        assert_eq!(l.expected_shares_for_withdraw(100_000_000).unwrap(), 100_000_000);

        // 1.5x price: ceil(100_000_000 * 1e12 / 1.5e12) = ceil(66_666_666.66...) = 66_666_667
        let l = lending_state(0, 1_500_000_000_000);
        assert_eq!(l.expected_shares_for_withdraw(100_000_000).unwrap(), 66_666_667);

        // Exact division: no ceiling bump
        // ceil(150 * 1e12 / 1.5e12) = ceil(100.0) = 100
        assert_eq!(l.expected_shares_for_withdraw(150).unwrap(), 100);

        // Price below 1e12: burns more shares than assets
        // ceil(100 * 1e12 / 0.9e12) = ceil(111.111...) = 112
        let l = lending_state(0, 900_000_000_000);
        let shares = l.expected_shares_for_withdraw(100).unwrap();
        assert_eq!(shares, 112);
        assert!(shares > 100, "sub-1e12 price must burn more shares than assets");

        // Tiny withdrawal: ceil always produces at least 1 share
        let l = lending_state(0, 1_500_000_000_000);
        assert_eq!(l.expected_shares_for_withdraw(1).unwrap(), 1);

        // Zero price returns None
        let l = lending_state(0, 0);
        assert!(l.expected_shares_for_withdraw(100).is_none());

        //zero assets returns 0 shares (no-op)
        let l = lending_state(0, 1_500_000_000_000);
        assert_eq!(l.expected_shares_for_withdraw(0).unwrap(), 0);
    }

    #[test]
    fn withdraw_shares_ceil_always_gte_deposit_shares_floor() {
        // withdraw (ceil) >= deposit (floor) for the same amount.
        for &(liq_price, tok_price) in &[
            (1_000_000_000_000u64, 1_000_000_000_000u64),
            (1_200_000_000_000, 1_500_000_000_000),
            (1_000_000_000_000, 2_000_000_000_000),
            (1_500_000_000_000, 1_000_000_000_000),
            (1_000_000_000_000, 1_100_000_000_000),
        ] {
            let l = lending_state(liq_price, tok_price);
            for &amount in &[1u64, 7, 100, 1_000_000, 100_000_000, 1_000_000_000_000] {
                let deposit_shares = l.expected_shares_for_deposit(amount).unwrap();
                let withdraw_shares = l.expected_shares_for_withdraw(amount).unwrap();
                assert!(
                    withdraw_shares >= deposit_shares,
                    "withdraw_shares ({}) < deposit_shares ({}) at liq={}, tok={}, amount={}",
                    withdraw_shares,
                    deposit_shares,
                    liq_price,
                    tok_price,
                    amount
                );
            }
        }
    }

    #[test]
    fn shares_for_withdrawal_overflow_at_exact_boundary() {
        // token_price < 1e12 amplifies output, making overflow reachable.
        let token_price = 1_000_000_000u64; // 0.001x -> 1000x amplification
        let l = lending_state(0, token_price);

        let safe = largest_safe_assets_for_shares(token_price);
        assert!(
            l.expected_shares_for_withdraw(safe).is_some(),
            "safe value {} should succeed",
            safe
        );

        let ovf = safe.saturating_add(1);
        if ovf > safe {
            assert!(
                l.expected_shares_for_withdraw(ovf).is_none(),
                "overflow value {} should fail",
                ovf
            );
        }

        assert!(safe < u64::MAX, "boundary should be below u64::MAX");
    }

    #[test]
    fn round_trip_deposit_then_redeem_at_same_prices() {
        // Deposit assets -> get shares -> redeem shares -> get assets back.
        // Rounding loss should be at most 1 token unit.
        let l = lending_state(1_200_000_000_000, 1_500_000_000_000);
        let deposit_amount = 100_000_000u64;

        let shares = l.expected_shares_for_deposit(deposit_amount).unwrap();
        let redeemed = l.expected_assets_for_redeem(shares).unwrap();

        assert!(redeemed <= deposit_amount, "redeem must not exceed deposit");
        assert!(
            deposit_amount - redeemed <= 1,
            "rounding loss {} exceeds 1",
            deposit_amount - redeemed
        );
    }

    #[test]
    fn round_trip_never_increases() {
        // Across various price combos: deposit -> redeem never returns more than deposited.
        for &(liq_price, tok_price) in &[
            (1_000_000_000_000u64, 1_000_000_000_000u64),
            (1_200_000_000_000, 1_500_000_000_000),
            (1_000_000_000_000, 2_000_000_000_000),
            (1_500_000_000_000, 1_200_000_000_000),
            (1_100_000_000_000, 1_100_000_000_000),
        ] {
            let l = lending_state(liq_price, tok_price);
            for &amount in &[1u64, 7, 100, 1_000_000, 100_000_000, 1_000_000_000_000] {
                let shares = l.expected_shares_for_deposit(amount).unwrap();
                let redeemed = l.expected_assets_for_redeem(shares).unwrap();
                assert!(
                    redeemed <= amount,
                    "round-trip {} -> {} shares -> {} redeemed should not exceed original (liq={}, tok={})",
                    amount, shares, redeemed, liq_price, tok_price
                );
            }
        }
    }

    #[test]
    fn round_trip_withdraw_then_deposit_shares_never_increase() {
        // Withdraw N assets (ceil shares) -> deposit same N assets (floor shares).
        // Shares burned on withdraw >= shares minted on deposit.
        for &(liq_price, tok_price) in &[
            (1_000_000_000_000u64, 1_000_000_000_000u64),
            (1_200_000_000_000, 1_500_000_000_000),
            (1_000_000_000_000, 2_000_000_000_000),
        ] {
            let l = lending_state(liq_price, tok_price);
            for &amount in &[1u64, 7, 100, 1_000_000, 100_000_000] {
                let burned = l.expected_shares_for_withdraw(amount).unwrap();
                let minted = l.expected_shares_for_deposit(amount).unwrap();
                assert!(
                    burned >= minted,
                    "burned {} < minted {} at amount={}, liq={}, tok={}",
                    burned, minted, amount, liq_price, tok_price
                );
            }
        }
    }

    #[test]
    fn round_trip_with_interest_accrual() {
        // Deposit at 1e12, then redeem at higher price -> profit
        let deposit_l = lending_state(1_000_000_000_000, 1_000_000_000_000);
        let redeem_l = lending_state(1_000_000_000_000, 1_500_000_000_000); // 1.5x yield

        let deposit_amount = 100_000_000u64;
        let shares = deposit_l.expected_shares_for_deposit(deposit_amount).unwrap();
        let redeemed = redeem_l.expected_assets_for_redeem(shares).unwrap();

        // 100M shares * 1.5e12 / 1e12 = 150M
        assert_eq!(redeemed, 150_000_000);
        assert!(redeemed > deposit_amount, "yield should produce profit");
    }

    #[test]
    fn round_trip_near_zero_amounts() {
        let l = lending_state(1_200_000_000_000, 1_500_000_000_000);

        // amount=1: deposit floors to 0 shares, redeem of 0 shares = 0
        let shares = l.expected_shares_for_deposit(1).unwrap();
        assert_eq!(shares, 0);
        let redeemed = l.expected_assets_for_redeem(shares).unwrap();
        assert_eq!(redeemed, 0);

        // amount=2: may produce 1 share depending on prices
        let shares = l.expected_shares_for_deposit(2).unwrap();
        let redeemed = l.expected_assets_for_redeem(shares).unwrap();
        assert!(redeemed <= 2);
    }

    #[test]
    fn assets_for_redeem_at_baseline_price() {
        // price at 1e12 -> assets == shares (1:1)
        let l = lending_state(0, 1_000_000_000_000);
        assert_eq!(l.expected_assets_for_redeem(100_000_000).unwrap(), 100_000_000);
    }

    #[test]
    fn assets_for_redeem_with_yield() {
        // 1.5x price: floor(100_000_000 * 1.5e12 / 1e12) = 150_000_000
        let l = lending_state(0, 1_500_000_000_000);
        assert_eq!(l.expected_assets_for_redeem(100_000_000).unwrap(), 150_000_000);

        // 2x price: floor(100_000_000 * 2e12 / 1e12) = 200_000_000
        let l = lending_state(0, 2_000_000_000_000);
        assert_eq!(l.expected_assets_for_redeem(100_000_000).unwrap(), 200_000_000);
    }

    #[test]
    fn assets_for_redeem_rounds_down() {
        // Non-divisible: floor(7 * 1.3e12+1 / 1e12) = floor(9.1000000000007) = 9
        let l = lending_state(0, 1_300_000_000_001);
        assert_eq!(l.expected_assets_for_redeem(7).unwrap(), 9);

        // floor(5 * 1.3e12+1 / 1e12) = floor(6.500000000005) = 6
        assert_eq!(l.expected_assets_for_redeem(5).unwrap(), 6);
    }

    #[test]
    fn assets_for_redeem_tiny_shares_floor_to_zero() {
        // floor(1 * 0.5e12 / 1e12) = floor(0.5) = 0
        let l = lending_state(0, 500_000_000_000);
        assert_eq!(l.expected_assets_for_redeem(1).unwrap(), 0);

        // floor(1 * 0.999e12 / 1e12) = floor(0.999) = 0
        let l = lending_state(0, 999_000_000_000);
        assert_eq!(l.expected_assets_for_redeem(1).unwrap(), 0);
    }

    #[test]
    fn assets_for_redeem_zero_price_returns_none() {
        let l = lending_state(0, 0);
        assert!(l.expected_assets_for_redeem(100).is_none());
    }

    #[test]
    fn assets_for_redeem_zero_shares_returns_zero() {
        let l = lending_state(0, 1_500_000_000_000);
        assert_eq!(l.expected_assets_for_redeem(0).unwrap(), 0);
    }

    #[test]
    fn assets_for_redeem_overflow_at_exact_boundary() {
        // Formula: assets = floor(shares * token_price / 1e12)
        // Overflow when shares * token_price exceeds u128 or result exceeds u64.
        // With high token_price, the output is amplified.
        let token_price = 200_000_000_000_000u64; // 200x
        let l = lending_state(0, token_price);

        let safe = largest_safe_raw_for_u64_exact(token_price);
        assert!(
            l.expected_assets_for_redeem(safe).is_some(),
            "safe value {} should succeed",
            safe
        );

        let ovf = overflow_raw_for_u64_exact(token_price);
        assert!(
            l.expected_assets_for_redeem(ovf).is_none(),
            "overflow value {} should fail",
            ovf
        );

        assert!(safe < u64::MAX, "boundary should be below u64::MAX");
    }
}
