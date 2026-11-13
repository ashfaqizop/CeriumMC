//! The parts of Mojang's Brigadier (1.3.11, MIT) that vanilla's parsers build on: the
//! [`StringReader`] cursor over a command string and [`CommandSyntaxException`].
//!
//! Vanilla replaces Brigadier's built-in English messages with translatable components
//! (`net.minecraft.commands.BrigadierExceptions`), so every error here carries a
//! [`Message`] with Minecraft's translation key and arguments. Turning a message into text
//! needs the language files and the text component system, which come with Phase 3; until
//! then, the key, the arguments, the input and the cursor are kept exactly.
//!
//! Strings are [`JString`]s and positions are UTF-16 code unit indices, as in Java, so error
//! cursors match vanilla's for any input.

mod reader;

pub use reader::StringReader;

use cerium_java::{JString, JStringBuilder};

/// An argument of a translatable message. `Component.translatableEscape` keeps numbers,
/// booleans and strings as they are (`TranslatableContents.isAllowedPrimitiveArgument`).
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    /// A string.
    Str(JString),
    /// An `int`.
    Int(i32),
    /// A `long`.
    Long(i64),
    /// A `float`.
    Float(f32),
    /// A `double`.
    Double(f64),
    /// A `boolean`.
    Bool(bool),
}

impl From<JString> for Arg {
    fn from(value: JString) -> Self {
        Self::Str(value)
    }
}

impl From<&str> for Arg {
    fn from(value: &str) -> Self {
        Self::Str(JString::from(value))
    }
}

/// A Brigadier `Message`: what vanilla puts in its exceptions.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// `Component.translatable(key, args)`.
    Translatable {
        /// The translation key, such as `parsing.int.invalid`.
        key: &'static str,
        /// The arguments substituted into the translation.
        args: Vec<Arg>,
    },
    /// A literal text.
    Literal(JString),
}

impl Message {
    /// A translatable message without arguments.
    #[must_use]
    pub fn translatable(key: &'static str) -> Self {
        Self::Translatable {
            key,
            args: Vec::new(),
        }
    }

    /// A translatable message with arguments.
    #[must_use]
    pub fn translatable_with(key: &'static str, args: Vec<Arg>) -> Self {
        Self::Translatable { key, args }
    }

    /// The translation key, if the message is translatable.
    #[must_use]
    pub fn key(&self) -> Option<&'static str> {
        match self {
            Self::Translatable { key, .. } => Some(key),
            Self::Literal(_) => None,
        }
    }
}

/// How many characters of input `CommandSyntaxException.getContext` shows before the cursor.
pub const CONTEXT_AMOUNT: usize = 10;

/// A `CommandSyntaxException`: a message, and optionally the input and the cursor where
/// parsing failed.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandSyntaxException {
    message: Message,
    input: Option<JString>,
    cursor: i32,
}

impl CommandSyntaxException {
    /// An exception without input context (`CommandExceptionType.create`).
    #[must_use]
    pub fn new(message: Message) -> Self {
        Self {
            message,
            input: None,
            cursor: -1,
        }
    }

    /// An exception at the reader's cursor (`createWithContext(reader)`).
    #[must_use]
    pub fn with_context(message: Message, reader: &StringReader) -> Self {
        Self::at(message, reader.string().clone(), reader.cursor())
    }

    /// An exception at `cursor` in `input`.
    #[must_use]
    pub fn at(message: Message, input: JString, cursor: usize) -> Self {
        Self {
            message,
            input: Some(input),
            cursor: i32::try_from(cursor).unwrap_or(i32::MAX),
        }
    }

    /// The message (`getRawMessage`).
    #[must_use]
    pub fn message(&self) -> &Message {
        &self.message
    }

    /// The input that failed to parse, if known.
    #[must_use]
    pub fn input(&self) -> Option<&JString> {
        self.input.as_ref()
    }

