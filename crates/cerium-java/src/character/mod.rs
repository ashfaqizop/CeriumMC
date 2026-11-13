//! `java.lang.Character` of JDK 25 (Unicode 16.0): `isWhitespace`, `digit`,
//! `isValidCodePoint`, `getName` and `codePointOf`.
//!
//! The data in `tables.rs` and `names.bin` is generated from the JDK by
//! `parity/tables/JdkCharData.java`.

mod tables;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::JString;
use crate::number;

/// `Character.MAX_CODE_POINT`.
pub const MAX_CODE_POINT: u32 = 0x0010_FFFF;

/// `Character.isWhitespace(char)`.
#[must_use]
pub fn is_whitespace(ch: u16) -> bool {
    tables::WHITESPACE.binary_search(&ch).is_ok()
}

/// `Character.digit(char, radix)`: the value of `ch` as a digit in `radix`, if it is one.
/// Decimal digits of every script count, as do the ASCII and fullwidth Latin letters.
#[must_use]
pub fn digit(ch: u16, radix: u32) -> Option<u32> {
    if !(2..=36).contains(&radix) {
        return None;
    }
    let i = tables::DIGITS.partition_point(|&(first, _, _)| first <= ch);
    let &(first, last, value) = tables::DIGITS.get(i.checked_sub(1)?)?;
    if ch > last {
        return None;
    }
    let d = u32::from(value) + u32::from(ch - first);
    (d < radix).then_some(d)
}

/// `Character.isValidCodePoint(codePoint)`.
#[must_use]
pub fn is_valid_code_point(code_point: i32) -> bool {
    u32::try_from(code_point).is_ok_and(|cp| cp <= MAX_CODE_POINT)
}

/// Whether `Character.getType(cp)` is not `UNASSIGNED`.
fn is_assigned(cp: u32) -> bool {
    let i = tables::ASSIGNED.partition_point(|&(first, _)| first <= cp);
    i > 0 && cp <= tables::ASSIGNED[i - 1].1
}

/// `Character.UnicodeBlock.of(cp).toString().replace('_', ' ')`.
fn block_name(cp: u32) -> Option<&'static str> {
    let i = tables::BLOCKS.partition_point(|&(first, _, _)| first <= cp);
    let &(_, last, name) = tables::BLOCKS.get(i.checked_sub(1)?)?;
    (cp <= last).then_some(name)
}

/// The explicit character names (`CharacterName`), decoded on first use.
struct Names {
    by_name: HashMap<Box<str>, u32>,
    by_code_point: HashMap<u32, Box<str>>,
}

fn names() -> &'static Names {
    static NAMES: OnceLock<Names> = OnceLock::new();
    NAMES.get_or_init(|| decode_names(include_bytes!("names.bin")))
}

/// A cursor over `names.bin`.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> &'a [u8] {
        let bytes = &self.data[self.pos..self.pos + n];
        self.pos += n;
        bytes
    }

    fn u8(&mut self) -> u8 {
        self.take(1)[0]
    }

    fn u16(&mut self) -> u16 {
        u16::from_be_bytes(self.take(2).try_into().expect("2 bytes"))
    }

    fn u32(&mut self) -> u32 {
        u32::from_be_bytes(self.take(4).try_into().expect("4 bytes"))
    }
}

/// Decodes the word-coded name table described in `parity/tables/JdkCharData.java`.
fn decode_names(data: &[u8]) -> Names {
    let mut reader = Reader { data, pos: 0 };
    let word_count = reader.u32();
    let words: Vec<&str> = (0..word_count)
        .map(|_| {
            let len = usize::from(reader.u8());
            std::str::from_utf8(reader.take(len)).expect("ASCII word")
        })
        .collect();
    let name_count = reader.u32() as usize;
    let mut by_name = HashMap::with_capacity(name_count);
    let mut by_code_point = HashMap::with_capacity(name_count);
    for _ in 0..name_count {
        let cp = (u32::from(reader.u8()) << 16) | u32::from(reader.u16());
        let tokens = reader.u8();
        let mut name = String::new();
        for i in 0..tokens {
            let token = reader.u16();
            if i > 0 {
                name.push(if token & 0x8000 == 0 { ' ' } else { '-' });
            }
            name.push_str(words[usize::from(token & 0x7FFF)]);
        }
        let name: Box<str> = name.into();
        by_name.insert(name.clone(), cp);
        by_code_point.insert(cp, name);
    }
    Names {
        by_name,
        by_code_point,
    }
}

