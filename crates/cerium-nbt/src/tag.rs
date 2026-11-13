//! The NBT tags: [`Tag`], [`ListTag`] and [`CompoundTag`].
//!
//! Each Java tag class is a variant of [`Tag`]. Their identity semantics follow Java:
//!
//! - equality is Java's `equals` (floats compare by `floatToIntBits`, so `NaN == NaN` and
//!   `0.0 != -0.0`), and [`Tag::java_hash_code`] is Java's `hashCode`;
//! - [`Clone`] stands for sharing the Java object (a compound keeps its exact `HashMap`
//!   layout), while [`Tag::copy`] is vanilla's `copy()`, which rebuilds compounds by putting
//!   each entry into a new map, so their iteration order can change;
//! - [`Tag::size_in_bytes`] is vanilla's estimate of the Java object's memory size.

use cerium_java::{JString, JavaHashMap};

/// `Tag.TAG_END` and the other type ids.
pub mod id {
    /// `TAG_END`.
    pub const END: u8 = 0;
    /// `TAG_BYTE`.
    pub const BYTE: u8 = 1;
    /// `TAG_SHORT`.
    pub const SHORT: u8 = 2;
    /// `TAG_INT`.
    pub const INT: u8 = 3;
    /// `TAG_LONG`.
    pub const LONG: u8 = 4;
    /// `TAG_FLOAT`.
    pub const FLOAT: u8 = 5;
    /// `TAG_DOUBLE`.
    pub const DOUBLE: u8 = 6;
    /// `TAG_BYTE_ARRAY`.
    pub const BYTE_ARRAY: u8 = 7;
    /// `TAG_STRING`.
    pub const STRING: u8 = 8;
    /// `TAG_LIST`.
    pub const LIST: u8 = 9;
    /// `TAG_COMPOUND`.
    pub const COMPOUND: u8 = 10;
    /// `TAG_INT_ARRAY`.
    pub const INT_ARRAY: u8 = 11;
    /// `TAG_LONG_ARRAY`.
    pub const LONG_ARRAY: u8 = 12;
}

/// `Tag.MAX_DEPTH`: how deeply lists and compounds may nest when read.
pub const MAX_DEPTH: i32 = 512;

/// `TagType.getName()` for a type id, as `TagTypes.getType(id)` resolves it.
#[must_use]
pub fn type_name(type_id: i32) -> String {
    match type_id {
        0 => "END".to_owned(),
        1 => "BYTE".to_owned(),
        2 => "SHORT".to_owned(),
        3 => "INT".to_owned(),
        4 => "LONG".to_owned(),
        5 => "FLOAT".to_owned(),
        6 => "DOUBLE".to_owned(),
        7 => "BYTE[]".to_owned(),
        8 => "STRING".to_owned(),
        9 => "LIST".to_owned(),
        10 => "COMPOUND".to_owned(),
        11 => "INT[]".to_owned(),
        12 => "LONG[]".to_owned(),
        _ => format!("INVALID[{type_id}]"),
    }
}

/// `TagType.getPrettyName()` for a type id.
#[must_use]
pub fn pretty_type_name(type_id: i32) -> String {
    match type_id {
        0 => "TAG_End".to_owned(),
        1 => "TAG_Byte".to_owned(),
        2 => "TAG_Short".to_owned(),
        3 => "TAG_Int".to_owned(),
        4 => "TAG_Long".to_owned(),
        5 => "TAG_Float".to_owned(),
        6 => "TAG_Double".to_owned(),
        7 => "TAG_Byte_Array".to_owned(),
        8 => "TAG_String".to_owned(),
        9 => "TAG_List".to_owned(),
        10 => "TAG_Compound".to_owned(),
        11 => "TAG_Int_Array".to_owned(),
        12 => "TAG_Long_Array".to_owned(),
        _ => format!("UNKNOWN_{type_id}"),
    }
}

