//! Bit-exact ports of the `org.joml.Math` methods vanilla calls (JOML 1.10.9, the version the
//! 26.3 server bundles), used by `com.mojang.math.GivensParameters` and `MatrixUtil`.
//!
//! JOML is configured by system properties that vanilla never sets, so `joml.fastmath` is
//! off: `sin` delegates to `java.lang.Math.sin` (the HotSpot intrinsic, [`hotspot::sin`]) and
//! the square roots to the correctly rounded `java.lang.Math.sqrt`. Each function follows
//! JOML's bytecode, including its float/double conversions and NaN comparisons.
//!
//! JOML is Copyright (c) 2015-2024 JOML (Kai Burjack and contributors), MIT license.

use crate::hotspot;

/// `(float) (java.lang.Math.PI / 2)`.
const PI_OVER_2_F: f32 = std::f32::consts::FRAC_PI_2;
/// `(float) (java.lang.Math.PI * 2)`.
const PI_TIMES_2_F: f32 = std::f32::consts::TAU;
/// `(float) java.lang.Math.PI`.
const PI_F: f32 = std::f32::consts::PI;

/// `Math.sin(float)`: `(float) java.lang.Math.sin(rad)`.
#[must_use]
pub fn sin(rad: f32) -> f32 {
    hotspot::sin(f64::from(rad)) as f32
}

/// `Math.sqrt(float)`: `(float) java.lang.Math.sqrt(r)`, the square root taken in double.
#[must_use]
pub fn sqrt(r: f32) -> f32 {
    f64::from(r).sqrt() as f32
}

/// `Math.invsqrt(float)`: `1.0F / (float) java.lang.Math.sqrt(r)`.
#[must_use]
pub fn invsqrt(r: f32) -> f32 {
    1.0 / sqrt(r)
}

/// `Math.abs(float)`: `java.lang.Math.abs`, which clears the sign bit (even of NaN).
#[must_use]
pub fn abs(r: f32) -> f32 {
    r.abs()
}

/// `Math.max(float, float)`: `a > b ? a : b`. Unlike `java.lang.Math.max`, a NaN `a` gives
/// `b`, and `max(-0.0, 0.0)` is `0.0` while `max(0.0, -0.0)` is `-0.0`.
#[must_use]
pub fn max(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// `Math.cosFromSin(float sin, float angle)`: the cosine of `angle` from its known sine, with
/// the sign chosen by reducing `angle + pi/2` into `[0, 2pi)` in float arithmetic.
#[must_use]
pub fn cos_from_sin(sin: f32, angle: f32) -> f32 {
    let cos = sqrt(1.0 - sin * sin);
    let a = angle + PI_OVER_2_F;
    // `(int)` saturates and maps NaN to 0, like Rust's `as`.
    let mut b = a - (a / PI_TIMES_2_F) as i32 as f32 * PI_TIMES_2_F;
    // `b < 0.0` widens to double (`dcmpg`): false for NaN.
    if f64::from(b) < 0.0 {
        b += PI_TIMES_2_F;
    }
    // `b >= PI_f` (`fcmpl`): false for NaN.
    if b >= PI_F { -cos } else { cos }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_jomls_float_casts() {
        assert_eq!(PI_OVER_2_F.to_bits(), 0x3FC9_0FDB); // 1.5707964f
        assert_eq!(PI_TIMES_2_F.to_bits(), 0x40C9_0FDB); // 6.2831855f
        assert_eq!(PI_F.to_bits(), 0x4049_0FDB); // 3.1415927f
    }

    #[test]
    fn max_is_not_java_lang_max() {
        assert_eq!(max(f32::NAN, 1.0), 1.0);
        assert!(max(1.0, f32::NAN).is_nan());
        assert_eq!(max(0.0, -0.0).to_bits(), 0x8000_0000);
    }

    #[test]
    fn cos_from_sin_picks_the_sign_of_the_cosine() {
        for angle in [-3.0_f32, -1.0, 0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 10.0] {
            let expected = f64::from(angle).cos();
            let actual = f64::from(cos_from_sin(sin(angle), angle));
            assert!((expected - actual).abs() < 1e-6, "angle {angle}");
        }
    }
}
