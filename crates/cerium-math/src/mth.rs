//! Vanilla's `Mth` math helpers, ported bit for bit.
//!
//! Java overloads become separate functions with a type suffix (`floor_f32`, `floor_f64`, …).
//! Integer arithmetic wraps exactly where Java's does. Float arithmetic is evaluated in the
//! same order as vanilla, because IEEE-754 results depend on it. The helpers that take a
//! random source live in `cerium-random`.

use std::f64::consts::{FRAC_PI_2, PI as PI_F64};
use std::sync::OnceLock;

use crate::java;

/// `(float) Math.PI`.
pub const PI: f32 = PI_F64 as f32;
/// `(float) (Math.PI / 2)`.
pub const HALF_PI: f32 = FRAC_PI_2 as f32;
/// `(float) (Math.PI * 2)`.
pub const TWO_PI: f32 = (PI_F64 * 2.0) as f32;
/// `(float) (Math.PI / 180.0)`.
pub const DEG_TO_RAD: f32 = (PI_F64 / 180.0) as f32;
/// `180.0F / (float) Math.PI`.
pub const RAD_TO_DEG: f32 = 180.0 / PI;
/// Tolerance used by [`equal_f32`] and [`equal_f64`].
pub const EPSILON: f32 = 1.0e-5;
/// `Mth.sqrt(2.0F)`.
pub const SQRT_OF_TWO: f32 = std::f32::consts::SQRT_2;

/// Number of entries in the sine table: one full turn.
const SIN_QUANTIZATION: usize = 65_536;
/// Table entries per radian: `65536 / (2π)`.
const SIN_SCALE: f64 = 10_430.378_350_470_453;
/// A quarter turn in table entries; `cos(x) = sin(x + π/2)`.
const COS_OFFSET: f64 = 16_384.0;

/// Entries in the arcsine and cosine tables used by [`atan2`].
pub const ATAN_TABLE_SIZE: usize = 257;
/// `2^44`: adding it to a value in `[0, 1]` leaves `value * 256` in the low mantissa bits.
const FRAC_BIAS: f64 = 17_592_186_044_416.0;
/// `0.16666666666666666`, the nearest double to 1/6.
const ONE_SIXTH: f64 = 1.0 / 6.0;
/// `0.6666667F`, the nearest float to 2/3.
const TWO_THIRDS: f32 = 2.0 / 3.0;

type AtanTables = ([f64; ATAN_TABLE_SIZE], [f64; ATAN_TABLE_SIZE]);

/// Positions of the lowest set bit for De Bruijn multiplication by `0x077CB531`.
const MULTIPLY_DE_BRUIJN_BIT_POSITION: [i32; 32] = [
    0, 1, 28, 2, 29, 14, 24, 3, 30, 22, 20, 15, 25, 17, 4, 8, 31, 27, 13, 23, 21, 19, 16, 7, 26,
    12, 18, 6, 11, 5, 10, 9,
];

/// Vanilla's 65536-entry sine table: entry `i` is `(float) sin(i * 2π / 65536)`.
#[must_use]
pub fn sin_table() -> &'static [f32] {
    static TABLE: OnceLock<Box<[f32]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..SIN_QUANTIZATION)
            .map(|i| libm::sin(i as f64 / SIN_SCALE) as f32)
            .collect()
    })
}

/// `(ASIN_TAB, COS_TAB)`: `Math.asin(i / 256)` and `Math.cos` of that, for `i` in `0..=256`.
///
/// The values are embedded exactly as HotSpot on x86_64 computes them (see
/// `parity/tables/AtanTables.java`): its `Math.cos` is not reproducible by any portable
/// implementation (entry 61 is not even correctly rounded).
fn atan_tables() -> &'static AtanTables {
    static TABLES: OnceLock<AtanTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let asin = crate::atan_tables::ASIN_BITS.map(f64::from_bits);
        let cos = crate::atan_tables::COS_BITS.map(f64::from_bits);
        (asin, cos)
    })
}

/// The arcsine table behind [`atan2`]: entry `i` is `asin(i / 256)`.
#[must_use]
pub fn asin_table() -> &'static [f64; ATAN_TABLE_SIZE] {
    &atan_tables().0
}

/// The cosine table behind [`atan2`]: entry `i` is `cos(asin(i / 256))`.
#[must_use]
pub fn cos_table() -> &'static [f64; ATAN_TABLE_SIZE] {
    &atan_tables().1
}

/// Sine from vanilla's 65536-entry lookup table.
#[must_use]
pub fn sin(value: f64) -> f32 {
    sin_table()[((value * SIN_SCALE) as i64 & 0xFFFF) as usize]
}

