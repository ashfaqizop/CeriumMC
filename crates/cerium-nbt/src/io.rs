//! Reading and writing tags: each type's `TagType.load` and `Tag.write`, and `NbtIo`.
//!
//! The order of reads, quota charges and checks follows vanilla exactly, because it decides
//! which exception a malformed input produces (for example, a negative array length fails the
//! quota's sign check before anything is read).

use cerium_java::JString;

use crate::data::{DataInput, DataOutput, StringFallbackOutput};
use crate::tag::{self, CompoundTag, ListTag, Tag, id, wrap_if_needed};
use crate::{NbtAccounter, NbtError};

/// `TagType.load` for type `type_id` (`TagTypes.getType(type_id)`, invalid ids included).
///
/// # Errors
///
/// As vanilla: end of input, malformed strings, quota and depth limits, malformed lists,
/// invalid type ids.
pub fn load(
    type_id: i8,
    input: &mut impl DataInput,
    accounter: &mut NbtAccounter,
) -> Result<Tag, NbtError> {
    Ok(match type_id as u8 {
        id::END => {
            accounter.account_bytes(8)?;
            Tag::End
        }
        id::BYTE => {
            accounter.account_bytes(9)?;
            Tag::Byte(input.read_byte()?)
        }
        id::SHORT => {
            accounter.account_bytes(10)?;
            Tag::Short(input.read_short()?)
        }
        id::INT => {
            accounter.account_bytes(12)?;
            Tag::Int(input.read_int()?)
        }
        id::LONG => {
            accounter.account_bytes(16)?;
            Tag::Long(input.read_long()?)
        }
        id::FLOAT => {
            accounter.account_bytes(12)?;
            Tag::Float(input.read_float()?)
        }
        id::DOUBLE => {
            accounter.account_bytes(16)?;
            Tag::Double(input.read_double()?)
        }
        id::BYTE_ARRAY => {
            accounter.account_bytes(24)?;
            let length = input.read_int()?;
            accounter.account_bytes_n(1, i64::from(length))?;
            let bytes = input.read_fully(length as usize)?;
            Tag::ByteArray(bytes.into_iter().map(|b| b as i8).collect())
        }
        id::STRING => Tag::String(load_string(input, accounter)?),
        id::LIST => {
            accounter.push_depth()?;
            let list = load_list(input, accounter);
            let popped = accounter.pop_depth();
            Tag::List(finish(list, popped)?)
        }
        id::COMPOUND => {
            accounter.push_depth()?;
            let compound = load_compound(input, accounter);
            let popped = accounter.pop_depth();
            Tag::Compound(finish(compound, popped)?)
        }
        id::INT_ARRAY => {
            accounter.account_bytes(24)?;
            let length = input.read_int()?;
            accounter.account_bytes_n(4, i64::from(length))?;
            let mut data = Vec::new();
            for _ in 0..length {
                data.push(input.read_int()?);
            }
            Tag::IntArray(data)
        }
        id::LONG_ARRAY => {
            accounter.account_bytes(24)?;
            let length = input.read_int()?;
            accounter.account_bytes_n(8, i64::from(length))?;
            let mut data = Vec::new();
            for _ in 0..length {
                data.push(input.read_long()?);
            }
            Tag::LongArray(data)
        }
        _ => return Err(NbtError::InvalidTagId(i32::from(type_id))),
    })
}

/// The result of a `try { ... } finally { popDepth(); }` block: the body's exception wins
/// over the pop's, as in Java.
fn finish<T>(body: Result<T, NbtError>, popped: Result<(), NbtError>) -> Result<T, NbtError> {
    let value = body?;
    popped?;
    Ok(value)
}

/// `StringTag.TYPE.load`: charges 36, reads, then charges 2 per UTF-16 code unit.
fn load_string(
    input: &mut impl DataInput,
    accounter: &mut NbtAccounter,
) -> Result<JString, NbtError> {
    accounter.account_bytes(36)?;
    let data = input.read_utf()?;
    accounter.account_bytes_n(2, data.len() as i64)?;
    Ok(data)
}

