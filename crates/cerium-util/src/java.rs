//! Hashing that matches the Java standard library bit for bit.

/// Java's `String.hashCode()`: `s[0]*31^(n-1) + ... + s[n-1]` over the string's UTF-16
/// code units, with 32-bit wrapping arithmetic.
#[must_use]
pub fn string_hash_code(value: &str) -> i32 {
    let mut hash = 0i32;
    for unit in value.encode_utf16() {
        hash = hash.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_for_known_strings() {
        // Reference values computed independently with the same UTF-16 definition.
        assert_eq!(string_hash_code(""), 0);
        assert_eq!(string_hash_code("a"), 97);
        assert_eq!(string_hash_code("hello"), 99_162_322);
        assert_eq!(string_hash_code("minecraft:overworld"), 1_104_210_353);
    }

    #[test]
    fn uses_utf16_code_units() {
        // U+1F600 is the surrogate pair D83D DE00: 0xD83D * 31 + 0xDE00.
        assert_eq!(string_hash_code("\u{1F600}"), 0xD83D * 31 + 0xDE00);
        assert_eq!(string_hash_code("\u{E9}"), 0xE9);
    }
}