/// Cosine from vanilla's 65536-entry lookup table.
#[must_use]
pub fn cos(value: f64) -> f32 {
    sin_table()[((value * SIN_SCALE + COS_OFFSET) as i64 & 0xFFFF) as usize]
}

/// `(float) Math.sqrt(x)`.
#[must_use]
pub fn sqrt(x: f32) -> f32 {
    f64::from(x).sqrt() as f32
}

/// `(int) Math.floor(v)`.
#[must_use]
pub fn floor_f32(v: f32) -> i32 {
    f64::from(v).floor() as i32
}

/// `(int) Math.floor(v)`.
#[must_use]
pub fn floor_f64(v: f64) -> i32 {
    v.floor() as i32
}

/// `(long) Math.floor(v)`.
#[must_use]
pub fn lfloor(v: f64) -> i64 {
    v.floor() as i64
}

/// `Math.abs(float)`.
#[must_use]
pub fn abs_f32(v: f32) -> f32 {
    v.abs()
}

/// `Math.abs(int)`; `i32::MIN` stays `i32::MIN`.
#[must_use]
pub fn abs_i32(v: i32) -> i32 {
    v.wrapping_abs()
}

/// `(int) Math.ceil(v)`.
#[must_use]
pub fn ceil_f32(v: f32) -> i32 {
    f64::from(v).ceil() as i32
}

/// `(int) Math.ceil(v)`.
#[must_use]
pub fn ceil_f64(v: f64) -> i32 {
    v.ceil() as i32
}

/// `(long) Math.ceil(v)`.
#[must_use]
pub fn ceil_long(v: f64) -> i64 {
    v.ceil() as i64
}

/// `min(max(value, min), max)`. Unlike [`i32::clamp`], it never panics.
#[must_use]
pub fn clamp_i32(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
}

/// `min(max(value, min), max)`. Unlike [`i64::clamp`], it never panics.
#[must_use]
pub fn clamp_i64(value: i64, min: i64, max: i64) -> i64 {
    value.max(min).min(max)
}

/// `value < min ? min : Math.min(value, max)`.
#[must_use]
pub fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else {
        java::min_f32(value, max)
    }
}

/// `value < min ? min : Math.min(value, max)`.
#[must_use]
pub fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else {
        java::min_f64(value, max)
    }
}

/// Linear interpolation with the factor clamped to `[0, 1]`.
#[must_use]
pub fn clamped_lerp_f64(factor: f64, min: f64, max: f64) -> f64 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        lerp_f64(factor, min, max)
    }
}

/// Linear interpolation with the factor clamped to `[0, 1]`.
#[must_use]
pub fn clamped_lerp_f32(factor: f32, min: f32, max: f32) -> f32 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        lerp_f32(factor, min, max)
    }
}

/// The larger absolute value of `a` and `b`.
#[must_use]
pub fn abs_max_i32(a: i32, b: i32) -> i32 {
    abs_i32(a).max(abs_i32(b))
}

/// The larger absolute value of `a` and `b`.
#[must_use]
pub fn abs_max_f32(a: f32, b: f32) -> f32 {
    java::max_f32(a.abs(), b.abs())
}

/// The larger absolute value of `a` and `b`.
#[must_use]
pub fn abs_max_f64(a: f64, b: f64) -> f64 {
    java::max_f64(a.abs(), b.abs())
}

/// Chebyshev distance between `(x0, z0)` and `(x1, z1)`.
#[must_use]
pub fn chessboard_distance(x0: i32, z0: i32, x1: i32, z1: i32) -> i32 {
    abs_max_i32(x1.wrapping_sub(x0), z1.wrapping_sub(z0))
}

/// `Math.floorDiv(a, b)`.
///
/// # Panics
///
/// Panics if `b` is zero.
#[must_use]
pub fn floor_div(a: i32, b: i32) -> i32 {
    java::floor_div_i32(a, b)
}

/// Whether `a` and `b` differ by less than [`EPSILON`].
#[must_use]
pub fn equal_f32(a: f32, b: f32) -> bool {
    (b - a).abs() < EPSILON
}

/// Whether `a` and `b` differ by less than [`EPSILON`] (the float constant, widened).
#[must_use]
pub fn equal_f64(a: f64, b: f64) -> bool {
    (b - a).abs() < f64::from(EPSILON)
}

/// `Math.floorMod(input, modulus)`.
///
/// # Panics
///
/// Panics if `modulus` is zero.
#[must_use]
pub fn positive_modulo_i32(input: i32, modulus: i32) -> i32 {
    java::floor_mod_i32(input, modulus)
}

