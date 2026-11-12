//! The 48-bit linear congruential generator from `java.util.Random`, in vanilla's three
//! flavours.

use std::marker::PhantomData;

use cerium_math::mth;
use cerium_util::java;

use crate::bits::{self, BitRandomSource};
use crate::gaussian::Gaussian;
use crate::{ForkableRandom, PositionalRandomFactory, RandomSource};

const MULTIPLIER: i64 = 0x5_DEEC_E66D;
const INCREMENT: i64 = 11;
const MODULUS_MASK: i64 = (1 << 48) - 1;

mod sealed {
    #[allow(unreachable_pub, reason = "sealed-trait pattern")]
    pub trait Sealed {}
}

/// The behaviour that distinguishes vanilla's three LCG classes.
pub trait LcgFlavor: sealed::Sealed {
    /// Whether `set_seed` clears the cached Gaussian.
    const RESETS_GAUSSIAN_ON_SEED: bool;
    /// Whether [`WorldgenRandom`](crate::WorldgenRandom) reads `next(bits)` directly
    /// (vanilla's `instanceof LegacyRandomSource` check).
    const DIRECT_WORLDGEN_BITS: bool;
}

/// Flavour of `LegacyRandomSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legacy {}

/// Flavour of `SingleThreadedRandomSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleThreaded {}

/// Flavour of `ThreadSafeLegacyRandomSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadSafe {}

impl sealed::Sealed for Legacy {}
impl sealed::Sealed for SingleThreaded {}
impl sealed::Sealed for ThreadSafe {}

impl LcgFlavor for Legacy {
    const RESETS_GAUSSIAN_ON_SEED: bool = true;
    const DIRECT_WORLDGEN_BITS: bool = true;
}

impl LcgFlavor for SingleThreaded {
    const RESETS_GAUSSIAN_ON_SEED: bool = true;
    const DIRECT_WORLDGEN_BITS: bool = false;
}

impl LcgFlavor for ThreadSafe {
    const RESETS_GAUSSIAN_ON_SEED: bool = false;
    const DIRECT_WORLDGEN_BITS: bool = false;
}

/// The `java.util.Random` LCG: `seed = (seed * 0x5DEECE66D + 11) mod 2^48`.
#[derive(Debug, Clone, PartialEq)]
pub struct LcgRandom<F: LcgFlavor> {
    seed: i64,
    gaussian: Gaussian,
    flavor: PhantomData<F>,
}

/// Vanilla's `LegacyRandomSource`.
pub type LegacyRandom = LcgRandom<Legacy>;

/// Vanilla's `SingleThreadedRandomSource`.
pub type SingleThreadedRandom = LcgRandom<SingleThreaded>;

/// Vanilla's `ThreadSafeLegacyRandomSource`.
pub type ThreadSafeLegacyRandom = LcgRandom<ThreadSafe>;

impl<F: LcgFlavor> LcgRandom<F> {
    /// Creates a generator with the given seed.
    #[must_use]
    pub fn new(seed: i64) -> Self {
        Self {
            seed: scramble(seed),
            gaussian: Gaussian::default(),
            flavor: PhantomData,
        }
    }
}

/// The initial scrambling `java.util.Random` applies to a seed.
fn scramble(seed: i64) -> i64 {
    (seed ^ MULTIPLIER) & MODULUS_MASK
}

impl<F: LcgFlavor> BitRandomSource for LcgRandom<F> {
    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT) & MODULUS_MASK;
        (self.seed >> (48 - bits)) as i32
    }
}

impl<F: LcgFlavor> RandomSource for LcgRandom<F> {
    fn set_seed(&mut self, seed: i64) {
        self.seed = scramble(seed);
        if F::RESETS_GAUSSIAN_ON_SEED {
            self.gaussian.reset();
        }
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

    fn next_bits_for_worldgen(&mut self, bits: u32) -> i32 {
        if F::DIRECT_WORLDGEN_BITS {
            self.next(bits)
        } else {
            (self.next_long() as u64 >> (64 - bits)) as i32
        }
    }
}

impl<F: LcgFlavor> ForkableRandom for LcgRandom<F> {
    type Fork = Self;
    type Positional = LegacyPositionalRandomFactory;

    fn fork(&mut self) -> Self {
        Self::new(self.next_long())
    }

    fn fork_positional(&mut self) -> LegacyPositionalRandomFactory {
        LegacyPositionalRandomFactory::new(self.next_long())
    }
}

/// Vanilla's `LegacyRandomSource.LegacyPositionalRandomFactory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyPositionalRandomFactory {
    seed: i64,
}

impl LegacyPositionalRandomFactory {
    /// Creates a factory with the given base seed.
    #[must_use]
    pub fn new(seed: i64) -> Self {
        Self { seed }
    }
}

impl PositionalRandomFactory for LegacyPositionalRandomFactory {
    type Random = LegacyRandom;

    fn at(&self, x: i32, y: i32, z: i32) -> LegacyRandom {
        LegacyRandom::new(mth::get_seed(x, y, z) ^ self.seed)
    }

    fn from_hash_of(&self, name: &str) -> LegacyRandom {
        LegacyRandom::new(i64::from(java::string_hash_code(name)) ^ self.seed)
    }

    fn from_seed(&self, seed: i64) -> LegacyRandom {
        LegacyRandom::new(seed)
    }

    fn parity_config_string(&self) -> String {
        format!("LegacyPositionalRandomFactory{{{}}}", self.seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_util_random() {
        // Values from java.util.Random(42): nextInt() then nextLong().
        let mut random = LegacyRandom::new(42);
        assert_eq!(random.next_int(), -1_170_105_035);
        assert_eq!(random.next_int_bounded(10), 3);
    }

    #[test]
    fn only_thread_safe_keeps_its_gaussian_across_reseeding() {
        // Deviates come in pairs: the first call computes both and caches the second.
        let mut reference = LegacyRandom::new(7);
        let first = reference.next_gaussian().to_bits();
        let second = reference.next_gaussian().to_bits();

        let mut legacy = LegacyRandom::new(7);
        legacy.next_gaussian();
        legacy.set_seed(7);
        assert_eq!(legacy.next_gaussian().to_bits(), first, "reseeding clears the cache");

        let mut thread_safe = ThreadSafeLegacyRandom::new(7);
        thread_safe.next_gaussian();
        thread_safe.set_seed(7);
        assert_eq!(thread_safe.next_gaussian().to_bits(), second, "the cache survives");
    }
}
