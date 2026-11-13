//! The parts of the Java standard library (JDK 25) whose exact behaviour vanilla depends on.
//!
//! - [`JString`] holds Java strings, which are UTF-16 and may have unpaired surrogates.
//! - [`mutf8`] is Modified UTF-8, the string encoding of `DataInput`/`DataOutput`.
//! - [`JavaHashMap`] is `java.util.HashMap`, including its iteration order.
//! - [`to_decimal`] has `Double.toString` and `Float.toString`.
//! - [`number`] has the integer and floating-point parsers and their error messages.
//! - [`character`] has `Character.isWhitespace`, `digit`, `getName` and `codePointOf`.
//! - [`uuid`] has `UUID.fromString`.
//!
//! Each is a transcription of the JDK's source, verified against Java 25 by golden tests.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "deliberate ports of Java's primitive conversions, keeping the JDK's variable names"
)]

pub mod character;
pub mod hash_map;
pub mod mutf8;
pub mod number;
mod string;
pub mod to_decimal;
mod to_decimal_table;
pub mod uuid;

pub use hash_map::{Equivalent, JavaHashMap, JavaKey};
pub use string::{JString, JStringBuilder};
