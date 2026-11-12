//! Vanilla's Xoroshiro128++ generator and the `RandomSource` built on it.

use cerium_math::mth;

use crate::gaussian::Gaussian;
use crate::support::{self, Seed128};
use crate::{DOUBLE_UNIT, FLOAT_UNIT, ForkableRandom, PositionalRandomFactory, RandomSource};

/// The Xoroshiro128++ generator (Blackman and Vigna), as used by vanilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xoroshiro128PlusPlus {
    lo: i64,
    hi: i64,
}

impl Xoroshiro128PlusPlus {
    /// Creates the generator. An all-zero state, which would only ever produce zeros, is
    /// replaced by vanilla's fallback constants.
    #[must_use]
    pub const fn new(lo: i64, hi: i64) -> Self {
        if (lo | hi) == 0 {
            Self {
                lo: support::GOLDEN_RATIO_64,
                hi: support::SILVER_RATIO_64,
            }
        } else {
            Self { lo, hi }
        }
    }

    /// Creates the generator from a 128-bit seed.
    #[must_use]
    pub const fn from_seed(seed: Seed128) -> Self {
        Self::new(seed.lo, seed.hi)
    }

    /// The generator's state as `(lo, hi)`.
    #[must_use]
    pub const fn state(&self) -> (i64, i64) {
        (self.lo, self.hi)
    }

    /// The next 64 random bits.
    pub fn next_long(&mut self) -> i64 {
        let s0 = self.lo;
        let mut s1 = self.hi;
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.lo = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.hi = s1.rotate_left(28);
        result
    }
}

/// Vanilla's `XoroshiroRandomSource`.
#[derive(Debug, Clone, PartialEq)]
pub struct XoroshiroRandom {
    rng: Xoroshiro128PlusPlus,
    gaussian: Gaussian,
}

impl XoroshiroRandom {
    /// Creates a generator from a 64-bit seed, upgraded to 128 bits and mixed.
    #[must_use]
    pub fn new(seed: i64) -> Self {
        Self::from_seed(support::upgrade_seed_to_128bit(seed))
    }

    /// Creates a generator from a 128-bit seed.
    #[must_use]
    pub fn from_seed(seed: Seed128) -> Self {
        Self::from_parts(seed.lo, seed.hi)
    }

    /// Creates a generator from the two halves of its state.
    #[must_use]
    pub fn from_parts(lo: i64, hi: i64) -> Self {
        Self {
            rng: Xoroshiro128PlusPlus::new(lo, hi),
            gaussian: Gaussian::default(),
        }
    }

    /// The underlying Xoroshiro128++ generator.
    #[must_use]
    pub fn generator(&self) -> &Xoroshiro128PlusPlus {
        &self.rng
    }

    /// The top `bits` bits of the next 64-bit output.
    fn next_bits(&mut self, bits: u32) -> i64 {
        (self.rng.next_long() as u64 >> (64 - bits)) as i64
    }
}

impl RandomSource for XoroshiroRandom {
    fn set_seed(&mut self, seed: i64) {
        self.rng = Xoroshiro128PlusPlus::from_seed(support::upgrade_seed_to_128bit(seed));
        self.gaussian.reset();
    }

    fn next_int(&mut self) -> i32 {
        self.rng.next_long() as i32
    }

    /// Lemire's nearly-divisionless method, as vanilla implements it.
    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "Bound must be positive");
        let bound_long = i64::from(bound);
        let mut multiplied = i64::from(self.next_int().cast_unsigned()) * bound_long;
        let mut fractional = multiplied & 0xFFFF_FFFF;
        if fractional < bound_long {
            let threshold = i64::from(bound.wrapping_neg().cast_unsigned() % bound.cast_unsigned());
            while fractional < threshold {
                multiplied = i64::from(self.next_int().cast_unsigned()) * bound_long;
                fractional = multiplied & 0xFFFF_FFFF;
            }
        }
        (multiplied >> 32) as i32
    }

    fn next_long(&mut self) -> i64 {
        self.rng.next_long()
    }

    fn next_boolean(&mut self) -> bool {
        (self.rng.next_long() & 1) != 0
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 * FLOAT_UNIT
    }

    fn next_double(&mut self) -> f64 {
        self.next_bits(53) as f64 * DOUBLE_UNIT
    }

    fn next_gaussian(&mut self) -> f64 {
        let mut gaussian = self.gaussian;
        let value = gaussian.next(|| self.next_double());
        self.gaussian = gaussian;
        value
    }

    fn consume_count(&mut self, rounds: i32) {
        for _ in 0..rounds {
            self.rng.next_long();
        }
    }
}

impl ForkableRandom for XoroshiroRandom {
    type Fork = Self;
    type Positional = XoroshiroPositionalRandomFactory;

    fn fork(&mut self) -> Self {
        let lo = self.rng.next_long();
        let hi = self.rng.next_long();
        Self::from_parts(lo, hi)
    }

    fn fork_positional(&mut self) -> XoroshiroPositionalRandomFactory {
        let lo = self.rng.next_long();
        let hi = self.rng.next_long();
        XoroshiroPositionalRandomFactory::new(lo, hi)
    }
}

/// Vanilla's `XoroshiroRandomSource.XoroshiroPositionalRandomFactory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XoroshiroPositionalRandomFactory {
    lo: i64,
    hi: i64,
}

impl XoroshiroPositionalRandomFactory {
    /// Creates a factory from a 128-bit base seed.
    #[must_use]
    pub fn new(lo: i64, hi: i64) -> Self {
        Self { lo, hi }
    }
}

impl PositionalRandomFactory for XoroshiroPositionalRandomFactory {
    type Random = XoroshiroRandom;

    fn at(&self, x: i32, y: i32, z: i32) -> XoroshiroRandom {
        XoroshiroRandom::from_parts(mth::get_seed(x, y, z) ^ self.lo, self.hi)
    }

    fn from_hash_of(&self, name: &str) -> XoroshiroRandom {
        let seed = support::seed_from_hash_of(name).xor(self.lo, self.hi);
        XoroshiroRandom::from_seed(seed)
    }

    fn from_seed(&self, seed: i64) -> XoroshiroRandom {
        XoroshiroRandom::from_parts(seed ^ self.lo, seed ^ self.hi)
    }

    fn parity_config_string(&self) -> String {
        format!("seedLo: {}, seedHi: {}", self.lo, self.hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_state_falls_back_to_the_ratios() {
        let rng = Xoroshiro128PlusPlus::new(0, 0);
        assert_eq!(
            rng.state(),
            (support::GOLDEN_RATIO_64, support::SILVER_RATIO_64)
        );
    }

    #[test]
    fn matches_the_reference_algorithm() {
        // xoroshiro128++ with s = {1, 2}: first output is rotl(1 + 2, 17) + 1.
        let mut rng = Xoroshiro128PlusPlus::new(1, 2);
        assert_eq!(rng.next_long(), (3_i64 << 17) + 1);
    }

    #[test]
    fn bounded_ints_stay_in_range() {
        let mut random = XoroshiroRandom::new(1234);
        for bound in [1, 2, 3, 7, 1_000, 1_431_655_765, i32::MAX] {
            for _ in 0..200 {
                let value = random.next_int_bounded(bound);
                assert!((0..bound).contains(&value));
            }
        }
    }
}
