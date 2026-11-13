//! Java's number parsers (JDK 25): `Integer`/`Long.parseInt`/`parseLong` and their unsigned
//! and `CharSequence`-range forms, `Byte`/`Short.parseX`, Guava's
//! `UnsignedBytes.parseUnsignedByte`, and `Double.parseDouble`/`Float.parseFloat`.
//!
//! What each parser accepts and the message of each `NumberFormatException` follow the JDK
//! exactly; vanilla shows some of these messages to players (SNBT number errors embed them).
//! Digits are `Character.digit` digits, so non-ASCII decimal digits count, as in Java.

use std::fmt;

use crate::character;
use crate::{JString, JStringBuilder};

/// A `java.lang.NumberFormatException`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumberFormatError {
    message: JString,
}

impl NumberFormatError {
    fn new(message: JString) -> Self {
        Self { message }
    }

    fn text(message: &str) -> Self {
        Self::new(JString::from(message))
    }

    /// The exception's message, exactly as Java words it.
    #[must_use]
    pub fn message(&self) -> &JString {
        &self.message
    }
}

impl fmt::Display for NumberFormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.message, f)
    }
}

impl std::error::Error for NumberFormatError {}

/// `NumberFormatException.forInputString`.
fn for_input_string(s: &JString, radix: u32) -> NumberFormatError {
    let mut message = JStringBuilder::new();
    message
        .push_str("For input string: \"")
        .push_jstring(s)
        .push_str("\"");
    if radix != 10 {
        message.push_str(&format!(" under radix {radix}"));
    }
    NumberFormatError::new(message.build())
}

/// `NumberFormatException.forCharSequence`.
fn for_char_sequence(s: &JString, begin: usize, end: usize, error: usize) -> NumberFormatError {
    let mut message = JStringBuilder::new();
    message
        .push_str(&format!("Error at index {} in: \"", error - begin))
        .push_jstring(&s.substring(begin, end))
        .push_str("\"");
    NumberFormatError::new(message.build())
}

fn check_radix(radix: u32) -> Result<(), NumberFormatError> {
    if radix < 2 {
        return Err(NumberFormatError::text(&format!(
            "radix {radix} less than Character.MIN_RADIX"
        )));
    }
    if radix > 36 {
        return Err(NumberFormatError::text(&format!(
            "radix {radix} greater than Character.MAX_RADIX"
        )));
    }
    Ok(())
}

/// `Character.digit(ch, radix)` as an `i32` (`-1` when `ch` is not a digit).
fn digit(ch: u16, radix: u32) -> i32 {
    character::digit(ch, radix).map_or(-1, |d| d as i32)
}

const SIGN_SENTINEL: i32 = !0xFF;
const MINUS: u16 = b'-' as u16;
const PLUS: u16 = b'+' as u16;

/// `Integer.parseInt(s, radix)`.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
pub fn parse_int(s: &JString, radix: u32) -> Result<i32, NumberFormatError> {
    check_radix(radix)?;
    let len = s.len();
    if len == 0 {
        return Err(for_input_string(&JString::new(), radix));
    }
    let r = radix as i32;
    let mut digit = SIGN_SENTINEL;
    let mut i = 0;
    let first = s.char_at(i);
    i += 1;
    if first != MINUS && first != PLUS {
        digit = self::digit(first, radix);
    }
    if digit >= 0 || digit == SIGN_SENTINEL && len > 1 {
        let limit = if first == MINUS {
            i32::MIN
        } else {
            i32::MIN + 1
        };
        let multmin = limit / r;
        let mut result = -(digit & 0xFF);
        let mut in_range = true;
        // Accumulating negatively avoids surprises near MAX_VALUE.
        while i < len {
            digit = self::digit(s.char_at(i), radix);
            i += 1;
            if digit < 0 {
                break;
            }
            in_range = result > multmin || result == multmin && digit <= r * multmin - limit;
            if !in_range {
                break;
            }
            result = r * result - digit;
        }
        if in_range && i == len && digit >= 0 {
            return Ok(if first == MINUS { result } else { -result });
        }
    }
    Err(for_input_string(s, radix))
}

/// `Long.parseLong(s, radix)`.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
pub fn parse_long(s: &JString, radix: u32) -> Result<i64, NumberFormatError> {
    check_radix(radix)?;
    let len = s.len();
    if len == 0 {
        return Err(for_input_string(&JString::new(), radix));
    }
    parse_long_core(s, 0, len, radix).map_err(|_| for_input_string(s, radix))
}