/// `(input % modulus + modulus) % modulus`.
#[must_use]
pub fn positive_modulo_f32(input: f32, modulus: f32) -> f32 {
    (input % modulus + modulus) % modulus
}

/// `(input % modulus + modulus) % modulus`.
#[must_use]
pub fn positive_modulo_f64(input: f64, modulus: f64) -> f64 {
    (input % modulus + modulus) % modulus
}

/// Whether `dividend % divisor == 0`.
///
/// # Panics
///
/// Panics if `divisor` is zero.
#[must_use]
pub fn is_multiple_of(dividend: i32, divisor: i32) -> bool {
    dividend.wrapping_rem(divisor) == 0
}

/// Packs an angle in degrees into a byte: 256 steps per full turn.
#[must_use]
pub fn pack_degrees(angle: f32) -> i8 {
    floor_f32(angle * 256.0 / 360.0) as i8
}

/// Unpacks a byte angle into degrees.
#[must_use]
pub fn unpack_degrees(rotation: i8) -> f32 {
    (i32::from(rotation) * 360) as f32 / 256.0
}

/// Wraps an angle into `[-180, 180)`.
#[must_use]
pub fn wrap_degrees_i32(angle: i32) -> i32 {
    let mut normalized = angle % 360;
    if normalized >= 180 {
        normalized -= 360;
    }
    if normalized < -180 {
        normalized += 360;
    }
    normalized
}

/// Wraps an angle into `[-180, 180)`.
#[must_use]
pub fn wrap_degrees_i64(angle: i64) -> f32 {
    let mut normalized = (angle % 360) as f32;
    if normalized >= 180.0 {
        normalized -= 360.0;
    }
    if normalized < -180.0 {
        normalized += 360.0;
    }
    normalized
}

/// Wraps an angle into `[-180, 180)`.
#[must_use]
pub fn wrap_degrees_f32(angle: f32) -> f32 {
    let mut normalized = angle % 360.0;
    if normalized >= 180.0 {
        normalized -= 360.0;
    }
    if normalized < -180.0 {
        normalized += 360.0;
    }
    normalized
}

/// Wraps an angle into `[-180, 180)`.
#[must_use]
pub fn wrap_degrees_f64(angle: f64) -> f64 {
    let mut normalized = angle % 360.0;
    if normalized >= 180.0 {
        normalized -= 360.0;
    }
    if normalized < -180.0 {
        normalized += 360.0;
    }
    normalized
}

/// Wraps an angle into `[-45, 45)`.
#[must_use]
pub fn wrap_degrees_90(angle: f32) -> f32 {
    let mut normalized = angle % 90.0;
    if normalized >= 45.0 {
        normalized -= 90.0;
    }
    if normalized < -45.0 {
        normalized += 90.0;
    }
    normalized
}

/// The signed, wrapped difference `to_angle - from_angle` in degrees.
#[must_use]
pub fn degrees_difference(from_angle: f32, to_angle: f32) -> f32 {
    wrap_degrees_f32(to_angle - from_angle)
}

/// The absolute, wrapped difference between two angles in degrees.
#[must_use]
pub fn degrees_difference_abs(angle_a: f32, angle_b: f32) -> f32 {
    degrees_difference(angle_a, angle_b).abs()
}

/// Moves `target_angle` toward `base_angle` so they differ by at most `max_angle_diff`.
#[must_use]
pub fn rotate_if_necessary(base_angle: f32, target_angle: f32, max_angle_diff: f32) -> f32 {
    let delta = degrees_difference(base_angle, target_angle);
    let clamped = clamp_f32(delta, -max_angle_diff, max_angle_diff);
    target_angle - clamped
}

/// Steps `current` toward `target` by at most `|increment|`.
#[must_use]
pub fn approach(current: f32, target: f32, increment: f32) -> f32 {
    let increment = increment.abs();
    if current < target {
        clamp_f32(current + increment, current, target)
    } else {
        clamp_f32(current - increment, target, current)
    }
}

/// Steps the angle `current` toward `target` by at most `|increment|` degrees.
#[must_use]
pub fn approach_degrees(current: f32, target: f32, increment: f32) -> f32 {
    let difference = degrees_difference(current, target);
    approach(current, current + difference, increment)
}

/// The smallest power of two `>= input`, with vanilla's wrapping behaviour.
#[must_use]
pub fn smallest_encompassing_power_of_two(input: i32) -> i32 {
    let mut result = input.wrapping_sub(1);
    result |= result >> 1;
    result |= result >> 2;
    result |= result >> 4;
    result |= result >> 8;
    result |= result >> 16;
    result.wrapping_add(1)
}

