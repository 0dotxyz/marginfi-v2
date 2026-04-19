use crate::assert_struct_size;

#[cfg(feature = "anchor")]
use anchor_lang::prelude::*;

use bytemuck::{Pod, Zeroable};
use fixed::types::I80F48;

use super::WrappedI80F48;

pub const INTEREST_CURVE_LEGACY: u8 = 0;
pub const INTEREST_CURVE_SEVEN_POINT: u8 = 1;
pub const CURVE_POINTS: usize = 5;

assert_struct_size!(InterestRateConfig, 240);
#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Debug, PartialEq, Eq, Copy, Clone, Pod, Zeroable, Default)]
pub struct InterestRateConfig {
    // TODO deprecate in 1.7
    pub optimal_utilization_rate: WrappedI80F48,
    // TODO deprecate in 1.7
    pub plateau_interest_rate: WrappedI80F48,
    // TODO deprecate in 1.7
    pub max_interest_rate: WrappedI80F48,

    // Fees
    /// Goes to insurance, funds `collected_insurance_fees_outstanding`
    pub insurance_fee_fixed_apr: WrappedI80F48,
    /// Goes to insurance, funds `collected_insurance_fees_outstanding`
    pub insurance_ir_fee: WrappedI80F48,
    /// Earned by the group, goes to `collected_group_fees_outstanding`
    pub protocol_fixed_fee_apr: WrappedI80F48,
    /// Earned by the group, goes to `collected_group_fees_outstanding`
    pub protocol_ir_fee: WrappedI80F48,
    pub protocol_origination_fee: WrappedI80F48,

    /// The base rate at utilization = 0
    /// * a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX
    pub zero_util_rate: u32,
    /// The base rate at utilization = 100
    /// * a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX
    pub hundred_util_rate: u32,
    /// The base rate at various points between 0 and 100%, exclusive. Essentially a piece-wise
    /// linear curve.
    /// * always in ascending order, e.g. points[0] = first kink point, points[1] = second kink
    ///   point, and so forth.
    /// * points where util = 0 are unused
    pub points: [RatePoint; CURVE_POINTS],

    /// Determines which interest rate curve implementation is active. 0 (INTEREST_CURVE_LEGACY) =
    /// legacy three point curve, 1 (INTEREST_CURVE_SEVEN_POINT) = multi-point curve.
    pub curve_type: u8,

    // Pad to nearest 8-byte multiple
    pub _pad0: [u8; 7],

    pub _padding1: [u8; 32],
    pub _padding2: [u8; 16],
    pub _padding3: [u8; 8],
}

#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Clone, Copy, Default, Zeroable, Pod, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct RatePoint {
    /// The utilization rate where `rate` applies
    /// * a %, as u32, out of 100%, e.g. 50% = .5 * u32::MAX
    pub util: u32,
    /// The base rate that applies
    /// * a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX
    pub rate: u32,
}

impl RatePoint {
    pub const fn new(util: u32, rate: u32) -> Self {
        Self { util, rate }
    }

    pub const fn rate(&self) -> u32 {
        self.rate
    }

    pub const fn util(&self) -> u32 {
        self.util
    }
}

/// Build a correctly sized slice of RatePoints from some arbitrary number of RatePoints.
/// * Performs no validation.
/// * If < CURVE_POINTS size, pads with zeros. If >, takes just the first CURVE_POINTS.
pub fn make_points(points: &[RatePoint]) -> [RatePoint; CURVE_POINTS] {
    let mut out = [RatePoint::default(); CURVE_POINTS];
    for (i, p) in points.iter().take(CURVE_POINTS).enumerate() {
        out[i] = *p;
    }
    out
}

/// Useful when converting an I80F48 (e.g. apr) into a percentage from 0-1000. Clamps to 1000% if
/// exceeding that amount. Clamps to zero for negative inputs.
pub fn milli_to_u32(value: I80F48) -> u32 {
    let max_percent: I80F48 = I80F48::from_num(10.0); // 1000%
    let clamped: I80F48 = value.min(max_percent).max(I80F48::ZERO);
    let ratio: I80F48 = clamped / max_percent;
    (ratio * I80F48::from_num(u32::MAX)).to_num::<u32>()
}

/// Useful when converting an I80F48 (e.g. utilization rate) into a percentage from 0-100. Clamps to
/// 100% if exceeding that amount. Clamps to zero for negative inputs.
pub fn centi_to_u32(value: I80F48) -> u32 {
    let max_percent: I80F48 = I80F48::from_num(1.0); // 100%
    let clamped: I80F48 = value.min(max_percent).max(I80F48::ZERO);
    let ratio: I80F48 = clamped / max_percent;
    (ratio * I80F48::from_num(u32::MAX)).to_num::<u32>()
}

/// Converts a rate u32 (0-1000% APR range) to I80F48 decimal.
/// Inverse of `milli_to_u32`.
/// Example: u32::MAX -> 10.0 (1000%), u32::MAX/10 -> ~1.0 (100%)
pub fn u32_to_milli(rate: u32) -> I80F48 {
    let ratio: I80F48 = I80F48::from_num(rate) / I80F48::from_num(u32::MAX);
    ratio * I80F48::from_num(10.0)
}

