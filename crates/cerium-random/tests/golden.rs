//! Replays a random program recorded from vanilla's generators (see `tools/cerium-golden`).
//!
//! The program creates generators of every vanilla type, then runs operations on them:
//! draws, reseeding, forking, positional factories, world-generation seeding and the `Mth`
//! helpers. State carries across steps, so any divergence shows up immediately.

use cerium_golden::{Case, Report, Value, cpu_vendor, load};
use cerium_math::hotspot::{self, Vendor};
use cerium_random::{
    ForkableRandom, LegacyPositionalRandomFactory, LegacyRandom, PositionalRandomFactory,
    RandomSource, SingleThreadedRandom, ThreadSafeLegacyRandom, WorldgenRandom,
    XoroshiroPositionalRandomFactory, XoroshiroRandom, mth_random, seed_slime_chunk, support,
};

/// One generator of any vanilla type.
enum Gen {
    Legacy(LegacyRandom),
    Single(SingleThreadedRandom),
    ThreadSafe(ThreadSafeLegacyRandom),
    Xoro(XoroshiroRandom),
    WgLegacy(WorldgenRandom<LegacyRandom>),
    WgSingle(WorldgenRandom<SingleThreadedRandom>),
    WgThreadSafe(WorldgenRandom<ThreadSafeLegacyRandom>),
    WgXoro(WorldgenRandom<XoroshiroRandom>),
}

/// One positional factory of either vanilla type.
enum Fac {
    Legacy(LegacyPositionalRandomFactory),
    Xoro(XoroshiroPositionalRandomFactory),
}

/// Runs `$body` with `$w` bound to the generator if it is a `WorldgenRandom`.
macro_rules! worldgen {
    ($target:expr, $w:ident => $body:expr) => {
        match $target {
            Gen::WgLegacy($w) => $body,
            Gen::WgSingle($w) => $body,
            Gen::WgThreadSafe($w) => $body,
            Gen::WgXoro($w) => $body,
            _ => panic!("not a WorldgenRandom"),
        }
    };
}

impl Gen {
    fn random(&mut self) -> &mut dyn RandomSource {
        match self {
            Self::Legacy(r) => r,
            Self::Single(r) => r,
            Self::ThreadSafe(r) => r,
            Self::Xoro(r) => r,
            Self::WgLegacy(r) => r,
            Self::WgSingle(r) => r,
            Self::WgThreadSafe(r) => r,
            Self::WgXoro(r) => r,
        }
    }

    /// Forks with vanilla's types: a `WorldgenRandom` forks its wrapped source.
    fn fork(&mut self) -> Self {
        match self {
            Self::Legacy(r) => Self::Legacy(r.fork()),
            Self::Single(r) => Self::Single(r.fork()),
            Self::ThreadSafe(r) => Self::ThreadSafe(r.fork()),
            Self::Xoro(r) => Self::Xoro(r.fork()),
            Self::WgLegacy(r) => Self::Legacy(r.fork()),
            Self::WgSingle(r) => Self::Single(r.fork()),
            Self::WgThreadSafe(r) => Self::ThreadSafe(r.fork()),
            Self::WgXoro(r) => Self::Xoro(r.fork()),
        }
    }

    fn fork_positional(&mut self) -> Fac {
        match self {
            Self::Legacy(r) => Fac::Legacy(r.fork_positional()),
            Self::Single(r) => Fac::Legacy(r.fork_positional()),
            Self::ThreadSafe(r) => Fac::Legacy(r.fork_positional()),
            Self::Xoro(r) => Fac::Xoro(r.fork_positional()),
            Self::WgLegacy(r) => Fac::Legacy(r.fork_positional()),
            Self::WgSingle(r) => Fac::Legacy(r.fork_positional()),
            Self::WgThreadSafe(r) => Fac::Legacy(r.fork_positional()),
            Self::WgXoro(r) => Fac::Xoro(r.fork_positional()),
        }
    }
}

impl Fac {
    fn at(&self, x: i32, y: i32, z: i32) -> Gen {
        match self {
            Self::Legacy(f) => Gen::Legacy(f.at(x, y, z)),
            Self::Xoro(f) => Gen::Xoro(f.at(x, y, z)),
        }
    }

    #[allow(clippy::wrong_self_convention, reason = "mirrors the factory trait")]
    fn from_hash_of(&self, name: &str) -> Gen {
        match self {
            Self::Legacy(f) => Gen::Legacy(f.from_hash_of(name)),
            Self::Xoro(f) => Gen::Xoro(f.from_hash_of(name)),
        }
    }