/// The side of the smallest square that holds `item_count` items.
///
/// # Panics
///
/// Panics if `item_count` is negative.
#[must_use]
pub fn smallest_square_side(item_count: i32) -> i32 {
    assert!(
        item_count >= 0,
        "itemCount must be greater than or equal to zero"
    );
    ceil_f64(f64::from(item_count).sqrt())
}

/// Whether `input` is a power of two.
#[must_use]
pub fn is_power_of_two_i32(input: i32) -> bool {
    input != 0 && (input & input.wrapping_sub(1)) == 0
}

/// Whether `input` is a power of two.
#[must_use]
pub fn is_power_of_two_i64(input: i64) -> bool {
    input != 0 && (input & input.wrapping_sub(1)) == 0
}

/// `ceil(log2(input))`, computed with a De Bruijn sequence.
#[must_use]
pub fn ceillog2(input: i32) -> i32 {
    let input = if is_power_of_two_i32(input) {
        input
    } else {
        smallest_encompassing_power_of_two(input)
    };
    let index = (i64::from(input).wrapping_mul(125_613_361) >> 27) as i32 & 31;
    MULTIPLY_DE_BRUIJN_BIT_POSITION[index as usize]
}

/// `floor(log2(input))`.
#[must_use]
pub fn log2(input: i32) -> i32 {
    ceillog2(input) - i32::from(!is_power_of_two_i32(input))
}

/// The fractional part `num - floor(num)`.
#[must_use]
pub fn frac_f32(num: f32) -> f32 {
    num - floor_f32(num) as f32
}

/// The fractional part `num - floor(num)`.
#[must_use]
pub fn frac_f64(num: f64) -> f64 {
    num - lfloor(num) as f64
}

/// The positional seed vanilla derives from block coordinates.
#[must_use]
pub fn get_seed(x: i32, y: i32, z: i32) -> i64 {
    let mut seed = i64::from(x.wrapping_mul(3_129_871))
        ^ i64::from(z).wrapping_mul(116_129_781)
        ^ i64::from(y);
    seed = seed
        .wrapping_mul(seed)
        .wrapping_mul(42_317_861)
        .wrapping_add(seed.wrapping_mul(11));
    seed >> 16
}

/// `(value - min) / (max - min)`.
#[must_use]
pub fn inverse_lerp_f64(value: f64, min: f64, max: f64) -> f64 {
    (value - min) / (max - min)
}

/// `(value - min) / (max - min)`.
#[must_use]
pub fn inverse_lerp_f32(value: f32, min: f32, max: f32) -> f32 {
    (value - min) / (max - min)
}

/// Vanilla's table-based `atan2` approximation.
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    let d2 = x * x + y * y;
    if d2.is_nan() {
        return f64::NAN;
    }
    let (mut x, mut y) = (x, y);
    let neg_y = y < 0.0;
    if neg_y {
        y = -y;
    }
    let neg_x = x < 0.0;
    if neg_x {
        x = -x;
    }
    let steep = y > x;
    if steep {
        std::mem::swap(&mut x, &mut y);
    }

    let rinv = fast_inv_sqrt(d2);
    x *= rinv;
    y *= rinv;
    let yp = FRAC_BIAS + y;
    let index = yp.to_bits() as i32 as usize;
    let (asin_tab, cos_tab) = atan_tables();
    let phi = asin_tab[index];
    let c_phi = cos_tab[index];
    let s_phi = yp - FRAC_BIAS;
    let sd = y * c_phi - x * s_phi;
    let d = (6.0 + sd * sd) * sd * ONE_SIXTH;
    let mut theta = phi + d;
    if steep {
        theta = FRAC_PI_2 - theta;
    }
    if neg_x {
        theta = PI_F64 - theta;
    }
    if neg_y {
        theta = -theta;
    }
    theta
}

/// `1.0F / (float) Math.sqrt(x)`, as computed by JOML.
#[must_use]
pub fn inv_sqrt_f32(x: f32) -> f32 {
    1.0 / sqrt(x)
}

/// `1.0 / Math.sqrt(x)`, as computed by JOML.
#[must_use]
pub fn inv_sqrt_f64(x: f64) -> f64 {
    1.0 / x.sqrt()
}

