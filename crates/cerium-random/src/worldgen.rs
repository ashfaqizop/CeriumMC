//! Vanilla's `WorldgenRandom`: a bit source with the world-generation seeding schemes.

use crate::bits::{self, BitRandomSource};
use crate::gaussian::Gaussian;
use crate::{ForkableRandom, RandomSource, SingleThreadedRandom};

/// Vanilla's `WorldgenRandom`, wrapping another generator.
///
/// It derives every value from `next(bits)`, which draws from the wrapped source, and counts
/// how many times it was called. Its own Gaussian cache is never cleared by `set_seed`,
/// exactly as in vanilla.
#[derive(Debug, Clone)]
pub struct WorldgenRandom<R> {
    source: R,
    count: i32,
    gaussian: Gaussian,
}

impl<R: RandomSource> WorldgenRandom<R> {
    /// Wraps `source`.
    #[must_use]
    pub fn new(source: R) -> Self {
        Self {
            source,
            count: 0,
            gaussian: Gaussian::default(),
        }
    }

    /// How many times `next(bits)` has been called. Wraps like a Java `int`.
    #[must_use]
    pub fn count(&self) -> i32 {
        self.count
    }

    /// The wrapped generator.
    #[must_use]
    pub fn source(&self) -> &R {
        &self.source
    }

    /// Seeds for decorating a chunk; returns the seed it set.
    pub fn set_decoration_seed(&mut self, seed: i64, chunk_x: i32, chunk_z: i32) -> i64 {
        self.set_seed(seed);
        let x_scale = self.next_long() | 1;
        let z_scale = self.next_long() | 1;
        let x_part = i64::from(chunk_x).wrapping_mul(x_scale);
        let z_part = i64::from(chunk_z).wrapping_mul(z_scale);
        let result = x_part.wrapping_add(z_part) ^ seed;
        self.set_seed(result);
        result
    }

    /// Seeds for one feature: `seed + index + 10000 * step`. The product wraps as an `int`.
    pub fn set_feature_seed(&mut self, seed: i64, index: i32, step: i32) {
        let step_offset = i64::from(10_000_i32.wrapping_mul(step));
        let result = seed
            .wrapping_add(i64::from(index))
            .wrapping_add(step_offset);
        self.set_seed(result);
    }

    /// Seeds for a large feature (carvers, structures) in chunk `(chunk_x, chunk_z)`.
    pub fn set_large_feature_seed(&mut self, seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(seed);
        let x_scale = self.next_long();
        let z_scale = self.next_long();
        let x_part = i64::from(chunk_x).wrapping_mul(x_scale);
        let z_part = i64::from(chunk_z).wrapping_mul(z_scale);
        self.set_seed(x_part ^ z_part ^ seed);
    }

    /// Seeds for a structure placement with a salt.
    pub fn set_large_feature_with_salt(&mut self, seed: i64, x: i32, z: i32, blend: i32) {
        let x_part = i64::from(x).wrapping_mul(341_873_128_712);
        let z_part = i64::from(z).wrapping_mul(132_897_987_541);
        let result = x_part
            .wrapping_add(z_part)
            .wrapping_add(seed)
            .wrapping_add(i64::from(blend));
        self.set_seed(result);
    }
}

/// The generator that decides whether chunk `(x, z)` is a slime chunk. The `int` products
/// wrap exactly as in vanilla.
#[must_use]
pub fn seed_slime_chunk(x: i32, z: i32, seed: i64, salt: i64) -> SingleThreadedRandom {
    let x_squared_term = i64::from(x.wrapping_mul(x).wrapping_mul(4_987_142));
    let x_term = i64::from(x.wrapping_mul(5_947_611));
    let z_squared_term = i64::from(z.wrapping_mul(z)).wrapping_mul(4_392_871);
    let z_term = i64::from(z.wrapping_mul(389_711));
    let mixed = seed
        .wrapping_add(x_squared_term)
        .wrapping_add(x_term)
        .wrapping_add(z_squared_term)
        .wrapping_add(z_term);
    SingleThreadedRandom::new(mixed ^ salt)
}

impl<R: RandomSource> BitRandomSource for WorldgenRandom<R> {
    fn next(&mut self, bits: u32) -> i32 {
        self.count = self.count.wrapping_add(1);
        self.source.next_bits_for_worldgen(bits)
    }
}

impl<R: RandomSource> RandomSource for WorldgenRandom<R> {
    fn set_seed(&mut self, seed: i64) {
        self.source.set_seed(seed);
    }

    fn next_int(&mut self) -> i32 {
        bits::next_int(self)
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        bits::next_int_bounded(self, bound)
    }

    fn next_long(&mut self) -> i64 {
        bits::next_long(self)
    }

    fn next_boolean(&mut self) -> bool {
        bits::next_boolean(self)
    }

    fn next_float(&mut self) -> f32 {
        bits::next_float(self)
    }

    fn next_double(&mut self) -> f64 {
        bits::next_double(self)
    }

    fn next_gaussian(&mut self) -> f64 {
        let mut gaussian = self.gaussian;
        let value = gaussian.next(|| bits::next_double(self));
        self.gaussian = gaussian;
        value
    }
}

impl<R: ForkableRandom> ForkableRandom for WorldgenRandom<R> {
    type Fork = R::Fork;
    type Positional = R::Positional;

    fn fork(&mut self) -> R::Fork {
        self.source.fork()
    }

    fn fork_positional(&mut self) -> R::Positional {
        self.source.fork_positional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::LegacyRandom;

    #[test]
    fn counts_every_bit_draw() {
        let mut random = WorldgenRandom::new(LegacyRandom::new(1));
        random.next_long();
        random.next_int();
        random.next_double();
        assert_eq!(random.count(), 5);
    }

    #[test]
    fn legacy_source_reads_bits_directly() {
        let mut direct = WorldgenRandom::new(LegacyRandom::new(99));
        let mut plain = LegacyRandom::new(99);
        assert_eq!(direct.next_int(), plain.next_int());
        assert_eq!(direct.next_long(), plain.next_long());
    }
}
