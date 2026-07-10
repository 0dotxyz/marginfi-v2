use anchor_lang::err;
use anchor_lang::prelude::*;
use fixed::types::I80F48;
use marginfi_type_crate::{
    constants::SECONDS_PER_YEAR,
    types::{
        InterestRateConfig, InterestRateConfigOpt, MarginfiGroup, RatePoint, INTEREST_CURVE_LEGACY,
        INTEREST_CURVE_SEVEN_POINT,
    },
};

use crate::{
    debug,
    errors::MarginfiError,
    math_error,
    prelude::MarginfiResult,
    set_if_some,
    state::{bank_cache::ComputedInterestRates, marginfi_group::MarginfiGroupImpl},
};

pub trait InterestRateConfigImpl {
    fn validate_seven_point(&self) -> MarginfiResult;
    fn validate_legacy(&self) -> MarginfiResult;
    fn create_interest_rate_calculator(&self, group: &MarginfiGroup) -> InterestRateCalc;
    fn validate(&self) -> MarginfiResult;
    fn update(&mut self, ir_config: &InterestRateConfigOpt);
}

impl InterestRateConfigImpl for InterestRateConfig {
    fn create_interest_rate_calculator(&self, group: &MarginfiGroup) -> InterestRateCalc {
        let group_bank_config = &group.get_group_bank_config();
        debug!(
            "Creating interest rate calculator with protocol fees: {}",
            group_bank_config.program_fees
        );
        InterestRateCalc {
            insurance_fixed_fee: self.insurance_fee_fixed_apr.into(),
            insurance_rate_fee: self.insurance_ir_fee.into(),
            protocol_fixed_fee: self.protocol_fixed_fee_apr.into(),
            protocol_rate_fee: self.protocol_ir_fee.into(),
            add_program_fees: group_bank_config.program_fees,
            program_fee_fixed: group.fee_state_cache.program_fee_fixed.into(),
            program_fee_rate: group.fee_state_cache.program_fee_rate.into(),
            zero_util_rate: self.zero_util_rate,
            hundred_util_rate: self.hundred_util_rate,
            points: self.points,
            curve_type: self.curve_type,
        }
    }

    fn validate(&self) -> MarginfiResult {
        match self.curve_type {
            INTEREST_CURVE_LEGACY => self.validate_legacy()?,
            INTEREST_CURVE_SEVEN_POINT => self.validate_seven_point()?,
            _ => panic!("unsupported curve type"),
        }

        Ok(())
    }

    fn validate_legacy(&self) -> MarginfiResult {
        msg!("Legacy interest curve type is deprecated and unsupported");
        err!(MarginfiError::InvalidConfig)
    }

    /// * the rate at zero is the lowest
    /// * utils in points are in ascending order, and non-decreasing rates
    /// * no "holes" in points, all padding is at the end of the slice
    /// * the rate at 100% util is the highest
    fn validate_seven_point(&self) -> MarginfiResult {
        let zero = self.zero_util_rate;
        let hundred = self.hundred_util_rate;

        // Collect used points (util > 0), enforce trailing padding
        let mut used: Vec<RatePoint> = Vec::with_capacity(self.points.len());
        let mut seen_padding = false;
        for (i, p) in self.points.iter().enumerate() {
            if p.util == 0 {
                // Padding: must be (0,0); once seen, all following must be padding as well.
                if p.rate != 0 {
                    msg!("Expected padding (zero rate) at {:?}", i);
                    return err!(MarginfiError::InvalidConfig);
                }
                seen_padding = true;
            } else {
                // No "holes": non-zero util after padding is not allowed.
                if seen_padding {
                    msg!("Expected padding at {:?} (no holes permitted)", i);
                    return err!(MarginfiError::InvalidConfig);
                }

                used.push(*p);
            }
        }

        // Points must be strictly increasing in util, and non-decreasing in rate
        for i in 1..used.len() {
            let prev = &used[i - 1];
            let curr = &used[i];

            if prev.util >= curr.util {
                msg!("util not ascending between {:?} {:?}", i - 1, i);
                msg!("utils: {:?} {:?}", prev.util, curr.util);
                return err!(MarginfiError::InvalidConfig);
            }
            if prev.rate > curr.rate {
                msg!("rate is decreasing between {:?} {:?}", i - 1, i);
                msg!("rates: {:?} {:?}", prev.rate, curr.rate);
                return err!(MarginfiError::InvalidConfig);
            }
        }

        // rate at zero < rate at 100%, and for each point p, 0_rate <= p <= 100%_rate
        let zero_lte_hundred = zero <= hundred;
        if !zero_lte_hundred {
            msg!("The zero rate is higher than the hundred rate");
            return err!(MarginfiError::InvalidConfig);
        }
        let p_between_zero_hundred = used.iter().all(|p| zero <= p.rate && p.rate <= hundred);
        if !p_between_zero_hundred {
            msg!("A point is not between 0 and 100 rates");
            return err!(MarginfiError::InvalidConfig);
        }

        Ok(())
    }

    fn update(&mut self, ir_config: &InterestRateConfigOpt) {
        set_if_some!(
            self.insurance_fee_fixed_apr,
            ir_config.insurance_fee_fixed_apr
        );
        set_if_some!(self.insurance_ir_fee, ir_config.insurance_ir_fee);
        set_if_some!(
            self.protocol_fixed_fee_apr,
            ir_config.protocol_fixed_fee_apr
        );
        set_if_some!(self.protocol_ir_fee, ir_config.protocol_ir_fee);
        set_if_some!(
            self.protocol_origination_fee,
            ir_config.protocol_origination_fee
        );
        set_if_some!(self.zero_util_rate, ir_config.zero_util_rate);
        set_if_some!(self.hundred_util_rate, ir_config.hundred_util_rate);
        set_if_some!(self.points, ir_config.points);

        // Note: If we ever support another curve type, this will become configurable.
        self.curve_type = INTEREST_CURVE_SEVEN_POINT;
    }
}

#[derive(Debug, Clone)]
/// Short for calculator
pub struct InterestRateCalc {
    // Fees
    insurance_fixed_fee: I80F48,
    insurance_rate_fee: I80F48,
    /// AKA group fixed fee
    protocol_fixed_fee: I80F48,
    /// AKA group rate fee
    protocol_rate_fee: I80F48,

    program_fee_fixed: I80F48,
    program_fee_rate: I80F48,

    add_program_fees: bool,
    zero_util_rate: u32,
    hundred_util_rate: u32,
    points: [RatePoint; 5],
    curve_type: u8,
}