/// The "fast inverse square root" bit trick with one Newton step.
#[must_use]
pub fn fast_inv_sqrt(x: f64) -> f64 {
    let half = 0.5 * x;
    let bits = 6_910_469_410_427_058_090_i64.wrapping_sub(x.to_bits() as i64 >> 1);
    let y = f64::from_bits(bits as u64);
    y * (1.5 - half * y * y)
}

/// The fast inverse cube root bit trick with two Newton steps.
#[must_use]
pub fn fast_inv_cube_root(x: f32) -> f32 {
    let bits = 1_419_967_116 - java::float_to_int_bits(x) / 3;
    let mut y = f32::from_bits(bits as u32);
    y = TWO_THIRDS * y + 1.0 / (3.0 * y * y * x);
    TWO_THIRDS * y + 1.0 / (3.0 * y * y * x)
}

/// Converts HSV (each in `[0, 1]`) to an RGB colour with alpha 0.
///
/// # Panics
///
/// Panics for hues that map outside the six colour sectors (for example negative hues),
/// where vanilla throws.
#[must_use]
pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> i32 {
    hsv_to_argb(hue, saturation, value, 0)
}

/// Converts HSV (each in `[0, 1]`) plus an alpha byte to a packed ARGB colour.
///
/// # Panics
///
/// Panics for hues that map outside the six colour sectors (for example negative hues),
/// where vanilla throws.
#[must_use]
pub fn hsv_to_argb(hue: f32, saturation: f32, value: f32, alpha: i32) -> i32 {
    let sector = (hue * 6.0) as i32 % 6;
    let f = hue * 6.0 - sector as f32;
    let p = value * (1.0 - saturation);
    let q = value * (1.0 - f * saturation);
    let t = value * (1.0 - (1.0 - f) * saturation);
    let (red, green, blue) = match sector {
        0 => (value, t, p),
        1 => (q, value, p),
        2 => (p, value, t),
        3 => (p, q, value),
        4 => (t, p, value),
        5 => (value, p, q),
        _ => panic!("cannot convert HSV ({hue}, {saturation}, {value}) to RGB"),
    };
    let channel = |c: f32| clamp_i32((c * 255.0) as i32, 0, 255);
    (alpha & 0xFF) << 24
        | (channel(red) & 0xFF) << 16
        | (channel(green) & 0xFF) << 8
        | channel(blue) & 0xFF
}

/// The 32-bit finalizer of `MurmurHash3`.
#[must_use]
pub fn murmur_hash3_mixer(hash: i32) -> i32 {
    let mut hash = hash.cast_unsigned();
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85EB_CA6B);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xC2B2_AE35);
    (hash ^ (hash >> 16)).cast_signed()
}

/// The smallest `i` in `[from, to)` for which `condition(i)` holds, or `to` if there is none.
/// `condition` must be monotonic: false, then true.
#[must_use]
pub fn binary_search(from: i32, to: i32, mut condition: impl FnMut(i32) -> bool) -> i32 {
    let mut from = from;
    let mut len = to.wrapping_sub(from);
    while len > 0 {
        let half = len / 2;
        let middle = from.wrapping_add(half);
        if condition(middle) {
            len = half;
        } else {
            from = middle.wrapping_add(1);
            len -= half + 1;
        }
    }
    from
}

/// Integer interpolation, rounding down.
#[must_use]
pub fn lerp_int(alpha: f32, p0: i32, p1: i32) -> i32 {
    p0.wrapping_add(floor_f32(alpha * p1.wrapping_sub(p0) as f32))
}

/// Discrete interpolation: `p0` at `alpha = 0`, reaching `p1` only at `alpha = 1`.
#[must_use]
pub fn lerp_discrete(alpha: f32, p0: i32, p1: i32) -> i32 {
    let delta = p1.wrapping_sub(p0);
    let step = floor_f32(alpha * delta.wrapping_sub(1) as f32);
    p0.wrapping_add(step).wrapping_add(i32::from(alpha > 0.0))
}

/// `p0 + alpha * (p1 - p0)`.
#[must_use]
pub fn lerp_f32(alpha: f32, p0: f32, p1: f32) -> f32 {
    p0 + alpha * (p1 - p0)
}

/// Bilinear interpolation.
#[must_use]
pub fn lerp2_f32(alpha1: f32, alpha2: f32, x00: f32, x10: f32, x01: f32, x11: f32) -> f32 {
    lerp_f32(
        alpha2,
        lerp_f32(alpha1, x00, x10),
        lerp_f32(alpha1, x01, x11),
    )
}

