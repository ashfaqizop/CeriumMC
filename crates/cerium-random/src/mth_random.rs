//! The `Mth` helpers that take a random source.

use cerium_math::mth;

use crate::{RandomSource, SingleThreadedRandom};

/// `Mth.nextInt`: a random `int` in `[min_inclusive, max_inclusive]`, or `min_inclusive`
/// when the range is empty.
pub fn next_int(
    random: &mut (impl RandomSource + ?Sized),
    min_inclusive: i32,
    max_inclusive: i32,
) -> i32 {
    if min_inclusive >= max_inclusive {
        return min_inclusive;
    }
    let bound = max_inclusive.wrapping_sub(min_inclusive).wrapping_add(1);
    random.next_int_bounded(bound).wrapping_add(min_inclusive)
}

/// `Mth.nextFloat`: a random `float` in `[min, max)`, or `min` when the range is empty.
pub fn next_float(random: &mut (impl RandomSource + ?Sized), min: f32, max: f32) -> f32 {
    if min >= max {
        return min;
    }
    random.next_float() * (max - min) + min
}

/// `Mth.nextDouble`: a random `double` in `[min, max)`, or `min` when the range is empty.
pub fn next_double(random: &mut (impl RandomSource + ?Sized), min: f64, max: f64) -> f64 {
    if min >= max {
        return min;
    }
    random.next_double() * (max - min) + min
}

/// `Mth.randomBetweenInclusive`: a random `int` in `[min, max_inclusive]`.
///
/// # Panics
///
/// Panics if the range is empty, where vanilla throws.
pub fn random_between_inclusive(
    random: &mut (impl RandomSource + ?Sized),
    min: i32,
    max_inclusive: i32,
) -> i32 {
    let bound = max_inclusive.wrapping_sub(min).wrapping_add(1);
    random.next_int_bounded(bound).wrapping_add(min)
}

/// `Mth.randomBetween`: a random `float` in `[min, max_exclusive)`.
pub fn random_between(
    random: &mut (impl RandomSource + ?Sized),
    min: f32,
    max_exclusive: f32,
) -> f32 {
    random.next_float() * (max_exclusive - min) + min
}

/// `Mth.normal`: `mean + gaussian * deviation`, with the Gaussian narrowed to `float` first.
pub fn normal(random: &mut (impl RandomSource + ?Sized), mean: f32, deviation: f32) -> f32 {
    mean + random.next_gaussian() as f32 * deviation
}

/// `Mth.createInsecureUUID`: a version-4 UUID as `(most significant, least significant)`.
pub fn create_insecure_uuid(random: &mut (impl RandomSource + ?Sized)) -> (i64, i64) {
    let most = (random.next_long() & !0xF000) | 0x4000;
    let least = (random.next_long() & 0x3FFF_FFFF_FFFF_FFFF) | i64::MIN;
    (most, least)
}

/// `Mth.wobble`: shifts `coord` by a deterministic amount below `5e-8`.
#[must_use]
pub fn wobble(coord: f64) -> f64 {
    let mut random = SingleThreadedRandom::new(i64::from(mth::floor_f64(coord * 3000.0)));
    coord + (2.0 * random.next_double() - 1.0) * 1.0e-7 / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::LegacyRandom;

    #[test]
    fn empty_ranges_return_the_minimum() {
        let mut random = LegacyRandom::new(5);
        assert_eq!(next_int(&mut random, 9, 3), 9);
        assert_eq!(next_float(&mut random, 2.0, 2.0), 2.0);
        assert_eq!(next_double(&mut random, 1.0, 0.0), 1.0);
    }

    #[test]
    fn insecure_uuids_are_version_4_variant_2() {
        let mut random = LegacyRandom::new(5);
        let (most, least) = create_insecure_uuid(&mut random);
        assert_eq!((most >> 12) & 0xF, 4);
        assert_eq!((least as u64) >> 62, 0b10);
    }

    #[test]
    fn works_through_a_trait_object() {
        let mut random = LegacyRandom::new(5);
        let dynamic: &mut dyn RandomSource = &mut random;
        let value = next_int(dynamic, 0, 10);
        assert!((0..=10).contains(&value));
    }
}
