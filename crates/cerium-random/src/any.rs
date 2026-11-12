//! Generators chosen at runtime: vanilla's `WorldgenRandom.Algorithm`.

use crate::{
    ForkableRandom, LegacyPositionalRandomFactory, LegacyRandom, PositionalRandomFactory,
    RandomSource, XoroshiroPositionalRandomFactory, XoroshiroRandom,
};

/// The world-generation random algorithm a world uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Algorithm {
    /// [`LegacyRandom`], used by worlds created with old world settings.
    Legacy,
    /// [`XoroshiroRandom`], used by modern worlds.
    Xoroshiro,
}

impl Algorithm {
    /// Creates a generator of this algorithm with the given seed.
    #[must_use]
    pub fn new_instance(self, seed: i64) -> AnyRandom {
        match self {
            Self::Legacy => AnyRandom::Legacy(LegacyRandom::new(seed)),
            Self::Xoroshiro => AnyRandom::Xoroshiro(XoroshiroRandom::new(seed)),
        }
    }
}

/// A generator of either algorithm.
#[derive(Debug, Clone)]
pub enum AnyRandom {
    /// A [`LegacyRandom`].
    Legacy(LegacyRandom),
    /// A [`XoroshiroRandom`].
    Xoroshiro(XoroshiroRandom),
}

/// Dispatches a method call to whichever generator `$any` holds.
macro_rules! dispatch {
    ($any:expr, $random:ident => $call:expr) => {
        match $any {
            AnyRandom::Legacy($random) => $call,
            AnyRandom::Xoroshiro($random) => $call,
        }
    };
}

impl RandomSource for AnyRandom {
    fn set_seed(&mut self, seed: i64) {
        dispatch!(self, random => random.set_seed(seed));
    }

    fn next_int(&mut self) -> i32 {
        dispatch!(self, random => random.next_int())
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        dispatch!(self, random => random.next_int_bounded(bound))
    }

    fn next_long(&mut self) -> i64 {
        dispatch!(self, random => random.next_long())
    }

    fn next_boolean(&mut self) -> bool {
        dispatch!(self, random => random.next_boolean())
    }

    fn next_float(&mut self) -> f32 {
        dispatch!(self, random => random.next_float())
    }

    fn next_double(&mut self) -> f64 {
        dispatch!(self, random => random.next_double())
    }

    fn next_gaussian(&mut self) -> f64 {
        dispatch!(self, random => random.next_gaussian())
    }

    fn consume_count(&mut self, rounds: i32) {
        dispatch!(self, random => random.consume_count(rounds));
    }

    fn next_bits_for_worldgen(&mut self, bits: u32) -> i32 {
        dispatch!(self, random => random.next_bits_for_worldgen(bits))
    }
}

impl ForkableRandom for AnyRandom {
    type Fork = Self;
    type Positional = AnyPositionalRandomFactory;

    fn fork(&mut self) -> Self {
        match self {
            Self::Legacy(random) => Self::Legacy(random.fork()),
            Self::Xoroshiro(random) => Self::Xoroshiro(random.fork()),
        }
    }

    fn fork_positional(&mut self) -> AnyPositionalRandomFactory {
        match self {
            Self::Legacy(random) => AnyPositionalRandomFactory::Legacy(random.fork_positional()),
            Self::Xoroshiro(random) => {
                AnyPositionalRandomFactory::Xoroshiro(random.fork_positional())
            }
        }
    }
}

/// A positional factory of either algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnyPositionalRandomFactory {
    /// A [`LegacyPositionalRandomFactory`].
    Legacy(LegacyPositionalRandomFactory),
    /// A [`XoroshiroPositionalRandomFactory`].
    Xoroshiro(XoroshiroPositionalRandomFactory),
}

impl PositionalRandomFactory for AnyPositionalRandomFactory {
    type Random = AnyRandom;

    fn at(&self, x: i32, y: i32, z: i32) -> AnyRandom {
        match self {
            Self::Legacy(factory) => AnyRandom::Legacy(factory.at(x, y, z)),
            Self::Xoroshiro(factory) => AnyRandom::Xoroshiro(factory.at(x, y, z)),
        }
    }

    fn from_hash_of(&self, name: &str) -> AnyRandom {
        match self {
            Self::Legacy(factory) => AnyRandom::Legacy(factory.from_hash_of(name)),
            Self::Xoroshiro(factory) => AnyRandom::Xoroshiro(factory.from_hash_of(name)),
        }
    }

    fn from_seed(&self, seed: i64) -> AnyRandom {
        match self {
            Self::Legacy(factory) => AnyRandom::Legacy(factory.from_seed(seed)),
            Self::Xoroshiro(factory) => AnyRandom::Xoroshiro(factory.from_seed(seed)),
        }
    }

    fn parity_config_string(&self) -> String {
        match self {
            Self::Legacy(factory) => factory.parity_config_string(),
            Self::Xoroshiro(factory) => factory.parity_config_string(),
        }
    }
}