/// `ListTag.readListCount`.
fn read_list_count(input: &mut impl DataInput) -> Result<i32, NbtError> {
    let count = input.read_int()?;
    if count < 0 {
        return Err(NbtError::Format(format!(
            "ListTag length cannot be negative: {count}"
        )));
    }
    Ok(count)
}

/// `ListTag.TYPE.loadList`.
fn load_list(input: &mut impl DataInput, accounter: &mut NbtAccounter) -> Result<ListTag, NbtError> {
    accounter.account_bytes(36)?;
    let type_id = input.read_byte()?;
    let count = read_list_count(input)?;
    if type_id == 0 && count > 0 {
        return Err(NbtError::Format("Missing type on ListTag".to_owned()));
    }
    accounter.account_bytes_n(4, i64::from(count))?;
    let mut list = ListTag::new();
    for _ in 0..count {
        list.add_and_unwrap(load(type_id, input, accounter)?);
    }
    Ok(list)
}

/// `CompoundTag.TYPE.loadCompound`.
fn load_compound(
    input: &mut impl DataInput,
    accounter: &mut NbtAccounter,
) -> Result<CompoundTag, NbtError> {
    accounter.account_bytes(48)?;
    let mut compound = CompoundTag::new();
    loop {
        let type_id = input.read_byte()?;
        if type_id == 0 {
            return Ok(compound);
        }
        let key = read_key(input, accounter)?;
        // `readNamedTagData`: an IOException becomes a ReportedNbtException naming the entry.
        let tag = load(type_id, input, accounter).map_err(|error| {
            error.report(Some(&key), || tag::type_name(i32::from(type_id)))
        })?;
        if compound.put(key, tag).is_none() {
            accounter.account_bytes(36)?;
        }
    }
}

/// `CompoundTag.TYPE.readString`: reads the key, then charges 28 and 2 per code unit.
fn read_key(input: &mut impl DataInput, accounter: &mut NbtAccounter) -> Result<JString, NbtError> {
    let key = input.read_utf()?;
    accounter.account_bytes(28)?;
    accounter.account_bytes_n(2, key.len() as i64)?;
    Ok(key)
}

/// `Tag.write`: the tag's payload, without its type id.
///
/// # Errors
///
/// When a string is too long for Modified UTF-8, or the output fails.
pub fn write(tag: &Tag, output: &mut impl DataOutput) -> Result<(), NbtError> {
    match tag {
        Tag::End => Ok(()),
        Tag::Byte(v) => output.write_byte(*v),
        Tag::Short(v) => output.write_short(*v),
        Tag::Int(v) => output.write_int(*v),
        Tag::Long(v) => output.write_long(*v),
        Tag::Float(v) => output.write_float(*v),
        Tag::Double(v) => output.write_double(*v),
        Tag::ByteArray(v) => {
            output.write_int(length(v.len()))?;
            let bytes: Vec<u8> = v.iter().map(|&b| b as u8).collect();
            output.write_bytes(&bytes)
        }
        Tag::String(s) => output.write_utf(s),
        Tag::List(list) => {
            let element_type = list.identify_raw_element_type();
            output.write_byte(element_type as i8)?;
            output.write_int(length(list.len()))?;
            for element in list {
                write(&wrap_if_needed(element_type, element), output)?;
            }
            Ok(())
        }
        Tag::Compound(compound) => {
            for (name, value) in compound.iter() {
                write_named_tag(name, value, output)?;
            }
            output.write_byte(0)
        }
        Tag::IntArray(v) => {
            output.write_int(length(v.len()))?;
            v.iter().try_for_each(|&i| output.write_int(i))
        }
        Tag::LongArray(v) => {
            output.write_int(length(v.len()))?;
            v.iter().try_for_each(|&l| output.write_long(l))
        }
    }
}

/// A Java array or list length.
fn length(len: usize) -> i32 {
    i32::try_from(len).expect("Java collections hold fewer than 2^31 elements")
}