/// Trilinear interpolation.
#[allow(clippy::too_many_arguments, reason = "mirrors vanilla's signature")]
#[must_use]
pub fn lerp3_f32(
    alpha1: f32,
    alpha2: f32,
    alpha3: f32,
    x000: f32,
    x100: f32,
    x010: f32,
    x110: f32,
    x001: f32,
    x101: f32,
    x011: f32,
    x111: f32,
) -> f32 {
    lerp_f32(
        alpha3,
        lerp2_f32(alpha1, alpha2, x000, x100, x010, x110),
        lerp2_f32(alpha1, alpha2, x001, x101, x011, x111),
    )
}

/// `p0 + alpha * (p1 - p0)`.
#[must_use]
pub fn lerp_f64(alpha: f64, p0: f64, p1: f64) -> f64 {
    p0 + alpha * (p1 - p0)
}

/// Bilinear interpolation.
#[must_use]
pub fn lerp2_f64(alpha1: f64, alpha2: f64, x00: f64, x10: f64, x01: f64, x11: f64) -> f64 {
    lerp_f64(
        alpha2,
        lerp_f64(alpha1, x00, x10),
        lerp_f64(alpha1, x01, x11),
    )
}

/// Trilinear interpolation.
#[allow(clippy::too_many_arguments, reason = "mirrors vanilla's signature")]
#[must_use]
pub fn lerp3_f64(
    alpha1: f64,
    alpha2: f64,
    alpha3: f64,
    x000: f64,
    x100: f64,
    x010: f64,
    x110: f64,
    x001: f64,
    x101: f64,
    x011: f64,
    x111: f64,
) -> f64 {
    lerp_f64(
        alpha3,
        lerp2_f64(alpha1, alpha2, x000, x100, x010, x110),
        lerp2_f64(alpha1, alpha2, x001, x101, x011, x111),
    )
}

/// Catmull-Rom spline through `p1` and `p2`.
#[rustfmt::skip]
#[must_use]
pub fn catmullrom(alpha: f32, p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    0.5 * (
        2.0 * p1
            + (p2 - p0) * alpha
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * alpha * alpha
            + (3.0 * p1 - p0 - 3.0 * p2 + p3) * alpha * alpha * alpha
    )
}

/// Quintic smoothstep `6x^5 - 15x^4 + 10x^3`.
#[must_use]
pub fn smoothstep(x: f32) -> f32 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// Derivative of [`smoothstep`].
#[must_use]
pub fn smoothstep_derivative(x: f32) -> f32 {
    30.0 * x * x * (x - 1.0) * (x - 1.0)
}

/// `-1`, `0` or `1`. NaN gives `-1`, as in vanilla.
#[must_use]
pub fn sign(number: f64) -> i32 {
    if number == 0.0 {
        0
    } else if number > 0.0 {
        1
    } else {
        -1
    }
}

/// Interpolates between two angles in degrees along the shorter arc.
#[must_use]
pub fn rot_lerp_f32(a: f32, from: f32, to: f32) -> f32 {
    from + a * wrap_degrees_f32(to - from)
}

/// Interpolates between two angles in degrees along the shorter arc.
#[must_use]
pub fn rot_lerp_f64(a: f64, from: f64, to: f64) -> f64 {
    from + a * wrap_degrees_f64(to - from)
}

/// Interpolates between two angles in radians along the shorter arc.
#[must_use]
pub fn rot_lerp_rad(a: f32, from: f32, to: f32) -> f32 {
    let mut diff = to - from;
    while diff < -PI {
        diff += TWO_PI;
    }
    while diff >= PI {
        diff -= TWO_PI;
    }
    from + a * diff
}

/// A triangle wave in `[-1, 1]` with the given period.
#[must_use]
pub fn triangle_wave(index: f32, period: f32) -> f32 {
    ((index % period - period * 0.5).abs() - period * 0.25) / (period * 0.25)
}

/// `x * x`.
#[must_use]
pub fn square_f32(x: f32) -> f32 {
    x * x
}

/// `x * x * x`.
#[must_use]
pub fn cube(x: f32) -> f32 {
    x * x * x
}

/// `x * x`.
#[must_use]
pub fn square_f64(x: f64) -> f64 {
    x * x
}

/// `x * x`, wrapping.
#[must_use]
pub fn square_i32(x: i32) -> i32 {
    x.wrapping_mul(x)
}

/// `x * x`, wrapping.
#[must_use]
pub fn square_i64(x: i64) -> i64 {
    x.wrapping_mul(x)
}

/// Maps `value` from one range to another, clamped to the target range.
#[must_use]
pub fn clamped_map_f64(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let factor = inverse_lerp_f64(value, from_min, from_max);
    clamped_lerp_f64(factor, to_min, to_max)
}

