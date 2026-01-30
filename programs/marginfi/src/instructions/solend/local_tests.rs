#[cfg(test)]
mod tests {
    use bytemuck::Zeroable;
    use fixed::types::I80F48;
    use marginfi_type_crate::types::price::{
        adjust_i128 as shared_adjust_i128, adjust_i64 as shared_adjust_i64,
        adjust_u64 as shared_adjust_u64, liq_to_col_ratio as shared_liq_to_col_ratio,
    };
    use solend_mocks::state::{
        convert_decimals, decimal_to_i80f48, CollateralExchangeRate, SolendMinimalReserve,
    };

    // WAD constant: 10^18 (Solend's fixed-point precision)
    const WAD: u128 = 1_000_000_000_000_000_000;
    const I80F48_FRAC_BITS: u32 = 48;
    const I80F48_SCALE_U128: u128 = 1u128 << I80F48_FRAC_BITS;
    const N: i128 = 1i128 << 48; // 2^48 for I80F48 scaling

    /// Compute the liquidity/collateral ratio in I80F48 bit representation
    fn ratio_bits(reserve: &SolendMinimalReserve) -> i128 {
        let (total_liq, total_col) = reserve.scaled_supplies().unwrap();

        if total_col == I80F48::ZERO {
            return N; // 1:1 ratio
        }

        let liq_bits = total_liq.to_bits();
        let col_bits = total_col.to_bits();
        (liq_bits * N) / col_bits
    }

    fn liq_col_ratio(reserve: &SolendMinimalReserve) -> Option<I80F48> {
        let (total_liq, total_col) = reserve.scaled_supplies().unwrap();
        shared_liq_to_col_ratio(total_liq, total_col)
    }

    /// Find the largest u64 raw value that won't overflow when adjusted
    fn largest_safe_raw_for_u64_exact(reserve: &SolendMinimalReserve) -> u64 {
        let rb = ratio_bits(reserve);
        if rb <= 0 {
            return u64::MAX;
        }
        let t = u64::MAX as i128;
        let safe = (((t + 1) * N - 1) / rb) as i128;
        if safe < 0 {
            0
        } else if safe > u64::MAX as i128 {
            u64::MAX
        } else {
            safe as u64
        }
    }

    fn overflow_raw_for_u64_exact(reserve: &SolendMinimalReserve) -> u64 {
        largest_safe_raw_for_u64_exact(reserve).saturating_add(1)
    }

    /// Find the largest i64 raw value that won't overflow when adjusted
    fn largest_safe_raw_for_i64_exact(reserve: &SolendMinimalReserve) -> i64 {
        let rb = ratio_bits(reserve);
        if rb <= 0 {
            return i64::MAX;
        }
        let t = i64::MAX as i128;
        let safe = (((t + 1) * N - 1) / rb) as i128;
        if safe < i64::MIN as i128 {
            i64::MIN
        } else if safe > i64::MAX as i128 {
            i64::MAX
        } else {
            safe as i64
        }
    }

    fn overflow_raw_for_i64_exact(reserve: &SolendMinimalReserve) -> i64 {
        largest_safe_raw_for_i64_exact(reserve).saturating_add(1)
    }

    fn solend_reserve(
        decimals: u8,
        available: u64,
        borrowed_wads: u128,
        fees_wads: u128,
        collateral_supply: u64,
    ) -> SolendMinimalReserve {
        let mut reserve = SolendMinimalReserve::zeroed();
        reserve.liquidity_mint_decimals = decimals;
        reserve.liquidity_available_amount = available;
        reserve.liquidity_borrowed_amount_wads = borrowed_wads.to_le_bytes();
        reserve.liquidity_accumulated_protocol_fees_wads = fees_wads.to_le_bytes();
        reserve.collateral_mint_total_supply = collateral_supply;
        reserve
    }

    /// Simple reserve with 1:1 exchange rate (no borrowed, no fees)
    fn simple_reserve(decimals: u8, liquidity: u64, collateral: u64) -> SolendMinimalReserve {
        solend_reserve(decimals, liquidity, 0, 0, collateral)
    }

