//! Bit-exact ports of the `org.joml.Math` methods vanilla calls (JOML 1.10.9, the version the
//! 26.3 server bundles), used by `com.mojang.math.GivensParameters` and `MatrixUtil`.
//!
//! JOML is configured by system properties that vanilla never sets, so `joml.fastmath` is
//! off: `sin` delegates to `java.lang.Math.sin` (the HotSpot intrinsic, [`hotspot::sin`]) and
//! the square roots to the correctly rounded `java.lang.Math.sqrt`. Each function follows
//! JOML's bytecode, including its float/double conversions and NaN comparisons.
//!
//! NaN results reproduce Java on x86_64 bit for bit on every platform. Here only `sqrt` of a
//! negative number creates a NaN (the x86 default NaN, which is negative; ARM's is positive),
//! and conversions between float and double keep a NaN's sign and leading payload bits, as
//! x86's `cvtss2sd` and `cvtsd2ss` do. Every other operation only propagates NaNs, which all
//! supported CPUs do identically.
//!
//! JOML is Copyright (c) 2015-2024 JOML (Kai Burjack and contributors), MIT license.

use crate::hotspot;

/// `(float) (java.lang.Math.PI / 2)`.
const PI_OVER_2_F: f32 = std::f32::consts::FRAC_PI_2;
/// `(float) (java.lang.Math.PI * 2)`.
const PI_TIMES_2_F: f32 = std::f32::consts::TAU;
/// `(float) java.lang.Math.PI`.
const PI_F: f32 = std::f32::consts::PI;

/// `(double) f` as x86 converts it (`cvtss2sd`): a NaN keeps its sign and payload, quieted.
fn widen(f: f32) -> f64 {
    if f.is_nan() {
        let bits = f.to_bits();
        let sign = u64::from(bits & 0x8000_0000) << 32;
        let payload = u64::from(bits & 0x003F_FFFF) << 29;
        f64::from_bits(sign | 0x7FF8_0000_0000_0000 | payload)
    } else {
        f64::from(f)
    }
}

/// `(float) d` as x86 converts it (`cvtsd2ss`): a NaN keeps its sign and the leading bits of
/// its payload, quieted.
fn narrow(d: f64) -> f32 {
    if d.is_nan() {
        let bits = d.to_bits();
        let sign = (bits >> 32) as u32 & 0x8000_0000;
        let payload = (bits >> 29) as u32 & 0x003F_FFFF;
        f32::from_bits(sign | 0x7FC0_0000 | payload)
    } else {
        d as f32
    }
}

/// `java.lang.Math.sqrt(double)` as x86 computes it (`sqrtsd`): a NaN is returned quieted, and
/// a negative number gives the default NaN.
fn sqrt_f64(d: f64) -> f64 {
    if d.is_nan() {
        f64::from_bits(d.to_bits() | 0x0008_0000_0000_0000)
    } else if d < 0.0 {
        f64::from_bits(0xFFF8_0000_0000_0000)
    } else {
        d.sqrt()
    }
}

/// `Math.sin(float)`: `(float) java.lang.Math.sin(rad)`.
#[must_use]
pub fn sin(rad: f32) -> f32 {
    narrow(hotspot::sin(widen(rad)))
}

/// `Math.sqrt(float)`: `(float) java.lang.Math.sqrt(r)`, the square root taken in double.
#[must_use]
pub fn sqrt(r: f32) -> f32 {
    narrow(sqrt_f64(widen(r)))
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
    if widen(b) < 0.0 {
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
    fn nan_bits_follow_x86() {
        assert_eq!(sqrt(-0.5).to_bits(), 0xFFC0_0000);
        assert_eq!(invsqrt(-1.0).to_bits(), 0xFFC0_0000);
        assert_eq!(sqrt(f32::from_bits(0x7F80_0001)).to_bits(), 0x7FC0_0001);
        assert_eq!(sin(f32::INFINITY).to_bits(), 0xFFC0_0000);
        assert_eq!(
            widen(f32::from_bits(0xFFC0_0001)).to_bits(),
            0xFFF8_0000_2000_0000
        );
        assert_eq!(
            narrow(f64::from_bits(0x7FF0_0000_2000_0000)).to_bits(),
            0x7FC0_0001
        );
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