/// An NBT tag.
#[derive(Clone, Debug)]
pub enum Tag {
    /// `EndTag`.
    End,
    /// `ByteTag`.
    Byte(i8),
    /// `ShortTag`.
    Short(i16),
    /// `IntTag`.
    Int(i32),
    /// `LongTag`.
    Long(i64),
    /// `FloatTag`.
    Float(f32),
    /// `DoubleTag`.
    Double(f64),
    /// `ByteArrayTag`.
    ByteArray(Vec<i8>),
    /// `StringTag`.
    String(JString),
    /// `ListTag`.
    List(ListTag),
    /// `CompoundTag`.
    Compound(CompoundTag),
    /// `IntArrayTag`.
    IntArray(Vec<i32>),
    /// `LongArrayTag`.
    LongArray(Vec<i64>),
}

/// `Long.hashCode`.
fn long_hash(v: i64) -> i32 {
    (v ^ ((v as u64) >> 32) as i64) as i32
}

/// `Float.floatToIntBits`: the bits with every NaN made the canonical NaN.
fn float_to_int_bits(v: f32) -> i32 {
    if v.is_nan() { 0x7FC0_0000 } else { v.to_bits() as i32 }
}

/// `Double.doubleToLongBits`.
fn double_to_long_bits(v: f64) -> i64 {
    if v.is_nan() { 0x7FF8_0000_0000_0000 } else { v.to_bits() as i64 }
}