    /// Encode a fractional value as WAD bytes
    fn encode_wad_fractional(integer: u64, numerator: u64, denominator: u64) -> [u8; 16] {
        let int_part = (integer as u128) * WAD;
        let frac_part = ((numerator as u128) * WAD) / (denominator as u128);
        (int_part + frac_part).to_le_bytes()
    }

    fn i80f48_bits_from_wad(raw: u128) -> i128 {
        let int_part = raw / WAD;
        let rem = raw % WAD;
        let frac_bits = (rem * I80F48_SCALE_U128) / WAD;
        ((int_part as i128) << I80F48_FRAC_BITS) | (frac_bits as i128)
    }

    fn i80f48_bits_from_ratio(numerator: i128, denominator: i128) -> i128 {
        (numerator << I80F48_FRAC_BITS) / denominator
    }

    fn i80f48_mul_bits(a: i128, b: i128) -> i128 {
        (a * b) >> I80F48_FRAC_BITS
    }

    fn i80f48_div_bits(a: i128, b: i128) -> i128 {
        (a << I80F48_FRAC_BITS) / b
    }

    #[test]
    fn decimal_to_i80f48_conversions() {
        // Zero
        assert_eq!(decimal_to_i80f48([0u8; 16]).unwrap(), I80F48::ZERO);

        // Integer 1.0
        let fixed = decimal_to_i80f48(WAD.to_le_bytes()).unwrap();
        assert_eq!(fixed.to_num::<u64>(), 1);

        // Other integers
        for value in [1u64, 100, 1_000_000] {
            let wad_value = (value as u128) * WAD;
            let fixed = decimal_to_i80f48(wad_value.to_le_bytes()).unwrap();
            assert_eq!(fixed.to_num::<u64>(), value);
        }

        // 0.5
        let fixed = decimal_to_i80f48((WAD / 2).to_le_bytes()).unwrap();
        assert_eq!(fixed.to_bits(), 1i128 << (I80F48_FRAC_BITS - 1));

        // 5.75
        let fixed = decimal_to_i80f48(encode_wad_fractional(5, 75, 100)).unwrap();
        let expected_bits = (5i128 << I80F48_FRAC_BITS) | (3i128 << (I80F48_FRAC_BITS - 2));
        assert_eq!(fixed.to_bits(), expected_bits);

        // Max u128 succeeds
        let fixed = decimal_to_i80f48(u128::MAX.to_le_bytes()).unwrap();
        assert_eq!(fixed.to_bits(), i80f48_bits_from_wad(u128::MAX));
    }

    #[test]
    fn convert_decimals_scaling() {
        let value = I80F48::from_num(100);

        // Same decimals: no change
        assert_eq!(convert_decimals(value, 6, 6).unwrap(), value);

        // Scale up: 6 -> 9 decimals (multiply by 1000)
        assert_eq!(
            convert_decimals(value, 6, 9).unwrap(),
            I80F48::from_num(100_000)
        );

        // Scale down: 9 -> 6 decimals (divide by 1000)
        let value2 = I80F48::from_num(100_000);
        assert_eq!(
            convert_decimals(value2, 9, 6).unwrap(),
            I80F48::from_num(100)
        );

        // Fractional result
        let result = convert_decimals(I80F48::from_num(1), 9, 6).unwrap();
        let expected = I80F48::from_bits(i80f48_bits_from_ratio(1, 1_000));
        assert_eq!(result, expected);
    }

