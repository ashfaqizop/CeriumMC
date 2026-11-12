//! Vanilla's `BitRandomSource`: generators built on a `next(bits)` primitive.
//!
//! The free functions are the interface's default methods. Each generator's
//! [`RandomSource`](crate::RandomSource) implementation calls them, so every generator
//! derives ints, longs, floats and doubles from its bits in exactly the same way.

use crate::{DOUBLE_UNIT, FLOAT_UNIT};

/// A generator whose every output is derived from `next(bits)`.
pub trait BitRandomSource {
    /// The next `bits` random bits (`1..=32`) in the low bits of an `int`, sign-extended
    /// from bit 31 as Java's narrowing cast does.
    fn next(&mut self, bits: u32) -> i32;
}

/// `nextInt()`: 32 random bits.
pub(crate) fn next_int(random: &mut impl BitRandomSource) -> i32 {
    random.next(32)
}

/// `nextInt(bound)`: a multiply-shift for powers of two, rejection sampling otherwise.
pub(crate) fn next_int_bounded(random: &mut impl BitRandomSource, bound: i32) -> i32 {
    assert!(bound > 0, "Bound must be positive");
    if (bound & (bound - 1)) == 0 {
        return ((i64::from(bound) * i64::from(random.next(31))) >> 31) as i32;
    }
    loop {
        let sample = random.next(31);
        let modulo = sample % bound;
        if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
            return modulo;
        }
    }
}

/// `nextLong()`: two 32-bit draws, the second added with sign extension.
pub(crate) fn next_long(random: &mut impl BitRandomSource) -> i64 {
    let upper = random.next(32);
    let lower = random.next(32);
    (i64::from(upper) << 32).wrapping_add(i64::from(lower))
}

/// `nextBoolean()`: one random bit.
pub(crate) fn next_boolean(random: &mut impl BitRandomSource) -> bool {
    random.next(1) != 0
}

/// `nextFloat()`: 24 random bits scaled into `[0, 1)`.
pub(crate) fn next_float(random: &mut impl BitRandomSource) -> f32 {
    random.next(24) as f32 * FLOAT_UNIT
}

/// `nextDouble()`: 26 + 27 random bits scaled into `[0, 1)`.
pub(crate) fn next_double(random: &mut impl BitRandomSource) -> f64 {
    let upper = random.next(26);
    let lower = random.next(27);
    let combined = (i64::from(upper) << 27) + i64::from(lower);
    combined as f64 * DOUBLE_UNIT
}
