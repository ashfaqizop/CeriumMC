//! `java.util.UUID.fromString` of JDK 25.
//!
//! Java accepts more than the canonical 8-4-4-4-12 form: any five dash-separated groups of
//! `Long.parseLong` hexadecimal (signs, other scripts' digits and short groups included) whose
//! values are then masked to their field widths. CeriumMC must accept and reject exactly the
//! same strings, since SNBT's `uuid(...)` builtin and vanilla's commands parse UUIDs this way.

use std::fmt;

use crate::number;
use crate::{JString, JStringBuilder};

/// The `IllegalArgumentException` (or its subclass `NumberFormatException`) `fromString`
/// throws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UuidFormatError {
    number_format: bool,
    message: JString,
}

impl UuidFormatError {
    /// The simple name of the exception's class: `IllegalArgumentException`, or
    /// `NumberFormatException` when a group is not a valid hexadecimal `long`.
    #[must_use]
    pub fn java_class(&self) -> &'static str {
        if self.number_format {
            "NumberFormatException"
        } else {
            "IllegalArgumentException"
        }
    }

    /// The exception's message, exactly as Java words it.
    #[must_use]
    pub fn message(&self) -> &JString {
        &self.message
    }
}

impl fmt::Display for UuidFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.message, f)
    }
}

impl std::error::Error for UuidFormatError {}

/// `UUID.parse4Nibbles`: four hex digits from `pos`, or -1 if any is not an ASCII hex digit.
fn parse_4_nibbles(name: &JString, pos: usize) -> i64 {
    let units: [u16; 4] = std::array::from_fn(|i| name.char_at(pos + i));
    if units.iter().any(|&unit| unit > 0xFF) {
        return -1;
    }
    units
        .iter()
        .try_fold(0i64, |value, &unit| {
            char::from(unit as u8)
                .to_digit(16)
                .map(|d| (value << 4) | i64::from(d))
        })
        .unwrap_or(-1)
}

/// `UUID.fromString(name)`: the UUID's most and least significant 64 bits.
///
/// # Errors
///
/// Where Java throws, with Java's message.
pub fn from_string(name: &JString) -> Result<(i64, i64), UuidFormatError> {
    let dash = u16::from(b'-');
    if name.len() == 36 && [8, 13, 18, 23].iter().all(|&i| name.char_at(i) == dash) {
        let parts = [0, 4, 9, 14, 19, 24, 28, 32].map(|pos| parse_4_nibbles(name, pos));
        if parts.iter().all(|&part| part >= 0) {
            let msb = (parts[0] << 48) | (parts[1] << 32) | (parts[2] << 16) | parts[3];
            let lsb = (parts[4] << 48) | (parts[5] << 32) | (parts[6] << 16) | parts[7];
            return Ok((msb, lsb));
        }
    }
    from_string_slow(name)
}

/// `UUID.fromString1`.
fn from_string_slow(name: &JString) -> Result<(i64, i64), UuidFormatError> {
    let len = name.len();
    if len > 36 {
        return Err(UuidFormatError {
            number_format: false,
            message: JString::from("UUID string too large"),
        });
    }
    let dash = u16::from(b'-');
    // Java's `indexOf` with -1 for "not found"; a search from -1 + 1 starts at 0.
    let index_of = |from: i64| name.index_of(dash, from as usize).map_or(-1, |i| i as i64);
    let dash1 = index_of(0);
    let dash2 = index_of(dash1 + 1);
    let dash3 = index_of(dash2 + 1);
    let dash4 = index_of(dash3 + 1);
    let dash5 = index_of(dash4 + 1);
    if dash4 < 0 || dash5 >= 0 {
        let mut message = JStringBuilder::new();
        message.push_str("Invalid UUID string: ").push_jstring(name);
        return Err(UuidFormatError {
            number_format: false,
            message: message.build(),
        });
    }
    let field = |begin: i64, end: i64| {
        number::parse_long_range(name, begin as usize, end as usize, 16).map_err(|error| {
            UuidFormatError {
                number_format: true,
                message: error.message().clone(),
            }
        })
    };
    let mut msb = field(0, dash1)? & 0xFFFF_FFFF;
    msb <<= 16;
    msb |= field(dash1 + 1, dash2)? & 0xFFFF;
    msb <<= 16;
    msb |= field(dash2 + 1, dash3)? & 0xFFFF;
    let mut lsb = field(dash3 + 1, dash4)? & 0xFFFF;
    lsb <<= 48;
    lsb |= field(dash4 + 1, len as i64)? & 0xFFFF_FFFF_FFFF;
    Ok((msb, lsb))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<(i64, i64), String> {
        from_string(&JString::from(s)).map_err(|e| e.message().to_string_lossy())
    }

    #[test]
    fn canonical_form() {
        assert_eq!(
            parse("f81d4fae-7dec-11d0-a765-00a0c91e6bf6"),
            Ok((
                0xF81D_4FAE_7DEC_11D0_u64 as i64,
                0xA765_00A0_C91E_6BF6_u64 as i64
            ))
        );
    }

    #[test]
    fn lenient_forms_java_accepts() {
        let expected = Ok((0x0000_0001_0002_0003, 0x0004_0000_0000_0005));
        assert_eq!(parse("1-2-3-4-5"), expected);
        assert_eq!(parse("+1-2-3-4-5"), expected);
        assert_eq!(parse("-1-2-3-4-5").map_err(|_| ()), Err(()));
    }

    #[test]
    fn errors_carry_javas_messages() {
        assert_eq!(
            parse("1-2-3-4"),
            Err("Invalid UUID string: 1-2-3-4".to_owned())
        );
        assert_eq!(
            parse(&"0".repeat(37)),
            Err("UUID string too large".to_owned())
        );
        assert_eq!(
            parse("1-2-3-4-"),
            Err("For input string: \"\" under radix 16".to_owned())
        );
        assert_eq!(
            parse("1-x-3-4-5"),
            Err("Error at index 0 in: \"x\"".to_owned())
        );
    }
}