    #[test]
    fn total_liquidity_calculation() {
        // Available only
        let reserve = simple_reserve(6, 1_000_000, 1_000_000);
        assert_eq!(
            reserve.calculate_total_liquidity().unwrap().to_num::<u64>(),
            1_000_000
        );

        // Available + borrowed
        let reserve = solend_reserve(6, 500_000, 500_000 * WAD, 0, 1_000_000);
        assert_eq!(
            reserve.calculate_total_liquidity().unwrap().to_num::<u64>(),
            1_000_000
        );

        // Available - fees
        let reserve = solend_reserve(6, 1_000, 0, 100 * WAD, 1_000);
        assert_eq!(
            reserve.calculate_total_liquidity().unwrap().to_num::<u64>(),
            900
        );

        // All components: available + borrowed - fees
        let reserve = solend_reserve(6, 1_000, 500 * WAD, 100 * WAD, 1_400);
        assert_eq!(
            reserve.calculate_total_liquidity().unwrap().to_num::<u64>(),
            1_400
        );

        // Fractional borrowed amount (123.456789)
        let borrowed_wads = encode_wad_fractional(123, 456789, 1_000_000);
        let borrowed_u128 = u128::from_le_bytes(borrowed_wads);
        let reserve = solend_reserve(6, 1_000, borrowed_u128, 0, 1_000);
        let total = reserve.calculate_total_liquidity().unwrap();
        let borrowed_bits = i80f48_bits_from_wad(borrowed_u128);
        let expected_bits = (1_000i128 << I80F48_FRAC_BITS) + borrowed_bits;
        assert_eq!(total.to_bits(), expected_bits);
    }

    #[test]
    fn collateral_to_liquidity_common_rates() {
        // 1:1 ratio
        let reserve = simple_reserve(6, 1_000_000, 1_000_000);
        assert_eq!(reserve.collateral_to_liquidity(100).unwrap(), 100);

        // 2:1 ratio (collateral worth 2x liquidity)
        let reserve = simple_reserve(6, 2_000_000, 1_000_000);
        assert_eq!(reserve.collateral_to_liquidity(100).unwrap(), 200);

        // 1:2 ratio (collateral worth 0.5x liquidity)
        let reserve = simple_reserve(6, 1_000_000, 2_000_000);
        assert_eq!(reserve.collateral_to_liquidity(100).unwrap(), 50);
    }

    #[test]
    fn collateral_to_liquidity_edge_cases() {
        // Zero collateral supply -> error
        let reserve = simple_reserve(6, 1_000_000, 0);
        assert!(reserve.collateral_to_liquidity(100).is_err());

        // Zero input -> returns 0
        let reserve = simple_reserve(6, 1_000_000, 1_000_000);
        assert_eq!(reserve.collateral_to_liquidity(0).unwrap(), 0);

        // Zero liquidity -> returns 0
        let reserve = simple_reserve(6, 0, 1_000_000);
        assert_eq!(reserve.collateral_to_liquidity(100).unwrap(), 0);
    }

    #[test]
    fn collateral_to_liquidity_with_accrued_interest() {
        // available=1000, borrowed=500 -> total=1500, collateral=1000, rate=1.5
        let reserve = solend_reserve(6, 1_000_000, 500_000 * WAD, 0, 1_000_000);
        assert_eq!(reserve.collateral_to_liquidity(100).unwrap(), 150);
    }

    #[test]
    fn liquidity_to_collateral_common_rates() {
        // 1:1 ratio
        let reserve = simple_reserve(6, 1_000_000, 1_000_000);
        assert_eq!(reserve.liquidity_to_collateral(100).unwrap(), 100);

        // 2:1 ratio
        let reserve = simple_reserve(6, 2_000_000, 1_000_000);
        assert_eq!(reserve.liquidity_to_collateral(200).unwrap(), 100);

        // 1:2 ratio
        let reserve = simple_reserve(6, 1_000_000, 2_000_000);
        assert_eq!(reserve.liquidity_to_collateral(100).unwrap(), 200);
    }

    #[test]
    fn liquidity_to_collateral_zero_collateral_supply() {
        // Zero collateral supply -> returns 0
        let reserve = simple_reserve(6, 1_000_000, 0);
        assert_eq!(reserve.liquidity_to_collateral(100).unwrap(), 0);

        // Zero liquidity supply -> error
        let reserve = simple_reserve(6, 0, 1_000_000);
        assert!(reserve.liquidity_to_collateral(100).is_err());
    }

