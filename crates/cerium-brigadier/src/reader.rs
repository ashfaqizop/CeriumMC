//! [`StringReader`]: Brigadier's cursor over a command string.

use cerium_java::{JString, JStringBuilder, character, number};

use crate::{CommandSyntaxException, builtin};

const ESCAPE: u16 = b'\\' as u16;
const DOUBLE_QUOTE: u16 = b'"' as u16;
const SINGLE_QUOTE: u16 = b'\'' as u16;

/// `com.mojang.brigadier.StringReader`: a string and a cursor into it, in UTF-16 code units.
///
/// Like Java's, the reader panics (`StringIndexOutOfBoundsException`) when it peeks or reads
/// past the end; callers check [`can_read`](Self::can_read) first, as vanilla does.
#[derive(Clone, Debug)]
pub struct StringReader {
    string: JString,
    cursor: usize,
}

impl StringReader {
    /// A reader at the start of `string`.
    #[must_use]
    pub fn new(string: JString) -> Self {
        Self { string, cursor: 0 }
    }

    /// The whole string (`getString`).
    #[must_use]
    pub fn string(&self) -> &JString {
        &self.string
    }

    /// The cursor (`getCursor`).
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the cursor (`setCursor`).
    pub fn set_cursor(&mut self, cursor: usize) {
        self.cursor = cursor;
    }

    /// The number of code units after the cursor (`getRemainingLength`).
    #[must_use]
    pub fn remaining_length(&self) -> usize {
        self.string.len().saturating_sub(self.cursor)
    }

    /// The string's length (`getTotalLength`).
    #[must_use]
    pub fn total_length(&self) -> usize {
        self.string.len()
    }

    /// The part before the cursor (`getRead`).
    #[must_use]
    pub fn read_part(&self) -> JString {
        self.string.substring(0, self.cursor)
    }

    /// The part from the cursor on (`getRemaining`).
    #[must_use]
    pub fn remaining(&self) -> JString {
        self.string.substring(self.cursor, self.string.len())
    }

    /// Whether `length` more code units can be read (`canRead(length)`).
    #[must_use]
    pub fn can_read_n(&self, length: usize) -> bool {
        self.cursor + length <= self.string.len()
    }

    /// Whether a code unit can be read (`canRead()`).
    #[must_use]
    pub fn can_read(&self) -> bool {
        self.can_read_n(1)
    }

    /// The code unit at the cursor (`peek()`).
    ///
    /// # Panics
    ///
    /// At the end of the string.
    #[must_use]
    pub fn peek(&self) -> u16 {
        self.string.char_at(self.cursor)
    }

    /// The code unit `offset` after the cursor (`peek(offset)`).
    ///
    /// # Panics
    ///
    /// Past the end of the string.
    #[must_use]
    pub fn peek_at(&self, offset: usize) -> u16 {
        self.string.char_at(self.cursor + offset)
    }

    /// Reads the code unit at the cursor (`read()`).
    ///
    /// # Panics
    ///
    /// At the end of the string.
    pub fn read(&mut self) -> u16 {
        let unit = self.string.char_at(self.cursor);
        self.cursor += 1;
        unit
    }

    /// Advances the cursor by one (`skip()`).
    pub fn skip(&mut self) {
        self.cursor += 1;
    }

    /// `isAllowedNumber`: `0`-`9`, `.` and `-`.
    #[must_use]
    pub fn is_allowed_number(c: u16) -> bool {
        matches!(c, 0x30..=0x39 | 0x2E | 0x2D)
    }

    /// `isQuotedStringStart`: `"` or `'`.
    #[must_use]
    pub fn is_quoted_string_start(c: u16) -> bool {
        c == DOUBLE_QUOTE || c == SINGLE_QUOTE
    }

    /// `isAllowedInUnquotedString`: ASCII letters and digits, `_`, `-`, `.` and `+`.
    #[must_use]
    pub fn is_allowed_in_unquoted_string(c: u16) -> bool {
        matches!(c, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A | 0x5F | 0x2D | 0x2E | 0x2B)
    }

    /// Skips `Character.isWhitespace` code units (`skipWhitespace`).
    pub fn skip_whitespace(&mut self) {
        while self.can_read() && character::is_whitespace(self.peek()) {
            self.skip();
        }
    }

    /// Reads a run of number characters, returning where it started and its text.
    fn read_number_text(&mut self) -> (usize, JString) {
        let start = self.cursor;
        while self.can_read() && Self::is_allowed_number(self.peek()) {
            self.skip();
        }
        (start, self.string.substring(start, self.cursor))
    }

