//! [`JString`]: a `java.lang.String`.
//!
//! A Java string is a sequence of UTF-16 code units, and nothing requires surrogates to be
//! paired. Vanilla's data can carry such strings: Modified UTF-8 encodes lone surrogates, and
//! SNBT's `\uD800` escape creates them. A Rust `String` cannot hold one, so CeriumMC keeps
//! strings that come from or go to vanilla formats in this type.
//!
//! Like Java's compact strings, a string whose code units are all at most `0xFF` is stored as
//! one byte per unit (Latin-1), and any other string as UTF-16. The representation is
//! canonical, so the derived equality and hashing are exact.
//!
//! Lengths, indices and comparisons are in UTF-16 code units, as in Java.

use std::cmp::Ordering;
use std::fmt;

/// A Java string: UTF-16 code units, possibly with unpaired surrogates.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct JString(Repr);

#[derive(Clone, PartialEq, Eq, Hash)]
enum Repr {
    /// Every code unit is at most `0xFF`.
    Latin1(Box<[u8]>),
    /// At least one code unit is above `0xFF`.
    Utf16(Box<[u16]>),
}

impl Default for Repr {
    fn default() -> Self {
        Self::Latin1(Box::default())
    }
}

impl JString {
    /// The empty string.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A string of the given UTF-16 code units.
    #[must_use]
    pub fn from_utf16(units: &[u16]) -> Self {
        if units.iter().all(|&unit| unit <= 0xFF) {
            Self(Repr::Latin1(units.iter().map(|&unit| unit as u8).collect()))
        } else {
            Self(Repr::Utf16(units.into()))
        }
    }

    /// A string whose code units are the given bytes (ISO-8859-1).
    #[must_use]
    pub fn from_latin1(bytes: &[u8]) -> Self {
        Self(Repr::Latin1(bytes.into()))
    }

    /// The number of UTF-16 code units (`length()`).
    #[must_use]
    pub fn len(&self) -> usize {
        match &self.0 {
            Repr::Latin1(bytes) => bytes.len(),
            Repr::Utf16(units) => units.len(),
        }
    }

    /// Whether the string has no code units (`isEmpty()`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The code unit at `index` (`charAt`).
    ///
    /// # Panics
    ///
    /// If `index` is out of bounds (Java's `StringIndexOutOfBoundsException`).
    #[must_use]
    pub fn char_at(&self, index: usize) -> u16 {
        match &self.0 {
            Repr::Latin1(bytes) => u16::from(bytes[index]),
            Repr::Utf16(units) => units[index],
        }
    }