    /// The cursor where parsing failed, or -1 without input.
    #[must_use]
    pub fn cursor(&self) -> i32 {
        self.cursor
    }

    /// `getContext`: up to 10 code units of input before the cursor (after `...` when more
    /// precede them), then `<--[HERE]`.
    #[must_use]
    pub fn context(&self) -> Option<JString> {
        let input = self.input.as_ref()?;
        let cursor = usize::try_from(self.cursor).ok()?.min(input.len());
        let mut builder = JStringBuilder::new();
        if cursor > CONTEXT_AMOUNT {
            builder.push_str("...");
        }
        builder
            .push_jstring(&input.substring(cursor.saturating_sub(CONTEXT_AMOUNT), cursor))
            .push_str("<--[HERE]");
        Some(builder.build())
    }
}

impl std::fmt::Display for CommandSyntaxException {
    /// The message's key or text, then Brigadier's position and context.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.message {
            Message::Translatable { key, .. } => f.write_str(key)?,
            Message::Literal(text) => write!(f, "{text}")?,
        }
        if let Some(context) = self.context() {
            write!(f, " at position {}: {context}", self.cursor)?;
        }
        Ok(())
    }
}

impl std::error::Error for CommandSyntaxException {}

/// The messages of Minecraft's `BrigadierExceptions`, which replace Brigadier's built-ins.
pub mod builtin {
    use super::{Arg, Message};

    fn with(key: &'static str, args: Vec<Arg>) -> Message {
        Message::translatable_with(key, args)
    }

    /// `doubleTooLow`: `argument.double.low` with the minimum and the value found.
    #[must_use]
    pub fn double_too_low(found: f64, min: f64) -> Message {
        with(
            "argument.double.low",
            vec![Arg::Double(min), Arg::Double(found)],
        )
    }

    /// `doubleTooHigh`: `argument.double.big` with the maximum and the value found.
    #[must_use]
    pub fn double_too_high(found: f64, max: f64) -> Message {
        with(
            "argument.double.big",
            vec![Arg::Double(max), Arg::Double(found)],
        )
    }

    /// `floatTooLow`.
    #[must_use]
    pub fn float_too_low(found: f32, min: f32) -> Message {
        with(
            "argument.float.low",
            vec![Arg::Float(min), Arg::Float(found)],
        )
    }

    /// `floatTooHigh`.
    #[must_use]
    pub fn float_too_high(found: f32, max: f32) -> Message {
        with(
            "argument.float.big",
            vec![Arg::Float(max), Arg::Float(found)],
        )
    }

    /// `integerTooLow`.
    #[must_use]
    pub fn integer_too_low(found: i32, min: i32) -> Message {
        with("argument.integer.low", vec![Arg::Int(min), Arg::Int(found)])
    }

    /// `integerTooHigh`.
    #[must_use]
    pub fn integer_too_high(found: i32, max: i32) -> Message {
        with("argument.integer.big", vec![Arg::Int(max), Arg::Int(found)])
    }

    /// `longTooLow`.
    #[must_use]
    pub fn long_too_low(found: i64, min: i64) -> Message {
        with("argument.long.low", vec![Arg::Long(min), Arg::Long(found)])
    }

    /// `longTooHigh`.
    #[must_use]
    pub fn long_too_high(found: i64, max: i64) -> Message {
        with("argument.long.big", vec![Arg::Long(max), Arg::Long(found)])
    }

    /// `literalIncorrect`: `argument.literal.incorrect` with the expected literal.
    #[must_use]
    pub fn literal_incorrect(expected: impl Into<Arg>) -> Message {
        with("argument.literal.incorrect", vec![expected.into()])
    }

    /// `readerExpectedStartOfQuote`.
    #[must_use]
    pub fn reader_expected_start_of_quote() -> Message {
        Message::translatable("parsing.quote.expected.start")
    }

    /// `readerExpectedEndOfQuote`.
    #[must_use]
    pub fn reader_expected_end_of_quote() -> Message {
        Message::translatable("parsing.quote.expected.end")
    }