    #[allow(clippy::wrong_self_convention, reason = "mirrors the factory trait")]
    fn from_seed(&self, seed: i64) -> Gen {
        match self {
            Self::Legacy(f) => Gen::Legacy(f.from_seed(seed)),
            Self::Xoro(f) => Gen::Xoro(f.from_seed(seed)),
        }
    }

    fn parity_config_string(&self) -> String {
        match self {
            Self::Legacy(f) => f.parity_config_string(),
            Self::Xoro(f) => f.parity_config_string(),
        }
    }
}

/// Every generator and factory the program has created, by id.
#[derive(Default)]
struct State {
    gens: Vec<Gen>,
    facs: Vec<Fac>,
}

fn id(case: &Case, arg: usize) -> usize {
    usize::try_from(case.int(arg)).expect("ids are non-negative")
}

impl State {
    fn add_gen(&mut self, case: &Case, arg: usize, generator: Gen) -> Vec<Value> {
        assert_eq!(
            id(case, arg),
            self.gens.len(),
            "line {}: id mismatch",
            case.line
        );
        self.gens.push(generator);
        Vec::new()
    }

    fn add_fac(&mut self, case: &Case, arg: usize, fac: Fac) -> Vec<Value> {
        assert_eq!(
            id(case, arg),
            self.facs.len(),
            "line {}: id mismatch",
            case.line
        );
        self.facs.push(fac);
        Vec::new()
    }

    fn source(&mut self, case: &Case) -> &mut dyn RandomSource {
        self.gens[id(case, 0)].random()
    }