/// Maps `value` from one range to another, clamped to the target range.
#[must_use]
pub fn clamped_map_f32(value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    let factor = inverse_lerp_f32(value, from_min, from_max);
    clamped_lerp_f32(factor, to_min, to_max)
}

/// Maps `value` from one range to another.
#[must_use]
pub fn map_f64(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let factor = inverse_lerp_f64(value, from_min, from_max);
    lerp_f64(factor, to_min, to_max)
}

/// Maps `value` from one range to another.
#[must_use]
pub fn map_f32(value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    let factor = inverse_lerp_f32(value, from_min, from_max);
    lerp_f32(factor, to_min, to_max)
}

/// Rounds `input` up to a multiple of `multiple`.
///
/// # Panics
///
/// Panics if `multiple` is zero.
#[must_use]
pub fn round_toward_i32(input: i32, multiple: i32) -> i32 {
    positive_ceil_div_i32(input, multiple).wrapping_mul(multiple)
}

/// Rounds `input` up to a multiple of `multiple`.
///
/// # Panics
///
/// Panics if `multiple` is zero.
#[must_use]
pub fn round_toward_i64(input: i64, multiple: i64) -> i64 {
    positive_ceil_div_i64(input, multiple).wrapping_mul(multiple)
}

/// `-Math.floorDiv(-input, divisor)`: division rounding toward positive infinity.
///
/// # Panics
///
/// Panics if `divisor` is zero.
#[must_use]
pub fn positive_ceil_div_i32(input: i32, divisor: i32) -> i32 {
    java::floor_div_i32(input.wrapping_neg(), divisor).wrapping_neg()
}

/// `-Math.floorDiv(-input, divisor)`: division rounding toward positive infinity.
///
/// # Panics
///
/// Panics if `divisor` is zero.
#[must_use]
pub fn positive_ceil_div_i64(input: i64, divisor: i64) -> i64 {
    java::floor_div_i64(input.wrapping_neg(), divisor).wrapping_neg()
}

/// `x * x + y * y`.
#[must_use]
pub fn length_squared_2d_f64(x: f64, y: f64) -> f64 {
    x * x + y * y
}

/// `x * x + y * y`.
#[must_use]
pub fn length_squared_2d_f32(x: f32, y: f32) -> f32 {
    x * x + y * y
}

/// Euclidean length of `(x, y)`.
#[must_use]
pub fn length_2d_f64(x: f64, y: f64) -> f64 {
    length_squared_2d_f64(x, y).sqrt()
}

/// Euclidean length of `(x, y)`.
#[must_use]
pub fn length_2d_f32(x: f32, y: f32) -> f32 {
    sqrt(length_squared_2d_f32(x, y))
}

/// `x * x + y * y + z * z`.
#[must_use]
pub fn length_squared_3d_f64(x: f64, y: f64, z: f64) -> f64 {
    x * x + y * y + z * z
}

/// Euclidean length of `(x, y, z)`.
#[must_use]
pub fn length_3d_f64(x: f64, y: f64, z: f64) -> f64 {
    length_squared_3d_f64(x, y, z).sqrt()
}

/// Euclidean length of `(x, y, z)`.
#[must_use]
pub fn length_3d_f32(x: f32, y: f32, z: f32) -> f32 {
    sqrt(length_squared_3d_f32(x, y, z))
}

/// `x * x + y * y + z * z`.
#[must_use]
pub fn length_squared_3d_f32(x: f32, y: f32, z: f32) -> f32 {
    x * x + y * y + z * z
}

/// Rounds `value` down to a multiple of `resolution`.
#[must_use]
pub fn quantize(value: f64, resolution: i32) -> i32 {
    floor_f64(value / f64::from(resolution)).wrapping_mul(resolution)
}

/// Iterates outward from `origin` (clamped into `[lower_bound, upper_bound]`), alternating
/// sides and starting on the positive one: `origin`, `origin + step`, `origin - step`,
/// `origin + 2*step`, … Once one side runs out of room, the other side continues alone.
///
/// # Panics
///
/// Panics if `lower_bound > upper_bound` or `step_size < 1`, where vanilla throws.
#[must_use]
pub fn out_from_origin(
    origin: i32,
    lower_bound: i32,
    upper_bound: i32,
    step_size: i32,
) -> OutFromOrigin {
    assert!(
        lower_bound <= upper_bound,
        "upperBound {upper_bound} expected to be > lowerBound {lower_bound}"
    );
    assert!(
        step_size >= 1,
        "step size expected to be >= 1, was {step_size}"
    );
    let origin = clamp_i32(origin, lower_bound, upper_bound);
    OutFromOrigin {
        origin,
        lower: lower_bound,
        upper: upper_bound,
        step: step_size,
        next: Some(origin),
    }
}