/// Converts a utilization u32 (0-100% range) to I80F48 decimal.
/// Inverse of `centi_to_u32`.
/// Example: u32::MAX -> 1.0 (100%), u32::MAX/2 -> ~0.5 (50%)
pub fn u32_to_centi(util: u32) -> I80F48 {
    I80F48::from_num(util) / I80F48::from_num(u32::MAX)
}

/// Useful when converting an I80F48 (e.g. leverage) into a value from 0-100. Clamps to 100 if
/// exceeding that amount. Clamps to zero for negative inputs.
pub fn basis_to_u32(value: I80F48) -> u32 {
    let max_value: I80F48 = I80F48::from_num(100.0); // 0-100 range
    let clamped: I80F48 = value.min(max_value).max(I80F48::ZERO);
    let ratio: I80F48 = clamped / max_value;
    (ratio * I80F48::from_num(u32::MAX)).to_num::<u32>()
}

/// Converts u32 back to leverage value (0-100 range)
pub fn u32_to_basis(value: u32) -> I80F48 {
    let ratio: I80F48 = I80F48::from_num(value) / I80F48::from_num(u32::MAX);
    ratio * I80F48::from_num(100.0)
}

#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct InterestRateConfigOpt {
    pub insurance_fee_fixed_apr: Option<WrappedI80F48>,
    pub insurance_ir_fee: Option<WrappedI80F48>,
    pub protocol_fixed_fee_apr: Option<WrappedI80F48>,
    pub protocol_ir_fee: Option<WrappedI80F48>,
    pub protocol_origination_fee: Option<WrappedI80F48>,

    /// The base rate at utilization = 0
    /// * a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX
    pub zero_util_rate: Option<u32>,
    /// The base rate at utilization = 100
    /// * a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX
    pub hundred_util_rate: Option<u32>,
    /// The base rate at various points between 0 and 100%, exclusive. Essentially a piece-wise
    /// linear curve.
    /// * always in ascending order, e.g. points[0] = first kink point, points[1] = second kink
    ///   point, and so forth.
    /// * points where util = 0 are unused
    pub points: Option<[RatePoint; 5]>,
}

#[repr(C)]
#[cfg_attr(feature = "anchor", derive(AnchorDeserialize, AnchorSerialize))]
#[derive(Default, Debug, PartialEq, Eq)]
pub struct InterestRateConfigCompact {
    // Fees
    pub insurance_fee_fixed_apr: WrappedI80F48,
    pub insurance_ir_fee: WrappedI80F48,
    pub protocol_fixed_fee_apr: WrappedI80F48,
    pub protocol_ir_fee: WrappedI80F48,
    pub protocol_origination_fee: WrappedI80F48,

    /// The base rate at utilization = 0
    /// * a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX
    pub zero_util_rate: u32,
    /// The base rate at utilization = 100
    /// * a %, as u32, out of 1000%, e.g. 100% = 0.1 * u32::MAX
    pub hundred_util_rate: u32,
    /// The base rate at various points between 0 and 100%, exclusive. Essentially a piece-wise
    /// linear curve.
    /// * always in ascending order, e.g. points[0] = first kink point, points[1] = second kink
    ///   point, and so forth.
    /// * points where util = 0 are unused
    pub points: [RatePoint; 5],
}

impl From<InterestRateConfigCompact> for InterestRateConfig {
    fn from(ir_config: InterestRateConfigCompact) -> Self {
        InterestRateConfig {
            optimal_utilization_rate: I80F48::ZERO.into(),
            plateau_interest_rate: I80F48::ZERO.into(),
            max_interest_rate: I80F48::ZERO.into(),
            insurance_fee_fixed_apr: ir_config.insurance_fee_fixed_apr,
            insurance_ir_fee: ir_config.insurance_ir_fee,
            protocol_fixed_fee_apr: ir_config.protocol_fixed_fee_apr,
            protocol_ir_fee: ir_config.protocol_ir_fee,
            protocol_origination_fee: ir_config.protocol_origination_fee,
            zero_util_rate: ir_config.zero_util_rate,
            hundred_util_rate: ir_config.hundred_util_rate,
            points: ir_config.points,
            curve_type: INTEREST_CURVE_SEVEN_POINT,
            _pad0: [0; 7],
            _padding1: [0; 32],
            _padding2: [0; 16],
            _padding3: [0; 8],
        }
    }
}

impl From<InterestRateConfig> for InterestRateConfigCompact {
    fn from(ir_config: InterestRateConfig) -> Self {
        InterestRateConfigCompact {
            insurance_fee_fixed_apr: ir_config.insurance_fee_fixed_apr,
            insurance_ir_fee: ir_config.insurance_ir_fee,
            protocol_fixed_fee_apr: ir_config.protocol_fixed_fee_apr,
            protocol_ir_fee: ir_config.protocol_ir_fee,
            protocol_origination_fee: ir_config.protocol_origination_fee,
            zero_util_rate: ir_config.zero_util_rate,
            hundred_util_rate: ir_config.hundred_util_rate,
            points: ir_config.points,
        }
    }
}
