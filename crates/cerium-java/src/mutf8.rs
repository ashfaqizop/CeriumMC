//! Modified UTF-8, the string encoding of `DataInput.readUTF` and `DataOutput.writeUTF`.
//!
//! Each UTF-16 code unit is encoded on its own: `0x01`-`0x7F` in one byte, `0x00` and
//! `0x80`-`0x7FF` in two, everything else (lone surrogates included) in three. A string is
//! written as its encoded length (an unsigned 16-bit big-endian integer) and the bytes.
//!
//! Decoding follows `DataInputStream.readUTF` of JDK 25 exactly, including what it accepts
//! (raw `0x00` bytes, overlong forms) and the messages of its `UTFDataFormatException`s.

use std::fmt;

use crate::JString;

/// The largest encoded length `writeUTF` accepts.
pub const MAX_ENCODED_LEN: usize = 65_535;

/// A `java.io.UTFDataFormatException`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UtfDataFormatError {
    message: String,
}

impl UtfDataFormatError {
    /// The exception's message, exactly as Java words it.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for UtfDataFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for UtfDataFormatError {}

/// Decodes the bytes of a `readUTF` string (after its length prefix).
///
/// # Errors
///
/// When the bytes are not valid Modified UTF-8, with Java's message.
pub fn decode(bytes: &[u8]) -> Result<JString, UtfDataFormatError> {
    if bytes.is_ascii() {
        return Ok(JString::from_latin1(bytes));
    }
    let malformed = |message: String| Err(UtfDataFormatError { message });
    let len = bytes.len();
    let mut units = Vec::with_capacity(len);
    let mut count = 0;
    while count < len {
        let c = u16::from(bytes[count]);
        match c >> 4 {
            0..=7 => {
                // 0xxxxxxx
                count += 1;
                units.push(c);
            }
            12 | 13 => {
                // 110xxxxx 10xxxxxx
                count += 2;
                if count > len {
                    return malformed("malformed input: partial character at end".to_owned());
                }
                let char2 = u16::from(bytes[count - 1]);
                if char2 & 0xC0 != 0x80 {
                    return malformed(format!("malformed input around byte {count}"));
                }
                units.push(((c & 0x1F) << 6) | (char2 & 0x3F));
            }
            14 => {
                // 1110xxxx 10xxxxxx 10xxxxxx
                count += 3;
                if count > len {
                    return malformed("malformed input: partial character at end".to_owned());
                }
                let char2 = u16::from(bytes[count - 2]);
                let char3 = u16::from(bytes[count - 1]);
                if char2 & 0xC0 != 0x80 || char3 & 0xC0 != 0x80 {
                    return malformed(format!("malformed input around byte {}", count - 1));
                }
                units.push(((c & 0x0F) << 12) | ((char2 & 0x3F) << 6) | (char3 & 0x3F));
            }
            // 10xxxxxx, 1111xxxx
            _ => return malformed(format!("malformed input around byte {count}")),
        }
    }
    Ok(JString::from_utf16(&units))
}

/// The number of bytes `writeUTF` encodes `s` into, without the length prefix.
#[must_use]
pub fn encoded_len(s: &JString) -> usize {
    s.units()
        .map(|unit| match unit {
            0x01..=0x7F => 1,
            0x00 | 0x80..=0x7FF => 2,
            _ => 3,
        })
        .sum()
}

/// Encodes `s` as `writeUTF` does, length prefix included.
///
/// # Errors
///
/// When the encoding is longer than 65535 bytes, with Java's message.
pub fn encode(s: &JString) -> Result<Vec<u8>, UtfDataFormatError> {
    let len = encoded_len(s);
    let Ok(prefix) = u16::try_from(len) else {
        return Err(too_long(s, len));
    };
    let mut out = Vec::with_capacity(len + 2);
    out.extend_from_slice(&prefix.to_be_bytes());
    for unit in s.units() {
        match unit {
            0x01..=0x7F => out.push(unit as u8),
            0x00 | 0x80..=0x7FF => {
                out.push(0xC0 | (unit >> 6) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
            _ => {
                out.push(0xE0 | (unit >> 12) as u8);
                out.push(0x80 | ((unit >> 6) & 0x3F) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
        }
    }
    Ok(out)
}

/// `DataOutputStream.tooLongMsg`. Only strings longer than 65535 / 3 code units can be too
/// long, so the head and tail substrings always exist.
fn too_long(s: &JString, len: usize) -> UtfDataFormatError {
    let n = s.len();
    let head = s.substring(0, 8);
    let tail = s.substring(n - 8, n);
    UtfDataFormatError {
        message: format!("encoded string ({head}...{tail}) too long: {len} bytes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_kind_of_unit() {
        let s = JString::from_utf16(&[0x41, 0x00, 0xE9, 0x7FF, 0x800, 0xD800, 0xFFFF]);
        let bytes = encode(&s).unwrap();
        assert_eq!(
            bytes,
            [
                0, 16, 0x41, 0xC0, 0x80, 0xC3, 0xA9, 0xDF, 0xBF, 0xE0, 0xA0, 0x80, 0xED, 0xA0,
                0x80, 0xEF, 0xBF, 0xBF
            ]
        );
        assert_eq!(decode(&bytes[2..]).unwrap(), s);
    }

    #[test]
    fn accepts_what_java_accepts() {
        // A raw NUL and an overlong encoding of 'A'.
        assert_eq!(decode(&[0x00, 0xC1, 0x81]).unwrap(), JString::from("\0A"));
    }

    #[test]
    fn rejects_like_java() {
        let message = |bytes: &[u8]| decode(bytes).unwrap_err().message().to_owned();
        assert_eq!(message(&[0x41, 0xC3]), "malformed input: partial character at end");
        assert_eq!(message(&[0x41, 0xC3, 0x41]), "malformed input around byte 3");
        assert_eq!(message(&[0xE0, 0x80, 0x41]), "malformed input around byte 2");
        assert_eq!(message(&[0x41, 0x80]), "malformed input around byte 1");
        assert_eq!(message(&[0xF0]), "malformed input around byte 0");
    }

    #[test]
    fn too_long_strings_fail_with_javas_message() {
        let s = JString::from("\u{800}".repeat(21_846).as_str());
        let error = encode(&s).unwrap_err();
        let head = "\u{800}".repeat(8);
        assert_eq!(
            error.message(),
            format!("encoded string ({head}...{head}) too long: 65538 bytes")
        );
        assert!(encode(&JString::from("\u{800}".repeat(21_845).as_str())).is_ok());
    }
}
