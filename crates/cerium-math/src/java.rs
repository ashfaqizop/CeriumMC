//! Java numeric semantics that differ from Rust's defaults.
//!
//! Each function reproduces the behaviour of the named `java.lang.Math` or
//! `java.lang.Float` method exactly (as of Java 25), including NaN and signed-zero handling.
//! Where Java throws an exception, the function returns `None`.

/// `Math.max(float, float)`: NaN if either argument is NaN, and `+0.0` beats `-0.0`.
#[must_use]
pub fn max_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

/// `Math.min(float, float)`: NaN if either argument is NaN, and `-0.0` beats `+0.0`.
#[must_use]
pub fn min_f32(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    if a <= b { a } else { b }
}

/// `Math.max(double, double)`: NaN if either argument is NaN, and `+0.0` beats `-0.0`.
#[must_use]
pub fn max_f64(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

/// `Math.min(double, double)`: NaN if either argument is NaN, and `-0.0` beats `+0.0`.
#[must_use]
pub fn min_f64(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    if a <= b { a } else { b }
}

/// `Math.floorDiv(int, int)`: division rounding toward negative infinity.
/// `i32::MIN / -1` wraps to `i32::MIN`, as in Java.
///
/// # Panics
///
/// Panics if `y` is zero, where Java throws `ArithmeticException`.
#[must_use]
pub fn floor_div_i32(x: i32, y: i32) -> i32 {
    let quotient = x.wrapping_div(y);
    if (x ^ y) < 0 && quotient.wrapping_mul(y) != x {
        quotient.wrapping_sub(1)
    } else {
        quotient
    }
}

/// `Math.floorDiv(long, long)`: division rounding toward negative infinity.
/// `i64::MIN / -1` wraps to `i64::MIN`, as in Java.
///
/// # Panics
///
/// Panics if `y` is zero, where Java throws `ArithmeticException`.
#[must_use]
pub fn floor_div_i64(x: i64, y: i64) -> i64 {
    let quotient = x.wrapping_div(y);
    if (x ^ y) < 0 && quotient.wrapping_mul(y) != x {
        quotient.wrapping_sub(1)
    } else {
        quotient
    }
}

/// `Math.floorMod(int, int)`: the remainder takes the sign of the divisor.
///
/// # Panics
///
/// Panics if `y` is zero, where Java throws `ArithmeticException`.
#[must_use]
pub fn floor_mod_i32(x: i32, y: i32) -> i32 {
    let remainder = x.wrapping_rem(y);
    if (remainder ^ y) < 0 && remainder != 0 {
        remainder + y
    } else {
        remainder
    }
}

/// `Math.floorMod(long, long)`: the remainder takes the sign of the divisor.
///
/// # Panics
///
/// Panics if `y` is zero, where Java throws `ArithmeticException`.
#[must_use]
pub fn floor_mod_i64(x: i64, y: i64) -> i64 {
    let remainder = x.wrapping_rem(y);
    if (remainder ^ y) < 0 && remainder != 0 {
        remainder + y
    } else {
        remainder
    }
}

/// `Float.floatToIntBits`: like [`f32::to_bits`], but every NaN maps to `0x7fc00000`.
#[must_use]
pub fn float_to_int_bits(value: f32) -> i32 {
    if value.is_nan() {
        0x7FC0_0000
    } else {
        value.to_bits().cast_signed()
    }
}

/// `Math.round(float)`: `floor(a + 1/2)` computed exactly, without the rounding error of
/// adding `0.5`. NaN gives 0 and out-of-range values saturate, like Java's `(int)` cast.
/// Unlike [`f32::round`], ties go toward positive infinity: `round(-2.5) == -2`.
#[must_use]
pub fn round_f32(a: f32) -> i32 {
    const SIGNIFICAND_WIDTH: i32 = 24;
    const EXP_BIAS: i32 = 127;
    const SIGNIF_BIT_MASK: i32 = 0x007F_FFFF;
    let int_bits = a.to_bits().cast_signed();
    let biased_exp = (int_bits & 0x7F80_0000) >> (SIGNIFICAND_WIDTH - 1);
    let shift = (SIGNIFICAND_WIDTH - 2 + EXP_BIAS) - biased_exp;
    if (shift & -32) == 0 {
        // a is finite with 2^-32 <= ulp(a) < 1
        let mut r = (int_bits & SIGNIF_BIT_MASK) | (SIGNIF_BIT_MASK + 1);
        if int_bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        a as i32
    }
}

/// `Math.round(double)`: `floor(a + 1/2)` computed exactly. NaN gives 0 and out-of-range values
/// saturate, like Java's `(long)` cast. Ties go toward positive infinity.
#[must_use]
pub fn round_f64(a: f64) -> i64 {
    const SIGNIFICAND_WIDTH: i64 = 53;
    const EXP_BIAS: i64 = 1023;
    const SIGNIF_BIT_MASK: i64 = 0x000F_FFFF_FFFF_FFFF;
    let long_bits = a.to_bits().cast_signed();
    let biased_exp = (long_bits & 0x7FF0_0000_0000_0000) >> (SIGNIFICAND_WIDTH - 1);
    let shift = (SIGNIFICAND_WIDTH - 2 + EXP_BIAS) - biased_exp;
    if (shift & -64) == 0 {
        // a is finite with 2^-64 <= ulp(a) < 1
        let mut r = (long_bits & SIGNIF_BIT_MASK) | (SIGNIF_BIT_MASK + 1);
        if long_bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        a as i64
    }
}

/// `Math.signum(double)`: zeros and NaN are returned unchanged, anything else is `±1.0`.
#[must_use]
pub fn signum_f64(d: f64) -> f64 {
    if d == 0.0 || d.is_nan() {
        d
    } else {
        copy_sign_f64(1.0, d)
    }
}

/// `Math.signum(float)`: zeros and NaN are returned unchanged, anything else is `±1.0`.
#[must_use]
pub fn signum_f32(f: f32) -> f32 {
    if f == 0.0 || f.is_nan() {
        f
    } else {
        copy_sign_f32(1.0, f)
    }
}

/// `Math.copySign(double, double)`: the sign bit of `sign`, even for NaN.
#[must_use]
pub fn copy_sign_f64(magnitude: f64, sign: f64) -> f64 {
    const SIGN_BIT_MASK: u64 = 0x8000_0000_0000_0000;
    f64::from_bits((sign.to_bits() & SIGN_BIT_MASK) | (magnitude.to_bits() & !SIGN_BIT_MASK))
}

/// `Math.copySign(float, float)`: the sign bit of `sign`, even for NaN.
#[must_use]
pub fn copy_sign_f32(magnitude: f32, sign: f32) -> f32 {
    const SIGN_BIT_MASK: u32 = 0x8000_0000;
    f32::from_bits((sign.to_bits() & SIGN_BIT_MASK) | (magnitude.to_bits() & !SIGN_BIT_MASK))
}

/// `Math.toRadians(double)`: `angdeg * DEGREES_TO_RADIANS`.
#[must_use]
pub fn to_radians(angdeg: f64) -> f64 {
    angdeg * 0.017_453_292_519_943_295
}

/// `Math.toDegrees(double)`: `angrad * RADIANS_TO_DEGREES`.
#[must_use]
pub fn to_degrees(angrad: f64) -> f64 {
    angrad * 57.295_779_513_082_32
}

/// `Math.nextUp(double)`.
#[must_use]
pub fn next_up_f64(d: f64) -> f64 {
    if d < f64::INFINITY {
        // Adding +0.0 turns -0.0 into +0.0.
        let transducer = (d + 0.0).to_bits().cast_signed();
        let step = if transducer >= 0 { 1 } else { -1 };
        f64::from_bits(transducer.wrapping_add(step).cast_unsigned())
    } else {
        d // NaN or +Infinity
    }
}

/// `Math.nextUp(float)`.
#[must_use]
pub fn next_up_f32(f: f32) -> f32 {
    if f < f32::INFINITY {
        // Adding +0.0 turns -0.0 into +0.0.
        let transducer = (f + 0.0).to_bits().cast_signed();
        let step = if transducer >= 0 { 1 } else { -1 };
        f32::from_bits(transducer.wrapping_add(step).cast_unsigned())
    } else {
        f // NaN or +Infinity
    }
}

/// `Math.nextDown(double)`.
#[must_use]
pub fn next_down_f64(d: f64) -> f64 {
    if d.is_nan() || d == f64::NEG_INFINITY {
        d
    } else if d == 0.0 {
        -f64::from_bits(1) // -Double.MIN_VALUE
    } else {
        let step = if d > 0.0 { -1 } else { 1 };
        f64::from_bits(d.to_bits().cast_signed().wrapping_add(step).cast_unsigned())
    }
}

/// `Math.nextDown(float)`.
#[must_use]
pub fn next_down_f32(f: f32) -> f32 {
    if f.is_nan() || f == f32::NEG_INFINITY {
        f
    } else if f == 0.0 {
        -f32::from_bits(1) // -Float.MIN_VALUE
    } else {
        let step = if f > 0.0 { -1 } else { 1 };
        f32::from_bits(f.to_bits().cast_signed().wrapping_add(step).cast_unsigned())
    }
}

/// `Math.clamp(long, int, int)`, the overload Java picks for `int` arguments. Returns `None`
/// where Java throws `IllegalArgumentException` (`min > max`).
#[must_use]
pub fn clamp_i64_to_i32(value: i64, min: i32, max: i32) -> Option<i32> {
    if min > max {
        return None;
    }
    Some(value.clamp(i64::from(min), i64::from(max)) as i32)
}

/// `Math.clamp(long, long, long)`. Returns `None` where Java throws `IllegalArgumentException`
/// (`min > max`).
#[must_use]
pub fn clamp_i64(value: i64, min: i64, max: i64) -> Option<i64> {
    if min > max {
        return None;
    }
    Some(value.clamp(min, max))
}

/// `Math.clamp(double, double, double)`: `min(max, max(value, min))` with Java's NaN and
/// signed-zero rules, so a NaN `value` stays NaN. Returns `None` where Java throws
/// `IllegalArgumentException`: a NaN bound, or `min > max` (where `+0.0 > -0.0`).
#[must_use]
pub fn clamp_f64(value: f64, min: f64, max: f64) -> Option<f64> {
    // Java tests `!(min < max)` first: false for equal bounds, NaN bounds and min > max.
    let ordered = min < max;
    if !ordered && (min.is_nan() || max.is_nan() || min.total_cmp(&max).is_gt()) {
        return None;
    }
    Some(min_f64(max, max_f64(value, min)))
}

/// `Math.clamp(float, float, float)`: `min(max, max(value, min))` with Java's NaN and
/// signed-zero rules. Returns `None` where Java throws `IllegalArgumentException`: a NaN bound,
/// or `min > max` (where `+0.0 > -0.0`).
#[must_use]
pub fn clamp_f32(value: f32, min: f32, max: f32) -> Option<f32> {
    // Java tests `!(min < max)` first: false for equal bounds, NaN bounds and min > max.
    let ordered = min < max;
    if !ordered && (min.is_nan() || max.is_nan() || min.total_cmp(&max).is_gt()) {
        return None;
    }
    Some(min_f32(max, max_f32(value, min)))
}

/// `Math.toIntExact(long)`. Returns `None` where Java throws `ArithmeticException`.
#[must_use]
pub fn to_int_exact(value: i64) -> Option<i32> {
    i32::try_from(value).ok()
}

/// `Math.absExact(int)`. Returns `None` where Java throws `ArithmeticException`
/// (`Integer.MIN_VALUE`).
#[must_use]
pub fn abs_exact_i32(a: i32) -> Option<i32> {
    a.checked_abs()
}

/// `Math.absExact(long)`. Returns `None` where Java throws `ArithmeticException`
/// (`Long.MIN_VALUE`).
#[must_use]
pub fn abs_exact_i64(a: i64) -> Option<i64> {
    a.checked_abs()
}

/// `Math.negateExact(int)`. Returns `None` where Java throws `ArithmeticException`.
#[must_use]
pub fn negate_exact_i32(a: i32) -> Option<i32> {
    a.checked_neg()
}

/// `Math.negateExact(long)`. Returns `None` where Java throws `ArithmeticException`.
#[must_use]
pub fn negate_exact_i64(a: i64) -> Option<i64> {
    a.checked_neg()
}

/// `Math.subtractExact(int, int)`. Returns `None` where Java throws `ArithmeticException`.
#[must_use]
pub fn subtract_exact_i32(x: i32, y: i32) -> Option<i32> {
    x.checked_sub(y)
}

/// `Math.subtractExact(long, long)`. Returns `None` where Java throws `ArithmeticException`.
#[must_use]
pub fn subtract_exact_i64(x: i64, y: i64) -> Option<i64> {
    x.checked_sub(y)
}

/// `Math.floorDivExact(int, int)`. Returns `None` where Java throws `ArithmeticException`:
/// division by zero, or `Integer.MIN_VALUE / -1`.
#[must_use]
pub fn floor_div_exact_i32(x: i32, y: i32) -> Option<i32> {
    let q = x.checked_div(y)?;
    if (x ^ y) < 0 && q * y != x {
        Some(q - 1)
    } else {
        Some(q)
    }
}

/// `Math.floorDivExact(long, long)`. Returns `None` where Java throws `ArithmeticException`:
/// division by zero, or `Long.MIN_VALUE / -1`.
#[must_use]
pub fn floor_div_exact_i64(x: i64, y: i64) -> Option<i64> {
    let q = x.checked_div(y)?;
    if (x ^ y) < 0 && q * y != x {
        Some(q - 1)
    } else {
        Some(q)
    }
}

/// `Math.powExact(int, int)` (new in Java 25). Returns `None` where Java throws
/// `ArithmeticException`: a negative exponent, or overflow. Like Java, the running product is
/// multiplied without an overflow check; only the squarings and the last product are checked.
#[must_use]
pub fn pow_exact_i32(x: i32, n: i32) -> Option<i32> {
    if n < 0 {
        return None;
    }
    if n == 0 {
        return Some(1);
    }
    if x == 0 || x == 1 {
        return Some(x);
    }
    if x == -1 {
        return Some(if n & 1 == 0 { 1 } else { -1 });
    }
    let (mut x, mut n, mut p) = (x, n.cast_unsigned(), 1_i32);
    while n > 1 {
        if n & 1 != 0 {
            p = p.wrapping_mul(x);
        }
        x = x.checked_mul(x)?;
        n >>= 1;
    }
    p.checked_mul(x)
}

/// `Math.powExact(long, int)` (new in Java 25). Returns `None` where Java throws
/// `ArithmeticException`: a negative exponent, or overflow. Like Java, the running product is
/// multiplied without an overflow check; only the squarings and the last product are checked.
#[must_use]
pub fn pow_exact_i64(x: i64, n: i32) -> Option<i64> {
    if n < 0 {
        return None;
    }
    if n == 0 {
        return Some(1);
    }
    if x == 0 || x == 1 {
        return Some(x);
    }
    if x == -1 {
        return Some(if n & 1 == 0 { 1 } else { -1 });
    }
    let (mut x, mut n, mut p) = (x, n.cast_unsigned(), 1_i64);
    while n > 1 {
        if n & 1 != 0 {
            p = p.wrapping_mul(x);
        }
        x = x.checked_mul(x)?;
        n >>= 1;
    }
    p.checked_mul(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_follows_java_not_rust() {
        assert_eq!(round_f64(-2.5), -2);
        assert_eq!(round_f64(2.5), 3);
        assert_eq!(round_f64(0.499_999_999_999_999_94), 0);
        assert_eq!(round_f64(f64::NAN), 0);
        assert_eq!(round_f64(1e300), i64::MAX);
        assert_eq!(round_f32(-0.5), 0);
        assert_eq!(round_f32(0.499_999_97), 0);
        assert_eq!(round_f32(-1e30), i32::MIN);
    }

    #[test]
    fn exact_arithmetic_reports_overflow() {
        assert_eq!(abs_exact_i32(i32::MIN), None);
        assert_eq!(negate_exact_i64(i64::MIN), None);
        assert_eq!(floor_div_exact_i32(i32::MIN, -1), None);
        assert_eq!(floor_div_exact_i32(1, 0), None);
        assert_eq!(floor_div_exact_i32(-7, 2), Some(-4));
        assert_eq!(pow_exact_i32(3, 19), Some(1_162_261_467));
        assert_eq!(pow_exact_i32(3, 20), None);
        assert_eq!(pow_exact_i32(2, -1), None);
        assert_eq!(pow_exact_i64(-1, 7), Some(-1));
    }

    #[test]
    fn clamp_follows_java() {
        assert_eq!(clamp_i64_to_i32(5_000_000_000, 0, 10), Some(10));
        assert_eq!(clamp_i64(1, 2, 1), None);
        assert!(clamp_f64(f64::NAN, 0.0, 1.0).is_some_and(f64::is_nan));
        assert_eq!(clamp_f64(0.5, f64::NAN, 1.0), None);
        assert_eq!(clamp_f64(0.5, 0.0, -0.0), None);
        assert_eq!(clamp_f64(0.5, -0.0, 0.0).map(f64::to_bits), Some(0));
        assert_eq!(clamp_f32(-1.0, -0.0, -0.0).map(f32::to_bits), Some(0x8000_0000));
    }

    #[test]
    fn max_and_min_follow_java_for_nan_and_signed_zero() {
        assert!(max_f32(f32::NAN, 1.0).is_nan());
        assert!(max_f32(1.0, f32::NAN).is_nan());
        assert!(min_f32(1.0, f32::NAN).is_nan());
        assert!(max_f32(-0.0, 0.0).is_sign_positive());
        assert!(max_f32(0.0, -0.0).is_sign_positive());
        assert!(min_f32(0.0, -0.0).is_sign_negative());
        assert!(min_f32(-0.0, 0.0).is_sign_negative());
        assert!(max_f64(-0.0, 0.0).is_sign_positive());
        assert!(min_f64(0.0, -0.0).is_sign_negative());
        assert!(max_f64(f64::NAN, 1.0).is_nan());
        assert!(min_f64(1.0, f64::NAN).is_nan());
    }

    #[test]
    fn floor_div_and_mod_match_java() {
        assert_eq!(floor_div_i32(7, 2), 3);
        assert_eq!(floor_div_i32(-7, 2), -4);
        assert_eq!(floor_div_i32(7, -2), -4);
        assert_eq!(floor_div_i32(-7, -2), 3);
        assert_eq!(floor_div_i32(i32::MIN, -1), i32::MIN);
        assert_eq!(floor_mod_i32(-7, 2), 1);
        assert_eq!(floor_mod_i32(7, -2), -1);
        assert_eq!(floor_mod_i32(-7, -2), -1);
        assert_eq!(floor_mod_i32(i32::MIN, -1), 0);
        assert_eq!(floor_div_i64(-7, 2), -4);
        assert_eq!(floor_mod_i64(-7, 2), 1);
        assert_eq!(floor_mod_i64(7, -2), -1);
    }

    #[test]
    fn float_to_int_bits_canonicalizes_nan() {
        assert_eq!(float_to_int_bits(f32::from_bits(0xFFC0_0001)), 0x7FC0_0000);
        assert_eq!(float_to_int_bits(1.0), 0x3F80_0000);
        assert_eq!(float_to_int_bits(-0.0), i32::MIN);
    }
}
