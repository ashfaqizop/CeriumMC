//! [`NbtError`]: the exceptions vanilla's NBT code throws, kept apart by Java class.

use std::fmt;

use cerium_java::JString;
use cerium_java::mutf8::UtfDataFormatError;

/// An exception from reading or writing NBT. Each variant is one Java exception class, with
/// its exact message, because vanilla treats them differently: only `IOException`s are
/// wrapped into a `ReportedNbtException`, and network code turns them into disconnect
/// reasons.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NbtError {
    /// `java.io.EOFException`. Java's `DataInputStream` throws it without a message; Netty's
    /// `ByteBufInputStream` says how many bytes were requested and how many were left.
    Eof {
        /// The message, if any.
        message: Option<String>,
    },
    /// `java.io.UTFDataFormatException` from Modified UTF-8.
    Utf(UtfDataFormatError),
    /// An `IOException`: `"Invalid tag id: <id>"`, from an unknown type id.
    InvalidTagId(i32),
    /// An `IOException`: `"Root tag must be a named compound tag"`.
    RootNotCompound,
    /// Another `IOException` from the underlying stream (a file or decompressor error).
    Io(String),
    /// `NbtAccounterException`: a size quota or the depth limit was exceeded.
    Accounter(String),
    /// `NbtFormatException`: a malformed list header.
    Format(String),
    /// `IllegalArgumentException` (`NbtAccounter` was given a negative size, or Netty was
    /// asked to skip a negative number of bytes).
    IllegalArgument(String),
    /// `IndexOutOfBoundsException` (Netty was asked to read a negative number of bytes).
    IndexOutOfBounds(String),
    /// `ReportedNbtException`: an `IOException` thrown while loading a tag, with the crash
    /// report details vanilla records ("Loading NBT data").
    Reported {
        /// The `IOException`.
        cause: Box<NbtError>,
        /// The "Tag name" detail: the compound key whose value failed, if a compound entry.
        tag_name: Option<JString>,
        /// The "Tag type" detail: the type's name (`COMPOUND`) for a compound entry, or the
        /// raw type id (`10`) for a root tag.
        tag_type: String,
    },
}

impl NbtError {
    /// The simple name of the Java exception class.
    #[must_use]
    pub fn java_class(&self) -> &'static str {
        match self {
            Self::Eof { .. } => "EOFException",
            Self::Utf(_) => "UTFDataFormatException",
            Self::InvalidTagId(_) | Self::RootNotCompound | Self::Io(_) => "IOException",
            Self::Accounter(_) => "NbtAccounterException",
            Self::Format(_) => "NbtFormatException",
            Self::IllegalArgument(_) => "IllegalArgumentException",
            Self::IndexOutOfBounds(_) => "IndexOutOfBoundsException",
            Self::Reported { .. } => "ReportedNbtException",
        }
    }

    /// Whether the exception is an `IOException` (which loaders wrap into
    /// [`NbtError::Reported`]).
    #[must_use]
    pub fn is_io_exception(&self) -> bool {
        matches!(
            self,
            Self::Eof { .. }
                | Self::Utf(_)
                | Self::InvalidTagId(_)
                | Self::RootNotCompound
                | Self::Io(_)
        )
    }

    /// The exception's message (`getMessage()`), `None` where Java's is `null`. A
    /// `ReportedException`'s message is its crash report's title.
    #[must_use]
    pub fn message(&self) -> Option<String> {
        match self {
            Self::Eof { message } => message.clone(),
            Self::Utf(error) => Some(error.message().to_owned()),
            Self::InvalidTagId(id) => Some(format!("Invalid tag id: {id}")),
            Self::RootNotCompound => Some("Root tag must be a named compound tag".to_owned()),
            Self::Io(message)
            | Self::Accounter(message)
            | Self::Format(message)
            | Self::IllegalArgument(message)
            | Self::IndexOutOfBounds(message) => Some(message.clone()),
            Self::Reported { .. } => Some("Loading NBT data".to_owned()),
        }
    }

    /// Wraps an `IOException` the way `readTagSafe`/`readNamedTagData` do; other exceptions
    /// pass through unchanged.
    pub(crate) fn report(self, tag_name: Option<&JString>, tag_type: impl FnOnce() -> String) -> Self {
        if self.is_io_exception() {
            Self::Reported {
                cause: Box::new(self),
                tag_name: tag_name.cloned(),
                tag_type: tag_type(),
            }
        } else {
            self
        }
    }
}

impl fmt::Display for NbtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_class())?;
        if let Some(message) = self.message() {
            write!(f, ": {message}")?;
        }
        if let Self::Reported { cause, .. } = self {
            write!(f, " (caused by {cause})")?;
        }
        Ok(())
    }
}

impl std::error::Error for NbtError {}

impl From<UtfDataFormatError> for NbtError {
    fn from(error: UtfDataFormatError) -> Self {
        Self::Utf(error)
    }
}