/// `Long.parseLong(s, beginIndex, endIndex, radix)`: parses the code units
/// `begin..end`, with the `CharSequence` form's error messages.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
///
/// # Panics
///
/// If the range is out of bounds (Java's `IndexOutOfBoundsException`).
pub fn parse_long_range(
    s: &JString,
    begin: usize,
    end: usize,
    radix: u32,
) -> Result<i64, NumberFormatError> {
    assert!(
        begin <= end && end <= s.len(),
        "range {begin}..{end} out of bounds"
    );
    check_radix(radix)?;
    if begin == end {
        return Err(for_input_string(&JString::new(), radix));
    }
    parse_long_core(s, begin, end, radix)
        .map_err(|error_index| for_char_sequence(s, begin, end, error_index))
}

/// The digit loop of `Long.parseLong` over `begin..end` (non-empty). Returns the
/// `CharSequence` form's error index on failure.
fn parse_long_core(s: &JString, begin: usize, end: usize, radix: u32) -> Result<i64, usize> {
    let r = i64::from(radix);
    let mut digit = SIGN_SENTINEL;
    let mut i = begin;
    let first = s.char_at(i);
    i += 1;
    if first != MINUS && first != PLUS {
        digit = self::digit(first, radix);
    }
    if digit >= 0 || digit == SIGN_SENTINEL && end - begin > 1 {
        let limit = if first == MINUS {
            i64::MIN
        } else {
            i64::MIN + 1
        };
        let multmin = limit / r;
        let mut result = -i64::from(digit & 0xFF);
        let mut in_range = true;
        while i < end {
            digit = self::digit(s.char_at(i), radix);
            i += 1;
            if digit < 0 {
                break;
            }
            in_range =
                result > multmin || result == multmin && digit <= (r * multmin - limit) as i32;
            if !in_range {
                break;
            }
            result = r * result - i64::from(digit);
        }
        if in_range && i == end && digit >= 0 {
            return Ok(if first == MINUS { result } else { -result });
        }
    }
    Err(i - usize::from(digit >= -1))
}

/// `Integer.parseUnsignedInt(s, radix)`.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
pub fn parse_unsigned_int(s: &JString, radix: u32) -> Result<i32, NumberFormatError> {
    check_radix(radix)?;
    let len = s.len();
    if len == 0 {
        return Err(for_input_string(s, radix));
    }
    let first = s.char_at(0);
    if first == MINUS {
        return Err(illegal_leading_minus(s));
    }
    let mut digit = SIGN_SENTINEL;
    let mut i = 1;
    if first != PLUS {
        digit = self::digit(first, radix);
    }
    if digit >= 0 || digit == SIGN_SENTINEL && len > 1 {
        let multmax = (u32::MAX / radix) as i32;
        let mut result = digit & 0xFF;
        let mut in_range = true;
        while i < len {
            digit = self::digit(s.char_at(i), radix);
            i += 1;
            if digit < 0 {
                break;
            }
            let bound = (radix as i32).wrapping_neg().wrapping_mul(multmax);
            in_range = (result as u32) < multmax as u32 || result == multmax && digit < bound;
            if !in_range {
                break;
            }
            result = (radix as i32).wrapping_mul(result).wrapping_add(digit);
        }
        if in_range && i == len && digit >= 0 {
            return Ok(result);
        }
    }
    if digit < 0 {
        return Err(for_input_string(s, radix));
    }
    Err(exceeds_unsigned(s, "int"))
}

/// `Long.parseUnsignedLong(s, radix)`.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
pub fn parse_unsigned_long(s: &JString, radix: u32) -> Result<i64, NumberFormatError> {
    check_radix(radix)?;
    let len = s.len();
    if len == 0 {
        return Err(for_input_string(s, radix));
    }
    let first = s.char_at(0);
    if first == MINUS {
        return Err(illegal_leading_minus(s));
    }
    let mut digit = SIGN_SENTINEL;
    let mut i = 1;
    if first != PLUS {
        digit = self::digit(first, radix);
    }
    if digit >= 0 || digit == SIGN_SENTINEL && len > 1 {
        let multmax = (u64::MAX / u64::from(radix)) as i64;
        let mut result = i64::from(digit & 0xFF);
        let mut in_range = true;
        while i < len {
            digit = self::digit(s.char_at(i), radix);
            i += 1;
            if digit < 0 {
                break;
            }
            let bound = i64::from(radix).wrapping_neg().wrapping_mul(multmax) as i32;
            in_range = (result as u64) < multmax as u64 || result == multmax && digit < bound;
            if !in_range {
                break;
            }
            result = i64::from(radix)
                .wrapping_mul(result)
                .wrapping_add(i64::from(digit));
        }
        if in_range && i == len && digit >= 0 {
            return Ok(result);
        }
    }
    if digit < 0 {
        return Err(for_input_string(s, radix));
    }
    Err(exceeds_unsigned(s, "long"))
}