impl PartialEq for Tag {
    /// Java's `equals` on the tag classes.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::End, Self::End) => true,
            (Self::Byte(a), Self::Byte(b)) => a == b,
            (Self::Short(a), Self::Short(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Long(a), Self::Long(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => float_to_int_bits(*a) == float_to_int_bits(*b),
            (Self::Double(a), Self::Double(b)) => {
                double_to_long_bits(*a) == double_to_long_bits(*b)
            }
            (Self::ByteArray(a), Self::ByteArray(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::List(a), Self::List(b)) => a == b,
            (Self::Compound(a), Self::Compound(b)) => a == b,
            (Self::IntArray(a), Self::IntArray(b)) => a == b,
            (Self::LongArray(a), Self::LongArray(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Tag {}

impl Tag {
    /// The type id (`getId()`).
    #[must_use]
    pub fn id(&self) -> u8 {
        match self {
            Self::End => id::END,
            Self::Byte(_) => id::BYTE,
            Self::Short(_) => id::SHORT,
            Self::Int(_) => id::INT,
            Self::Long(_) => id::LONG,
            Self::Float(_) => id::FLOAT,
            Self::Double(_) => id::DOUBLE,
            Self::ByteArray(_) => id::BYTE_ARRAY,
            Self::String(_) => id::STRING,
            Self::List(_) => id::LIST,
            Self::Compound(_) => id::COMPOUND,
            Self::IntArray(_) => id::INT_ARRAY,
            Self::LongArray(_) => id::LONG_ARRAY,
        }
    }

    /// A string tag (`StringTag.valueOf`).
    #[must_use]
    pub fn string(value: impl Into<JString>) -> Self {
        Self::String(value.into())
    }

    /// `ByteTag.valueOf(boolean)`.
    #[must_use]
    pub fn boolean(value: bool) -> Self {
        Self::Byte(i8::from(value))
    }

    /// Java's `hashCode()`.
    ///
    /// `EndTag` does not override `hashCode`, so Java uses an identity hash that differs
    /// between runs; this returns 0 for it.
    #[must_use]
    pub fn java_hash_code(&self) -> i32 {
        fn array_hash(hashes: impl Iterator<Item = i32>) -> i32 {
            hashes.fold(1i32, |h, e| h.wrapping_mul(31).wrapping_add(e))
        }
        match self {
            Self::End => 0,
            Self::Byte(v) => i32::from(*v),
            Self::Short(v) => i32::from(*v),
            Self::Int(v) => *v,
            Self::Long(v) => long_hash(*v),
            Self::Float(v) => float_to_int_bits(*v),
            Self::Double(v) => long_hash(double_to_long_bits(*v)),
            Self::ByteArray(v) => array_hash(v.iter().map(|&b| i32::from(b))),
            Self::String(v) => v.hash_code(),
            Self::List(list) => array_hash(list.iter().map(Self::java_hash_code)),
            Self::Compound(compound) => compound.java_hash_code(),
            Self::IntArray(v) => array_hash(v.iter().copied()),
            Self::LongArray(v) => array_hash(v.iter().map(|&l| long_hash(l))),
        }
    }

    /// Vanilla's `copy()`: primitives and strings are shared, arrays are cloned, and lists
    /// and compounds are copied deeply (compounds entry by entry into new maps).
    #[must_use]
    pub fn copy(&self) -> Self {
        match self {
            Self::List(list) => Self::List(list.copy()),
            Self::Compound(compound) => Self::Compound(compound.copy()),
            other => other.clone(),
        }
    }

    /// `sizeInBytes()`: vanilla's estimate of the object's memory size.
    #[must_use]
    pub fn size_in_bytes(&self) -> i32 {
        let n = |len: usize| i32::try_from(len).unwrap_or(i32::MAX);
        match self {
            Self::End => 8,
            Self::Byte(_) => 9,
            Self::Short(_) => 10,
            Self::Int(_) | Self::Float(_) => 12,
            Self::Long(_) | Self::Double(_) => 16,
            Self::ByteArray(v) => 24i32.wrapping_add(n(v.len())),
            Self::String(s) => 36i32.wrapping_add(2i32.wrapping_mul(n(s.len()))),
            Self::List(list) => list.size_in_bytes(),
            Self::Compound(compound) => compound.size_in_bytes(),
            Self::IntArray(v) => 24i32.wrapping_add(4i32.wrapping_mul(n(v.len()))),
            Self::LongArray(v) => 24i32.wrapping_add(8i32.wrapping_mul(n(v.len()))),
        }
    }

    /// Whether this is a `NumericTag`.
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::Byte(_)
                | Self::Short(_)
                | Self::Int(_)
                | Self::Long(_)
                | Self::Float(_)
                | Self::Double(_)
        )
    }

    /// `NumericTag.longValue()` (Java's narrowing and widening conversions).
    #[must_use]
    pub fn as_long(&self) -> Option<i64> {
        Some(match *self {
            Self::Byte(v) => i64::from(v),
            Self::Short(v) => i64::from(v),
            Self::Int(v) => i64::from(v),
            Self::Long(v) => v,
            // `(long) f`: saturating, NaN to 0, as Rust's `as`.
            Self::Float(v) => v as i64,
            Self::Double(v) => v as i64,
            _ => return None,
        })
    }

    /// `NumericTag.intValue()`.
    #[must_use]
    pub fn as_int(&self) -> Option<i32> {
        Some(match *self {
            Self::Long(v) => v as i32,
            Self::Float(v) => v as i32,
            Self::Double(v) => v as i32,
            ref other => other.as_long()? as i32,
        })
    }

    /// `NumericTag.shortValue()`: `(short)` of the int value (floats saturate to int first).
    #[must_use]
    pub fn as_short(&self) -> Option<i16> {
        self.as_int().map(|v| v as i16)
    }

    /// `NumericTag.byteValue()`: `(byte)` of the int value (floats saturate to int first).
    #[must_use]
    pub fn as_byte(&self) -> Option<i8> {
        self.as_int().map(|v| v as i8)
    }

    /// `NumericTag.floatValue()`.
    #[must_use]
    pub fn as_float(&self) -> Option<f32> {
        Some(match *self {
            Self::Byte(v) => f32::from(v),
            Self::Short(v) => f32::from(v),
            Self::Int(v) => v as f32,
            Self::Long(v) => v as f32,
            Self::Float(v) => v,
            Self::Double(v) => v as f32,
            _ => return None,
        })
    }

    /// `NumericTag.doubleValue()`.
    #[must_use]
    pub fn as_double(&self) -> Option<f64> {
        Some(match *self {
            Self::Byte(v) => f64::from(v),
            Self::Short(v) => f64::from(v),
            Self::Int(v) => f64::from(v),
            Self::Long(v) => v as f64,
            Self::Float(v) => f64::from(v),
            Self::Double(v) => v,
            _ => return None,
        })
    }

    /// `Tag.asBoolean()`: a number whose byte value is not 0.
    #[must_use]
    pub fn as_boolean(&self) -> Option<bool> {
        self.as_byte().map(|b| b != 0)
    }

    /// `Tag.asString()`.
    #[must_use]
    pub fn as_string(&self) -> Option<&JString> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// `Tag.asCompound()`.
    #[must_use]
    pub fn as_compound(&self) -> Option<&CompoundTag> {
        match self {
            Self::Compound(c) => Some(c),
            _ => None,
        }
    }

    /// `Tag.asList()`.
    #[must_use]
    pub fn as_list(&self) -> Option<&ListTag> {
        match self {
            Self::List(l) => Some(l),
            _ => None,
        }
    }
}

impl From<CompoundTag> for Tag {
    fn from(value: CompoundTag) -> Self {
        Self::Compound(value)
    }
}

impl From<ListTag> for Tag {
    fn from(value: ListTag) -> Self {
        Self::List(value)
    }
}

/// `ListTag`: a list of tags. Since 1.21.5 the elements may have different types; such a list
/// is written as a list of compounds, each wrapping its element under the key `""`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListTag {
    list: Vec<Tag>,
}

impl ListTag {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A list of the given elements.
    #[must_use]
    pub fn from_vec(list: Vec<Tag>) -> Self {
        Self { list }
    }

    /// The number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// The elements.
    pub fn iter(&self) -> std::slice::Iter<'_, Tag> {
        self.list.iter()
    }

    /// The elements, mutably.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Tag> {
        self.list.iter_mut()
    }

    /// The element at `index`, if any (`getNullable`).
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Tag> {
        self.list.get(index)
    }

    /// Appends `tag` (`add`).
    pub fn push(&mut self, tag: Tag) {
        self.list.push(tag);
    }

    /// Inserts `tag` at `index` (`add(index, tag)`).
    ///
    /// # Panics
    ///
    /// If `index > len()`.
    pub fn insert(&mut self, index: usize, tag: Tag) {
        self.list.insert(index, tag);
    }

    /// Replaces the element at `index` (`set`), returning the old one.
    ///
    /// # Panics
    ///
    /// If `index` is out of bounds.
    pub fn set(&mut self, index: usize, tag: Tag) -> Tag {
        std::mem::replace(&mut self.list[index], tag)
    }

    /// Removes the element at `index`.
    ///
    /// # Panics
    ///
    /// If `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> Tag {
        self.list.remove(index)
    }

    /// Removes every element.
    pub fn clear(&mut self) {
        self.list.clear();
    }

    /// `addAndUnwrap`: appends `tag`, unwrapping a compound whose only key is `""`.
    pub fn add_and_unwrap(&mut self, tag: Tag) {
        match tag {
            Tag::Compound(compound) => self.list.push(try_unwrap(compound)),
            other => self.list.push(other),
        }
    }

    /// `identifyRawElementType`: the shared element type id, `COMPOUND` for a mixed list, or
    /// `END` for an empty one.
    #[must_use]
    pub fn identify_raw_element_type(&self) -> u8 {
        let mut homogeneous = id::END;
        for element in &self.list {
            let element_type = element.id();
            if homogeneous == id::END {
                homogeneous = element_type;
            } else if homogeneous != element_type {
                return id::COMPOUND;
            }
        }
        homogeneous
    }

    /// `copy()`.
    #[must_use]
    pub fn copy(&self) -> Self {
        Self {
            list: self.list.iter().map(Tag::copy).collect(),
        }
    }

    /// `sizeInBytes()`.
    #[must_use]
    pub fn size_in_bytes(&self) -> i32 {
        let len = i32::try_from(self.list.len()).unwrap_or(i32::MAX);
        let base = 36i32.wrapping_add(4i32.wrapping_mul(len));
        self.list
            .iter()
            .fold(base, |size, tag| size.wrapping_add(tag.size_in_bytes()))
    }

    /// The compound at `index` (`getCompound`).
    #[must_use]
    pub fn get_compound(&self, index: usize) -> Option<&CompoundTag> {
        self.get(index).and_then(Tag::as_compound)
    }

    /// The list at `index` (`getList`).
    #[must_use]
    pub fn get_list(&self, index: usize) -> Option<&ListTag> {
        self.get(index).and_then(Tag::as_list)
    }

    /// The string at `index` (`getString`).
    #[must_use]
    pub fn get_string(&self, index: usize) -> Option<&JString> {
        self.get(index).and_then(Tag::as_string)
    }

    /// The number at `index` as an int (`getInt` / `getIntOr`).
    #[must_use]
    pub fn get_int(&self, index: usize) -> Option<i32> {
        self.get(index).and_then(Tag::as_int)
    }

    /// The number at `index` as a double (`getDouble` / `getDoubleOr`).
    #[must_use]
    pub fn get_double(&self, index: usize) -> Option<f64> {
        self.get(index).and_then(Tag::as_double)
    }

    /// The compounds among the elements, in order (`compoundStream`).
    pub fn compounds(&self) -> impl Iterator<Item = &CompoundTag> {
        self.list.iter().filter_map(Tag::as_compound)
    }

    /// The elements as a vector.
    #[must_use]
    pub fn into_vec(self) -> Vec<Tag> {
        self.list
    }
}

impl<'a> IntoIterator for &'a ListTag {
    type Item = &'a Tag;
    type IntoIter = std::slice::Iter<'a, Tag>;

