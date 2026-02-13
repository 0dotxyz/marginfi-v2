use anchor_lang::prelude::*;

use crate::constants::{FOUR_DECIMALS, TWELVE_DECIMALS};
use crate::{errors::ErrorCodes, events::*, state::*};

use library::math::{casting::*, safe_math::*};

/// Interest rate model data
#[account(zero_copy)]
#[derive(InitSpace, Default)]
#[repr(C, packed)]
pub struct RateModel {
    pub mint: Pubkey, // The token mint this applies to
    pub version: u8,  // Rate model version (1 or 2)

    // For both rate v1 and v2
    pub rate_at_zero: u16, // Rate at utilization 0% (in 1e2: 100% = 10_000; 1% = 100 -> max value 65_535)
    pub kink1_utilization: u16, // Utilization at kink1 (in 1e2: 100% = 10_000; 1% = 100 -> max value 65_535)
    pub rate_at_kink1: u16, // Rate at utilization kink1 (in 1e2: 100% = 10_000; 1% = 100 -> max value 65_535)
    pub rate_at_max: u16,   // Rate at 100% utilization

    // Additional fields for rate v2
    pub kink2_utilization: u16, // Utilization at kink2 (in 1e2: 100% = 10_000; 1% = 100 -> max value 65_535)
    pub rate_at_kink2: u16, // Rate at utilization kink2 (in 1e2: 100% = 10_000; 1% = 100 -> max value 65_535)
}

pub const MAX_RATE: u16 = 65535;

impl RateModel {
    pub fn init(&mut self, mint: Pubkey) -> Result<()> {
        self.mint = mint;
        Ok(())
    }

    pub fn set_rate_v1(&mut self, rate_data: RateDataV1Params) -> Result<()> {
        // kink must not be 0 or >= 100% (being 0 or 100% would lead to division through 0 at calculation time)
        if rate_data.kink == 0 ||
                rate_data.kink >= FOUR_DECIMALS||
                // for the last part of rate curve a spike increase must be present as utilization grows.
                // declining rate is supported before kink. kink to max must be increasing.
                // @dev Note rates can be equal, that leads to a 0 slope which is supported in calculation code.
                rate_data.rate_at_utilization_kink > rate_data.rate_at_utilization_max
        {
            return Err(ErrorCodes::InvalidParams.into());
        }

        self.version = 1;
        self.rate_at_zero = rate_data.rate_at_utilization_zero.cast()?;
        self.kink1_utilization = rate_data.kink.cast()?;
        self.rate_at_kink1 = rate_data.rate_at_utilization_kink.cast()?;
        self.rate_at_max = rate_data.rate_at_utilization_max.cast()?;

        Ok(())
    }

    pub fn set_rate_v2(&mut self, rate_data: RateDataV2Params) -> Result<()> {
        if
        // kink can not be 0, >= 100% or >= kink2 (would lead to division through 0 at calculation time)
        rate_data.kink1 == 0 ||
                rate_data.kink1 >= FOUR_DECIMALS ||
                rate_data.kink1 >= rate_data.kink2 ||
                // kink2 can not be >= 100% (must be > kink1 already checked)
                rate_data.kink2 >= FOUR_DECIMALS ||
                // for the last part of rate curve a spike increase must be present as utilization grows.
                // declining rate is supported before kink2. kink2 to max must be increasing.
                // @dev Note rates can be equal, that leads to a 0 slope which is supported in calculation code.
                rate_data.rate_at_utilization_kink2 > rate_data.rate_at_utilization_max
        {
            return Err(ErrorCodes::InvalidParams.into());
        }

        self.version = 2;
        self.rate_at_zero = rate_data.rate_at_utilization_zero.cast()?;
        self.kink1_utilization = rate_data.kink1.cast()?;
        self.rate_at_kink1 = rate_data.rate_at_utilization_kink1.cast()?;
        self.kink2_utilization = rate_data.kink2.cast()?;
        self.rate_at_kink2 = rate_data.rate_at_utilization_kink2.cast()?;
        self.rate_at_max = rate_data.rate_at_utilization_max.cast()?;

        Ok(())
    }