impl InterestRateCalc {
    /// Return interest rate charged to borrowers and to depositors.
    /// Rate is denominated in APR (0-).
    ///
    /// Return ComputedInterestRates
    pub fn calc_interest_rate(
        &self,
        utilization_ratio: I80F48,
    ) -> MarginfiResult<ComputedInterestRates> {
        let Fees {
            insurance_fee_rate,
            insurance_fee_fixed,
            group_fee_rate,
            group_fee_fixed,
            protocol_fee_rate,
            protocol_fee_fixed,
        } = self.get_fees();

        let fee_ir: I80F48 = insurance_fee_rate + group_fee_rate + protocol_fee_rate;
        let fee_fixed: I80F48 = insurance_fee_fixed + group_fee_fixed + protocol_fee_fixed;

        let base_rate_apr: I80F48 = match self.curve_type {
            INTEREST_CURVE_LEGACY => {
                msg!("Legacy interest curve type is deprecated and unsupported");
                return err!(MarginfiError::InvalidConfig);
            }
            INTEREST_CURVE_SEVEN_POINT => self
                .interest_rate_multipoint_curve(utilization_ratio)
                .ok_or_else(math_error!())?,
            _ => panic!("unsupported curve type"),
        };

        // Lending rate is adjusted for utilization ratio to symmetrize payments between borrowers and depositors.
        let lending_rate_apr: I80F48 = base_rate_apr
            .checked_mul(utilization_ratio)
            .ok_or_else(math_error!())?;

        // Borrowing rate is adjusted for fees.
        // borrowing_rate = base_rate + base_rate * rate_fee + total_fixed_fee_apr
        let borrowing_rate_apr: I80F48 = base_rate_apr
            .checked_mul(I80F48::ONE.checked_add(fee_ir).ok_or_else(math_error!())?)
            .ok_or_else(math_error!())?
            .checked_add(fee_fixed)
            .ok_or_else(math_error!())?;

        let group_fee_apr: I80F48 = calc_fee_rate(base_rate_apr, group_fee_rate, group_fee_fixed)
            .ok_or_else(math_error!())?;
        let insurance_fee_apr: I80F48 =
            calc_fee_rate(base_rate_apr, insurance_fee_rate, insurance_fee_fixed)
                .ok_or_else(math_error!())?;
        let protocol_fee_apr: I80F48 =
            calc_fee_rate(base_rate_apr, protocol_fee_rate, protocol_fee_fixed)
                .ok_or_else(math_error!())?;

        assert!(lending_rate_apr >= I80F48::ZERO);
        assert!(borrowing_rate_apr >= I80F48::ZERO);
        assert!(group_fee_apr >= I80F48::ZERO);
        assert!(insurance_fee_apr >= I80F48::ZERO);
        assert!(protocol_fee_apr >= I80F48::ZERO);

        Ok(ComputedInterestRates {
            base_rate_apr,
            lending_rate_apr,
            borrowing_rate_apr,
            group_fee_apr,
            insurance_fee_apr,
            protocol_fee_apr,
        })
    }

    /// Locates ur on a piecewise linear interest rate function with seven points:
    /// * Points defined as 1: (0, Y1), 2-6: (X2-6, Y2-6), 7: (100, Y7), where 0 < X2-6 < 100
    #[inline]
    fn interest_rate_multipoint_curve(&self, ur: I80F48) -> Option<I80F48> {
        let zero_rate: I80F48 = Self::rate_from_u32(self.zero_util_rate);
        let hundred_rate: I80F48 = Self::rate_from_u32(self.hundred_util_rate);

        // The first point is at (0, zero_rate)
        let mut prev_util: I80F48 = I80F48::ZERO;
        let mut prev_rate: I80F48 = zero_rate;
        // Sanity check: clamp the UR in case we somehow exceeded 100% or went negative
        let ur: I80F48 = ur.max(I80F48::ZERO).min(I80F48::ONE);

        for point in self.points.iter().filter(|point| point.util() != 0) {
            let point_util: I80F48 = Self::util_from_u32(point.util());
            let point_rate: I80F48 = Self::rate_from_u32(point.rate());

            if ur <= point_util {
                return Self::lerp(prev_util, prev_rate, point_util, point_rate, ur);
            }

            prev_util = point_util;
            prev_rate = point_rate;
        }

        Self::lerp(prev_util, prev_rate, I80F48::ONE, hundred_rate, ur)
    }

    /// Given two points (start_x, start_y) and (end_x, end_y), and a target x between start_x and
    /// end_x, linearly interpolates y at the given x.
    ///
    /// * returns start_y if end_x <= start_x or target < start_x
    /// * returns end_y if target > end_x
    /// * None if end_y < start_y. Note: this means curves where the rate decreases as the
    ///   utilization goes up are unsupported, though there's no reason you would generally want to
    ///   do that anyways.
    #[inline]
    fn lerp(
        start_x: I80F48,
        start_y: I80F48,
        end_x: I80F48,
        end_y: I80F48,
        target_x: I80F48,
    ) -> Option<I80F48> {
        if end_x <= start_x {
            return Some(start_y);
        }
        if target_x < start_x {
            return None;
        }
        if target_x > end_x {
            return None;
        }
        if end_y < start_y {
            return None;
        }

        let delta_x: I80F48 = end_x - start_x;
        if delta_x.is_zero() {
            return Some(start_y);
        }

        // Safe: start_x < target_x
        let offset: I80F48 = target_x - start_x;
        // Safe: delta_x nonzero
        let proportion: I80F48 = offset / delta_x;
        // Safe: end_y > start_y
        let delta_y: I80F48 = end_y - start_y;
        let scaled_delta: I80F48 = delta_y.checked_mul(proportion)?;
        // Safe: start_y + scaled_delta < end_y
        Some(start_y + scaled_delta)
    }

    #[inline]
    fn rate_from_u32(rate: u32) -> I80F48 {
        let ratio: I80F48 = I80F48::from_num(rate) / I80F48::from_num(u32::MAX);
        ratio * I80F48::from_num(10)
    }

    #[inline]
    fn util_from_u32(util: u32) -> I80F48 {
        I80F48::from_num(util) / I80F48::from_num(u32::MAX)
    }