    #[test]
    fn round_trip_never_increases() {
        fn assert_col_liq_round_trip(reserve: &SolendMinimalReserve, amount: u64) {
            let to_liq = reserve.collateral_to_liquidity(amount).unwrap();
            let back = reserve.liquidity_to_collateral(to_liq).unwrap();
            assert!(
                back <= amount,
                "col->liq->col {} -> {} -> {} should not increase",
                amount,
                to_liq,
                back
            );
        }

        fn assert_liq_col_round_trip(reserve: &SolendMinimalReserve, amount: u64) {
            let to_col = reserve.liquidity_to_collateral(amount).unwrap();
            let back = reserve.collateral_to_liquidity(to_col).unwrap();
            assert!(
                back <= amount,
                "liq->col->liq {} -> {} -> {} should not increase",
                amount,
                to_col,
                back
            );
        }

        let test_amounts = [10u64, 100, 1_000, 10_000, 100_000];
        let reserves = [
            simple_reserve(6, 1_000_000, 1_000_000), // 1:1
            simple_reserve(6, 2_000_000, 1_000_000), // 2:1
            simple_reserve(6, 1_000_000, 2_000_000), // 1:2
            simple_reserve(6, 1_500_000, 1_000_000), // 1.5:1
        ];

        for reserve in &reserves {
            for &amount in &test_amounts {
                assert_col_liq_round_trip(reserve, amount);
                assert_liq_col_round_trip(reserve, amount);
            }
        }
    }

    #[test]
    fn adjust_oracle_price() {
        // 1:1 ratio
        let reserve = simple_reserve(6, 1_000_000, 1_000_000);
        let ratio = liq_col_ratio(&reserve).unwrap();
        assert_eq!(
            shared_adjust_i64(1_000_000i64, ratio).unwrap(),
            1_000_000i64
        );

        // 2:1 ratio
        let reserve = simple_reserve(6, 2_000_000, 1_000_000);
        let ratio = liq_col_ratio(&reserve).unwrap();
        assert_eq!(
            shared_adjust_i64(1_000_000i64, ratio).unwrap(),
            2_000_000i64
        );
    }

    #[test]
    fn adjust_oracle_price_with_interest() {
        // available=1000, borrowed=200 -> total=1200, collateral=1000, rate=1.2
        let reserve = solend_reserve(6, 1_000_000, 200_000 * WAD, 0, 1_000_000);
        let price = 1_000_000i64;

        let ratio = liq_col_ratio(&reserve).unwrap();
        let adjusted = shared_adjust_i64(price, ratio).unwrap();
        let total_liq_bits = i80f48_bits_from_ratio(1_200_000, 1_000_000);
        let total_col_bits = i80f48_bits_from_ratio(1_000_000, 1_000_000);
        let raw_bits = (price as i128) << I80F48_FRAC_BITS;
        let adjusted_bits =
            i80f48_div_bits(i80f48_mul_bits(raw_bits, total_liq_bits), total_col_bits);
        let expected = (adjusted_bits >> I80F48_FRAC_BITS) as i64;
        assert_eq!(adjusted, expected);
    }

    #[test]
    fn adjust_i128_switchboard() {
        // Switchboard uses 18 decimals
        let reserve = simple_reserve(6, 1_500_000, 1_000_000); // 1.5x rate
        let price: i128 = 1_000_000_000_000_000_000;
        assert_eq!(
            shared_adjust_i128(price, liq_col_ratio(&reserve).unwrap()).unwrap(),
            1_500_000_000_000_000_000i128
        );
    }

    #[test]
    fn adjust_oracle_zero_collateral_returns_raw() {
        let reserve = simple_reserve(6, 1_000_000, 0);
        let ratio = liq_col_ratio(&reserve);
        let price = 1_000_000i64;
        let adjusted = match ratio {
            Some(r) => shared_adjust_i64(price, r).unwrap(),
            None => price,
        };
        assert_eq!(adjusted, price);
    }

    #[test]
    fn exchange_rate_struct_conversions() {
        // 2:1 liquidity:collateral ratio -> exchange rate = 0.5
        let reserve = simple_reserve(6, 2_000_000, 1_000_000);
        let rate = CollateralExchangeRate::from_reserve(&reserve).unwrap();
        let expected = I80F48::from_bits(i80f48_bits_from_ratio(1_000_000, 2_000_000));
        assert_eq!(rate.0, expected);

        assert_eq!(rate.collateral_to_liquidity(100).unwrap(), 200);
        assert_eq!(rate.liquidity_to_collateral(200).unwrap(), 100);

        // Empty reserve -> initial rate of 1.0
        let empty_reserve = simple_reserve(6, 0, 0);
        let rate = CollateralExchangeRate::from_reserve(&empty_reserve).unwrap();
        assert_eq!(rate.0, I80F48::from_num(1));
    }