/// `CompoundTag.writeNamedTag`: the type id, then (except for an end tag) the name and the
/// payload. An end tag value thus ends the compound early when read back, as in vanilla.
fn write_named_tag(name: &JString, tag: &Tag, output: &mut impl DataOutput) -> Result<(), NbtError> {
    output.write_byte(tag.id() as i8)?;
    if tag.id() != id::END {
        output.write_utf(name)?;
        write(tag, output)?;
    }
    Ok(())
}

/// `NbtIo.readTagSafe`: loads the payload, wrapping an `IOException` with the raw type id.
fn read_tag_safe(
    input: &mut impl DataInput,
    accounter: &mut NbtAccounter,
    type_id: i8,
) -> Result<Tag, NbtError> {
    load(type_id, input, accounter).map_err(|error| error.report(None, || type_id.to_string()))
}

/// `NbtIo.readAnyTag`: a type id and a payload, as in packets.
///
/// # Errors
///
/// As [`load`]; `IOException`s from the payload come wrapped in [`NbtError::Reported`].
pub fn read_any_tag(input: &mut impl DataInput, accounter: &mut NbtAccounter) -> Result<Tag, NbtError> {
    let type_id = input.read_byte()?;
    if type_id == 0 {
        return Ok(Tag::End);
    }
    read_tag_safe(input, accounter, type_id)
}

/// `NbtIo.readUnnamedTag`: a type id, a (skipped) name and a payload, as in files.
///
/// # Errors
///
/// As [`read_any_tag`].
pub fn read_unnamed_tag(
    input: &mut impl DataInput,
    accounter: &mut NbtAccounter,
) -> Result<Tag, NbtError> {
    let type_id = input.read_byte()?;
    if type_id == 0 {
        return Ok(Tag::End);
    }
    skip_string(input)?;
    read_tag_safe(input, accounter, type_id)
}

/// `StringTag.skipString`: skips a length-prefixed string without decoding it.
///
/// # Errors
///
/// At the end of the input (the length), or from Netty's skip.
pub fn skip_string(input: &mut impl DataInput) -> Result<(), NbtError> {
    let len = input.read_unsigned_short()?;
    input.skip_bytes(i32::from(len))?;
    Ok(())
}

/// `NbtIo.read(input, accounter)`: a root compound written by [`write_root`].
///
/// # Errors
///
/// As [`read_unnamed_tag`], and `IOException("Root tag must be a named compound tag")`.
pub fn read(input: &mut impl DataInput, accounter: &mut NbtAccounter) -> Result<CompoundTag, NbtError> {
    match read_unnamed_tag(input, accounter)? {
        Tag::Compound(compound) => Ok(compound),
        _ => Err(NbtError::RootNotCompound),
    }
}

/// `NbtIo.writeAnyTag`: the type id and (unless an end tag) the payload, as in packets.
///
/// # Errors
///
/// As [`write`].
pub fn write_any_tag(tag: &Tag, output: &mut impl DataOutput) -> Result<(), NbtError> {
    output.write_byte(tag.id() as i8)?;
    if tag.id() != id::END {
        write(tag, output)?;
    }
    Ok(())
}

/// `NbtIo.writeUnnamedTag`: the type id, an empty name and the payload.
///
/// # Errors
///
/// As [`write`].
pub fn write_unnamed_tag(tag: &Tag, output: &mut impl DataOutput) -> Result<(), NbtError> {
    output.write_byte(tag.id() as i8)?;
    if tag.id() != id::END {
        output.write_utf(&JString::new())?;
        write(tag, output)?;
    }
    Ok(())
}