    fn into_iter(self) -> Self::IntoIter {
        self.list.iter()
    }
}

impl FromIterator<Tag> for ListTag {
    fn from_iter<I: IntoIterator<Item = Tag>>(iter: I) -> Self {
        Self {
            list: iter.into_iter().collect(),
        }
    }
}

/// `ListTag.tryUnwrap`: the element of a compound whose only key is `""`, else the compound.
fn try_unwrap(compound: CompoundTag) -> Tag {
    if compound.len() == 1
        && let Some(value) = compound.get("")
    {
        return value.clone();
    }
    Tag::Compound(compound)
}

/// `ListTag.isWrapper`.
fn is_wrapper(compound: &CompoundTag) -> bool {
    compound.len() == 1 && compound.contains("")
}

/// `ListTag.wrapIfNeeded`: in a list written as compounds, an element that is not a
/// compound, or that looks like a wrapper, is wrapped under the key `""`.
pub(crate) fn wrap_if_needed(element_type: u8, tag: &Tag) -> std::borrow::Cow<'_, Tag> {
    use std::borrow::Cow;
    if element_type != id::COMPOUND {
        return Cow::Borrowed(tag);
    }
    match tag {
        Tag::Compound(compound) if !is_wrapper(compound) => Cow::Borrowed(tag),
        _ => {
            // `new CompoundTag(Map.of("", tag))`: a one-entry map.
            let mut wrapper = CompoundTag::new();
            wrapper.put("", tag.clone());
            Cow::Owned(Tag::Compound(wrapper))
        }
    }
}