    /// The code units in order.
    pub fn units(&self) -> impl ExactSizeIterator<Item = u16> + Clone + '_ {
        let (latin1, utf16): (&[u8], &[u16]) = match &self.0 {
            Repr::Latin1(bytes) => (bytes, &[]),
            Repr::Utf16(units) => (&[], units),
        };
        Units {
            latin1: latin1.iter(),
            utf16: utf16.iter(),
        }
    }

    /// The code units as a vector.
    #[must_use]
    pub fn to_utf16(&self) -> Vec<u16> {
        self.units().collect()
    }

    /// The code units `begin..end` (`substring(begin, end)`).
    ///
    /// # Panics
    ///
    /// If the range is out of bounds or reversed.
    #[must_use]
    pub fn substring(&self, begin: usize, end: usize) -> Self {
        match &self.0 {
            Repr::Latin1(bytes) => Self::from_latin1(&bytes[begin..end]),
            Repr::Utf16(units) => Self::from_utf16(&units[begin..end]),
        }
    }

    /// The bytes of a Latin-1 string, or `None` if a code unit is above `0xFF`.
    #[must_use]
    pub fn as_latin1(&self) -> Option<&[u8]> {
        match &self.0 {
            Repr::Latin1(bytes) => Some(bytes),
            Repr::Utf16(_) => None,
        }
    }

    /// The string as `&str` if every code unit is ASCII.
    #[must_use]
    pub fn as_ascii(&self) -> Option<&str> {
        self.as_latin1()
            .filter(|bytes| bytes.is_ascii())
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
    }

    /// `hashCode()`: `s[0]*31^(n-1) + ... + s[n-1]` with 32-bit wrapping arithmetic.
    #[must_use]
    pub fn hash_code(&self) -> i32 {
        self.units()
            .fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(i32::from(unit)))
    }

    /// `compareTo`: lexicographic order of the code units, then by length.
    #[must_use]
    pub fn compare_to(&self, other: &Self) -> Ordering {
        if let (Repr::Latin1(a), Repr::Latin1(b)) = (&self.0, &other.0) {
            return a.cmp(b);
        }
        self.units().cmp(other.units())
    }

    /// The string as UTF-8, or `None` if it contains an unpaired surrogate.
    #[must_use]
    pub fn to_std_string(&self) -> Option<String> {
        match &self.0 {
            Repr::Latin1(bytes) => Some(bytes.iter().map(|&byte| char::from(byte)).collect()),
            Repr::Utf16(units) => String::from_utf16(units).ok(),
        }
    }

    /// The string as UTF-8 with each unpaired surrogate replaced by U+FFFD.
    #[must_use]
    pub fn to_string_lossy(&self) -> String {
        match &self.0 {
            Repr::Latin1(bytes) => bytes.iter().map(|&byte| char::from(byte)).collect(),
            Repr::Utf16(units) => String::from_utf16_lossy(units),
        }
    }

    /// Whether the code units equal `other`'s UTF-16 encoding.
    #[must_use]
    pub fn equals_str(&self, other: &str) -> bool {
        match &self.0 {
            // A Latin-1 string equals `other` exactly when `other` is the same code points.
            Repr::Latin1(bytes) => {
                let mut chars = other.chars();
                bytes
                    .iter()
                    .all(|&byte| chars.next() == Some(char::from(byte)))
                    && chars.next().is_none()
            }
            Repr::Utf16(units) => units.iter().copied().eq(other.encode_utf16()),
        }
    }

    /// `equalsIgnoreCase` for ASCII: equal after mapping `a`-`z` to `A`-`Z`.
    ///
    /// Java also folds some non-ASCII letters onto ASCII ones (`"ſ"` equals `"s"`). Every
    /// caller in CeriumMC compares against an ASCII word where the input either is ASCII or is
    /// rejected regardless, which this documents at each call site.
    #[must_use]
    pub fn equals_ignore_ascii_case(&self, other: &str) -> bool {
        self.len() == other.len()
            && self
                .units()
                .zip(other.bytes())
                .all(|(unit, byte)| u8::try_from(unit).is_ok_and(|b| b.eq_ignore_ascii_case(&byte)))
    }

    /// `trim()`: the string without leading and trailing code units at most `' '`.
    #[must_use]
    pub fn trim(&self) -> Self {
        let units = self.to_utf16();
        let start = units.iter().position(|&unit| unit > 0x20).unwrap_or(units.len());
        let end = units.iter().rposition(|&unit| unit > 0x20).map_or(start, |last| last + 1);
        Self::from_utf16(&units[start..end])
    }

    /// The index of the last occurrence of `unit` (`lastIndexOf(char)`).
    #[must_use]
    pub fn last_index_of(&self, unit: u16) -> Option<usize> {
        self.to_utf16().iter().rposition(|&u| u == unit)
    }

    /// The index of the first occurrence of `unit` at or after `from` (`indexOf(char, from)`).
    #[must_use]
    pub fn index_of(&self, unit: u16, from: usize) -> Option<usize> {
        self.units().skip(from).position(|u| u == unit).map(|i| i + from)
    }
}

impl PartialOrd for JString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Java's `compareTo` order.
impl Ord for JString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare_to(other)
    }
}

impl From<&str> for JString {
    fn from(value: &str) -> Self {
        if value.chars().all(|c| u32::from(c) <= 0xFF) {
            Self(Repr::Latin1(value.chars().map(|c| u32::from(c) as u8).collect()))
        } else {
            Self(Repr::Utf16(value.encode_utf16().collect()))
        }
    }
}

impl From<String> for JString {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

impl PartialEq<str> for JString {
    fn eq(&self, other: &str) -> bool {
        self.equals_str(other)
    }
}

impl PartialEq<&str> for JString {
    fn eq(&self, other: &&str) -> bool {
        self.equals_str(other)
    }
}

/// Prints the string as UTF-8, with unpaired surrogates replaced by U+FFFD.
impl fmt::Display for JString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_lossy())
    }
}

