//! Bit-exact ports of vanilla's math helpers.
//!
//! - [`mth`]: vanilla's `Mth` utility functions: lookup-table trigonometry, rounding, angle
//!   wrapping, interpolation and positional seeds.
//! - [`java`]: Java primitive semantics that differ from Rust's defaults (NaN-aware
//!   min/max, floor division, NaN-canonicalizing bit casts).
//!
//! Every function gives the same result as vanilla on every platform. Transcendental
//! functions come from the pure-Rust `libm` crate, never from the platform's math library,
//! so results don't depend on the operating system or CPU.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "these are deliberate, bit-exact ports of Java's primitive conversions and comparisons"
)]

pub mod java;
pub mod mth;