    // Calculates borrow rate from utilization for a token
    pub fn calc_borrow_rate_from_utilization(&self, utilization: u128) -> Result<u16> {
        let rate: u128 = if self.version == 1 {
            self.calc_rate_v1(utilization)?
        } else if self.version == 2 {
            self.calc_rate_v2(utilization)?
        } else {
            return Err(ErrorCodes::UnsupportedRateVersion.into());
        };

        // Hard cap for borrow rate at maximum value 16 bits (65535)
        if rate > MAX_RATE.cast()? {
            emit!(LogBorrowRateCap { token: self.mint });
            return Ok(MAX_RATE);
        }

        Ok(rate.cast()?)
    }

    // Calculates the borrow rate based on utilization for rate data version 1 (with one kink)
    fn calc_rate_v1(&self, utilization: u128) -> Result<u128> {
        // y = mx + c
        // y is borrow rate
        // x is utilization
        // m = slope
        // c is constant

        let y1: u128;
        let y2: u128;
        let x1: u128;
        let x2: u128;

        // Extract kink1
        let kink1 = self.kink1_utilization.cast()?;

        if utilization < kink1 {
            // If utilization is less than kink1
            y1 = self.rate_at_zero.cast()?;
            y2 = self.rate_at_kink1.cast()?;
            x1 = 0; // 0%
            x2 = kink1;
        } else {
            // If utilization is greater than or equal to kink1
            y1 = self.rate_at_kink1.cast()?;
            y2 = self.rate_at_max.cast()?;
            x1 = kink1;
            x2 = FOUR_DECIMALS; // 100%
        }

        Ok(get_rate(y1, y2, x1, x2, utilization)?)
    }

    // Calculates the borrow rate based on utilization for rate data version 2 (with two kinks)
    fn calc_rate_v2(&self, utilization: u128) -> Result<u128> {
        // y = mx + c
        // y is borrow rate
        // x is utilization
        // m = slope
        // c is constant

        let y1: u128;
        let y2: u128;
        let x1: u128;
        let x2: u128;

        // Extract kink1 and kink2
        let kink1 = self.kink1_utilization.cast()?;
        let kink2 = self.kink2_utilization.cast()?;

        if utilization < kink1 {
            // If utilization is less than kink1
            y1 = self.rate_at_zero.cast()?;
            y2 = self.rate_at_kink1.cast()?;
            x1 = 0; // 0%
            x2 = kink1;
        } else if utilization < kink2 {
            // If utilization is between kink1 and kink2
            y1 = self.rate_at_kink1.cast()?;
            y2 = self.rate_at_kink2.cast()?;
            x1 = kink1;
            x2 = kink2;
        } else {
            // If utilization is greater than or equal to kink2
            y1 = self.rate_at_kink2.cast()?;
            y2 = self.rate_at_max.cast()?;
            x1 = kink2;
            x2 = FOUR_DECIMALS; // 100%
        }

        Ok(get_rate(y1, y2, x1, x2, utilization)?)
    }
}