    /// `readInt`.
    ///
    /// # Errors
    ///
    /// `parsing.int.expected` without digits; `parsing.int.invalid` (cursor back at the start)
    /// when `Integer.parseInt` rejects them.
    pub fn read_int(&mut self) -> Result<i32, CommandSyntaxException> {
        let (start, number) = self.read_number_text();
        if number.is_empty() {
            return Err(CommandSyntaxException::with_context(
                builtin::reader_expected_int(),
                self,
            ));
        }
        number::parse_int(&number, 10).map_err(|_| {
            self.cursor = start;
            CommandSyntaxException::with_context(builtin::reader_invalid_int(number), self)
        })
    }

    /// `readLong`.
    ///
    /// # Errors
    ///
    /// `parsing.long.expected` or `parsing.long.invalid`, as [`read_int`](Self::read_int).
    pub fn read_long(&mut self) -> Result<i64, CommandSyntaxException> {
        let (start, number) = self.read_number_text();
        if number.is_empty() {
            return Err(CommandSyntaxException::with_context(
                builtin::reader_expected_long(),
                self,
            ));
        }
        number::parse_long(&number, 10).map_err(|_| {
            self.cursor = start;
            CommandSyntaxException::with_context(builtin::reader_invalid_long(number), self)
        })
    }

    /// `readDouble`.
    ///
    /// # Errors
    ///
    /// `parsing.double.expected` or `parsing.double.invalid`.
    pub fn read_double(&mut self) -> Result<f64, CommandSyntaxException> {
        let (start, number) = self.read_number_text();
        if number.is_empty() {
            return Err(CommandSyntaxException::with_context(
                builtin::reader_expected_double(),
                self,
            ));
        }
        number::parse_double(&number).map_err(|_| {
            self.cursor = start;
            CommandSyntaxException::with_context(builtin::reader_invalid_double(number), self)
        })
    }

    /// `readFloat`.
    ///
    /// # Errors
    ///
    /// `parsing.float.expected` or `parsing.float.invalid`.
    pub fn read_float(&mut self) -> Result<f32, CommandSyntaxException> {
        let (start, number) = self.read_number_text();
        if number.is_empty() {
            return Err(CommandSyntaxException::with_context(
                builtin::reader_expected_float(),
                self,
            ));
        }
        number::parse_float(&number).map_err(|_| {
            self.cursor = start;
            CommandSyntaxException::with_context(builtin::reader_invalid_float(number), self)
        })
    }

    /// `readUnquotedString`: the longest run of unquoted-string characters (maybe empty).
    pub fn read_unquoted_string(&mut self) -> JString {
        let start = self.cursor;
        while self.can_read() && Self::is_allowed_in_unquoted_string(self.peek()) {
            self.skip();
        }
        self.string.substring(start, self.cursor)
    }

    /// `readQuotedString`: an empty string at the end of input, else a quoted string.
    ///
    /// # Errors
    ///
    /// `parsing.quote.expected.start` when no quote follows, or the errors of
    /// [`read_string_until`](Self::read_string_until).
    pub fn read_quoted_string(&mut self) -> Result<JString, CommandSyntaxException> {
        if !self.can_read() {
            return Ok(JString::new());
        }
        let next = self.peek();
        if !Self::is_quoted_string_start(next) {
            return Err(CommandSyntaxException::with_context(
                builtin::reader_expected_start_of_quote(),
                self,
            ));
        }
        self.skip();
        self.read_string_until(next)
    }

    /// `readStringUntil`: reads up to the unescaped `terminator`; only the terminator and the
    /// backslash may be escaped.
    ///
    /// # Errors
    ///
    /// `parsing.quote.escape` (cursor on the escaped character) or
    /// `parsing.quote.expected.end`.
    pub fn read_string_until(
        &mut self,
        terminator: u16,
    ) -> Result<JString, CommandSyntaxException> {
        let mut result = JStringBuilder::new();
        let mut escaped = false;
        while self.can_read() {
            let c = self.read();
            if escaped {
                if c != terminator && c != ESCAPE {
                    self.cursor -= 1;
                    let character = JString::from_utf16(&[c]);
                    return Err(CommandSyntaxException::with_context(
                        builtin::reader_invalid_escape(character),
                        self,
                    ));
                }
                result.push_unit(c);
                escaped = false;
            } else if c == ESCAPE {
                escaped = true;
            } else if c == terminator {
                return Ok(result.build());
            } else {
                result.push_unit(c);
            }
        }
        Err(CommandSyntaxException::with_context(
            builtin::reader_expected_end_of_quote(),
            self,
        ))
    }

