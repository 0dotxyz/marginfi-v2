#[cfg(test)]
mod tests {
    use std::{i64, u64};

    use bytemuck::Zeroable;
    use fixed::types::I80F48;
    use kamino_mocks::state::{u68f60_to_i80f48, MinimalReserve};
    use marginfi_type_crate::types::price::{adjust_i128, adjust_i64, adjust_u64};

    const FRAC_BITS_DIFF: u32 = 60 - 48;

    // Exact overflow threshold helpers
    fn ratio_bits(bank: &MinimalReserve) -> i128 {
        const N: i128 = 1i128 << 48;
        let (liq, col) = bank.scaled_supplies().unwrap();
        let liq_bits = liq.to_bits();
        let col_bits = col.to_bits();
        (liq_bits * N) / col_bits
    }

    fn largest_safe_raw_for_u64_exact(bank: &MinimalReserve) -> u64 {
        const N: i128 = 1i128 << 48;
        let rb = ratio_bits(bank);
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

    fn overflow_raw_for_u64_exact(bank: &MinimalReserve) -> u64 {
        let safe = largest_safe_raw_for_u64_exact(bank);
        safe.saturating_add(1)
    }

    fn largest_safe_raw_for_i64_exact(bank: &MinimalReserve) -> i64 {
        const N: i128 = 1i128 << 48;
        let rb = ratio_bits(bank);
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

    fn overflow_raw_for_i64_exact(bank: &MinimalReserve) -> i64 {
        let safe = largest_safe_raw_for_i64_exact(bank);
        safe.saturating_add(1)
    }

    /// Delta from I80F48 quantization when applying a ratio:
    ///   Δ = floor(raw * eps_bits / 2^48)
    /// Assumes `raw >= 0`.
    #[inline]
    fn i80f48_delta(raw: i128, eps_bits: i128) -> i128 {
        const N: i128 = 1i128 << 48; // 2^48
        (raw * eps_bits) / N
    }

    /// Compute eps_bits for a reserve fixture:
    ///   eps_bits = ratio_bits - k * 2^48
    /// where:
    ///   ratio_bits = floor( (floor(liq*N) * N) / floor(col*N) )
    ///   liq = available_amount / 10^mint_decimals
    ///   col = mint_total_supply / 10^mint_decimals
    ///   k   = integer part of (available_amount / mint_total_supply)
    #[inline]
    fn i80f48_eps_bits(available_amount: u64, mint_total_supply: u64, mint_decimals: u32) -> i128 {
        const N: i128 = 1i128 << 48;
        let q: i128 = 10i128.pow(mint_decimals);

        let liq_bits = (available_amount as i128 * N) / q; // floor(liq*N)
        let col_bits = (mint_total_supply as i128 * N) / q; // floor(col*N)
        let ratio_bits = (liq_bits * N) / col_bits; // floor((liq_bits*N)/col_bits)
        let k = (available_amount / mint_total_supply) as i128;

        ratio_bits - k * N
    }

    /// Just the fields we care about for oracle pricing and liquidity/collateral token conversion
    fn generic_reserve(supply: u64, mint_decimals: u64, mint_total_supply: u64) -> MinimalReserve {
        let mut r = MinimalReserve::zeroed();
        r.available_amount = supply;
        r.mint_total_supply = mint_total_supply;
        r.mint_decimals = mint_decimals;
        r
    }

    #[test]
    fn adjust_u64_basic_scaling_produces_expected_ratio() {
        // 10:1
        let bank = generic_reserve(10_000_000, 8, 1_000_000);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let got = adjust_u64(42, liq_to_col_ratio).unwrap();
        assert_eq!(got, 420);
    }

    #[test]
    fn adjust_i64_basic_scaling_produces_expected_ratio() {
        // 10:1
        let bank = generic_reserve(10_000_000, 8, 1_000_000);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let got = adjust_i64(42, liq_to_col_ratio).unwrap();
        assert_eq!(got, 420);
    }

    #[test]
    fn adjust_i128_basic_scaling_produces_expected_ratio() {
        let bank = generic_reserve(10_000_000, 8, 1_000_000);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        const ONE_E18: i128 = 1_000_000_000_000_000_000;
        let raw = 42 * ONE_E18;

        let got = adjust_i128(raw, liq_to_col_ratio).unwrap();

        let eps = i80f48_eps_bits(10_000_000, 1_000_000, 8);
        let k = (10_000_000u64 / 1_000_000u64) as i128; // = 10 for this fixture

        let expected = k * raw + i80f48_delta(raw, eps);
        assert_eq!(got, expected);
    }

    #[test]
    fn adjust_u64_no_overflows() {
        let bank = generic_reserve(u64::MAX, 0, u64::MAX);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let res = adjust_u64(42, liq_to_col_ratio);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 42);

        // Note: 2^15 is the largest value that wouldn't overflow (2^80 / 2^64), i.e. I80's max
        // whole component when multiplied by u64 max (the max possibly supply in practice)
        let res = adjust_u64(32768, liq_to_col_ratio);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 32768);

        // Note: More typical decimal values go longer before overflow
        let bank = generic_reserve(u64::MAX, 8, u64::MAX);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let res = adjust_u64(32_76_800_000_000, liq_to_col_ratio);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 32_76_800_000_000);
    }

    #[test]
    fn adjust_i64_no_overflows() {
        let bank = generic_reserve(u64::MAX, 0, u64::MAX);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let res = adjust_i64(42, liq_to_col_ratio);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 42);

        let res = adjust_i64(32768, liq_to_col_ratio);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 32768);

        let bank = generic_reserve(u64::MAX, 8, u64::MAX);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let res = adjust_i64(32_76_800_000_000, liq_to_col_ratio);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 32_76_800_000_000);
    }

    #[test]
    fn adjust_i128_no_overflows() {
        let bank = generic_reserve(u64::MAX, 0, u64::MAX);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let res = adjust_i128(42_000_000_000_000_000_000i128, liq_to_col_ratio);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 42_000_000_000_000_000_000i128);

        let bank = generic_reserve(u64::MAX, 8, u64::MAX);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        // Large Switchboard value with 18 decimals
        let res = adjust_i128(32_768_000_000_000_000_000_000i128, liq_to_col_ratio);
        assert!(res.is_some());
        assert_eq!(res.unwrap(), 32_768_000_000_000_000_000_000i128);
    }

    #[test]
    fn adjust_u64_overflow_detected_as_error() {
        // ratio > 1 ensures we can trigger overflow on the output cast
        let bank = generic_reserve(200_000_000, 8, 1_000_000); // ~200:1
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let safe = largest_safe_raw_for_u64_exact(&bank);
        assert!(
            adjust_u64(safe, liq_to_col_ratio).is_some(),
            "largest safe raw should succeed"
        );

        let ovf = overflow_raw_for_u64_exact(&bank);
        assert!(
            adjust_u64(ovf, liq_to_col_ratio).is_none(),
            "raw above threshold must overflow"
        );
    }

    #[test]
    fn adjust_i64_overflow_detected_as_error() {
        let bank = generic_reserve(200_000_000, 8, 1_000_000);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();

        let safe = largest_safe_raw_for_i64_exact(&bank);
        assert!(adjust_i64(safe, liq_to_col_ratio).is_some());

        let ovf = overflow_raw_for_i64_exact(&bank);
        assert!(adjust_i64(ovf, liq_to_col_ratio).is_none());
    }

    #[test]
    fn adjust_i128_overflow_detected_as_error_on_input_conversion() {
        let bank = generic_reserve(200_000_000, 8, 1_000_000);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();
        let res = adjust_i128(i128::MAX, liq_to_col_ratio);
        assert!(res.is_none()); // no panic, clean None
    }

    #[test]
    fn adjust_i128_overflow_detected_as_error_on_multiply() {
        let bank = generic_reserve(200_000_000, 8, 1_000_000);
        // Find a raw that fits into I80F48 as an input but overflows during multiply
        let rb = ratio_bits(&bank);
        const _N: i128 = 1i128 << 48;
        let max_bits = I80F48::MAX.to_bits();
        // floor(MAX / ratio) in integer domain for raw
        let safe_raw = (max_bits / rb) as i128;
        let raw = safe_raw.saturating_add(1);
        let (total_liq, total_col) = bank.scaled_supplies().unwrap();
        let liq_to_col_ratio = total_liq.checked_div(total_col).unwrap();
        let res = adjust_i128(raw, liq_to_col_ratio);
        assert!(res.is_none());
    }

    #[test]
    fn u68f60_to_i80f48_zero_input_yields_zero() {
        let bits_le = [0u8; 16];
        let fixed = u68f60_to_i80f48(bits_le);
        assert_eq!(fixed.to_bits(), 0);
        assert_eq!(fixed.to_num::<u64>(), 0);
    }

    #[test]
    fn u68f60_to_i80f48_integer_one_is_converted_correctly() {
        // U68F60 bits for exactly 1.0 is (1 << 60)
        let raw_val: u128 = 1u128 << 60;
        let bits_le = raw_val.to_le_bytes();

        let fixed = u68f60_to_i80f48(bits_le);
        let expected_bits = (raw_val >> FRAC_BITS_DIFF) as i128;
        assert_eq!(fixed.to_bits(), expected_bits);
        assert_eq!(fixed.to_num::<u64>(), 1);
    }

    #[test]
    fn u68f60_to_i80f48_fractional_value_is_truncated_and_converted() {
        // 5.75 in U68F60 is:
        //   5 << 60  plus  0.75 * 2^60  = 3/4 * 2^60 = 3 << 58
        let raw_val: u128 = (5u128 << 60) + (3u128 << 58);
        let bits_le = raw_val.to_le_bytes();

        let fixed = u68f60_to_i80f48(bits_le);
        let expected_bits = (raw_val >> FRAC_BITS_DIFF) as i128;

        assert_eq!(fixed.to_bits(), expected_bits);
        // allow a tiny rounding error here on the f64 conversion
        assert!((fixed.to_num::<f64>() - 5.75).abs() < 1e-9);
    }

    #[test]
    fn u68f60_to_i80f48_truncation_drops_low_bits() {
        // embed a known 12-bit pattern in the low bits
        let low: u128 = 0xABC; // <= 0xFFF
        let raw_val: u128 = (7u128 << 60) | low;
        let bits_le = raw_val.to_le_bytes();

        let fixed = u68f60_to_i80f48(bits_le);
        let expected_raw = raw_val >> FRAC_BITS_DIFF; // Note: no low value

        assert_eq!(fixed.to_bits() as u128, expected_raw);
        assert_eq!(fixed.to_num::<u64>(), 7); // Note: no low value here

        // verify the dropped bits are exactly the low 12
        assert_eq!(raw_val - (expected_raw << FRAC_BITS_DIFF), low);
    }

    #[test]
    fn u68f60_to_i80f48_max_input_no_wrap() {
        // all-ones input
        let raw_val = u128::MAX;
        let bits_le = raw_val.to_le_bytes();

        let fixed = u68f60_to_i80f48(bits_le);
        let expected_bits = (raw_val >> FRAC_BITS_DIFF) as i128;

        // Note: 2 ^ 116 is the largest number we can get (2^128 - 12 bits)
        assert_eq!(fixed.to_bits(), expected_bits);
        // sanity: should not have wrapped into a negative i128
        assert!(fixed.to_bits() >= 0, "wrapped into negative!");
    }

    #[test]
    fn collateral_to_liquidity_simple_ratio_round_trip() {
        // 1000 / 200 = 5
        let r = generic_reserve(1000, 6, 200);

        // 10 collateral -> 10 * 5 = 50 liquidity
        let liq = r.collateral_to_liquidity(10).unwrap();
        assert_eq!(liq, 50);

        // and back: 50 liquidity -> 50 * (200/1000) = 50 * 0.2 = 10 collateral
        let col = r.liquidity_to_collateral(50).unwrap();
        assert_eq!(col, 10);
    }

    #[test]
    fn collateral_to_liquidity_large_inputs() {
        let r = generic_reserve(1000, 6, 200);
        // cannot unwrap into u64 again because u64::MAX * 5 > u64::MAX
        let liq = r.collateral_to_liquidity(u64::MAX);
        assert!(liq.is_err());
        let col = r.liquidity_to_collateral(u64::MAX).unwrap();
        assert_eq!(col, u64::MAX / 5);

        let r = generic_reserve(200, 6, 1000);
        let liq = r.collateral_to_liquidity(u64::MAX).unwrap();
        assert_eq!(liq, u64::MAX / 5);
        let col = r.liquidity_to_collateral(u64::MAX);
        assert!(col.is_err());

        // No overflow if the ratio is the same
        let r = generic_reserve(200, 0, 200);
        let liq = r.collateral_to_liquidity(u64::MAX);
        assert!(liq.is_ok());
        assert_eq!(liq.unwrap(), u64::MAX);
        let col = r.liquidity_to_collateral(u64::MAX);
        assert!(col.is_ok());
        assert_eq!(col.unwrap(), u64::MAX);
    }

    #[test]
    fn collateral_to_liquidity_fractional_truncation() {
        // supply=3, mint_total_supply=2 -> ratio = 3/2 = 1.5
        let r = generic_reserve(300, 6, 200);

        // floor(1 * 3/2) = 1
        assert_eq!(r.collateral_to_liquidity(1).unwrap(), 1);
        // floor(1 * 2/3) = 0
        assert_eq!(r.liquidity_to_collateral(1).unwrap(), 0);

        // a slightly larger collateral: floor(5 * 3/2) = floor(7.5) = 7
        assert_eq!(r.collateral_to_liquidity(5).unwrap(), 7);
        // floor(7 * 2/3) = 4
        assert_eq!(r.liquidity_to_collateral(7).unwrap(), 4);
    }

    // Note: Conversion in both directions rounds down, which leads to the phenonoma demonstrated in
    // the next tests,  where repeated conversions will always result in a value that's equal or
    // smaller than the initial value.
    #[test]
    fn collateral_to_liquidity_round_trip_never_increases() {
        for &(supply, mint) in &[
            (7, 14),
            (14, 7),
            (5, 5),
            (5_000_000, 5_000_000),
            (75_000_000, 55_000_000),
            (125_000_000, 555_000_000),
        ] {
            let r = generic_reserve(supply, 6, mint);
            for coll in &[
                1,
                3,
                10,
                50,
                50_000_000,
                225_555_555_555,
                225_555_555_555_777_777,
            ] {
                let liq = r.collateral_to_liquidity(*coll).unwrap();
                let back = r.liquidity_to_collateral(liq).unwrap();
                assert!(
                    back <= *coll,
                    "round-trip {} -> {} -> {} should not exceed original",
                    coll,
                    liq,
                    back
                );
            }
        }
    }

    #[test]
    fn liquidity_to_collateral_round_trip_never_increases() {
        for &(supply, mint) in &[
            (7, 14),
            (14, 7),
            (5, 5),
            (5_000_000, 5_000_000),
            (75_000_000, 55_000_000),
            (125_000_000, 555_000_000),
        ] {
            let r = generic_reserve(supply, 6, mint);
            for liq in &[
                1,
                3,
                10,
                50,
                50_000_000,
                225_555_555_555,
                225_555_555_555_777_777,
            ] {
                let col = r.liquidity_to_collateral(*liq).unwrap();
                let back_liq = r.collateral_to_liquidity(col).unwrap();
                assert!(
                    back_liq <= *liq,
                    "round-trip {} -> {} -> {} should not exceed original",
                    liq,
                    col,
                    back_liq
                );
            }
        }
    }

    #[test]
    fn collateral_to_liquidity_error_on_zero_mint_supply() {
        // mint_total_supply = 0 => scaled_supplies divides by zero => KaminoReserveZeroMintSupply
        let r = generic_reserve(100, 0, 0);
        assert!(r.collateral_to_liquidity(1).is_err());
        let r = generic_reserve(0, 0, 100);
        assert!(r.liquidity_to_collateral(1).is_err());
    }

    #[test]
    fn collateral_to_liquidity_non_6_mint_decimals_round_trip() {
        // Note: Like the above these, most of these conversions end up rounded down!

        // supply=10000 lamports, mint_decimals=8, mint_total_supply=200 lamports ⇒ ratio = 10000/2000 = 5
        let r = generic_reserve(10000, 8, 2000);
        // 10000 collateral lamports → 10000 * 5 = 50000 liquidity lamports
        let liq = r.collateral_to_liquidity(10000).unwrap();
        assert!(50000 - liq < 2);
        // back: 50000 liquidity → 50000 * (200/1000) = 10000 collateral
        let col = r.liquidity_to_collateral(50000).unwrap();
        assert!(10000 - col < 2);

        // supply=50000 lamports, mint_decimals=4, mint_total_supply=250 lamports ⇒ ratio = 50000/2500 = 20
        let r = generic_reserve(50000, 4, 2500);
        // 200 collateral → 200 * 20 = 4000 liquidity
        let liq = r.collateral_to_liquidity(200).unwrap();
        assert!(4000 - liq < 2);
        // back: 4000 liquidity → 4000 * (2500/50000) = 200 collateral
        let col = r.liquidity_to_collateral(4000).unwrap();
        assert!(200 - col < 2);

        // supply=2500 lamports, mint_decimals=10, mint_total_supply=250 lamports ⇒ ratio = 2500/50000 = 0.05
        let r = generic_reserve(2500, 10, 50000);
        // 50_000_000 collateral → 50_000_000 * 0.05 = 2_500_000 liquidity
        let liq = r.collateral_to_liquidity(50_000_000).unwrap();
        assert!(2_500_000 - liq < 2);
        // back: 2_500_000 liquidity → 2_500_000 * (50000/2500) = 50_000_000 collateral
        let col = r.liquidity_to_collateral(2_500_000).unwrap();
        assert!(50_000_000 - col < 2);
    }
}