    #[test]
    fn overflow_detected_on_collateral_conversions() {
        // High ratio (200:1) - collateral_to_liquidity overflows
        let reserve_high_liq = simple_reserve(6, 200_000_000, 1_000_000);
        assert!(reserve_high_liq.collateral_to_liquidity(u64::MAX).is_err());
        assert_eq!(
            reserve_high_liq.liquidity_to_collateral(u64::MAX).unwrap(),
            u64::MAX / 200
        );

        // Low ratio (1:200) - liquidity_to_collateral overflows
        let reserve_high_col = simple_reserve(6, 1_000_000, 200_000_000);
        assert!(reserve_high_col.liquidity_to_collateral(u64::MAX).is_err());
        assert_eq!(
            reserve_high_col.collateral_to_liquidity(u64::MAX).unwrap(),
            u64::MAX / 200
        );

        // 1:1 ratio - both directions work with u64::MAX
        let reserve_1_1 = simple_reserve(6, 1_000_000, 1_000_000);
        assert!(reserve_1_1.collateral_to_liquidity(u64::MAX).is_ok());
        assert!(reserve_1_1.liquidity_to_collateral(u64::MAX).is_ok());
    }

    #[test]
    fn adjust_u64_overflow_at_exact_boundary() {
        // ~200:1 ratio to trigger overflow
        let reserve = simple_reserve(6, 200_000_000, 1_000_000);
        let ratio = liq_col_ratio(&reserve).unwrap();

        let safe = largest_safe_raw_for_u64_exact(&reserve);
        assert!(
            shared_adjust_u64(safe, ratio).is_some(),
            "safe value {} should succeed",
            safe
        );

        let ovf = overflow_raw_for_u64_exact(&reserve);
        assert!(
            shared_adjust_u64(ovf, ratio).is_none(),
            "overflow value {} should fail",
            ovf
        );

        assert!(safe < u64::MAX);
    }

    #[test]
    fn adjust_i64_overflow_at_exact_boundary() {
        let reserve = simple_reserve(6, 200_000_000, 1_000_000);
        let ratio = liq_col_ratio(&reserve).unwrap();

        let safe = largest_safe_raw_for_i64_exact(&reserve);
        assert!(
            shared_adjust_i64(safe, ratio).is_some(),
            "safe value {} should succeed",
            safe
        );

        let ovf = overflow_raw_for_i64_exact(&reserve);
        assert!(
            shared_adjust_i64(ovf, ratio).is_none(),
            "overflow value {} should fail",
            ovf
        );

        assert!(safe < i64::MAX);
    }

    #[test]
    fn adjust_i128_overflow_detection() {
        let reserve = simple_reserve(6, 1_000_000, 1_000_000);
        let ratio = liq_col_ratio(&reserve).unwrap();
        let max_raw = I80F48::MAX.checked_to_num::<i128>().unwrap();
        assert!(shared_adjust_i128(max_raw, ratio).is_some());

        // High ratio reserve: large values overflow during multiply
        let reserve_200x = simple_reserve(6, 200_000_000, 1_000_000);
        let ratio = liq_col_ratio(&reserve_200x).unwrap();
        let max_before_overflow = I80F48::MAX.checked_div(I80F48::from_num(200)).unwrap();
        let raw_over = max_before_overflow
            .checked_to_num::<i128>()
            .unwrap()
            .checked_add(1)
            .unwrap();
        assert!(shared_adjust_i128(raw_over, ratio).is_none());

        // Normal Switchboard values should work
        let ratio = liq_col_ratio(&reserve).unwrap();
        assert!(shared_adjust_i128(1_000_000_000_000_000_000i128, ratio).is_some());
    }
}