fn illegal_leading_minus(s: &JString) -> NumberFormatError {
    let mut message = JStringBuilder::new();
    message
        .push_str("Illegal leading minus sign on unsigned string ")
        .push_jstring(s)
        .push_str(".");
    NumberFormatError::new(message.build())
}

fn exceeds_unsigned(s: &JString, kind: &str) -> NumberFormatError {
    let mut message = JStringBuilder::new();
    message
        .push_str("String value ")
        .push_jstring(s)
        .push_str(&format!(" exceeds range of unsigned {kind}."));
    NumberFormatError::new(message.build())
}

/// `Byte.parseByte` and `Short.parseShort`: `parseInt`, then a range check.
fn parse_narrow(s: &JString, radix: u32, min: i32, max: i32) -> Result<i32, NumberFormatError> {
    let i = parse_int(s, radix)?;
    if i < min || i > max {
        let mut message = JStringBuilder::new();
        message
            .push_str("Value out of range. Value:\"")
            .push_jstring(s)
            .push_str(&format!("\" Radix:{radix}"));
        return Err(NumberFormatError::new(message.build()));
    }
    Ok(i)
}

/// `Byte.parseByte(s, radix)`.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
pub fn parse_byte(s: &JString, radix: u32) -> Result<i8, NumberFormatError> {
    parse_narrow(s, radix, i8::MIN.into(), i8::MAX.into()).map(|i| i as i8)
}

/// `Short.parseShort(s, radix)`.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
pub fn parse_short(s: &JString, radix: u32) -> Result<i16, NumberFormatError> {
    parse_narrow(s, radix, i16::MIN.into(), i16::MAX.into()).map(|i| i as i16)
}

/// Guava's `UnsignedBytes.parseUnsignedByte(s, radix)`: `parseInt`, then a check that the
/// value fits in 8 unsigned bits.
///
/// # Errors
///
/// As Guava's `NumberFormatException`.
pub fn parse_unsigned_byte(s: &JString, radix: u32) -> Result<i8, NumberFormatError> {
    let parse = parse_int(s, radix)?;
    if parse >> 8 == 0 {
        Ok(parse as i8)
    } else {
        Err(NumberFormatError::text(&format!("out of range: {parse}")))
    }
}

/// The binary formats `FloatingDecimal` converts to.
struct Format {
    /// Precision in bits.
    p: i32,
    q_min: i32,
    e_min: i32,
    e_max: i32,
    ep_min: i64,
    ep_max: i64,
    hex_count: usize,
}

const BINARY_32: Format = Format {
    p: 24,
    q_min: -149,
    e_min: -126,
    e_max: 127,
    ep_min: -46,
    ep_max: 40,
    hex_count: 8,
};

const BINARY_64: Format = Format {
    p: 53,
    q_min: -1074,
    e_min: -1022,
    e_max: 1023,
    ep_min: -324,
    ep_max: 310,
    hex_count: 15,
};

/// What `FloatingDecimal.readJavaFormatString` found.
enum Scanned {
    Zero {
        negative: bool,
    },
    Infinity {
        negative: bool,
    },
    NaN,
    /// A finite binary value `c * 2^q` that was rounded directly (hexadecimal input).
    Binary {
        negative: bool,
        q: i32,
        c: i64,
    },
    /// The decimal `0.<digits> * 10^e`, still to be rounded.
    Decimal {
        negative: bool,
        digits: String,
        e: i64,
    },
}