impl fmt::Debug for JString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_std_string() {
            Some(s) => fmt::Debug::fmt(&s, f),
            None => write!(f, "JString({:04X?})", self.to_utf16()),
        }
    }
}

/// A `StringBuilder`: appends code units and builds a [`JString`].
#[derive(Clone, Debug, Default)]
pub struct JStringBuilder {
    units: Vec<u16>,
}

impl JStringBuilder {
    /// An empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends `s` (UTF-16 encoded).
    pub fn push_str(&mut self, s: &str) -> &mut Self {
        self.units.extend(s.encode_utf16());
        self
    }

    /// Appends the code units of `s`.
    pub fn push_jstring(&mut self, s: &JString) -> &mut Self {
        self.units.extend(s.units());
        self
    }

    /// Appends one code unit (`append(char)`).
    pub fn push_unit(&mut self, unit: u16) -> &mut Self {
        self.units.push(unit);
        self
    }

    /// Appends code units.
    pub fn push_units(&mut self, units: &[u16]) -> &mut Self {
        self.units.extend_from_slice(units);
        self
    }

    /// The number of code units so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.units.len()
    }

    /// Whether nothing was appended.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// The code units so far.
    #[must_use]
    pub fn units(&self) -> &[u16] {
        &self.units
    }

    /// Replaces the code unit at `index` (`setCharAt`).
    ///
    /// # Panics
    ///
    /// If `index` is out of bounds.
    pub fn set_unit(&mut self, index: usize, unit: u16) {
        self.units[index] = unit;
    }

    /// The string built so far (`toString()`).
    #[must_use]
    pub fn build(&self) -> JString {
        JString::from_utf16(&self.units)
    }
}

/// The code units of a [`JString`]; exactly one of the two iterators is non-empty.
#[derive(Clone)]
struct Units<'a> {
    latin1: std::slice::Iter<'a, u8>,
    utf16: std::slice::Iter<'a, u16>,
}

impl Iterator for Units<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        match self.latin1.next() {
            Some(&byte) => Some(u16::from(byte)),
            None => self.utf16.next().copied(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.latin1.len() + self.utf16.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for Units<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_representation_is_canonical() {
        assert_eq!(JString::from("abc"), JString::from_utf16(&[0x61, 0x62, 0x63]));
        assert_eq!(JString::from("\u{FF}"), JString::from_latin1(&[0xFF]));
        assert!(JString::from("\u{100}").as_latin1().is_none());
        assert_eq!(JString::from("\u{1F600}").len(), 2);
    }

    #[test]
    fn hash_code_matches_java() {
        assert_eq!(JString::from("").hash_code(), 0);
        assert_eq!(JString::from("hello").hash_code(), 99_162_322);
        assert_eq!(JString::from("\u{1F600}").hash_code(), 0xD83D * 31 + 0xDE00);
    }

    #[test]
    fn compare_to_uses_code_units() {
        // U+FF61 sorts after the surrogate pair of U+1F600 (0xD83D...) in UTF-16 order,
        // although it comes first by code point.
        let halfwidth = JString::from("\u{FF61}");
        let emoji = JString::from("\u{1F600}");
        assert_eq!(halfwidth.compare_to(&emoji), Ordering::Greater);
        assert_eq!(JString::from("ab").compare_to(&JString::from("abc")), Ordering::Less);
    }

    #[test]
    fn lone_surrogates_survive() {
        let lone = JString::from_utf16(&[0x61, 0xD800]);
        assert_eq!(lone.len(), 2);
        assert_eq!(lone.to_std_string(), None);
        assert_eq!(lone.to_string_lossy(), "a\u{FFFD}");
    }

    #[test]
    fn helpers_follow_java() {
        assert_eq!(JString::from(" \u{1}x y\t").trim(), "x y");
        assert!(JString::from("TrUe").equals_ignore_ascii_case("true"));
        assert!(!JString::from("tru").equals_ignore_ascii_case("true"));
        assert_eq!(JString::from("a b c").last_index_of(u16::from(b' ')), Some(3));
        assert_eq!(JString::from("a{b}").index_of(u16::from(b'}'), 1), Some(3));
        assert!(JString::from("é") == "é");
    }
}
