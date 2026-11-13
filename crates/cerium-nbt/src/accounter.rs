//! [`NbtAccounter`]: the size quota and depth limit applied while reading NBT.

use crate::NbtError;
use crate::tag::MAX_DEPTH;

/// `NbtAccounter.DEFAULT_NBT_QUOTA`: 2 MiB, used for NBT in packets.
pub const DEFAULT_NBT_QUOTA: i64 = 2_097_152;
/// `NbtAccounter.UNCOMPRESSED_NBT_QUOTA`: 100 MiB.
pub const UNCOMPRESSED_NBT_QUOTA: i64 = 104_857_600;

/// `NbtAccounter`: charges each tag read against a quota of estimated memory use, and counts
/// list and compound nesting.
///
/// Like Java, the quota check `usage + size > quota` is done in wrapping 64-bit arithmetic, so
/// with the unlimited quota an overflowing sum is accepted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NbtAccounter {
    quota: i64,
    usage: i64,
    max_depth: i32,
    depth: i32,
}

impl NbtAccounter {
    /// `new NbtAccounter(quota, maxDepth)`.
    #[must_use]
    pub fn new(quota: i64, max_depth: i32) -> Self {
        Self {
            quota,
            usage: 0,
            max_depth,
            depth: 0,
        }
    }

    /// `NbtAccounter.create(quota)`: the given quota and the default depth limit of 512.
    #[must_use]
    pub fn create(quota: i64) -> Self {
        Self::new(quota, MAX_DEPTH)
    }

    /// `NbtAccounter.defaultQuota()`: 2 MiB.
    #[must_use]
    pub fn default_quota() -> Self {
        Self::create(DEFAULT_NBT_QUOTA)
    }

    /// `NbtAccounter.uncompressedQuota()`: 100 MiB.
    #[must_use]
    pub fn uncompressed_quota() -> Self {
        Self::create(UNCOMPRESSED_NBT_QUOTA)
    }

    /// `NbtAccounter.unlimitedHeap()`: `Long.MAX_VALUE`.
    #[must_use]
    pub fn unlimited_heap() -> Self {
        Self::create(i64::MAX)
    }

    /// `accountBytes(bytesPerEntry, count)`.
    ///
    /// # Errors
    ///
    /// As [`account_bytes`](Self::account_bytes).
    pub fn account_bytes_n(&mut self, bytes_per_entry: i64, count: i64) -> Result<(), NbtError> {
        self.account_bytes(bytes_per_entry.wrapping_mul(count))
    }

    /// `accountBytes(size)`.
    ///
    /// # Errors
    ///
    /// `IllegalArgumentException` for a negative size, `NbtAccounterException` when the quota
    /// would be exceeded.
    pub fn account_bytes(&mut self, size: i64) -> Result<(), NbtError> {
        if size < 0 {
            return Err(NbtError::IllegalArgument(format!(
                "Tried to account NBT tag with negative size: {size}"
            )));
        }
        if self.usage.wrapping_add(size) > self.quota {
            return Err(NbtError::Accounter(format!(
                "Tried to read NBT tag that was too big; tried to allocate: {} + {size} bytes \
                 where max allowed: {}",
                self.usage, self.quota
            )));
        }
        self.usage = self.usage.wrapping_add(size);
        Ok(())
    }

    /// `pushDepth()`.
    ///
    /// # Errors
    ///
    /// `NbtAccounterException` at the depth limit.
    pub fn push_depth(&mut self) -> Result<(), NbtError> {
        if self.depth >= self.max_depth {
            return Err(NbtError::Accounter(format!(
                "Tried to read NBT tag with too high complexity, depth > {}",
                self.max_depth
            )));
        }
        self.depth += 1;
        Ok(())
    }

    /// `popDepth()`.
    ///
    /// # Errors
    ///
    /// `NbtAccounterException` when already at the top level.
    pub fn pop_depth(&mut self) -> Result<(), NbtError> {
        if self.depth <= 0 {
            return Err(NbtError::Accounter(
                "NBT-Accounter tried to pop stack-depth at top-level".to_owned(),
            ));
        }
        self.depth -= 1;
        Ok(())
    }

    /// The bytes accounted so far (`getUsage`).
    #[must_use]
    pub fn usage(&self) -> i64 {
        self.usage
    }

    /// The current nesting depth (`getDepth`).
    #[must_use]
    pub fn depth(&self) -> i32 {
        self.depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_and_depth_follow_vanilla() {
        let mut accounter = NbtAccounter::create(10);
        assert_eq!(accounter.account_bytes(8), Ok(()));
        assert_eq!(
            accounter.account_bytes(3).unwrap_err().message().unwrap(),
            "Tried to read NBT tag that was too big; tried to allocate: 8 + 3 bytes where max \
             allowed: 10"
        );
        assert_eq!(
            accounter.account_bytes_n(4, -1).unwrap_err().java_class(),
            "IllegalArgumentException"
        );
        let mut deep = NbtAccounter::new(i64::MAX, 1);
        assert_eq!(deep.push_depth(), Ok(()));
        assert!(deep.push_depth().is_err());
        assert_eq!(deep.pop_depth(), Ok(()));
        assert!(deep.pop_depth().is_err());
    }

    #[test]
    fn the_unlimited_quota_accepts_overflow_like_java() {
        let mut accounter = NbtAccounter::unlimited_heap();
        assert_eq!(accounter.account_bytes(i64::MAX), Ok(()));
        // usage + size wraps negative, which is not greater than the quota.
        assert_eq!(accounter.account_bytes(10), Ok(()));
    }
}
