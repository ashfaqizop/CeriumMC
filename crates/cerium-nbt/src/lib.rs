//! Vanilla's NBT (`net.minecraft.nbt`), 1:1.
//!
//! - [`tag`] has the tags: [`Tag`], [`ListTag`] and [`CompoundTag`] (a `java.util.HashMap`,
//!   so entries are written in vanilla's order).
//! - [`io`] reads and writes tags (`TagType.load`, `Tag.write`, `NbtIo`), over the file and
//!   packet flavours of [`data`].
//! - [`NbtAccounter`] limits the memory and nesting a read may use.
//! - [`visitor`] streams tags through `StreamTagVisitor`s, as vanilla does to read only some
//!   fields of a chunk.
//! - [`print`] turns tags into SNBT (`toString()` and the indented structure-file form).
//!
//! Errors are [`NbtError`]s, one variant per Java exception class, with Java's messages.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    reason = "Java's byte, int and long conversions are reproduced deliberately"
)]

mod accounter;
pub mod data;
mod error;
pub mod io;
pub mod print;
pub mod tag;
pub mod visitor;

pub use accounter::{DEFAULT_NBT_QUOTA, NbtAccounter, UNCOMPRESSED_NBT_QUOTA};
pub use error::NbtError;
pub use tag::{CompoundTag, ListTag, Tag};