fn get_rate(y1: u128, y2: u128, x1: u128, x2: u128, utilization: u128) -> Result<u128> {
    // Calculate slope with twelve decimal precision
    // m = (y2 - y1) / (x2 - x1)

    // calculating slope with twelve decimal precision. m = (y2 - y1) / (x2 - x1).
    // utilization of x2 can not be <= utilization of x1 (so no underflow or 0 divisor)
    // y is in 1e2 so can not overflow when multiplied with TWELVE_DECIMALS
    let num: i128 = y2
        .cast::<i128>()?
        .safe_sub(y1.cast()?)?
        .safe_mul(TWELVE_DECIMALS.cast()?)?;

    let den: i128 = x2.safe_sub(x1)?.cast()?;
    let slope: i128 = num.safe_div(den)?;

    // calculating constant at 12 decimal precision. slope is already in 12 decimal hence only multiple with y1. c = y - mx.
    // maximum y1_ value is 65535. 65535 * 1e12 can not overflow int128
    // maximum slope is 65535 - 0 * TWELVE_DECIMALS / 1 = 65535 * 1e12;
    // maximum x1_ is 100% (9_999 actually) => slope_ * x1_ can not overflow int128
    // subtraction most extreme case would be  0 - max value slope_ * x1_ => can not underflow int128
    let constant: i128 = y1
        .cast::<i128>()?
        .safe_mul(TWELVE_DECIMALS.cast()?)?
        .safe_sub(slope.safe_mul(x1.cast()?)?)?;

    // calculating new borrow rate
    // - slope_ max value is 65535 * 1e12,
    // - utilization max value is let's say 500% (extreme case where borrow rate increases borrow amount without new supply)
    // - constant max value is 65535 * 1e12
    // so max values are 65535 * 1e12 * 50_000 + 65535 * 1e12 -> 3.2768*10^21, which easily fits int128
    // divisor TWELVE_DECIMALS can not be 0
    let rate: i128 = slope
        .safe_mul(utilization.cast()?)?
        .safe_add(constant)?
        .safe_div(TWELVE_DECIMALS.cast()?)?;

    // Rate should not be negative
    if rate < 0 {
        return Err(ErrorCodes::BorrowRateNegative.into());
    }

    Ok(rate.cast()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a mock mint pubkey
    fn mock_mint() -> Pubkey {
        Pubkey::new_unique()
    }

    // Helper function to create RateDataV1Params
    fn create_rate_v1_params() -> RateDataV1Params {
        RateDataV1Params {
            kink: 9000,                     // 90%
            rate_at_utilization_zero: 0,    // 0%
            rate_at_utilization_kink: 300,  // 3%
            rate_at_utilization_max: 10000, // 100%
        }
    }

    // Helper function to create RateDataV2Params
    fn create_rate_v2_params() -> RateDataV2Params {
        RateDataV2Params {
            kink1: 8500,                    // 85%
            kink2: 9300,                    // 93%
            rate_at_utilization_zero: 0,    // 0%
            rate_at_utilization_kink1: 600, // 6%
            rate_at_utilization_kink2: 800, // 8%
            rate_at_utilization_max: 10000, // 100%
        }
    }

    #[test]
    fn test_rate_model_init() {
        let mut rate_model = RateModel::default();
        let mint = mock_mint();

        let result = rate_model.init(mint);
        assert!(result.is_ok());
        assert_eq!(rate_model.mint, mint);
    }

    // V1 Rate Calculation Tests
    #[test]
    fn test_calc_borrow_rate_v1_at_zero_utilization() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        let rate = rate_model.calc_borrow_rate_from_utilization(0).unwrap();
        assert_eq!(rate, 0); // Should be 0% at 0% utilization
    }

    #[test]
    fn test_calc_borrow_rate_v1_at_kink() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        let rate = rate_model.calc_borrow_rate_from_utilization(9000).unwrap(); // 90% utilization
        assert_eq!(rate, 300); // Should be 3% at kink
    }

    #[test]
    fn test_calc_borrow_rate_v1_at_max_utilization() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        let rate = rate_model
            .calc_borrow_rate_from_utilization(FOUR_DECIMALS)
            .unwrap(); // 100% utilization
        assert_eq!(rate, 10000); // Should be 100% at max utilization
    }

    #[test]
    fn test_calc_borrow_rate_v1_before_kink() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        // Test at 45% utilization (halfway to kink)
        let rate = rate_model.calc_borrow_rate_from_utilization(4500).unwrap();
        assert_eq!(rate, 149); // @dev Should be 1.5%, but due to precision loss, it's 1.49% (halfway between 0% and 3%)
    }

    #[test]
    fn test_calc_borrow_rate_v1_before_kink_custom() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        // Test at 45% utilization (halfway to kink)
        let rate = rate_model.calc_borrow_rate_from_utilization(577).unwrap();
        assert_eq!(rate, 19); // Should be .19%
    }

    #[test]
    fn test_calc_borrow_rate_v1_after_kink() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        // Test at 95% utilization (halfway between kink and max)
        let rate = rate_model.calc_borrow_rate_from_utilization(9500).unwrap();
        assert_eq!(rate, 5150); // Should be 51.5% (halfway between 3% and 100%)
    }

    // V2 Rate Calculation Tests
    #[test]
    fn test_calc_borrow_rate_v2_at_zero_utilization() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        let rate = rate_model.calc_borrow_rate_from_utilization(0).unwrap();
        assert_eq!(rate, 0); // Should be 0% at 0% utilization
    }

    #[test]
    fn test_calc_borrow_rate_v2_at_kink1() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        let rate = rate_model.calc_borrow_rate_from_utilization(8500).unwrap(); // 85% utilization
        assert_eq!(rate, 600); // Should be 6% at kink1
    }

    #[test]
    fn test_calc_borrow_rate_v2_at_kink2() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        let rate = rate_model.calc_borrow_rate_from_utilization(9300).unwrap(); // 93% utilization
        assert_eq!(rate, 800); // Should be 8% at kink2
    }

    #[test]
    fn test_calc_borrow_rate_v2_at_max_utilization() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        let rate = rate_model
            .calc_borrow_rate_from_utilization(FOUR_DECIMALS)
            .unwrap(); // 100% utilization
        assert_eq!(rate, 9999); // @dev Should be 100% at max utilization, but due to precision loss, it's 99.99%
    }

    #[test]
    fn test_calc_borrow_rate_v2_before_kink1() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        // Test at 42.5% utilization (halfway to kink1)
        let rate = rate_model.calc_borrow_rate_from_utilization(4250).unwrap();
        assert_eq!(rate, 299); // @dev Should be 3%, but due to precision loss, it's 2.99% (halfway between 0% and 6%)
    }

    #[test]
    fn test_calc_borrow_rate_v2_between_kinks() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        // Test at 89% utilization (halfway between kink1 and kink2)
        let rate = rate_model.calc_borrow_rate_from_utilization(8900).unwrap();
        assert_eq!(rate, 700); // Should be 7% (halfway between 6% and 8%)
    }

    #[test]
    fn test_calc_borrow_rate_v2_after_kink2() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        // Test at 96.5% utilization (halfway between kink2 and max)
        let rate = rate_model.calc_borrow_rate_from_utilization(9650).unwrap();
        assert_eq!(rate, 5399); // @dev Should be 54%, but due to precision loss, it's 53.99% (halfway between 8% and 100%)
    }

    // Error Cases for V1
    #[test]
    fn test_set_rate_v1_kink_zero() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v1_params();
        rate_data.kink = 0;

        let result = rate_model.set_rate_v1(rate_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_rate_v1_kink_at_max() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v1_params();
        rate_data.kink = FOUR_DECIMALS; // 100%

        let result = rate_model.set_rate_v1(rate_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_rate_v1_declining_rate_after_kink() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v1_params();
        rate_data.rate_at_utilization_kink = 11000; // Higher than max rate
        rate_data.rate_at_utilization_max = 10000;

        let result = rate_model.set_rate_v1(rate_data);
        assert!(result.is_err());
    }

    // Error Cases for V2
    #[test]
    fn test_set_rate_v2_kink1_zero() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v2_params();
        rate_data.kink1 = 0;

        let result = rate_model.set_rate_v2(rate_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_rate_v2_kink1_at_max() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v2_params();
        rate_data.kink1 = FOUR_DECIMALS; // 100%

        let result = rate_model.set_rate_v2(rate_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_rate_v2_kink2_at_max() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v2_params();
        rate_data.kink2 = FOUR_DECIMALS; // 100%

        let result = rate_model.set_rate_v2(rate_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_rate_v2_kink1_greater_than_kink2() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v2_params();
        rate_data.kink1 = 9500; // Greater than kink2
        rate_data.kink2 = 9300;

        let result = rate_model.set_rate_v2(rate_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_rate_v2_declining_rate_after_kink2() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v2_params();
        rate_data.rate_at_utilization_kink2 = 11000; // Higher than max rate
        rate_data.rate_at_utilization_max = 10000;

        let result = rate_model.set_rate_v2(rate_data);
        assert!(result.is_err());
    }

    // Rate Cap Tests
    #[test]
    fn test_rate_cap_v1() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v1_params();
        // Set extreme rates that would exceed MAX_RATE
        rate_data.rate_at_utilization_max = MAX_RATE as u128;
        rate_model.set_rate_v1(rate_data).unwrap();

        // Test with very high utilization (200%)
        let rate = rate_model.calc_borrow_rate_from_utilization(20000).unwrap();
        assert_eq!(rate, MAX_RATE); // Should be capped at MAX_RATE
    }

    #[test]
    fn test_rate_cap_v2() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v2_params();
        // Set extreme rates that would exceed MAX_RATE
        rate_data.rate_at_utilization_max = MAX_RATE as u128;
        rate_model.set_rate_v2(rate_data).unwrap();

        // Test with very high utilization (200%)
        let rate = rate_model.calc_borrow_rate_from_utilization(20000).unwrap();
        assert_eq!(rate, MAX_RATE); // Should be capped at MAX_RATE
    }

    // Unsupported Version Test
    #[test]
    fn test_unsupported_rate_version() {
        let mut rate_model = RateModel::default();
        rate_model.version = 3; // Unsupported version

        let result = rate_model.calc_borrow_rate_from_utilization(5000);
        assert!(result.is_err());
    }

    // Edge Case: Equal Rates (Zero Slope)
    #[test]
    fn test_v1_equal_rates_zero_slope() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v1_params();
        rate_data.rate_at_utilization_kink = 300;
        rate_data.rate_at_utilization_max = 300; // Same as kink rate

        let result = rate_model.set_rate_v1(rate_data);
        assert!(result.is_ok());

        // Rate should remain constant after kink
        let rate_before_kink = rate_model.calc_borrow_rate_from_utilization(8000).unwrap();
        let rate_after_kink = rate_model.calc_borrow_rate_from_utilization(9500).unwrap();
        assert!(rate_after_kink >= rate_before_kink);
    }

    #[test]
    fn test_v2_equal_rates_zero_slope() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v2_params();
        rate_data.rate_at_utilization_kink2 = 800;
        rate_data.rate_at_utilization_max = 800; // Same as kink2 rate

        let result = rate_model.set_rate_v2(rate_data);
        assert!(result.is_ok());

        // Rate should remain constant after kink2
        let rate_at_kink2 = rate_model.calc_borrow_rate_from_utilization(9300).unwrap();
        let rate_after_kink2 = rate_model.calc_borrow_rate_from_utilization(9800).unwrap();
        assert_eq!(rate_at_kink2, rate_after_kink2);
    }

    // Precision Tests
    #[test]
    fn test_v1_rate_precision() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        // Test at exactly 1% utilization
        let rate = rate_model.calc_borrow_rate_from_utilization(100).unwrap();
        // With linear interpolation from 0% to 90%: rate should be (300 * 100) / 9000 = 3.33...
        // Rounded down should be 3
        assert!(rate <= 4 && rate >= 3);
    }

    #[test]
    fn test_v2_rate_precision() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        // Test at exactly 1% utilization
        let rate = rate_model.calc_borrow_rate_from_utilization(100).unwrap();
        // With linear interpolation from 0% to 85%: rate should be (600 * 100) / 8500 = 7.05...
        // Rounded down should be 7
        assert!(rate <= 8 && rate >= 7);
    }

    // Boundary Tests
    #[test]
    fn test_v1_boundary_conditions() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        // Test just before kink
        let rate_before = rate_model.calc_borrow_rate_from_utilization(8999).unwrap();
        // Test just after kink
        let rate_after = rate_model.calc_borrow_rate_from_utilization(9001).unwrap();

        // Rate should increase significantly after kink
        assert!(rate_after > rate_before);
    }

    #[test]
    fn test_v2_boundary_conditions() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        // Test boundaries around kink1
        let rate_before_kink1 = rate_model.calc_borrow_rate_from_utilization(8499).unwrap();
        let rate_after_kink1 = rate_model.calc_borrow_rate_from_utilization(8501).unwrap();

        // Test boundaries around kink2
        let rate_before_kink2 = rate_model.calc_borrow_rate_from_utilization(9299).unwrap();
        let rate_after_kink2 = rate_model.calc_borrow_rate_from_utilization(9301).unwrap();

        // Rates should be continuous but slopes should change
        assert!(rate_after_kink1 >= rate_before_kink1);
        assert!(rate_after_kink2 >= rate_before_kink2);
    }

    // High Utilization Tests (>100%)
    #[test]
    fn test_v1_high_utilization() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        // Test at 150% utilization
        let rate = rate_model.calc_borrow_rate_from_utilization(15000).unwrap();
        // Should extrapolate linearly beyond 100%
        assert!(rate >= 10000); // Should be higher than max rate
    }

    #[test]
    fn test_v2_high_utilization() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        // Test at 150% utilization
        let rate = rate_model.calc_borrow_rate_from_utilization(15000).unwrap();
        // Should extrapolate linearly beyond 100%
        assert!(rate >= 10000); // Should be higher than max rate
    }

    // Comprehensive Rate Curve Tests
    #[test]
    fn test_v1_rate_curve_monotonic() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v1(create_rate_v1_params()).unwrap();

        let mut prev_rate = 0;
        // Test that rates are monotonically increasing
        for utilization in (0..=12000).step_by(1000) {
            let rate = rate_model
                .calc_borrow_rate_from_utilization(utilization)
                .unwrap();
            assert!(rate >= prev_rate, "Rate should be monotonically increasing");
            prev_rate = rate;
        }
    }

    #[test]
    fn test_v2_rate_curve_monotonic() {
        let mut rate_model = RateModel::default();
        rate_model.set_rate_v2(create_rate_v2_params()).unwrap();

        let mut prev_rate = 0;
        // Test that rates are monotonically increasing
        for utilization in (0..=12000).step_by(1000) {
            let rate = rate_model
                .calc_borrow_rate_from_utilization(utilization)
                .unwrap();
            assert!(rate >= prev_rate, "Rate should be monotonically increasing");
            prev_rate = rate;
        }
    }

    // Additional Edge Cases
    #[test]
    fn test_v1_minimal_kink_difference() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v1_params();
        rate_data.kink = 1; // Very small kink
        rate_data.rate_at_utilization_kink = 1;

        let result = rate_model.set_rate_v1(rate_data);
        assert!(result.is_ok());

        let rate = rate_model.calc_borrow_rate_from_utilization(1).unwrap();
        assert_eq!(rate, 1);
    }

    #[test]
    fn test_v2_minimal_kink_difference() {
        let mut rate_model = RateModel::default();
        let mut rate_data = create_rate_v2_params();
        rate_data.kink1 = 1;
        rate_data.kink2 = 2; // Very small difference
        rate_data.rate_at_utilization_kink1 = 1;
        rate_data.rate_at_utilization_kink2 = 2;

        let result = rate_model.set_rate_v2(rate_data);
        assert!(result.is_ok());

        let rate1 = rate_model.calc_borrow_rate_from_utilization(1).unwrap();
        let rate2 = rate_model.calc_borrow_rate_from_utilization(2).unwrap();
        assert_eq!(rate1, 1);
        assert_eq!(rate2, 2);
    }

    // Test with maximum allowed values
    #[test]
    fn test_v1_max_values() {
        let mut rate_model = RateModel::default();
        let rate_data = RateDataV1Params {
            kink: FOUR_DECIMALS - 1, // 99.99%
            rate_at_utilization_zero: MAX_RATE as u128,
            rate_at_utilization_kink: MAX_RATE as u128,
            rate_at_utilization_max: MAX_RATE as u128,
        };

        let result = rate_model.set_rate_v1(rate_data);
        assert!(result.is_ok());

        let rate = rate_model.calc_borrow_rate_from_utilization(5000).unwrap();
        assert_eq!(rate, MAX_RATE);
    }

    #[test]
    fn test_v2_max_values() {
        let mut rate_model = RateModel::default();
        let rate_data = RateDataV2Params {
            kink1: FOUR_DECIMALS - 2, // 99.98%
            kink2: FOUR_DECIMALS - 1, // 99.99%
            rate_at_utilization_zero: MAX_RATE as u128,
            rate_at_utilization_kink1: MAX_RATE as u128,
            rate_at_utilization_kink2: MAX_RATE as u128,
            rate_at_utilization_max: MAX_RATE as u128,
        };

        let result = rate_model.set_rate_v2(rate_data);
        assert!(result.is_ok());

        let rate = rate_model.calc_borrow_rate_from_utilization(5000).unwrap();
        assert_eq!(rate, MAX_RATE);
    }
}