/// `CompoundTag`: named tags in a `java.util.HashMap`, whose iteration order is the order
/// entries are written in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompoundTag {
    tags: JavaHashMap<JString, Tag>,
}

impl CompoundTag {
    /// An empty compound (`new CompoundTag()`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A compound over an existing map.
    #[must_use]
    pub fn from_map(tags: JavaHashMap<JString, Tag>) -> Self {
        Self { tags }
    }

    /// The underlying map.
    #[must_use]
    pub fn map(&self) -> &JavaHashMap<JString, Tag> {
        &self.tags
    }

    /// The number of entries (`size()`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.tags.len()
    }

    /// Whether there are no entries (`isEmpty()`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// The entries in iteration order (`entrySet()`).
    pub fn iter(&self) -> cerium_java::hash_map::Iter<'_, JString, Tag> {
        self.tags.iter()
    }

    /// The keys in iteration order (`keySet()`).
    pub fn keys(&self) -> impl Iterator<Item = &JString> {
        self.tags.keys()
    }

    /// `put(name, tag)`: returns the previous tag.
    pub fn put(&mut self, name: impl Into<JString>, tag: Tag) -> Option<Tag> {
        self.tags.insert(name.into(), tag)
    }

    /// `putByte`.
    pub fn put_byte(&mut self, name: impl Into<JString>, value: i8) {
        self.put(name, Tag::Byte(value));
    }

    /// `putShort`.
    pub fn put_short(&mut self, name: impl Into<JString>, value: i16) {
        self.put(name, Tag::Short(value));
    }

    /// `putInt`.
    pub fn put_int(&mut self, name: impl Into<JString>, value: i32) {
        self.put(name, Tag::Int(value));
    }

    /// `putLong`.
    pub fn put_long(&mut self, name: impl Into<JString>, value: i64) {
        self.put(name, Tag::Long(value));
    }

    /// `putFloat`.
    pub fn put_float(&mut self, name: impl Into<JString>, value: f32) {
        self.put(name, Tag::Float(value));
    }

    /// `putDouble`.
    pub fn put_double(&mut self, name: impl Into<JString>, value: f64) {
        self.put(name, Tag::Double(value));
    }

    /// `putString`.
    pub fn put_string(&mut self, name: impl Into<JString>, value: impl Into<JString>) {
        self.put(name, Tag::String(value.into()));
    }

    /// `putBoolean`.
    pub fn put_boolean(&mut self, name: impl Into<JString>, value: bool) {
        self.put(name, Tag::boolean(value));
    }

    /// The tag under `name` (`get`).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Tag> {
        self.tags.get(name)
    }

    /// The tag under `name`, with a Java string key.
    #[must_use]
    pub fn get_j(&self, name: &JString) -> Option<&Tag> {
        self.tags.get(name)
    }

    /// The tag under `name`, mutably.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Tag> {
        self.tags.get_mut(name)
    }

    /// Whether `name` is present (`contains`).
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.tags.contains_key(name)
    }

    /// `remove(name)`.
    pub fn remove(&mut self, name: &str) -> Option<Tag> {
        self.tags.remove(name)
    }

    /// `getByte(name)`: a number's byte value.
    #[must_use]
    pub fn get_byte(&self, name: &str) -> Option<i8> {
        self.get(name).and_then(Tag::as_byte)
    }

    /// `getShort(name)`.
    #[must_use]
    pub fn get_short(&self, name: &str) -> Option<i16> {
        self.get(name).and_then(Tag::as_short)
    }

    /// `getInt(name)`.
    #[must_use]
    pub fn get_int(&self, name: &str) -> Option<i32> {
        self.get(name).and_then(Tag::as_int)
    }

    /// `getLong(name)`.
    #[must_use]
    pub fn get_long(&self, name: &str) -> Option<i64> {
        self.get(name).and_then(Tag::as_long)
    }

    /// `getFloat(name)`.
    #[must_use]
    pub fn get_float(&self, name: &str) -> Option<f32> {
        self.get(name).and_then(Tag::as_float)
    }

    /// `getDouble(name)`.
    #[must_use]
    pub fn get_double(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(Tag::as_double)
    }

    /// `getBoolean(name)`.
    #[must_use]
    pub fn get_boolean(&self, name: &str) -> Option<bool> {
        self.get(name).and_then(Tag::as_boolean)
    }

    /// `getString(name)`.
    #[must_use]
    pub fn get_string(&self, name: &str) -> Option<&JString> {
        self.get(name).and_then(Tag::as_string)
    }

    /// `getCompound(name)`.
    #[must_use]
    pub fn get_compound(&self, name: &str) -> Option<&CompoundTag> {
        self.get(name).and_then(Tag::as_compound)
    }

    /// `getList(name)`.
    #[must_use]
    pub fn get_list(&self, name: &str) -> Option<&ListTag> {
        self.get(name).and_then(Tag::as_list)
    }

    /// `getByteArray(name)`.
    #[must_use]
    pub fn get_byte_array(&self, name: &str) -> Option<&[i8]> {
        match self.get(name)? {
            Tag::ByteArray(v) => Some(v),
            _ => None,
        }
    }

    /// `getIntArray(name)`.
    #[must_use]
    pub fn get_int_array(&self, name: &str) -> Option<&[i32]> {
        match self.get(name)? {
            Tag::IntArray(v) => Some(v),
            _ => None,
        }
    }

    /// `getLongArray(name)`.
    #[must_use]
    pub fn get_long_array(&self, name: &str) -> Option<&[i64]> {
        match self.get(name)? {
            Tag::LongArray(v) => Some(v),
            _ => None,
        }
    }

    /// `copy()`: a new map, filled by putting a copy of each entry in iteration order.
    #[must_use]
    pub fn copy(&self) -> Self {
        let mut tags = JavaHashMap::new();
        for (key, tag) in &self.tags {
            tags.insert(key.clone(), tag.copy());
        }
        Self { tags }
    }

    /// `shallowCopy()`: `new HashMap<>(tags)`, which presizes the table.
    #[must_use]
    pub fn shallow_copy(&self) -> Self {
        Self {
            tags: JavaHashMap::from_map(&self.tags),
        }
    }

    /// `merge(other)`: puts copies of `other`'s entries, merging compounds into compounds.
    pub fn merge(&mut self, other: &CompoundTag) -> &mut Self {
        for (name, other_tag) in &other.tags {
            if let Tag::Compound(other_compound) = other_tag
                && let Some(Tag::Compound(own)) = self.tags.get_mut(name)
            {
                own.merge(other_compound);
                continue;
            }
            self.tags.insert(name.clone(), other_tag.copy());
        }
        self
    }

    /// `sizeInBytes()`.
    #[must_use]
    pub fn size_in_bytes(&self) -> i32 {
        self.tags.iter().fold(48i32, |size, (key, tag)| {
            let key_len = i32::try_from(key.len()).unwrap_or(i32::MAX);
            size.wrapping_add(28)
                .wrapping_add(2i32.wrapping_mul(key_len))
                .wrapping_add(36)
                .wrapping_add(tag.size_in_bytes())
        })
    }

    /// `hashCode()`: the map's hash code.
    #[must_use]
    pub fn java_hash_code(&self) -> i32 {
        self.tags.java_hash_code(Tag::java_hash_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_follows_java() {
        assert_eq!(Tag::Float(f32::NAN), Tag::Float(f32::from_bits(0x7FC0_0001)));
        assert_ne!(Tag::Double(0.0), Tag::Double(-0.0));
        assert_ne!(Tag::Int(1), Tag::Long(1));
    }

    #[test]
    fn numbers_convert_like_java() {
        assert_eq!(Tag::Double(1e10).as_int(), Some(i32::MAX));
        assert_eq!(Tag::Double(300.7).as_byte(), Some(44));
        assert_eq!(Tag::Long(0x1_0000_0001).as_int(), Some(1));
        assert_eq!(Tag::Float(f32::NAN).as_long(), Some(0));
        assert_eq!(Tag::string("x").as_int(), None);
    }

    #[test]
    fn hash_codes_follow_java() {
        assert_eq!(Tag::Long(-1).java_hash_code(), 0);
        assert_eq!(Tag::Float(1.0).java_hash_code(), 0x3F80_0000);
        assert_eq!(Tag::IntArray(vec![1, 2]).java_hash_code(), (31 + 1) * 31 + 2);
        let mut compound = CompoundTag::new();
        compound.put_int("a", 5);
        assert_eq!(compound.java_hash_code(), 97 ^ 5);
    }

    #[test]
    fn lists_unwrap_and_identify_types() {
        let mut list = ListTag::new();
        assert_eq!(list.identify_raw_element_type(), id::END);
        let mut wrapper = CompoundTag::new();
        wrapper.put("", Tag::Int(3));
        list.add_and_unwrap(Tag::Compound(wrapper));
        assert_eq!(list.get(0), Some(&Tag::Int(3)));
        list.push(Tag::string("s"));
        assert_eq!(list.identify_raw_element_type(), id::COMPOUND);
    }

    #[test]
    fn sizes_follow_vanilla() {
        let mut compound = CompoundTag::new();
        compound.put_string("ab", "xyz");
        assert_eq!(compound.size_in_bytes(), 48 + 28 + 4 + 36 + (36 + 6));
        assert_eq!(ListTag::from_vec(vec![Tag::Byte(1)]).size_in_bytes(), 36 + 4 + 9);
    }

    #[test]
    fn merge_combines_nested_compounds() {
        let mut inner = CompoundTag::new();
        inner.put_int("a", 1);
        let mut target = CompoundTag::new();
        target.put("c", Tag::Compound(inner));
        let mut other_inner = CompoundTag::new();
        other_inner.put_int("b", 2);
        let mut other = CompoundTag::new();
        other.put("c", Tag::Compound(other_inner));
        target.merge(&other);
        let merged = target.get_compound("c").unwrap();
        assert_eq!((merged.get_int("a"), merged.get_int("b")), (Some(1), Some(2)));
    }
}