    /// `readerInvalidEscape`, with the escaped character.
    #[must_use]
    pub fn reader_invalid_escape(character: impl Into<Arg>) -> Message {
        with("parsing.quote.escape", vec![character.into()])
    }

    /// `readerInvalidBool`, with the value read.
    #[must_use]
    pub fn reader_invalid_bool(value: impl Into<Arg>) -> Message {
        with("parsing.bool.invalid", vec![value.into()])
    }

    /// `readerInvalidInt`, with the text read.
    #[must_use]
    pub fn reader_invalid_int(value: impl Into<Arg>) -> Message {
        with("parsing.int.invalid", vec![value.into()])
    }

    /// `readerExpectedInt`.
    #[must_use]
    pub fn reader_expected_int() -> Message {
        Message::translatable("parsing.int.expected")
    }

    /// `readerInvalidLong`, with the text read.
    #[must_use]
    pub fn reader_invalid_long(value: impl Into<Arg>) -> Message {
        with("parsing.long.invalid", vec![value.into()])
    }

    /// `readerExpectedLong`.
    #[must_use]
    pub fn reader_expected_long() -> Message {
        Message::translatable("parsing.long.expected")
    }

    /// `readerInvalidDouble`, with the text read.
    #[must_use]
    pub fn reader_invalid_double(value: impl Into<Arg>) -> Message {
        with("parsing.double.invalid", vec![value.into()])
    }

    /// `readerExpectedDouble`.
    #[must_use]
    pub fn reader_expected_double() -> Message {
        Message::translatable("parsing.double.expected")
    }

    /// `readerInvalidFloat`, with the text read.
    #[must_use]
    pub fn reader_invalid_float(value: impl Into<Arg>) -> Message {
        with("parsing.float.invalid", vec![value.into()])
    }

    /// `readerExpectedFloat`.
    #[must_use]
    pub fn reader_expected_float() -> Message {
        Message::translatable("parsing.float.expected")
    }

    /// `readerExpectedBool`.
    #[must_use]
    pub fn reader_expected_bool() -> Message {
        Message::translatable("parsing.bool.expected")
    }

    /// `readerExpectedSymbol`, with the expected symbol.
    #[must_use]
    pub fn reader_expected_symbol(symbol: impl Into<Arg>) -> Message {
        with("parsing.expected", vec![symbol.into()])
    }

    /// `dispatcherUnknownCommand`.
    #[must_use]
    pub fn dispatcher_unknown_command() -> Message {
        Message::translatable("command.unknown.command")
    }

    /// `dispatcherUnknownArgument`.
    #[must_use]
    pub fn dispatcher_unknown_argument() -> Message {
        Message::translatable("command.unknown.argument")
    }

    /// `dispatcherExpectedArgumentSeparator`.
    #[must_use]
    pub fn dispatcher_expected_argument_separator() -> Message {
        Message::translatable("command.expected.separator")
    }

    /// `dispatcherParseException`, with the parse error's message.
    #[must_use]
    pub fn dispatcher_parse_exception(message: impl Into<Arg>) -> Message {
        with("command.exception", vec![message.into()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_shows_ten_units_before_the_cursor() {
        let input = JString::from("give @p minecraft:diamond 99x");
        let error = CommandSyntaxException::at(builtin::reader_expected_int(), input, 28);
        assert_eq!(error.context().unwrap(), "...diamond 99<--[HERE]");
        let short =
            CommandSyntaxException::at(builtin::reader_expected_int(), JString::from("ab"), 1);
        assert_eq!(short.context().unwrap(), "a<--[HERE]");
        assert_eq!(
            short.to_string(),
            "parsing.int.expected at position 1: a<--[HERE]"
        );
        assert_eq!(
            CommandSyntaxException::new(builtin::reader_expected_int()).context(),
            None
        );
    }
}