/// `Double.parseDouble(s)`.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
pub fn parse_double(s: &JString) -> Result<f64, NumberFormatError> {
    Ok(match scan(s, &BINARY_64)? {
        Scanned::Zero { negative } => sign(negative, 0.0),
        Scanned::Infinity { negative } => sign(negative, f64::INFINITY),
        Scanned::NaN => f64::NAN,
        Scanned::Binary { negative, q, c } => {
            let be = if c < 1 << 52 {
                0
            } else {
                i64::from(q) + (1022 + 53)
            };
            let bits =
                (u64::from(negative) << 63) | ((be as u64) << 52) | (c as u64 & ((1 << 52) - 1));
            f64::from_bits(bits)
        }
        Scanned::Decimal {
            negative,
            digits,
            e,
        } => {
            let value: f64 = format!("0.{digits}e{e}").parse().expect("valid decimal");
            sign(negative, value)
        }
    })
}

/// `Float.parseFloat(s)`.
///
/// # Errors
///
/// As Java's `NumberFormatException`.
pub fn parse_float(s: &JString) -> Result<f32, NumberFormatError> {
    Ok(match scan(s, &BINARY_32)? {
        Scanned::Zero { negative } => sign(negative, 0.0f64) as f32,
        Scanned::Infinity { negative } => sign(negative, f64::INFINITY) as f32,
        Scanned::NaN => f32::NAN,
        Scanned::Binary { negative, q, c } => {
            let be = if c < 1 << 23 { 0 } else { q + (126 + 24) };
            let bits =
                (u32::from(negative) << 31) | ((be as u32) << 23) | (c as u32 & ((1 << 23) - 1));
            f32::from_bits(bits)
        }
        Scanned::Decimal {
            negative,
            digits,
            e,
        } => {
            let value: f32 = format!("0.{digits}e{e}").parse().expect("valid decimal");
            if negative { -value } else { value }
        }
    })
}

fn sign(negative: bool, value: f64) -> f64 {
    if negative { -value } else { value }
}

/// `FloatingDecimal.check`: the generic syntax error, with an over-long input shortened to
/// its first 497 and last 498 code units around `" ... "`.
fn syntax_error(s: &JString) -> NumberFormatError {
    const MAX_OUT: usize = 1_000;
    const OMITTED: &str = " ... ";
    const L_HALF: usize = (MAX_OUT - OMITTED.len()) / 2;
    const R_HALF: usize = MAX_OUT - (L_HALF + OMITTED.len());
    let mut message = JStringBuilder::new();
    message.push_str("For input string: \"");
    let len = s.len();
    if len > MAX_OUT {
        message
            .push_jstring(&s.substring(0, L_HALF))
            .push_str(OMITTED)
            .push_jstring(&s.substring(len - R_HALF, len));
    } else {
        message.push_jstring(s);
    }
    message.push_str("\"");
    NumberFormatError::new(message.build())
}

