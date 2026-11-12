//! Bit-exact ports of vanilla's random number generators.
//!
//! | Vanilla class | CeriumMC type |
//! |---|---|
//! | `LegacyRandomSource` | [`LegacyRandom`] |
//! | `SingleThreadedRandomSource` | [`SingleThreadedRandom`] |
//! | `ThreadSafeLegacyRandomSource` | [`ThreadSafeLegacyRandom`] |
//! | `XoroshiroRandomSource` | [`XoroshiroRandom`] |
//! | `Xoroshiro128PlusPlus` | [`Xoroshiro128PlusPlus`] |
//! | `WorldgenRandom` | [`WorldgenRandom`] |
//! | `LegacyPositionalRandomFactory` (nested) | [`LegacyPositionalRandomFactory`] |
//! | `XoroshiroPositionalRandomFactory` (nested) | [`XoroshiroPositionalRandomFactory`] |
//! | `RandomSupport` | [`support`] |
//! | `WorldgenRandom.Algorithm` | [`Algorithm`], [`AnyRandom`] |
//! | `Mth` helpers that take a random source | [`mth_random`] |
//!
//! The three linear congruential generators share one algorithm, but they differ in ways
//! that change their output, so they are distinct types:
//!
//! - `set_seed` clears the cached Gaussian for [`LegacyRandom`] and [`SingleThreadedRandom`],
//!   but not for [`ThreadSafeLegacyRandom`].
//! - [`WorldgenRandom`] reads bits straight from the LCG only when wrapping a
//!   [`LegacyRandom`]; every other source is sampled through `next_long() >>> (64 - bits)`.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "these are deliberate, bit-exact ports of Java's primitive conversions and comparisons"
)]

mod any;
mod bits;
mod gaussian;
mod lcg;
pub mod mth_random;
pub mod support;
mod worldgen;
mod xoroshiro;

pub use any::{Algorithm, AnyPositionalRandomFactory, AnyRandom};
pub use bits::BitRandomSource;
pub use lcg::{
    LcgFlavor, LcgRandom, Legacy, LegacyPositionalRandomFactory, LegacyRandom, SingleThreaded,
    SingleThreadedRandom, ThreadSafe, ThreadSafeLegacyRandom,
};
pub use worldgen::{WorldgenRandom, seed_slime_chunk};
pub use xoroshiro::{Xoroshiro128PlusPlus, XoroshiroPositionalRandomFactory, XoroshiroRandom};

/// `2^-24`: scales 24 random bits into a float in `[0, 1)`.
pub(crate) const FLOAT_UNIT: f32 = 1.0 / 16_777_216.0;

/// `2^-53`: scales 53 random bits into a double in `[0, 1)`.
pub(crate) const DOUBLE_UNIT: f64 = 1.0 / 9_007_199_254_740_992.0;

/// Vanilla's `RandomSource`: the operations every generator supports.
///
/// The trait is object-safe, so code can take `&mut dyn RandomSource` the way vanilla
/// passes `RandomSource` around. Forking lives in [`ForkableRandom`].
pub trait RandomSource {
    /// Re-seeds the generator.
    fn set_seed(&mut self, seed: i64);

    /// A uniformly random `int`.
    fn next_int(&mut self) -> i32;

    /// A uniformly random `int` in `[0, bound)`.
    ///
    /// # Panics
    ///
    /// Panics if `bound <= 0`, where vanilla throws `IllegalArgumentException`.
    fn next_int_bounded(&mut self, bound: i32) -> i32;

    /// A uniformly random `long`.
    fn next_long(&mut self) -> i64;

    /// A uniformly random `boolean`.
    fn next_boolean(&mut self) -> bool;

    /// A uniformly random `float` in `[0, 1)`.
    fn next_float(&mut self) -> f32;

    /// A uniformly random `double` in `[0, 1)`.
    fn next_double(&mut self) -> f64;

    /// A normally distributed `double` (mean 0, standard deviation 1).
    fn next_gaussian(&mut self) -> f64;

    /// A uniformly random `int` in `[min, max_inclusive]`. Arithmetic wraps like Java's.
    ///
    /// # Panics
    ///
    /// Panics if the range is empty or wider than `i32::MAX`.
    fn next_int_between_inclusive(&mut self, min: i32, max_inclusive: i32) -> i32 {
        let bound = max_inclusive.wrapping_sub(min).wrapping_add(1);
        self.next_int_bounded(bound).wrapping_add(min)
    }

    /// `mean + spread * (next_double() - next_double())`: a triangular distribution.
    fn triangle_f64(&mut self, mean: f64, spread: f64) -> f64 {
        let first = self.next_double();
        let second = self.next_double();
        mean + spread * (first - second)
    }

    /// `mean + spread * (next_float() - next_float())`: a triangular distribution.
    fn triangle_f32(&mut self, mean: f32, spread: f32) -> f32 {
        let first = self.next_float();
        let second = self.next_float();
        mean + spread * (first - second)
    }

    /// Advances the generator by `rounds` calls to [`next_int`](Self::next_int).
    fn consume_count(&mut self, rounds: i32) {
        for _ in 0..rounds {
            self.next_int();
        }
    }

    /// A uniformly random `int` in `[origin, bound)`.
    ///
    /// # Panics
    ///
    /// Panics if `origin >= bound`, where vanilla throws `IllegalArgumentException`.
    fn next_int_range(&mut self, origin: i32, bound: i32) -> i32 {
        assert!(origin < bound, "bound - origin is non positive");
        origin.wrapping_add(self.next_int_bounded(bound.wrapping_sub(origin)))
    }

    /// The bits [`WorldgenRandom`] draws from this source: `(int) (next_long() >>> (64 - bits))`.
    /// [`LegacyRandom`] overrides this to read its LCG directly, as vanilla does.
    #[doc(hidden)]
    fn next_bits_for_worldgen(&mut self, bits: u32) -> i32 {
        (self.next_long() as u64 >> (64 - bits)) as i32
    }
}

/// Generators that can split off independent child generators.
pub trait ForkableRandom: RandomSource {
    /// The type produced by [`fork`](Self::fork).
    type Fork: ForkableRandom;
    /// The type produced by [`fork_positional`](Self::fork_positional).
    type Positional: PositionalRandomFactory;

    /// Creates a new generator seeded from this one.
    fn fork(&mut self) -> Self::Fork;

    /// Creates a factory for position- and name-keyed generators, seeded from this one.
    fn fork_positional(&mut self) -> Self::Positional;
}

/// Vanilla's `PositionalRandomFactory`: deterministic generators keyed by position or name.
pub trait PositionalRandomFactory {
    /// The generators this factory creates.
    type Random: ForkableRandom;

    /// A generator for block position `(x, y, z)`.
    fn at(&self, x: i32, y: i32, z: i32) -> Self::Random;

    /// A generator for a name, such as an identifier like `minecraft:ore_gold`.
    fn from_hash_of(&self, name: &str) -> Self::Random;

    /// A generator for an explicit seed.
    fn from_seed(&self, seed: i64) -> Self::Random;

    /// The text vanilla's `parityConfigString` appends; used to compare configurations.
    fn parity_config_string(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_are_the_vanilla_constants() {
        assert_eq!(FLOAT_UNIT, "5.9604645E-8".parse::<f32>().unwrap());
        assert_eq!(DOUBLE_UNIT, "1.1102230246251565E-16".parse::<f64>().unwrap());
    }
}
