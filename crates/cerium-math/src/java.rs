//! Java numeric semantics that differ from Rust's defaults.
//!
//! Each function reproduces the behaviour of the named `java.lang.Math` or
//! `java.lang.Float` method exactly, including NaN and signed-zero handling.

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

#[cfg(test)]
mod tests {
    use super::*;

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
