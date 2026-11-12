# Phase 2.1 research: `Mth` and the random number generators

Studied: vanilla **26.3**, mainly `net.minecraft.util.Mth`, `net.minecraft.util.RandomSource`
and `net.minecraft.world.level.levelgen.*Random*`. This note describes behaviour in our own
words; it contains no Mojang code.

## Method: the bytecode is the source of truth

Decompiled source can misstate arithmetic. Vineflower prints `BitRandomSource.nextDouble` as
`combined * 1.110223E-16F`. Read as Java source, that is float math, which would throw away
29 bits of precision. The bytecode is actually `l2d` followed by `dmul` with the double
2⁻⁵³. Every port of numeric code is therefore checked against `javap -c` output, and then
proven by golden tests against the running vanilla jar.

## Java semantics that Rust doesn't share

| Java behaviour | Where it matters | CeriumMC |
|---|---|---|
| `int * int` wraps at 32 bits *before* widening to `long` | `Mth.getSeed`, `WorldgenRandom.setFeatureSeed`, `seedSlimeChunk` | `wrapping_mul` on `i32`, then `i64::from` |
| `Math.max`/`min` on floats return NaN if either side is NaN; `-0.0 < +0.0` | `Mth.clamp(float…)`, `absMax` | `java::max_f32` and friends |
| `Math.floorMod` takes the divisor's sign | `positiveModulo` | `java::floor_mod_*` (not `rem_euclid`) |
| `Float.floatToIntBits` canonicalizes NaN | `fastInvCubeRoot` | `java::float_to_int_bits` |
| `(int)` casts of `double` saturate, and NaN becomes 0 | every `floor`/`ceil` | Rust `as` behaves the same |

## The random generators

| Vanilla | Algorithm | Gaussian cache cleared by `setSeed`? | `WorldgenRandom` bit source |
|---|---|---|---|
| `LegacyRandomSource` | 48-bit LCG (`java.util.Random`) | yes | the LCG's `next(bits)` directly |
| `SingleThreadedRandomSource` | same LCG | yes | `nextLong() >>> (64 - bits)` |
| `ThreadSafeLegacyRandomSource` | same LCG | **no** | `nextLong() >>> (64 - bits)` |
| `XoroshiroRandomSource` | Xoroshiro128++ | yes | `nextLong() >>> (64 - bits)` |
| `WorldgenRandom` | wraps one of the above | **never** (its own cache) | — |

- Only an actual `LegacyRandomSource` gets `WorldgenRandom`'s direct path, because vanilla
  checks with `instanceof`. A worldgen random wrapping the single-threaded flavour draws
  different bits from the same seed, so the three LCG classes are separate Rust types
  (`LcgRandom<Flavor>`).
- `XoroshiroRandomSource.nextInt(bound)` uses Lemire's multiply-and-reject method, while the
  LCGs use `java.util.Random`'s modulo rejection.
- 64-bit seeds are spread into Xoroshiro's 128-bit state with the golden- and silver-ratio
  constants, followed by Stafford's Mix13 (a SplitMix64 finalizer variant).
- Name-keyed generators (`fromHashOf`) hash the name differently per algorithm:
  - Xoroshiro: MD5 of the name's UTF-8 bytes, read as two big-endian longs;
  - legacy: Java's UTF-16 `String.hashCode()`, sign-extended.
- Gaussians use the Marsaglia polar method and cache the second deviate.

## Transcendental functions

Vanilla builds its tables and Gaussians with `Math.sin`, `Math.cos`, `Math.asin` and `Math.log`.
On HotSpot these are intrinsics (Intel LIBM) or fdlibm (`asin`). CeriumMC uses the pure-Rust
`libm` crate (an fdlibm port) instead of the platform's math library, so every OS and
architecture computes identical bits. The golden tests compare all 65,536 sine-table entries
and both 257-entry `atan2` tables with vanilla's actual arrays, read by reflection.

## Golden testing

`parity/golden/GoldenMain.java` is our own code, compiled in CI against the official jar:

- `mth.txt` records every `Mth` function, each with thousands of inputs biased toward edge
  cases (NaN, ±0, infinities, subnormals, `int` and `long` extremes), plus the lookup tables.
- `random.txt` records a 250,000-step program over every generator type: creation, all draw
  methods, reseeding, forking, positional factories (`at`, `fromHashOf`, `fromSeed`),
  world-generation seeding, slime chunks and the `Mth` random helpers. State carries between
  steps, so any divergence surfaces immediately.

Rust replays both files with zero tolerance, comparing floats by bits (any NaN matches any
NaN). It does this on Linux x86_64, then again on Linux ARM64, Windows, macOS and 32-bit ARMv7
under QEMU. The recorded data never leaves the CI cache.