    /// `readString`: a quoted string, or else an unquoted one.
    ///
    /// # Errors
    ///
    /// The errors of [`read_string_until`](Self::read_string_until).
    pub fn read_string(&mut self) -> Result<JString, CommandSyntaxException> {
        if !self.can_read() {
            return Ok(JString::new());
        }
        let next = self.peek();
        if Self::is_quoted_string_start(next) {
            self.skip();
            return self.read_string_until(next);
        }
        Ok(self.read_unquoted_string())
    }

    /// `readBoolean`: exactly `true` or `false`.
    ///
    /// # Errors
    ///
    /// `parsing.bool.expected` for an empty string, `parsing.bool.invalid` (cursor back at the
    /// start) for anything else.
    pub fn read_boolean(&mut self) -> Result<bool, CommandSyntaxException> {
        let start = self.cursor;
        let value = self.read_string()?;
        if value.is_empty() {
            return Err(CommandSyntaxException::with_context(
                builtin::reader_expected_bool(),
                self,
            ));
        }
        if value == "true" {
            return Ok(true);
        }
        if value == "false" {
            return Ok(false);
        }
        self.cursor = start;
        Err(CommandSyntaxException::with_context(
            builtin::reader_invalid_bool(value),
            self,
        ))
    }

    /// `expect(c)`: skips `c`, which must be next.
    ///
    /// # Errors
    ///
    /// `parsing.expected` with `c`.
    pub fn expect(&mut self, c: u16) -> Result<(), CommandSyntaxException> {
        if self.can_read() && self.peek() == c {
            self.skip();
            Ok(())
        } else {
            let symbol = JString::from_utf16(&[c]);
            Err(CommandSyntaxException::with_context(
                builtin::reader_expected_symbol(symbol),
                self,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Arg, Message};

    fn reader(s: &str) -> StringReader {
        StringReader::new(JString::from(s))
    }

    fn key(error: &CommandSyntaxException) -> &'static str {
        error.message().key().unwrap()
    }

    #[test]
    fn numbers() {
        let mut r = reader("12 -3.5x");
        assert_eq!(r.read_int(), Ok(12));
        r.skip_whitespace();
        assert_eq!(r.read_double(), Ok(-3.5));
        assert_eq!(r.peek(), u16::from(b'x'));
        let error = reader("1-2").read_int().unwrap_err();
        assert_eq!(key(&error), "parsing.int.invalid");
        assert_eq!(error.cursor(), 0);
        assert_eq!(
            error.message(),
            &Message::translatable_with("parsing.int.invalid", vec![Arg::from("1-2")])
        );
        assert_eq!(
            key(&reader("x").read_long().unwrap_err()),
            "parsing.long.expected"
        );
    }

    #[test]
    fn strings() {
        let mut r = reader(r#""a\"b" 'c\\d' plain+1 "x\y""#);
        assert_eq!(r.read_string().unwrap(), "a\"b");
        r.skip_whitespace();
        assert_eq!(r.read_quoted_string().unwrap(), "c\\d");
        r.skip_whitespace();
        assert_eq!(r.read_string().unwrap(), "plain+1");
        r.skip_whitespace();
        let error = r.read_string().unwrap_err();
        assert_eq!(key(&error), "parsing.quote.escape");
        assert_eq!(error.cursor(), 25);
        assert_eq!(
            key(&reader("\"open").read_string().unwrap_err()),
            "parsing.quote.expected.end"
        );
    }

    #[test]
    fn booleans_and_symbols() {
        assert_eq!(reader("true").read_boolean(), Ok(true));
        let error = reader("True").read_boolean().unwrap_err();
        assert_eq!(key(&error), "parsing.bool.invalid");
        assert_eq!(error.cursor(), 0);
        let mut r = reader("a");
        assert_eq!(
            key(&r.expect(u16::from(b'b')).unwrap_err()),
            "parsing.expected"
        );
        assert_eq!(r.expect(u16::from(b'a')), Ok(()));
    }

    #[test]
    fn whitespace_is_javas() {
        let mut r = reader("\u{3000}\t x");
        r.skip_whitespace();
        assert_eq!(r.cursor(), 3);
        let mut nbsp = reader("\u{A0}x");
        nbsp.skip_whitespace();
        assert_eq!(nbsp.cursor(), 0);
    }
}