    pub fn get_fees(&self) -> Fees {
        let (protocol_fee_rate, protocol_fee_fixed) = if self.add_program_fees {
            (self.program_fee_rate, self.program_fee_fixed)
        } else {
            (I80F48::ZERO, I80F48::ZERO)
        };

        Fees {
            insurance_fee_rate: self.insurance_rate_fee,
            insurance_fee_fixed: self.insurance_fixed_fee,
            group_fee_rate: self.protocol_rate_fee,
            group_fee_fixed: self.protocol_fixed_fee,
            protocol_fee_rate,
            protocol_fee_fixed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fees {
    pub insurance_fee_rate: I80F48,
    pub insurance_fee_fixed: I80F48,
    pub group_fee_rate: I80F48,
    pub group_fee_fixed: I80F48,
    pub protocol_fee_rate: I80F48,
    pub protocol_fee_fixed: I80F48,
}

/// Calculates the fee rate for a given base rate and fees specified.
/// The returned rate is only the fee rate without the base rate.
///
/// Used for calculating the fees charged to the borrowers.
fn calc_fee_rate(base_rate: I80F48, rate_fees: I80F48, fixed_fees: I80F48) -> Option<I80F48> {
    if rate_fees.is_zero() {
        return Some(fixed_fees);
    }

    base_rate.checked_mul(rate_fees)?.checked_add(fixed_fees)
}

/// Calculates the accrued interest payment per period `time_delta` in a principal value `value` for interest rate (in APR) `arp`.
/// Result is the new principal value.
fn calc_accrued_interest_payment_per_period(
    apr: I80F48,
    time_delta: u64,
    value: I80F48,
) -> Option<I80F48> {
    let ir_per_period: I80F48 = apr
        .checked_mul(time_delta.into())?
        .checked_div(SECONDS_PER_YEAR)?;

    let new_value: I80F48 = value.checked_mul(I80F48::ONE.checked_add(ir_per_period)?)?;

    Some(new_value)
}

/// Calculates the interest payment for a given period `time_delta` in a principal value `value` for interest rate (in APR) `arp`.
/// Result is the interest payment.
fn calc_interest_payment_for_period(apr: I80F48, time_delta: u64, value: I80F48) -> Option<I80F48> {
    if apr.is_zero() {
        return Some(I80F48::ZERO);
    }

    let interest_payment: I80F48 = value
        .checked_mul(apr)?
        .checked_mul(time_delta.into())?
        .checked_div(SECONDS_PER_YEAR)?;

    Some(interest_payment)
}

/// We use a simple interest rate model that auto settles the accrued interest into the lending account balances.
/// The plan is to move to a compound interest model in the future.
///
/// Simple interest rate model:
/// - `P` - principal
/// - `i` - interest rate (per second)
/// - `t` - time (in seconds)
///
/// `P_t = P_0 * (1 + i) * t`
///
/// We use two interest rates, one for lending and one for borrowing.
///
/// Lending interest rate:
/// - `i_l` - lending interest rate
/// - `i` - base interest rate
/// - `ur` - utilization rate
///
/// `i_l` = `i` * `ur`
///
/// Borrowing interest rate:
/// - `i_b` - borrowing interest rate
/// - `i` - base interest rate
/// - `f_i` - interest rate fee
/// - `f_f` - fixed fee
///
/// `i_b = i * (1 + f_i) + f_f`
///
pub fn calc_interest_rate_accrual_state_changes(
    time_delta: u64,
    total_assets_amount: I80F48,
    total_liabilities_amount: I80F48,
    interest_rate_calc: &InterestRateCalc,
    asset_share_value: I80F48,
    liability_share_value: I80F48,
) -> MarginfiResult<InterestRateStateChanges> {
    // If the cache is empty, we need to calculate the interest rates
    let utilization_rate: I80F48 = total_liabilities_amount
        .checked_div(total_assets_amount)
        .ok_or_else(math_error!())?;
    debug!(
        "Utilization rate: {}, time delta {}s",
        utilization_rate, time_delta
    );
    let interest_rates = interest_rate_calc.calc_interest_rate(utilization_rate)?;

    debug!("{:#?}", interest_rates);

    let ComputedInterestRates {
        lending_rate_apr,
        borrowing_rate_apr,
        group_fee_apr,
        insurance_fee_apr,
        protocol_fee_apr,
        ..
    } = interest_rates;

    Ok(InterestRateStateChanges {
        new_asset_share_value: calc_accrued_interest_payment_per_period(
            lending_rate_apr,
            time_delta,
            asset_share_value,
        )
        .ok_or_else(math_error!())?,
        new_liability_share_value: calc_accrued_interest_payment_per_period(
            borrowing_rate_apr,
            time_delta,
            liability_share_value,
        )
        .ok_or_else(math_error!())?,
        insurance_fees_collected: calc_interest_payment_for_period(
            insurance_fee_apr,
            time_delta,
            total_liabilities_amount,
        )
        .ok_or_else(math_error!())?,
        group_fees_collected: calc_interest_payment_for_period(
            group_fee_apr,
            time_delta,
            total_liabilities_amount,
        )
        .ok_or_else(math_error!())?,
        protocol_fees_collected: calc_interest_payment_for_period(
            protocol_fee_apr,
            time_delta,
            total_liabilities_amount,
        )
        .ok_or_else(math_error!())?,
    })
}

pub struct InterestRateStateChanges {
    pub new_asset_share_value: I80F48,
    pub new_liability_share_value: I80F48,
    pub insurance_fees_collected: I80F48,
    pub group_fees_collected: I80F48,
    pub protocol_fees_collected: I80F48,
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        assert_eq_with_tolerance,
        state::{bank::BankImpl, bank_cache::ComputedInterestRates},
    };

    use super::*;
    use fixed::types::I80F48;
    use fixed_macro::types::I80F48;
    use marginfi_type_crate::{
        constants::{PROTOCOL_FEE_FIXED_DEFAULT, PROTOCOL_FEE_RATE_DEFAULT},
        types::{
            centi_to_u32, make_points, milli_to_u32, Bank, BankConfig, InterestRateConfig,
            RatePoint, INTEREST_CURVE_SEVEN_POINT,
        },
    };
    use solana_sdk::clock::Clock;
    #[cfg(not(feature = "client"))]
    use solana_sdk::pubkey::Pubkey;

    #[test]
    /// Tests that the interest payment for a 1 year period with 100% APR is 1.
    fn interest_payment_100apr_1year() {
        let apr = I80F48::ONE;
        let time_delta = 31_536_000; // 1 year
        let value = I80F48::ONE;

        assert_eq_with_tolerance!(
            calc_interest_payment_for_period(apr, time_delta, value).unwrap(),
            I80F48::ONE,
            I80F48!(0.001)
        );
    }

    /// Tests that the interest payment for a 1 year period with 50% APR is 0.5.
    #[test]
    fn interest_payment_50apr_1year() {
        let apr = I80F48::from_num(0.5);
        let time_delta = 31_536_000; // 1 year
        let value = I80F48::ONE;

        assert_eq_with_tolerance!(
            calc_interest_payment_for_period(apr, time_delta, value).unwrap(),
            I80F48::from_num(0.5),
            I80F48!(0.001)
        );
    }
    /// P: 1_000_000
    /// Apr: 12%
    /// Time: 1 second
    #[test]
    fn interest_payment_12apr_1second() {
        let apr = I80F48!(0.12);
        let time_delta = 1;
        let value = I80F48!(1_000_000);

        assert_eq_with_tolerance!(
            calc_interest_payment_for_period(apr, time_delta, value).unwrap(),
            I80F48!(0.0038),
            I80F48!(0.001)
        );
    }

    #[test]
    /// apr: 100%
    /// time: 1 year
    /// principal: 2
    /// expected: 4
    fn accrued_interest_apr100_year1() {
        assert_eq_with_tolerance!(
            calc_accrued_interest_payment_per_period(I80F48!(1), 31_536_000, I80F48!(2)).unwrap(),
            I80F48!(4),
            I80F48!(0.001)
        );
    }

    #[test]
    /// apr: 50%
    /// time: 1 year
    /// principal: 2
    /// expected: 3
    fn accrued_interest_apr50_year1() {
        assert_eq_with_tolerance!(
            calc_accrued_interest_payment_per_period(I80F48!(0.5), 31_536_000, I80F48!(2)).unwrap(),
            I80F48!(3),
            I80F48!(0.001)
        );
    }

    fn apr_to_u32(apr: f64) -> u32 {
        ((apr / 10.0) * (u32::MAX as f64)).round() as u32
    }

    fn util_to_u32(util: f64) -> u32 {
        (util * (u32::MAX as f64)).round() as u32
    }

    fn sample_multipoint_calc() -> InterestRateCalc {
        InterestRateCalc {
            insurance_fixed_fee: I80F48::ZERO,
            insurance_rate_fee: I80F48::ZERO,
            protocol_fixed_fee: I80F48::ZERO,
            protocol_rate_fee: I80F48::ZERO,
            program_fee_fixed: I80F48::ZERO,
            program_fee_rate: I80F48::ZERO,
            add_program_fees: false,
            zero_util_rate: apr_to_u32(0.05),
            hundred_util_rate: apr_to_u32(0.40),
            points: [
                RatePoint::new(util_to_u32(0.20), apr_to_u32(0.10)),
                RatePoint::new(util_to_u32(0.60), apr_to_u32(0.15)),
                RatePoint::default(),
                RatePoint::default(),
                RatePoint::default(),
            ],
            curve_type: INTEREST_CURVE_SEVEN_POINT,
        }
    }

    #[test]
    fn multipoint_curve_matches_zero_util_rate() {
        let calc = sample_multipoint_calc();
        let rate = calc
            .interest_rate_multipoint_curve(I80F48::ZERO)
            .expect("zero util rate");
        assert_eq_with_tolerance!(rate, I80F48!(0.05), I80F48!(0.0001));
    }

    #[test]
    fn multipoint_curve_interpolates_between_points() {
        let calc = sample_multipoint_calc();
        let rate = calc
            .interest_rate_multipoint_curve(I80F48!(0.4))
            .expect("interpolated rate");
        assert_eq_with_tolerance!(rate, I80F48!(0.125), I80F48!(0.0001));
    }

    #[test]
    fn calc_interest_rate_uses_multipoint_curve() {
        let calc = sample_multipoint_calc();
        let ComputedInterestRates { base_rate_apr, .. } = calc
            .calc_interest_rate(I80F48!(0.4))
            .expect("computed rate");
        assert_eq_with_tolerance!(base_rate_apr, I80F48!(0.125), I80F48!(0.0001));
    }

    #[test]
    fn legacy_curve_validation_is_rejected() {
        let config = InterestRateConfig {
            curve_type: INTEREST_CURVE_LEGACY,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn legacy_curve_rate_calculation_is_rejected() {
        let config = InterestRateConfig {
            curve_type: INTEREST_CURVE_LEGACY,
            ..Default::default()
        };
        assert!(config
            .create_interest_rate_calculator(&MarginfiGroup::default())
            .calc_interest_rate(I80F48!(0.5))
            .is_err());
    }

    #[test]
    /// apr: 12%
    /// time: 1 second
    /// principal: 1_000_000
    /// expected: 1_038
    fn accrued_interest_apr12_year1() {
        assert_eq_with_tolerance!(
            calc_accrued_interest_payment_per_period(I80F48!(0.12), 1, I80F48!(1_000_000)).unwrap(),
            I80F48!(1_000_000.0038),
            I80F48!(0.001)
        );
    }

    #[test]
    /// ur: 0
    /// protocol_fixed_fee: 0.01
    fn ir_config_calc_interest_rate_pff_01() {
        let config = InterestRateConfig {
            protocol_fixed_fee_apr: I80F48!(0.01).into(),
            zero_util_rate: apr_to_u32(0.0),
            hundred_util_rate: apr_to_u32(0.4),
            points: make_points(&[RatePoint::new(util_to_u32(0.6), apr_to_u32(0.4))]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };

        let ComputedInterestRates {
            base_rate_apr,
            lending_rate_apr: lending_apr,
            borrowing_rate_apr: borrow_apr,
            group_fee_apr: group_fees_apr,
            insurance_fee_apr: insurance_apr,
            protocol_fee_apr,
        } = config
            .create_interest_rate_calculator(&MarginfiGroup::default())
            .calc_interest_rate(I80F48!(0.6))
            .unwrap();

        assert_eq_with_tolerance!(base_rate_apr, I80F48!(0.4), I80F48!(0.001));
        assert_eq_with_tolerance!(lending_apr, I80F48!(0.24), I80F48!(0.001));
        assert_eq_with_tolerance!(borrow_apr, I80F48!(0.41), I80F48!(0.001));
        assert_eq_with_tolerance!(group_fees_apr, I80F48!(0.01), I80F48!(0.001));
        assert_eq_with_tolerance!(insurance_apr, I80F48!(0), I80F48!(0.001));
        assert_eq_with_tolerance!(protocol_fee_apr, I80F48!(0), I80F48!(0.001));
    }

    #[test]
    /// ur: 0.5
    /// protocol_fixed_fee: 0.01
    /// seven-point curve with a point at util=0.5, rate=0.4
    fn ir_config_calc_interest_rate_pff_01_ur_05() {
        let config = InterestRateConfig {
            protocol_fixed_fee_apr: I80F48!(0.01).into(),
            insurance_ir_fee: I80F48!(0.1).into(),
            zero_util_rate: apr_to_u32(0.0),
            hundred_util_rate: apr_to_u32(0.4),
            points: make_points(&[RatePoint::new(util_to_u32(0.5), apr_to_u32(0.4))]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };

        let ComputedInterestRates {
            base_rate_apr,
            lending_rate_apr: lending_apr,
            borrowing_rate_apr: borrow_apr,
            group_fee_apr: group_fees_apr,
            insurance_fee_apr: insurance_apr,
            protocol_fee_apr: _,
        } = config
            .create_interest_rate_calculator(&MarginfiGroup::default())
            .calc_interest_rate(I80F48!(0.5))
            .unwrap();

        assert_eq_with_tolerance!(base_rate_apr, I80F48!(0.4), I80F48!(0.001));
        assert_eq_with_tolerance!(lending_apr, I80F48!(0.2), I80F48!(0.001));
        assert_eq_with_tolerance!(borrow_apr, I80F48!(0.45), I80F48!(0.001));
        assert_eq_with_tolerance!(group_fees_apr, I80F48!(0.01), I80F48!(0.001));
        assert_eq_with_tolerance!(insurance_apr, I80F48!(0.04), I80F48!(0.001));
    }

    #[test]
    fn calc_fee_rate_1() {
        let rate = I80F48!(0.4);
        let fee_ir = I80F48!(0.05);
        let fee_fixed = I80F48!(0.01);

        assert_eq!(
            calc_fee_rate(rate, fee_ir, fee_fixed).unwrap(),
            I80F48!(0.03)
        );
    }

    /// ur: 0.8
    /// protocol_fixed_fee: 0.01
    /// seven-point curve with a point at util=0.4, rate=0.4 and 100%-util rate of 3.0
    /// insurance_ir_fee: 0.1
    #[test]
    fn ir_config_calc_interest_rate_pff_01_ur_08() {
        let config = InterestRateConfig {
            protocol_fixed_fee_apr: I80F48!(0.01).into(),
            insurance_ir_fee: I80F48!(0.1).into(),
            zero_util_rate: apr_to_u32(0.0),
            hundred_util_rate: apr_to_u32(3.0),
            points: make_points(&[RatePoint::new(util_to_u32(0.4), apr_to_u32(0.4))]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };

        let ComputedInterestRates {
            base_rate_apr,
            lending_rate_apr: lending_apr,
            borrowing_rate_apr: borrow_apr,
            group_fee_apr: group_fees_apr,
            insurance_fee_apr: insurance_apr,
            protocol_fee_apr: _,
        } = config
            .create_interest_rate_calculator(&MarginfiGroup::default())
            .calc_interest_rate(I80F48!(0.7))
            .unwrap();

        assert_eq_with_tolerance!(base_rate_apr, I80F48!(1.7), I80F48!(0.001));
        assert_eq_with_tolerance!(lending_apr, I80F48!(1.19), I80F48!(0.001));
        assert_eq_with_tolerance!(borrow_apr, I80F48!(1.88), I80F48!(0.001));
        assert_eq_with_tolerance!(group_fees_apr, I80F48!(0.01), I80F48!(0.001));
        assert_eq_with_tolerance!(insurance_apr, I80F48!(0.17), I80F48!(0.001));
    }

    #[test]
    fn ir_accrual_failing_fuzz_test_example() -> anyhow::Result<()> {
        let ir_config = InterestRateConfig {
            protocol_fixed_fee_apr: I80F48!(0.01).into(),
            insurance_ir_fee: I80F48!(0.1).into(),
            zero_util_rate: apr_to_u32(0.0),
            hundred_util_rate: apr_to_u32(3.0),
            points: make_points(&[RatePoint::new(util_to_u32(0.4), apr_to_u32(0.4))]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };

        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let mut bank = Bank {
            asset_share_value: I80F48::ONE.into(),
            liability_share_value: I80F48::ONE.into(),
            total_liability_shares: I80F48!(207_112_621_602).into(),
            total_asset_shares: I80F48!(10_000_000_000_000).into(),
            last_update: current_timestamp,
            config: BankConfig {
                asset_weight_init: I80F48!(0.5).into(),
                asset_weight_maint: I80F48!(0.75).into(),
                liability_weight_init: I80F48!(1.5).into(),
                liability_weight_maint: I80F48!(1.25).into(),
                borrow_limit: u64::MAX,
                deposit_limit: u64::MAX,
                interest_rate_config: ir_config,
                ..Default::default()
            },
            ..Default::default()
        };

        let pre_net_assets = bank.get_asset_amount(bank.total_asset_shares.into())?
            - bank.get_liability_amount(bank.total_liability_shares.into())?;

        let mut clock = Clock::default();

        clock.unix_timestamp = current_timestamp + 3600;

        bank.accrue_interest(
            current_timestamp,
            &MarginfiGroup::default(),
            #[cfg(not(feature = "client"))]
            Pubkey::default(),
        )
        .unwrap();

        let post_collected_fees = I80F48::from(bank.collected_group_fees_outstanding)
            + I80F48::from(bank.collected_insurance_fees_outstanding);

        let post_net_assets = bank.get_asset_amount(bank.total_asset_shares.into())?
            + post_collected_fees
            - bank.get_liability_amount(bank.total_liability_shares.into())?;

        assert_eq_with_tolerance!(pre_net_assets, post_net_assets, I80F48!(1));

        Ok(())
    }

    #[test]
    fn interest_rate_accrual_test_0() -> anyhow::Result<()> {
        let ir_config = InterestRateConfig {
            protocol_fixed_fee_apr: I80F48!(0.01).into(),
            insurance_ir_fee: I80F48!(0.1).into(),
            zero_util_rate: apr_to_u32(0.0),
            hundred_util_rate: apr_to_u32(3.0),
            points: make_points(&[RatePoint::new(util_to_u32(0.4), apr_to_u32(0.4))]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };

        let ur = I80F48!(207_112_621_602) / I80F48!(10_000_000_000_000);
        let mut group = MarginfiGroup::default();
        group.group_flags = 1;
        group.fee_state_cache.program_fee_fixed = PROTOCOL_FEE_FIXED_DEFAULT.into();
        group.fee_state_cache.program_fee_rate = PROTOCOL_FEE_RATE_DEFAULT.into();

        let ComputedInterestRates {
            base_rate_apr,
            lending_rate_apr: lending_apr,
            borrowing_rate_apr: borrow_apr,
            group_fee_apr,
            insurance_fee_apr,
            protocol_fee_apr,
        } = ir_config
            .create_interest_rate_calculator(&group)
            .calc_interest_rate(ur)
            .expect("interest rate calculation failed");

        println!("ur: {}", ur);
        println!("base_apr: {}", base_rate_apr);
        println!("lending_apr: {}", lending_apr);
        println!("borrow_apr: {}", borrow_apr);
        println!("group_fee_apr: {}", group_fee_apr);
        println!("insurance_fee_apr: {}", insurance_fee_apr);

        assert_eq_with_tolerance!(
            borrow_apr,
            (lending_apr / ur) + group_fee_apr + insurance_fee_apr + protocol_fee_apr,
            I80F48!(0.001)
        );

        Ok(())
    }

    #[test]
    fn interest_rate_accrual_test_0_no_protocol_fees() -> anyhow::Result<()> {
        let ir_config = InterestRateConfig {
            protocol_fixed_fee_apr: I80F48!(0.01).into(),
            insurance_ir_fee: I80F48!(0.1).into(),
            zero_util_rate: apr_to_u32(0.0),
            hundred_util_rate: apr_to_u32(3.0),
            points: make_points(&[RatePoint::new(util_to_u32(0.4), apr_to_u32(0.4))]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };

        let ur = I80F48!(207_112_621_602) / I80F48!(10_000_000_000_000);

        let ComputedInterestRates {
            base_rate_apr,
            lending_rate_apr: lending_apr,
            borrowing_rate_apr: borrow_apr,
            group_fee_apr,
            insurance_fee_apr,
            protocol_fee_apr,
        } = ir_config
            .create_interest_rate_calculator(&MarginfiGroup::default())
            .calc_interest_rate(ur)
            .expect("interest rate calculation failed");

        println!("ur: {}", ur);
        println!("base_apr: {}", base_rate_apr);
        println!("lending_apr: {}", lending_apr);
        println!("borrow_apr: {}", borrow_apr);
        println!("group_fee_apr: {}", group_fee_apr);
        println!("insurance_fee_apr: {}", insurance_fee_apr);

        assert!(protocol_fee_apr.is_zero());

        assert_eq_with_tolerance!(
            borrow_apr,
            (lending_apr / ur) + group_fee_apr + insurance_fee_apr,
            I80F48!(0.001)
        );

        Ok(())
    }

    #[test]
    fn test_accruing_interest() -> anyhow::Result<()> {
        let ir_config = InterestRateConfig {
            protocol_fixed_fee_apr: I80F48!(0.01).into(),
            insurance_ir_fee: I80F48!(0.1).into(),
            zero_util_rate: apr_to_u32(0.0),
            hundred_util_rate: apr_to_u32(3.0),
            points: make_points(&[RatePoint::new(util_to_u32(0.4), apr_to_u32(0.4))]),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        };

        let mut group = MarginfiGroup::default();
        group.group_flags = 1;
        group.fee_state_cache.program_fee_fixed = PROTOCOL_FEE_FIXED_DEFAULT.into();
        group.fee_state_cache.program_fee_rate = PROTOCOL_FEE_RATE_DEFAULT.into();

        let liab_share_value = I80F48!(1.0);
        let asset_share_value = I80F48!(1.0);

        let total_liability_shares = I80F48!(207_112_621_602);
        let total_asset_shares = I80F48!(10_000_000_000_000);

        let old_total_liability_amount = liab_share_value * total_liability_shares;
        let old_total_asset_amount = asset_share_value * total_asset_shares;

        let InterestRateStateChanges {
            new_asset_share_value,
            new_liability_share_value: new_liab_share_value,
            insurance_fees_collected: insurance_collected,
            group_fees_collected,
            protocol_fees_collected,
        } = calc_interest_rate_accrual_state_changes(
            3600,
            total_asset_shares,
            total_liability_shares,
            &ir_config.create_interest_rate_calculator(&group),
            asset_share_value,
            liab_share_value,
        )
        .unwrap();

        let new_total_liability_amount = total_liability_shares * new_liab_share_value;
        let new_total_asset_amount = total_asset_shares * new_asset_share_value;

        println!("new_asset_share_value: {}", new_asset_share_value);
        println!("new_liab_share_value: {}", new_liab_share_value);
        println!("group_fees_collected: {}", group_fees_collected);
        println!("insurance_collected: {}", insurance_collected);
        println!("protocol_fees_collected: {}", protocol_fees_collected);

        println!("new_total_liability_amount: {}", new_total_liability_amount);
        println!("new_total_asset_amount: {}", new_total_asset_amount);

        println!("old_total_liability_amount: {}", old_total_liability_amount);
        println!("old_total_asset_amount: {}", old_total_asset_amount);

        let total_fees_collected =
            group_fees_collected + insurance_collected + protocol_fees_collected;

        println!("total_fee_collected: {}", total_fees_collected);

        println!(
            "diff: {}",
            ((new_total_asset_amount - new_total_liability_amount) + total_fees_collected)
                - (old_total_asset_amount - old_total_liability_amount)
        );

        assert_eq_with_tolerance!(
            (new_total_asset_amount - new_total_liability_amount) + total_fees_collected,
            old_total_asset_amount - old_total_liability_amount,
            I80F48::ONE
        );

        Ok(())
    }

    #[test]
    fn lerp_returns_start_y_when_end_x_le_start_x() {
        // end_x < start_x
        let out = InterestRateCalc::lerp(
            I80F48!(1.0),
            I80F48!(2.0),
            I80F48!(0.5),
            I80F48!(5.0),
            I80F48!(0.75),
        );
        assert_eq!(out, Some(I80F48!(2.0)));

        // end_x == start_x
        let out = InterestRateCalc::lerp(
            I80F48!(1.0),
            I80F48!(2.0),
            I80F48!(1.0),
            I80F48!(5.0),
            I80F48!(1.0),
        );
        assert_eq!(out, Some(I80F48!(2.0)));
    }

    #[test]
    fn lerp_none_when_target_before_start() {
        // target_x < start_x
        let out = InterestRateCalc::lerp(
            I80F48!(0.2),
            I80F48!(1.5),
            I80F48!(0.8),
            I80F48!(3.0),
            I80F48!(0.1),
        );
        assert!(out.is_none());
    }

    #[test]
    fn lerp_none_when_target_after_end() {
        // target_x > end_x
        let out = InterestRateCalc::lerp(
            I80F48!(0.0),
            I80F48!(0.0),
            I80F48!(1.0),
            I80F48!(4.0),
            I80F48!(1.5),
        );
        assert!(out.is_none());
    }

    // NOTE: we don't support decreasing curves because that would be silly for our use-case. There
    // is no circumstance (we think) where interest should decline as utilization increases.
    #[test]
    fn lerp_returns_none_when_end_y_less_than_start_y_and_target_in_range() {
        // target_x in (start_x, end_x) but end_y < start_y => None
        let out = InterestRateCalc::lerp(
            I80F48!(0.0),
            I80F48!(5.0),
            I80F48!(1.0),
            I80F48!(3.0),
            I80F48!(0.5),
        );
        assert_eq!(out, None);
    }

    #[test]
    fn lerp_interpolates_linearly_basic_cases() {
        // Simple 0 -> 1 over x in [0,1]
        let out = InterestRateCalc::lerp(
            I80F48!(0.0),
            I80F48!(0.0),
            I80F48!(1.0),
            I80F48!(1.0),
            I80F48!(0.25),
        );
        assert_eq!(out, Some(I80F48!(0.25)));

        // Halfway
        let out = InterestRateCalc::lerp(
            I80F48!(0.0),
            I80F48!(2.0),
            I80F48!(10.0),
            I80F48!(6.0),
            I80F48!(5.0),
        );
        // 2 + (6 - 2) / 2
        assert_eq!(out, Some(I80F48!(4.0)));

        // Non-zero start X, non-zero start Y, non-unit span
        let out = InterestRateCalc::lerp(
            I80F48!(2.0),
            I80F48!(1.0),
            I80F48!(6.0),
            I80F48!(3.0),
            I80F48!(3.0),
        );
        // y goes 1.0 -> 3.0 as x goes 2.0 -> 6.0; at x=3.0 (25% along), y=1.5
        assert_eq!(out, Some(I80F48!(1.5)));
    }

    #[test]
    fn lerp_interpolates_at_range_boundaries() {
        // target == start_x returns start_y
        let out = InterestRateCalc::lerp(
            I80F48!(2.0),
            I80F48!(10.0),
            I80F48!(5.0),
            I80F48!(13.0),
            I80F48!(2.0),
        );
        assert_eq!(out, Some(I80F48!(10.0)));

        // target == end_x returns end_y
        let out = InterestRateCalc::lerp(
            I80F48!(2.0),
            I80F48!(10.0),
            I80F48!(5.0),
            I80F48!(13.0),
            I80F48!(5.0),
        );
        assert_eq!(out, Some(I80F48!(13.0)));
    }

    #[test]
    fn lerp_large_numbers_u64_max_span() {
        let start_x = I80F48::from(0u64);
        let end_x = I80F48::from(u64::MAX);
        let start_y = I80F48::from(0u64);
        let end_y = I80F48::from(u64::MAX);

        // Halfway (target = MAX/2) -> expect MAX/2
        let out =
            InterestRateCalc::lerp(start_x, start_y, end_x, end_y, I80F48::from(u64::MAX / 2));
        let u64_tolerance: I80F48 = I80F48::from(u64::MAX) * I80F48!(0.00001);
        assert!(out.is_some());
        assert_eq_with_tolerance!(out.unwrap(), I80F48::from(u64::MAX / 2), u64_tolerance);

        // Quarter (target = MAX/4) -> expect MAX/4
        let out =
            InterestRateCalc::lerp(start_x, start_y, end_x, end_y, I80F48::from(u64::MAX / 4));
        assert!(out.is_some());
        assert_eq_with_tolerance!(out.unwrap(), I80F48::from(u64::MAX / 4), u64_tolerance);
    }

    #[test]
    fn lerp_large_numbers_i80f48_max_span() {
        let start_x: I80F48 = I80F48!(0.0);
        let end_x: I80F48 = I80F48!(1.0);
        let target: I80F48 = I80F48!(1.0);
        // slightly below max
        let start_y: I80F48 = I80F48::MAX - I80F48!(12345678); // slightly below max
        let end_y: I80F48 = I80F48::MAX;

        let out = InterestRateCalc::lerp(start_x, start_y, end_x, end_y, target);

        assert!(out.is_some());
        assert_eq!(out.unwrap(), I80F48::MAX);
    }

    /// A basic multi-point curve
    fn mk_calc(
        zero_rate_0_to_10: f32,
        hundred_rate_0_to_10: f32,
        interior: &[(f32, f32)],
    ) -> InterestRateCalc {
        let pts: Vec<RatePoint> = interior
            .iter()
            .map(|(u, r)| {
                RatePoint::new(
                    centi_to_u32(I80F48::from_num(*u)),
                    milli_to_u32(I80F48::from_num(*r)),
                )
            })
            .collect();

        InterestRateCalc {
            insurance_fixed_fee: I80F48::ZERO,
            insurance_rate_fee: I80F48::ZERO,
            protocol_fixed_fee: I80F48::ZERO,
            protocol_rate_fee: I80F48::ZERO,
            program_fee_fixed: I80F48::ZERO,
            program_fee_rate: I80F48::ZERO,
            add_program_fees: false,

            zero_util_rate: milli_to_u32(I80F48::from_num(zero_rate_0_to_10)),
            hundred_util_rate: milli_to_u32(I80F48::from_num(hundred_rate_0_to_10)),
            points: make_points(&pts),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
        }
    }

    // Note: Tolerance is pretty crappy (relatively speaking) because of u32 math, but this isn't
    // especially important since the different between 1% base rate and 1.000001 is trivial
    const TOLERANCE: I80F48 = I80F48!(0.000001);

    #[test]
    fn mpc_clamps_ur_below_zero_to_zero_rate() {
        // zero=1.5, hundred=9.0, no interior points
        let calc = mk_calc(1.5, 9.0, &[]);
        // negative ur should clamp to 0.0
        let ur: I80F48 = I80F48!(0) - I80F48!(0.25);
        let out: I80F48 = calc.interest_rate_multipoint_curve(ur).unwrap();
        assert_eq_with_tolerance!(out, I80F48!(1.5), TOLERANCE);
    }

    #[test]
    fn mpc_clamps_ur_above_one_to_hundred_rate() {
        let calc = mk_calc(0.5, 8.25, &[]);
        // ur > 1 clamps to 1.0 → hundred rate
        let out: I80F48 = calc.interest_rate_multipoint_curve(I80F48!(1.25)).unwrap();
        assert_eq_with_tolerance!(out, I80F48!(8.25), TOLERANCE);
    }

    #[test]
    fn mpc_on_point_returns_point_rate_exactly() {
        // zero=1.0; points at 0.25→2.5 and 0.5→5.0
        let calc = mk_calc(1.0, 9.0, &[(0.25, 2.5), (0.5, 5.0)]);
        let out: I80F48 = calc.interest_rate_multipoint_curve(I80F48!(0.25)).unwrap();
        assert_eq_with_tolerance!(out, I80F48!(2.5), TOLERANCE);
    }

    #[test]
    fn mpc_between_zero_and_first_point_interpolates() {
        // (0, 1.0) to (0.4, 5.0); ur=0.2 → halfway → 3.0
        let calc = mk_calc(1.0, 9.0, &[(0.4, 5.0)]);
        let out: I80F48 = calc.interest_rate_multipoint_curve(I80F48!(0.2)).unwrap();
        assert_eq_with_tolerance!(out, I80F48!(3.0), TOLERANCE);
    }

    #[test]
    fn mpc_between_internal_points_interpolates() {
        // points: (0.25,2.0), (0.5,6.0), (0.75,8.0)
        let calc = mk_calc(0.5, 9.5, &[(0.25, 2.0), (0.5, 6.0), (0.75, 8.0)]);
        // halfway between 0.25 and 0.5 → 4.0
        let out: I80F48 = calc.interest_rate_multipoint_curve(I80F48!(0.375)).unwrap();
        assert_eq_with_tolerance!(out, I80F48!(4.0), TOLERANCE);
    }

    #[test]
    fn mpc_after_last_point_to_one_interpolates_to_hundred_rate() {
        // last interior point at 0.9 with rate 9.0; hundred=10.0
        let calc = mk_calc(1.0, 10.0, &[(0.3, 2.0), (0.6, 6.0), (0.9, 9.0)]);
        // ur=0.95 → halfway from 9.0 to 10.0 → 9.5
        let out: I80F48 = calc.interest_rate_multipoint_curve(I80F48!(0.95)).unwrap();
        assert_eq_with_tolerance!(out, I80F48!(9.5), TOLERANCE);
    }

    #[test]
    fn mpc_decreasing_segment_returns_none() {
        // decreasing between (0.5,6.0) → (0.75, 4.0)
        let calc = mk_calc(1.0, 9.0, &[(0.25, 2.0), (0.5, 6.0), (0.75, 4.0)]);
        let out = calc.interest_rate_multipoint_curve(I80F48!(0.6));
        assert!(
            out.is_none(),
            "expected None for decreasing segment, got {out:?}"
        );
    }

    // Note: this state is invalid, but should be handled just in case...
    #[test]
    fn mpc_zero_util_points_are_ignored() {
        // First point has util=0 (ignored). Effective first segment: (0,1.0) → (0.5,5.0)
        let calc = mk_calc(1.0, 9.0, &[(0.0, 8.0), (0.5, 5.0)]);
        // ur=0.25 → halfway between 1.0 and 5.0 → 3.0
        let out: I80F48 = calc.interest_rate_multipoint_curve(I80F48!(0.25)).unwrap();
        assert_eq_with_tolerance!(out, I80F48!(3.0), TOLERANCE);
    }

    #[test]
    fn mpc_exactly_zero_and_one_match_endpoints() {
        let calc = mk_calc(1.2, 7.8, &[(0.25, 2.0), (0.5, 6.0)]);
        let at_zero: I80F48 = calc.interest_rate_multipoint_curve(I80F48!(0.0)).unwrap();
        assert_eq_with_tolerance!(at_zero, I80F48!(1.2), TOLERANCE);

        let at_one: I80F48 = calc.interest_rate_multipoint_curve(I80F48!(1.0)).unwrap();
        assert_eq_with_tolerance!(at_one, I80F48!(7.8), TOLERANCE);
    }

    // Small helper to make a RatePoint with %util (0–100) and %rate (0–1000)
    fn rp(util_pct_0to100: f64, rate_pct_0to1000: f64) -> RatePoint {
        RatePoint::new(
            centi_to_u32(I80F48::from_num(util_pct_0to100)),
            milli_to_u32(I80F48::from_num(rate_pct_0to1000)),
        )
    }

    fn cfg(
        zero_rate_pct_0to1000: f64,
        hundred_rate_pct_0to1000: f64,
        pts: &[RatePoint],
    ) -> InterestRateConfig {
        InterestRateConfig {
            zero_util_rate: milli_to_u32(I80F48::from_num(zero_rate_pct_0to1000)),
            hundred_util_rate: milli_to_u32(I80F48::from_num(hundred_rate_pct_0to1000)),
            points: make_points(pts),
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            ..Default::default()
        }
    }

    #[test]
    fn v7c_valid_curve_happy_path() {
        // zero < hundred, utils strictly increasing, rates non-decreasing, and all points within [zero, hundred].
        let c = cfg(0.5, 6.0, &[rp(0.20, 1.0), rp(0.40, 2.0), rp(0.60, 3.0)]);
        assert!(c.validate_seven_point().is_ok());
    }

    #[test]
    fn v7c_all_padding_is_ok() {
        // No points, just linear between 0 and 100
        let c = cfg(0.1, 1.0, &[]);
        assert!(c.validate_seven_point().is_ok());
    }

    #[test]
    fn v7c_padding_must_be_zero_rate() {
        // A padding entry (util==0) with non-zero rate is invalid.
        let bad = rp(0.0, 0.5);
        // rp(0, 0.5) makes util==0 and rate>0
        let c = cfg(0.1, 1.0, &[bad]);
        assert!(c.validate_seven_point().is_err());
    }

    #[test]
    fn v7c_no_holes_after_padding() {
        // Once padding is seen, no later non-zero util is allowed.
        let c = cfg(
            0.1,
            1.0,
            &[
                rp(0.20, 0.2),
                rp(0.0, 0.0),  // padding begins
                rp(0.30, 0.3), // hole after padding -> invalid
            ],
        );
        assert!(c.validate_seven_point().is_err());
    }

    #[test]
    fn v7c_utils_must_strictly_increase() {
        // Equal utils => invalid
        let c_equal = cfg(
            0.1,
            1.0,
            &[
                rp(0.20, 0.2),
                rp(0.20, 0.25), // same util
            ],
        );
        assert!(c_equal.validate_seven_point().is_err());

        // Decreasing util => invalid
        let c_dec = cfg(0.1, 1.0, &[rp(40.0, 0.4), rp(30.0, 0.45)]);
        assert!(c_dec.validate_seven_point().is_err());
    }

    #[test]
    fn v7c_rates_must_be_nondecreasing() {
        // A later point with a lower rate is invalid.
        let c = cfg(
            0.1,
            1.0,
            &[
                rp(0.20, 0.4),
                rp(0.40, 0.3), // decreased
            ],
        );
        assert!(c.validate_seven_point().is_err());
    }

    #[test]
    fn v7c_zero_rate_must_be_leq_hundred_rate() {
        // zero > hundred => invalid even if points look fine
        let c = cfg(
            5.0,
            1.0, // smaller than zero rate
            &[rp(0.20, 3.0), rp(0.40, 4.0)],
        );
        assert!(c.validate_seven_point().is_err());
    }

    #[test]
    fn v7c_point_rates_must_lie_within_zero_and_hundred() {
        // Point less than zero rate
        let c_low = cfg(
            1.0,
            5.0,
            &[
                rp(0.20, 0.5), // below zero rate (1.0)
            ],
        );
        assert!(c_low.validate_seven_point().is_err());

        // Point greater than hundred rate
        let c_high = cfg(
            1.0,
            5.0,
            &[
                rp(0.20, 6.0), // above hundred rate (5.0)
            ],
        );
        assert!(c_high.validate_seven_point().is_err());
    }

    #[test]
    fn v7c_extra_points_are_truncated_by_make_points() {
        let too_many_points = vec![
            rp(0.1, 0.5),
            rp(0.11, 0.6),
            rp(0.12, 0.7),
            rp(0.13, 0.8),
            rp(0.14, 0.9),
            rp(0.15, 1.0),
            rp(0.16, 1.1),
            rp(0.17, 1.2),
            rp(0.18, 1.3),
        ];
        let c = cfg(0.3, 2.0, &too_many_points);
        assert!(c.validate_seven_point().is_ok());
    }

    #[test]
    fn v7c_trailing_padding_ok() {
        // Valid used points, followed by explicit (util=0, rate=0) padding, still ok (but not
        // neccessary, make_points creates the padding)
        let c = cfg(
            0.2,
            2.0,
            &[
                rp(0.25, 0.4),
                rp(0.50, 0.8),
                rp(0.75, 1.6),
                rp(0.0, 0.0), // explicit padding (still ok)
            ],
        );
        assert!(c.validate_seven_point().is_ok());
    }
}