/// `NbtIo.write(tag, output)`: a root compound for a file, writing `""` in place of any
/// string too long for Modified UTF-8 (`writeUnnamedTagWithFallback`).
///
/// # Errors
///
/// When the output fails.
pub fn write_root(tag: &CompoundTag, output: impl DataOutput) -> Result<(), NbtError> {
    let mut output = StringFallbackOutput::new(output);
    write_unnamed_tag(&Tag::Compound(tag.clone()), &mut output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{ByteBufInput, ByteBufOutput, StreamInput, StreamOutput};

    fn sample() -> CompoundTag {
        let mut compound = CompoundTag::new();
        compound.put_string("name", "Steve");
        compound.put_int("level", 30);
        let mut list = ListTag::new();
        list.push(Tag::Int(1));
        list.push(Tag::string("mixed"));
        compound.put("list", Tag::List(list));
        compound.put("bytes", Tag::ByteArray(vec![-1, 0, 1]));
        compound
    }

    #[test]
    fn files_round_trip() {
        let compound = sample();
        let mut output = StreamOutput::new(Vec::new());
        write_root(&compound, &mut output).unwrap();
        let bytes = output.into_inner();
        assert_eq!(&bytes[..3], [id::COMPOUND, 0, 0]);
        let read_back = read(&mut StreamInput::new(&bytes[..]), &mut NbtAccounter::unlimited_heap());
        assert_eq!(read_back, Ok(compound));
    }

    #[test]
    fn packets_round_trip() {
        let tag = Tag::Compound(sample());
        let mut buffer = Vec::new();
        write_any_tag(&tag, &mut ByteBufOutput::new(&mut buffer)).unwrap();
        let mut input = ByteBufInput::new(&buffer);
        let read_back = read_any_tag(&mut input, &mut NbtAccounter::default_quota());
        assert_eq!(read_back, Ok(tag));
        assert_eq!(input.position(), buffer.len());
    }

    #[test]
    fn mixed_lists_are_written_as_wrapped_compounds() {
        let mut list = ListTag::new();
        list.push(Tag::Byte(1));
        list.push(Tag::Short(2));
        let mut buffer = Vec::new();
        write(&Tag::List(list), &mut ByteBufOutput::new(&mut buffer)).unwrap();
        // Two compounds {"": value}: entry type, empty name, value, end.
        assert_eq!(buffer, [10, 0, 0, 0, 2, 1, 0, 0, 1, 0, 2, 0, 0, 0, 2, 0]);
    }

    #[test]
    fn errors_are_wrapped_like_vanilla() {
        // A compound whose int entry "a" is cut off.
        let bytes = [10, 3, 0, 1, b'a', 0, 0];
        let error = read_any_tag(&mut ByteBufInput::new(&bytes), &mut NbtAccounter::default_quota())
            .unwrap_err();
        let NbtError::Reported { cause, tag_name, tag_type } = error else {
            panic!("expected a ReportedNbtException, got {error:?}");
        };
        assert_eq!(cause.java_class(), "EOFException");
        assert_eq!(tag_name, Some(JString::from("a")));
        assert_eq!(tag_type, "INT");
        // A negative list length is an NbtFormatException, never wrapped.
        let list = [9, 1, 0xFF, 0xFF, 0xFF, 0xFF];
        let error = read_any_tag(&mut ByteBufInput::new(&list), &mut NbtAccounter::default_quota())
            .unwrap_err();
        assert_eq!(error.message().as_deref(), Some("ListTag length cannot be negative: -1"));
        // A negative byte array length fails the accounter's sign check.
        let array = [7, 0xFF, 0xFF, 0xFF, 0xFE];
        let error = read_any_tag(&mut ByteBufInput::new(&array), &mut NbtAccounter::default_quota())
            .unwrap_err();
        assert_eq!(error.java_class(), "IllegalArgumentException");
    }

    #[test]
    fn depth_is_limited() {
        let mut bytes = vec![9];
        for _ in 0..600 {
            bytes.extend([9, 0, 0, 0, 1]);
        }
        let error = read_any_tag(&mut ByteBufInput::new(&bytes), &mut NbtAccounter::default_quota())
            .unwrap_err();
        assert_eq!(
            error.message().as_deref(),
            Some("Tried to read NBT tag with too high complexity, depth > 512")
        );
    }
}
