//! Vanilla's `RandomSupport`: seed mixing and 128-bit seed derivation.

use cerium_util::md5;

/// `0x9E3779B97F4A7C15`: 2^64 divided by the golden ratio.
pub const GOLDEN_RATIO_64: i64 = 0x9E37_79B9_7F4A_7C15_u64 as i64;

/// `0x6A09E667F3BCC909`: the fractional bits of the square root of 2 (the silver ratio).
pub const SILVER_RATIO_64: i64 = 0x6A09_E667_F3BC_C909;

/// Stafford's "Mix13" variant of the `SplitMix64` finalizer.
#[must_use]
pub fn mix_stafford13(z: i64) -> i64 {
    let mut z = z as u64;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) as i64
}

/// A 128-bit seed: vanilla's `RandomSupport.Seed128bit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Seed128 {
    /// The low 64 bits.
    pub lo: i64,
    /// The high 64 bits.
    pub hi: i64,
}

impl Seed128 {
    /// Creates a seed from its two halves.
    #[must_use]
    pub const fn new(lo: i64, hi: i64) -> Self {
        Self { lo, hi }
    }

    /// XORs each half with the given values.
    #[must_use]
    pub const fn xor(self, lo: i64, hi: i64) -> Self {
        Self::new(self.lo ^ lo, self.hi ^ hi)
    }

    /// Applies [`mix_stafford13`] to each half.
    #[must_use]
    pub fn mixed(self) -> Self {
        Self::new(mix_stafford13(self.lo), mix_stafford13(self.hi))
    }
}

/// Spreads a 64-bit legacy seed into 128 bits without mixing.
#[must_use]
pub const fn upgrade_seed_to_128bit_unmixed(legacy_seed: i64) -> Seed128 {
    let lo = legacy_seed ^ SILVER_RATIO_64;
    let hi = lo.wrapping_add(GOLDEN_RATIO_64);
    Seed128::new(lo, hi)
}

/// Spreads a 64-bit legacy seed into 128 bits, then mixes both halves.
#[must_use]
pub fn upgrade_seed_to_128bit(legacy_seed: i64) -> Seed128 {
    upgrade_seed_to_128bit_unmixed(legacy_seed).mixed()
}

/// A 128-bit seed from the MD5 of `input`'s UTF-8 bytes, read as two big-endian longs.
#[must_use]
pub fn seed_from_hash_of(input: &str) -> Seed128 {
    let digest = md5::digest(input.as_bytes());
    let mut lo = [0u8; 8];
    let mut hi = [0u8; 8];
    lo.copy_from_slice(&digest[..8]);
    hi.copy_from_slice(&digest[8..]);
    Seed128::new(i64::from_be_bytes(lo), i64::from_be_bytes(hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_equal_vanillas_signed_longs() {
        assert_eq!(GOLDEN_RATIO_64, -7_046_029_254_386_353_131);
        assert_eq!(SILVER_RATIO_64, 7_640_891_576_956_012_809);
    }

    #[test]
    fn zero_mixes_to_zero() {
        assert_eq!(mix_stafford13(0), 0);
    }

    #[test]
    fn hash_seed_reads_md5_big_endian() {
        // MD5("") = d41d8cd98f00b204 e9800998ecf8427e
        let seed = seed_from_hash_of("");
        assert_eq!(seed.lo as u64, 0xD41D_8CD9_8F00_B204);
        assert_eq!(seed.hi as u64, 0xE980_0998_ECF8_427E);
    }
}
