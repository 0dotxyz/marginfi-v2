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

/// Multiply an `i128` by `liq_to_col_ratio`, returning `None` on overflow.
#[inline]
pub fn adjust_i128(raw: i128, liq_to_col_ratio: I80F48) -> Option<i128> {
    let raw_fx: I80F48 = i80_from_i128_checked(raw)?;
    let adj_fx: I80F48 = raw_fx.checked_mul(liq_to_col_ratio)?;
    adj_fx.checked_to_num::<i128>()
}

/// Multiply an `i64` by `liq_to_col_ratio`, returning `None` on overflow.
#[inline]
pub fn adjust_i64(raw: i64, liq_to_col_ratio: I80F48) -> Option<i64> {
    I80F48::from_num(raw)
        .checked_mul(liq_to_col_ratio)?
        .checked_to_num::<i64>()
}

/// Multiply a `u64` by `liq_to_col_ratio`, returning `None` on overflow.
#[inline]
pub fn adjust_u64(raw: u64, liq_to_col_ratio: I80F48) -> Option<u64> {
    I80F48::from_num(raw)
        .checked_mul(liq_to_col_ratio)?
        .checked_to_num::<u64>()
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