/// `Character.getName(codePoint)`: the Unicode name, or for an assigned code point without one
/// its block's name and its hex value (`"CJK UNIFIED IDEOGRAPHS 4E00"`). `None` for an
/// unassigned code point; Java throws for an invalid one, for which this also returns `None`.
#[must_use]
pub fn name(code_point: u32) -> Option<String> {
    if code_point > MAX_CODE_POINT {
        return None;
    }
    if let Some(name) = names().by_code_point.get(&code_point) {
        return Some(name.to_string());
    }
    if !is_assigned(code_point) {
        return None;
    }
    let hex = format!("{code_point:X}");
    Some(block_name(code_point).map_or_else(|| hex.clone(), |block| format!("{block} {hex}")))
}

/// `name.trim().toUpperCase(Locale.ROOT)`, if the result is ASCII.
///
/// Every name `codePointOf` can match is ASCII, so a non-ASCII result means no match. Besides
/// `a`-`z`, exactly ten characters uppercase to ASCII in Java 25 (checked against the JDK).
fn ascii_upper(name: &JString) -> Option<String> {
    let mut upper = String::with_capacity(name.len());
    for unit in name.trim().units() {
        match unit {
            0x00..=0x7F => upper.push(char::from(unit as u8).to_ascii_uppercase()),
            0x00DF => upper.push_str("SS"),
            0x0131 => upper.push('I'),
            0x017F => upper.push('S'),
            0xFB00 => upper.push_str("FF"),
            0xFB01 => upper.push_str("FI"),
            0xFB02 => upper.push_str("FL"),
            0xFB03 => upper.push_str("FFI"),
            0xFB04 => upper.push_str("FFL"),
            0xFB05 | 0xFB06 => upper.push_str("ST"),
            _ => return None,
        }
    }
    Some(upper)
}

/// `Character.codePointOf(name)`: the code point with the given name, case-insensitively and
/// ignoring surrounding whitespace; also accepts `getName`'s "block hex" fallback names.
/// `None` where Java throws `IllegalArgumentException`.
#[must_use]
pub fn code_point_of(name: &JString) -> Option<u32> {
    let upper = ascii_upper(name)?;
    if let Some(&cp) = names().by_name.get(upper.as_str()) {
        return Some(cp);
    }
    let off = upper.rfind(' ')?;
    let upper_j = JString::from(upper.as_str());
    let cp = number::parse_long_range(&upper_j, off + 1, upper.len(), 16).ok()?;
    // `Integer.parseInt` fails on anything outside the int range.
    let cp = i32::try_from(cp).ok()?;
    if !is_valid_code_point(cp) {
        return None;
    }
    let cp = cp as u32;
    (self::name(cp).as_deref() == Some(upper.as_str())).then_some(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_javas() {
        for ch in [0x09, 0x0A, 0x1F, 0x20, 0x1680, 0x2000, 0x2028, 0x3000] {
            assert!(is_whitespace(ch), "{ch:X}");
        }
        // No-break spaces are not whitespace in Java.
        for ch in [0x00A0, 0x2007, 0x202F, 0x0085, 0x41] {
            assert!(!is_whitespace(ch), "{ch:X}");
        }
    }

    #[test]
    fn digits_cover_scripts_and_letters() {
        assert_eq!(digit(u16::from(b'7'), 10), Some(7));
        assert_eq!(digit(u16::from(b'f'), 16), Some(15));
        assert_eq!(digit(u16::from(b'g'), 16), None);
        assert_eq!(digit(u16::from(b'Z'), 36), Some(35));
        assert_eq!(digit(0x0663, 10), Some(3)); // ARABIC-INDIC DIGIT THREE
        assert_eq!(digit(0xFF21, 16), Some(10)); // FULLWIDTH LATIN CAPITAL LETTER A
        assert_eq!(digit(u16::from(b'1'), 1), None);
    }

    #[test]
    fn names_resolve_both_ways() {
        assert_eq!(name(0x41).as_deref(), Some("LATIN CAPITAL LETTER A"));
        assert_eq!(name(0x4E00).as_deref(), Some("CJK UNIFIED IDEOGRAPHS 4E00"));
        assert_eq!(name(0xD800).as_deref(), Some("HIGH SURROGATES D800"));
        assert_eq!(name(0x0378), None);
        let of = |s: &str| code_point_of(&JString::from(s));
        assert_eq!(of("latin small letter a"), Some(0x61));
        assert_eq!(of("  SNOWMAN "), Some(0x2603));
        assert_eq!(of("cjk unified ideographs 4e00"), Some(0x4E00));
        assert_eq!(of("CJK UNIFIED IDEOGRAPHS 04E00"), None);
        assert_eq!(of("LATıN SMALL LETTER A"), Some(0x61));
        assert_eq!(of("no such character"), None);
    }
}