/// `FloatingDecimal.readJavaFormatString`: the syntax check and the parts of the value.
#[allow(
    clippy::too_many_lines,
    reason = "a line-by-line transcription of the JDK method"
)]
fn scan(s: &JString, fmt: &Format) -> Result<Scanned, NumberFormatError> {
    let units = s.to_utf16();
    let len = units.len();
    let at = |i: usize| i32::from(units[i]);
    let check = |ok: bool| if ok { Ok(()) } else { Err(syntax_error(s)) };
    let skip_whitespaces = |mut i: usize| {
        while i < len && units[i] <= 0x20 {
            i += 1;
        }
        i
    };
    let is_digit = |ch: i32, is_dec: bool| {
        (0x30..=0x39).contains(&ch) || !is_dec && (0x61..=0x66).contains(&(ch | 0x20))
    };

    // Skip leading whitespaces.
    let mut i = skip_whitespaces(0);
    if i == len {
        return Err(NumberFormatError::text("empty String"));
    }

    // Scan the optional significand sign.
    let mut negative = false;
    let mut ch = at(i);
    if ch == i32::from(MINUS) || ch == i32::from(PLUS) {
        negative = ch == i32::from(MINUS);
        i += 1;
    }

    // Symbolic values and hex notation.
    let mut is_dec = true;
    if i < len {
        ch = at(i);
        let symbolic = |word: &str| {
            let word: Vec<u16> = word.encode_utf16().collect();
            let high = i + word.len();
            check(high <= len && units[i..high] == word[..] && skip_whitespaces(high) == len)
        };
        if ch == i32::from(b'I') {
            symbolic("Infinity")?;
            return Ok(Scanned::Infinity { negative });
        }
        if ch == i32::from(b'N') {
            symbolic("NaN")?;
            return Ok(Scanned::NaN);
        }
        if ch == i32::from(b'0') && i + 1 < len && at(i + 1) | 0x20 == i32::from(b'x') {
            is_dec = false;
            i += 2;
        }
    }

    let mut pt = 0; // index after the point, 0 iff absent
    let start = i;
    let point = |pt: &mut usize, i: usize| {
        if *pt != 0 {
            return Err(NumberFormatError::text("multiple points"));
        }
        *pt = i;
        Ok(())
    };

    // Skip optional leading zeros, including an optional point.
    while i < len && (at(i) == i32::from(b'0') || at(i) == i32::from(b'.')) {
        ch = at(i);
        i += 1;
        if ch == i32::from(b'.') {
            point(&mut pt, i)?;
        }
    }
    let lz = i;

    // The remaining significand, and the index after its last non-zero digit.
    let mut tnz = 0;
    while i < len && (is_digit(at(i), is_dec) || at(i) == i32::from(b'.')) {
        ch = at(i);
        i += 1;
        if ch == i32::from(b'.') {
            point(&mut pt, i)?;
        } else if ch != i32::from(b'0') {
            tnz = i;
        }
    }
    check(i - start > usize::from(pt != 0))?;
    let stop = i;

    // The exponent: optional for decimal, mandatory for hex.
    let mut ep: i64 = 0;
    let mut has_exp = false;
    if i < len {
        ch = at(i) | 0x20;
        if ch == i32::from(b'e') && is_dec || ch == i32::from(b'p') && !is_dec {
            i += 1;
            let mut esign_minus = false;
            let mut esign_explicit = false;
            if i < len && (at(i) == i32::from(MINUS) || at(i) == i32::from(PLUS)) {
                esign_minus = at(i) == i32::from(MINUS);
                esign_explicit = true;
                i += 1;
            }
            while i < len && is_digit(at(i), true) {
                // Accumulate, clamping at 10^10.
                ep = if ep < 10_000_000_000 / 10 {
                    10 * ep + i64::from(at(i) - i32::from(b'0'))
                } else {
                    10_000_000_000
                };
                i += 1;
            }
            check(i - stop >= 3 || i - stop == 2 && !esign_explicit)?;
            if esign_minus {
                ep = -ep;
            }
            has_exp = true;
        }
    }
    check(is_dec || has_exp)?;

    // Skip an optional [FfDd] suffix.
    if i < len {
        ch = at(i) | 0x20;
        if ch == i32::from(b'f') || ch == i32::from(b'd') {
            i += 1;
        }
    }
    check(skip_whitespaces(i) == len)?;

    // The input is syntactically correct.
    if tnz == 0 {
        return Ok(Scanned::Zero { negative });
    }

    // Move the point virtually after the last non-zero digit.
    let emult: i64 = if is_dec { 1 } else { 4 };
    let mut n = (tnz - lz) as i64;
    if pt == 0 {
        ep += emult * (stop - tnz) as i64;
    } else {
        ep += emult * (pt as i64 - tnz as i64);
        if pt > tnz {
            ep -= emult;
        } else if lz < pt {
            n -= 1;
        }
    }

    let significant = || {
        units[lz..tnz]
            .iter()
            .filter(|&&unit| unit != u16::from(b'.'))
            .map(|&unit| char::from(unit as u8))
    };

    if !is_dec {
        // Hexadecimal: keep enough digits for rounding, plus a sticky bit.
        let le = (n as usize).min(fmt.hex_count);
        let mut c: i64 = significant().take(le).fold(0, |c, digit| {
            (c << 4) | i64::from(digit.to_digit(16).expect("hex digit"))
        });
        if n as usize > le {
            c |= 1;
            ep += 4 * (n - le as i64);
        }
        let bl = 64 - c.leading_zeros() as i32;
        if ep < i64::from(fmt.q_min - bl) {
            return Ok(Scanned::Zero { negative });
        }
        if ep > i64::from(fmt.e_max - bl + 1) {
            return Ok(Scanned::Infinity { negative });
        }
        let mut q = ep as i32;
        let shr;
        if q > fmt.e_min - bl {
            shr = bl - fmt.p;
            q += shr;
        } else {
            shr = fmt.q_min - q;
            q = fmt.q_min;
        }
        if shr > 0 {
            let thr = 1i64 << shr;
            let tail = (c & (thr - 1)) << 1;
            c = (c as u64 >> shr) as i64;
            if tail > thr || tail == thr && c & 1 != 0 {
                c += 1;
                if c >= 1 << fmt.p {
                    c = (c as u64 >> 1) as i64;
                    q += 1;
                }
            }
        } else {
            c <<= -shr;
        }
        return Ok(Scanned::Binary { negative, q, c });
    }

    // Decimal: 0.<digits> * 10^e, with zero and infinity decided by the exponent alone.
    let e = (ep + n).clamp(fmt.ep_min, fmt.ep_max);
    if e == fmt.ep_min {
        return Ok(Scanned::Zero { negative });
    }
    if e == fmt.ep_max {
        return Ok(Scanned::Infinity { negative });
    }
    Ok(Scanned::Decimal {
        negative,
        digits: significant().collect(),
        e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message<T: fmt::Debug>(result: Result<T, NumberFormatError>) -> String {
        result.unwrap_err().message().to_string_lossy()
    }

    fn j(s: &str) -> JString {
        JString::from(s)
    }

    #[test]
    fn integers_parse_like_java() {
        assert_eq!(parse_int(&j("-2147483648"), 10), Ok(i32::MIN));
        assert_eq!(parse_int(&j("+7f"), 16), Ok(127));
        assert_eq!(parse_int(&j("\u{0661}\u{0662}"), 10), Ok(12));
        assert_eq!(
            message(parse_int(&j("2147483648"), 10)),
            "For input string: \"2147483648\""
        );
        assert_eq!(
            message(parse_int(&j("-"), 16)),
            "For input string: \"-\" under radix 16"
        );
        assert_eq!(message(parse_int(&j(""), 10)), "For input string: \"\"");
        assert_eq!(parse_long(&j("-9223372036854775808"), 10), Ok(i64::MIN));
        assert_eq!(
            message(parse_byte(&j("128"), 10)),
            "Value out of range. Value:\"128\" Radix:10"
        );
        assert_eq!(parse_short(&j("-8000"), 16), Ok(i16::MIN));
    }

    #[test]
    fn unsigned_integers_parse_like_java() {
        assert_eq!(parse_unsigned_int(&j("4294967295"), 10), Ok(-1));
        assert_eq!(
            message(parse_unsigned_int(&j("4294967296"), 10)),
            "String value 4294967296 exceeds range of unsigned int."
        );
        assert_eq!(
            message(parse_unsigned_long(&j("-1"), 10)),
            "Illegal leading minus sign on unsigned string -1."
        );
        assert_eq!(parse_unsigned_long(&j("ffffffffffffffff"), 16), Ok(-1));
        assert_eq!(parse_unsigned_byte(&j("255"), 10), Ok(-1));
        assert_eq!(
            message(parse_unsigned_byte(&j("256"), 10)),
            "out of range: 256"
        );
    }

    #[test]
    fn ranges_report_the_failing_index() {
        let s = j("12-x4-+");
        assert_eq!(parse_long_range(&s, 0, 2, 16), Ok(0x12));
        assert_eq!(
            message(parse_long_range(&s, 3, 5, 16)),
            "Error at index 0 in: \"x4\""
        );
        assert_eq!(
            message(parse_long_range(&s, 6, 7, 16)),
            "Error at index 1 in: \"+\""
        );
    }

    #[test]
    fn floating_point_parses_like_java() {
        assert_eq!(parse_double(&j(" 1.5e3d ")), Ok(1500.0));
        assert_eq!(parse_double(&j("-.5")), Ok(-0.5));
        assert_eq!(parse_double(&j("1.")), Ok(1.0));
        assert_eq!(parse_double(&j("0x1.8p1")), Ok(3.0));
        assert_eq!(parse_double(&j("1e400")), Ok(f64::INFINITY));
        assert_eq!(
            parse_double(&j("-0e5")).map(f64::to_bits),
            Ok((-0.0f64).to_bits())
        );
        assert!(parse_double(&j("NaN")).unwrap().is_nan());
        assert_eq!(parse_float(&j("3.4028236e38")), Ok(f32::INFINITY));
        assert_eq!(parse_float(&j("1.4e-45")), Ok(f32::from_bits(1)));
        assert_eq!(parse_float(&j("0x1p-150")), Ok(0.0));
        assert_eq!(message(parse_double(&j(""))), "empty String");
        assert_eq!(message(parse_double(&j("1..2"))), "multiple points");
        assert_eq!(message(parse_double(&j("1e"))), "For input string: \"1e\"");
        assert_eq!(
            message(parse_double(&j("0x1"))),
            "For input string: \"0x1\""
        );
    }
}