    #[allow(clippy::too_many_lines, reason = "one arm per recorded operation")]
    fn step(&mut self, c: &Case) -> Vec<Value> {
        use Value::{Bool, Double, Float, Int, Long, Str};

        let i = |k| c.int(k);
        let l = |k| c.long(k);
        let one = match c.op.as_str() {
            "new.legacy" => return self.add_gen(c, 0, Gen::Legacy(LegacyRandom::new(l(1)))),
            "new.single" => {
                let generator = Gen::Single(SingleThreadedRandom::new(l(1)));
                return self.add_gen(c, 0, generator);
            }
            "new.threadsafe" => {
                let generator = Gen::ThreadSafe(ThreadSafeLegacyRandom::new(l(1)));
                return self.add_gen(c, 0, generator);
            }
            "new.xoro" => return self.add_gen(c, 0, Gen::Xoro(XoroshiroRandom::new(l(1)))),
            "new.xoro128" => {
                let generator = Gen::Xoro(XoroshiroRandom::from_parts(l(1), l(2)));
                return self.add_gen(c, 0, generator);
            }
            "new.wg.legacy" => {
                let generator = Gen::WgLegacy(WorldgenRandom::new(LegacyRandom::new(l(1))));
                return self.add_gen(c, 0, generator);
            }
            "new.wg.single" => {
                let generator = Gen::WgSingle(WorldgenRandom::new(SingleThreadedRandom::new(l(1))));
                return self.add_gen(c, 0, generator);
            }
            "new.wg.threadsafe" => {
                let source = ThreadSafeLegacyRandom::new(l(1));
                let generator = Gen::WgThreadSafe(WorldgenRandom::new(source));
                return self.add_gen(c, 0, generator);
            }
            "new.wg.xoro" => {
                let generator = Gen::WgXoro(WorldgenRandom::new(XoroshiroRandom::new(l(1))));
                return self.add_gen(c, 0, generator);
            }
            "ni" => Int(self.source(c).next_int()),
            "nib" => Int(self.source(c).next_int_bounded(i(1))),
            "nibi" => Int(self.source(c).next_int_between_inclusive(i(1), i(2))),
            "nir" => Int(self.source(c).next_int_range(i(1), i(2))),
            "nl" => Long(self.source(c).next_long()),
            "nb" => Bool(self.source(c).next_boolean()),
            "nf" => Float(self.source(c).next_float()),
            "nd" => Double(self.source(c).next_double()),
            "ng" => Double(self.source(c).next_gaussian()),
            "trid" => Double(self.source(c).triangle_f64(c.double(1), c.double(2))),
            "trif" => Float(self.source(c).triangle_f32(c.float(1), c.float(2))),
            "cc" => {
                self.source(c).consume_count(i(1));
                return Vec::new();
            }
            "ss" => {
                self.source(c).set_seed(l(1));
                return Vec::new();
            }
            "fork" => {
                let child = self.gens[id(c, 0)].fork();
                return self.add_gen(c, 1, child);
            }
            "fp" => {
                let fac = self.gens[id(c, 0)].fork_positional();
                return self.add_fac(c, 1, fac);
            }
            "wg.dec" => {
                let target = &mut self.gens[id(c, 0)];
                let seed = worldgen!(target, w => w.set_decoration_seed(l(1), i(2), i(3)));
                Long(seed)
            }
            "wg.feat" => {
                let target = &mut self.gens[id(c, 0)];
                worldgen!(target, w => w.set_feature_seed(l(1), i(2), i(3)));
                return Vec::new();
            }
            "wg.large" => {
                let target = &mut self.gens[id(c, 0)];
                worldgen!(target, w => w.set_large_feature_seed(l(1), i(2), i(3)));
                return Vec::new();
            }
            "wg.salt" => {
                let target = &mut self.gens[id(c, 0)];
                worldgen!(target, w => w.set_large_feature_with_salt(l(1), i(2), i(3), i(4)));
                return Vec::new();
            }
            "wg.count" => {
                let target = &mut self.gens[id(c, 0)];
                Int(worldgen!(target, w => w.count()))
            }
            "at" => {
                let child = self.facs[id(c, 0)].at(i(2), i(3), i(4));
                return self.add_gen(c, 1, child);
            }
            "hash" => {
                let child = self.facs[id(c, 0)].from_hash_of(&c.string(2));
                return self.add_gen(c, 1, child);
            }
            "fseed" => {
                let child = self.facs[id(c, 0)].from_seed(l(2));
                return self.add_gen(c, 1, child);
            }
            "fstr" => Str(self.facs[id(c, 0)].parity_config_string()),
            "mth.nextint" => Int(mth_random::next_int(self.source(c), i(1), i(2))),
            "mth.nextfloat" => {
                let value = mth_random::next_float(self.source(c), c.float(1), c.float(2));
                Float(value)
            }
            "mth.nextdouble" => {
                let value = mth_random::next_double(self.source(c), c.double(1), c.double(2));
                Double(value)
            }
            "mth.rbi" => {
                let value = mth_random::random_between_inclusive(self.source(c), i(1), i(2));
                Int(value)
            }
            "mth.rb" => {
                let value = mth_random::random_between(self.source(c), c.float(1), c.float(2));
                Float(value)
            }
            "mth.normal" => Float(mth_random::normal(self.source(c), c.float(1), c.float(2))),
            "mth.uuid" => {
                let (most, least) = mth_random::create_insecure_uuid(self.source(c));
                return vec![Long(most), Long(least)];
            }
            "slime" => {
                let child = Gen::Single(seed_slime_chunk(i(1), i(2), l(3), l(4)));
                return self.add_gen(c, 0, child);
            }
            "mix" => Long(support::mix_stafford13(l(0))),
            "seed128" => {
                let seed = support::upgrade_seed_to_128bit(l(0));
                return vec![Long(seed.lo), Long(seed.hi)];
            }
            "seed128u" => {
                let seed = support::upgrade_seed_to_128bit_unmixed(l(0));
                return vec![Long(seed.lo), Long(seed.hi)];
            }
            "seedhash" => {
                let seed = support::seed_from_hash_of(&c.string(0));
                return vec![Long(seed.lo), Long(seed.hi)];
            }
            "wobble" => Double(mth_random::wobble(c.double(0))),
            other => panic!("line {}: unknown op {other}", c.line),
        };
        vec![one]
    }
}

#[test]
fn random_program_matches_vanilla() {
    let Some(cases) = load("random.txt") else {
        return;
    };
    // `nextGaussian` uses HotSpot's `Math.log`, which depends on the recording CPU's vendor.
    let name = cpu_vendor().expect("golden data was loaded");
    let vendor = Vendor::from_cpuid(&name)
        .unwrap_or_else(|| panic!("golden data was recorded on an unknown CPU vendor {name:?}"));
    hotspot::set_vendor(vendor);
    let mut state = State::default();
    let mut report = Report::new("random.txt");
    for case in &cases {
        let actual = state.step(case);
        report.check(case, &actual);
    }
    report.finish();
}
