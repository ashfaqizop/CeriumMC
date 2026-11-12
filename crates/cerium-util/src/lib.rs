//! Small building blocks shared across CeriumMC crates.
//!
//! - [`md5`]: the MD5 digest, which vanilla uses to derive random seeds from names and to
//!   build offline-mode player UUIDs.
//! - [`java`]: hashing that matches the Java standard library bit for bit.

pub mod java;
pub mod md5;