/// Iterator returned by [`out_from_origin`].
#[derive(Debug, Clone)]
pub struct OutFromOrigin {
    origin: i32,
    lower: i32,
    upper: i32,
    step: i32,
    next: Option<i32>,
}

impl OutFromOrigin {
    fn has_next(&self, cursor: i32) -> bool {
        let distance = self.origin.wrapping_sub(cursor).wrapping_abs();
        self.origin.wrapping_sub(distance) >= self.lower
            || self.origin.wrapping_add(distance) <= self.upper
    }

    fn advance(&self, cursor: i32) -> i32 {
        let previous_was_negative = cursor <= self.origin;
        let distance = self.origin.wrapping_sub(cursor).wrapping_abs();
        let positive = self.origin.wrapping_add(distance).wrapping_add(self.step);
        if !previous_was_negative || positive > self.upper {
            let back = if previous_was_negative { self.step } else { 0 };
            let attempted = self.origin.wrapping_sub(distance).wrapping_sub(back);
            if attempted >= self.lower {
                return attempted;
            }
        }
        positive
    }
}

impl Iterator for OutFromOrigin {
    type Item = i32;

    fn next(&mut self) -> Option<i32> {
        let cursor = self.next?;
        if !self.has_next(cursor) {
            self.next = None;
            return None;
        }
        self.next = Some(self.advance(cursor));
        Some(cursor)
    }
}

impl std::iter::FusedIterator for OutFromOrigin {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_their_java_definitions() {
        assert_eq!(SQRT_OF_TWO, 2.0_f64.sqrt() as f32);
        assert_eq!(PI.to_bits(), 0x4049_0FDB);
        assert_eq!(RAD_TO_DEG, 180.0_f32 / PI);
        assert_eq!(ONE_SIXTH, "0.16666666666666666".parse::<f64>().unwrap());
        assert_eq!(TWO_THIRDS, "0.6666667".parse::<f32>().unwrap());
        assert_eq!(FRAC_BIAS.to_bits(), 4_805_340_802_404_319_232);
    }

    #[test]
    fn sin_and_cos_hit_exact_table_points() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
    }

    #[test]
    fn get_seed_matches_known_values() {
        assert_eq!(get_seed(0, 0, 0), 0);
        // With x = 1 the starting seed is 3129871; then (s * s * 42317861 + s * 11) >> 16.
        let s: i64 = 3_129_871;
        let squared = s.wrapping_mul(s).wrapping_mul(42_317_861);
        assert_eq!(get_seed(1, 0, 0), squared.wrapping_add(s * 11) >> 16);
    }

    #[test]
    fn powers_of_two_and_logs() {
        assert_eq!(smallest_encompassing_power_of_two(5), 8);
        assert_eq!(smallest_encompassing_power_of_two(8), 8);
        assert_eq!(ceillog2(8), 3);
        assert_eq!(ceillog2(9), 4);
        assert_eq!(log2(9), 3);
        assert_eq!(log2(1), 0);
        assert!(is_power_of_two_i64(1 << 40));
    }

    #[test]
    fn wrapping_angles() {
        assert_eq!(wrap_degrees_i32(180), -180);
        assert_eq!(wrap_degrees_i32(-181), 179);
        assert_eq!(wrap_degrees_f32(540.0), -180.0);
        assert_eq!(wrap_degrees_f64(-190.0), 170.0);
        assert_eq!(pack_degrees(90.0), 64);
        assert_eq!(unpack_degrees(-128), -180.0);
    }

    #[test]
    fn out_from_origin_alternates_sides() {
        let values: Vec<i32> = out_from_origin(5, 3, 8, 1).collect();
        assert_eq!(values, [5, 6, 4, 7, 3, 8]);
        let values: Vec<i32> = out_from_origin(0, 0, 4, 2).collect();
        assert_eq!(values, [0, 2, 4]);
    }

    #[test]
    fn binary_search_finds_first_true() {
        assert_eq!(binary_search(0, 100, |i| i >= 37), 37);
        assert_eq!(binary_search(0, 100, |_| false), 100);
    }

    #[test]
    fn murmur_mixer_is_the_fmix32_finalizer() {
        assert_eq!(murmur_hash3_mixer(0), 0);
        assert_eq!(murmur_hash3_mixer(1), 0x514E_28B7);
    }
}
