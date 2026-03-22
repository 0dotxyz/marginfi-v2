use fixed::types::I80F48;

use crate::constants::EXP_10_I80F48;

/// Convert an `i128` into `I80F48` only if it fits without overflow.
#[inline]
pub fn i80_from_i128_checked(x: i128) -> Option<I80F48> {
    const FRAC_BITS: u32 = 48;
    const SHIFTED_MAX_I128: i128 = i128::MAX >> FRAC_BITS;
    const SHIFTED_MIN_I128: i128 = i128::MIN >> FRAC_BITS;

    if !(SHIFTED_MIN_I128..=SHIFTED_MAX_I128).contains(&x) {
        return None;
    }

    Some(I80F48::from_bits(x << FRAC_BITS))
}

/// Multiply two `I80F48` values, returning `None` on overflow.
#[inline]
pub fn mul_i80f48(value: I80F48, multiplier: I80F48) -> Option<I80F48> {
    value.checked_mul(multiplier)
}

/// Multiply and divide `I80F48` values, returning `None` on overflow or divide-by-zero.
#[inline]
pub fn mul_div_i80f48(value: I80F48, numerator: I80F48, denominator: I80F48) -> Option<I80F48> {
    if denominator == I80F48::ZERO {
        return None;
    }

    value.checked_mul(numerator)?.checked_div(denominator)
}

/// Multiply an `i128` by an `I80F48` multiplier, returning `None` on overflow.
#[inline]
pub fn mul_i128_by_i80f48(value: i128, multiplier: I80F48) -> Option<i128> {
    let value_i80f48: I80F48 = i80_from_i128_checked(value)?;
    let product_i80f48 = mul_i80f48(value_i80f48, multiplier)?;
    product_i80f48.checked_to_num::<i128>()
}

/// Multiply an `i64` by an `I80F48` multiplier, returning `None` on overflow.
#[inline]
pub fn mul_i64_by_i80f48(value: i64, multiplier: I80F48) -> Option<i64> {
    let value_i80f48 = I80F48::from_num(value);
    let product_i80f48 = mul_i80f48(value_i80f48, multiplier)?;
    product_i80f48.checked_to_num::<i64>()
}

/// Multiply a `u64` by an `I80F48` multiplier, returning `None` on overflow.
#[inline]
pub fn mul_u64_by_i80f48(value: u64, multiplier: I80F48) -> Option<u64> {
    let value_i80f48 = I80F48::from_num(value);
    let product_i80f48 = mul_i80f48(value_i80f48, multiplier)?;
    product_i80f48.checked_to_num::<u64>()
}

/// Multiply a `u128` by a `u128` ratio (`numerator/denominator`) with floor rounding.
#[inline]
pub fn mul_div_u128(value: u128, numerator: u128, denominator: u128) -> Option<u128> {
    if denominator == 0 {
        return None;
    }

    value.checked_mul(numerator)?.checked_div(denominator)
}

/// Multiply an `i64` by a `u128` ratio (`numerator/denominator`) with floor rounding.
#[inline]
pub fn mul_div_i64(value: i64, numerator: u128, denominator: u128) -> Option<i64> {
    let value_u128 = u128::try_from(value).ok()?;
    let adjusted_value = mul_div_u128(value_u128, numerator, denominator)?;
    adjusted_value.try_into().ok()
}

/// Multiply a `u64` by a `u128` ratio (`numerator/denominator`) with floor rounding.
#[inline]
pub fn mul_div_u64(value: u64, numerator: u128, denominator: u128) -> Option<u64> {
    let adjusted_value = mul_div_u128(value as u128, numerator, denominator)?;
    adjusted_value.try_into().ok()
}

/// Multiply an `i128` by a `u128` ratio (`numerator/denominator`) with floor rounding.
#[inline]
pub fn mul_div_i128(value: i128, numerator: u128, denominator: u128) -> Option<i128> {
    let value_u128 = u128::try_from(value).ok()?;
    let adjusted_value = mul_div_u128(value_u128, numerator, denominator)?;
    adjusted_value.try_into().ok()
}

/// Convert collateral tokens to liquidity tokens given scaled supplies.
/// Returns None on overflow or divide-by-zero.
#[inline]
pub fn collateral_to_liquidity_from_scaled(
    collateral: u64,
    total_liq: I80F48,
    total_col: I80F48,
) -> Option<u64> {
    if total_col == I80F48::ZERO {
        return None;
    }

    I80F48::from_num(collateral)
        .checked_mul(total_liq)?
        .checked_div(total_col)?
        .checked_to_num::<u64>()
}

/// Convert liquidity tokens to collateral tokens given scaled supplies.
/// Returns None on overflow or divide-by-zero.
#[inline]
pub fn liquidity_to_collateral_from_scaled(
    liquidity: u64,
    total_liq: I80F48,
    total_col: I80F48,
) -> Option<u64> {
    if total_liq == I80F48::ZERO {
        return None;
    }

    I80F48::from_num(liquidity)
        .checked_mul(total_col)?
        .checked_div(total_liq)?
        .checked_to_num::<u64>()
}

/// Compute liquidity-to-collateral ratio; returns None if total_col is zero.
#[inline]
pub fn liq_to_col_ratio(total_liq: I80F48, total_col: I80F48) -> Option<I80F48> {
    if total_col == I80F48::ZERO {
        None
    } else {
        total_liq.checked_div(total_col)
    }
}

/// Compute collateral-to-liquidity ratio; returns None if total_liq is zero.
#[inline]
pub fn col_to_liq_ratio(total_liq: I80F48, total_col: I80F48) -> Option<I80F48> {
    if total_liq == I80F48::ZERO {
        None
    } else {
        total_col.checked_div(total_liq)
    }
}

/// Scale raw total_liq and total_col by 10^decimals. Returns None on overflow or bad index.
#[inline]
pub fn scale_supplies(
    total_liq_raw: I80F48,
    total_col_raw: u64,
    decimals: u8,
) -> Option<(I80F48, I80F48)> {
    let scale: I80F48 = *EXP_10_I80F48.get(decimals as usize)?;
    let total_liq: I80F48 = total_liq_raw.checked_div(scale)?;
    let total_col: I80F48 = I80F48::from_num(total_col_raw).checked_div(scale)?;
    Some((total_liq, total_col))
}

/// Convert between decimal precisions. Returns None on unsupported diff or overflow.
#[inline]
pub fn convert_decimals(n: I80F48, from_dec: u8, to_dec: u8) -> Option<I80F48> {
    if from_dec == to_dec {
        return Some(n);
    }

    let diff = (to_dec as i32) - (from_dec as i32);
    let abs = diff.unsigned_abs() as usize;

    if abs > 23 {
        return None;
    }

    let scale: I80F48 = EXP_10_I80F48[abs];

    let out: I80F48 = if diff > 0 {
        n.checked_mul(scale)?
    } else {
        n.checked_div(scale)?
    };

    Some(out)
}
