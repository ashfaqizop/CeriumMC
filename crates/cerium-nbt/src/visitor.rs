//! Streaming NBT: `StreamTagVisitor`, each type's `parse` and `skip`, `Tag.accept`, and
//! vanilla's visitors (`SkipAll`, `CollectToTag`, `CollectFields`, `SkipFields`).
//!
//! Vanilla uses these to read only some fields of large files, such as a chunk's status,
//! without building the whole tree. The control flow is transcribed from the bytecode
//! (`javap`), including its quirks: a `BREAK` returned while parsing a compound's value only
//! ends that value, and the quota is charged only for the parts actually parsed.
//!
//! Type ids are `i8`s as read; ids outside 0..=12 are `TagTypes.getType`'s invalid types,
//! which throw `IOException("Invalid tag id: ...")` when parsed or skipped.

use std::collections::HashMap;

use cerium_java::JString;

use crate::data::DataInput;
use crate::io::skip_string;
use crate::tag::{CompoundTag, ListTag, Tag, id, wrap_if_needed};
use crate::{NbtAccounter, NbtError};

/// `StreamTagVisitor.ValueResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueResult {
    /// Go on.
    Continue,
    /// Stop the current container.
    Break,
    /// Stop everything.
    Halt,
}

/// `StreamTagVisitor.EntryResult`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryResult {
    /// Visit the entry.
    Enter,
    /// Skip the entry.
    Skip,
    /// Skip the rest of the container.
    Break,
    /// Stop everything.
    Halt,
}

/// `StreamTagVisitor`: receives a tag's contents as they are read.
#[allow(missing_docs, reason = "each method is the Java method of the same name")]
pub trait StreamTagVisitor {
    fn visit_end(&mut self) -> ValueResult;
    fn visit_string(&mut self, value: &JString) -> ValueResult;
    fn visit_byte(&mut self, value: i8) -> ValueResult;
    fn visit_short(&mut self, value: i16) -> ValueResult;
    fn visit_int(&mut self, value: i32) -> ValueResult;
    fn visit_long(&mut self, value: i64) -> ValueResult;
    fn visit_float(&mut self, value: f32) -> ValueResult;
    fn visit_double(&mut self, value: f64) -> ValueResult;
    fn visit_byte_array(&mut self, value: &[i8]) -> ValueResult;
    fn visit_int_array(&mut self, value: &[i32]) -> ValueResult;
    fn visit_long_array(&mut self, value: &[i64]) -> ValueResult;
    /// `visitList(elementType, size)`.
    fn visit_list(&mut self, element_type: i8, size: i32) -> ValueResult;
    /// `visitEntry(type)`: a compound entry's type, before its name is read.
    fn visit_entry_type(&mut self, type_id: i8) -> EntryResult;
    /// `visitEntry(type, id)`: a compound entry's type and name.
    fn visit_entry(&mut self, type_id: i8, name: &JString) -> EntryResult;
    /// `visitElement(type, index)`.
    fn visit_element(&mut self, type_id: i8, index: i32) -> EntryResult;
    fn visit_container_end(&mut self) -> ValueResult;
    /// `visitRootEntry(type)`.
    fn visit_root_entry(&mut self, type_id: i8) -> ValueResult;
}

/// The fixed payload size of the `StaticSize` types.
fn static_size(type_id: i8) -> Option<i32> {
    match type_id as u8 {
        id::BYTE => Some(1),
        id::SHORT => Some(2),
        id::INT | id::FLOAT => Some(4),
        id::LONG | id::DOUBLE => Some(8),
        _ => None,
    }
}

fn is_valid(type_id: i8) -> bool {
    (0..=12).contains(&type_id)
}

/// `TagType.skip(input, accounter)`.
///
/// # Errors
///
/// As vanilla (end of input, depth limit, invalid type ids, Netty's negative skips).
pub fn skip(
    type_id: i8,
    input: &mut impl DataInput,
    accounter: &mut NbtAccounter,
) -> Result<(), NbtError> {
    if let Some(size) = static_size(type_id) {
        input.skip_bytes(size)?;
        return Ok(());
    }
    match type_id as u8 {
        id::END => Ok(()),
        id::BYTE_ARRAY => skip_counted(input, 1),
        id::INT_ARRAY => skip_counted(input, 4),
        id::LONG_ARRAY => skip_counted(input, 8),
        id::STRING => skip_string(input),
        id::LIST => {
            accounter.push_depth()?;
            let body = (|| {
                let element_type = input.read_byte()?;
                let count = input.read_int()?;
                skip_n(element_type, input, count, accounter)
            })();
            let popped = accounter.pop_depth();
            body.and(popped)
        }
        id::COMPOUND => {
            accounter.push_depth()?;
            let body = skip_entries(input, accounter);
            let popped = accounter.pop_depth();
            body.and(popped)
        }
        _ => Err(NbtError::InvalidTagId(i32::from(type_id))),
    }
}

/// `skipBytes(readInt() * elementSize)`, with Java's wrapping multiplication.
fn skip_counted(input: &mut impl DataInput, element_size: i32) -> Result<(), NbtError> {
    let length = input.read_int()?;
    input.skip_bytes(length.wrapping_mul(element_size))?;
    Ok(())
}

/// Skips the entries of a compound up to and including its end byte.
fn skip_entries(input: &mut impl DataInput, accounter: &mut NbtAccounter) -> Result<(), NbtError> {
    loop {
        let type_id = input.read_byte()?;
        if type_id == 0 {
            return Ok(());
        }
        skip_string(input)?;
        skip(type_id, input, accounter)?;
    }
}

/// `TagType.skip(input, count, accounter)`: one multiplied `skipBytes` for the static types,
/// else `count` single skips (none for a negative count).
///
/// # Errors
///
/// As [`skip`].
pub fn skip_n(
    type_id: i8,
    input: &mut impl DataInput,
    count: i32,
    accounter: &mut NbtAccounter,
) -> Result<(), NbtError> {
    if let Some(size) = static_size(type_id) {
        input.skip_bytes(size.wrapping_mul(count))?;
        return Ok(());
    }
    if type_id == 0 {
        return Ok(());
    }
    if !is_valid(type_id) {
        return Err(NbtError::InvalidTagId(i32::from(type_id)));
    }
    for _ in 0..count {
        skip(type_id, input, accounter)?;
    }
    Ok(())
}

/// `TagType.parse(input, visitor, accounter)`.
///
/// # Errors
///
/// As vanilla. Unlike loading, parsing never wraps exceptions in a `ReportedNbtException`.
pub fn parse(
    type_id: i8,
    input: &mut impl DataInput,
    visitor: &mut (impl StreamTagVisitor + ?Sized),
    accounter: &mut NbtAccounter,
) -> Result<ValueResult, NbtError> {
    Ok(match type_id as u8 {
        id::END => {
            accounter.account_bytes(8)?;
            visitor.visit_end()
        }
        id::BYTE => {
            accounter.account_bytes(9)?;
            visitor.visit_byte(input.read_byte()?)
        }
        id::SHORT => {
            accounter.account_bytes(10)?;
            visitor.visit_short(input.read_short()?)
        }
        id::INT => {
            accounter.account_bytes(12)?;
            visitor.visit_int(input.read_int()?)
        }
        id::LONG => {
            accounter.account_bytes(16)?;
            visitor.visit_long(input.read_long()?)
        }
        id::FLOAT => {
            accounter.account_bytes(12)?;
            visitor.visit_float(input.read_float()?)
        }
        id::DOUBLE => {
            accounter.account_bytes(16)?;
            visitor.visit_double(input.read_double()?)
        }
        id::BYTE_ARRAY => {
            accounter.account_bytes(24)?;
            let length = input.read_int()?;
            accounter.account_bytes_n(1, i64::from(length))?;
            let bytes: Vec<i8> = input
                .read_fully(length as usize)?
                .into_iter()
                .map(|b| b as i8)
                .collect();
            visitor.visit_byte_array(&bytes)
        }
        id::STRING => {
            accounter.account_bytes(36)?;
            let data = input.read_utf()?;
            accounter.account_bytes_n(2, data.len() as i64)?;
            visitor.visit_string(&data)
        }
        id::LIST => {
            accounter.push_depth()?;
            let body = parse_list(input, visitor, accounter);
            let popped = accounter.pop_depth();
            let result = body?;
            popped?;
            result
        }
        id::COMPOUND => {
            accounter.push_depth()?;
            let body = parse_compound(input, visitor, accounter);
            let popped = accounter.pop_depth();
            let result = body?;
            popped?;
            result
        }
        id::INT_ARRAY => {
            accounter.account_bytes(24)?;
            let length = input.read_int()?;
            accounter.account_bytes_n(4, i64::from(length))?;
            let mut data = Vec::new();
            for _ in 0..length {
                data.push(input.read_int()?);
            }
            visitor.visit_int_array(&data)
        }
        id::LONG_ARRAY => {
            accounter.account_bytes(24)?;
            let length = input.read_int()?;
            accounter.account_bytes_n(8, i64::from(length))?;
            let mut data = Vec::new();
            for _ in 0..length {
                data.push(input.read_long()?);
            }
            visitor.visit_long_array(&data)
        }
        _ => return Err(NbtError::InvalidTagId(i32::from(type_id))),
    })
}

/// `ListTag.TYPE.parseList`.
fn parse_list(
    input: &mut impl DataInput,
    visitor: &mut (impl StreamTagVisitor + ?Sized),
    accounter: &mut NbtAccounter,
) -> Result<ValueResult, NbtError> {
    accounter.account_bytes(36)?;
    let element_type = input.read_byte()?;
    let count = input.read_int()?;
    if count < 0 {
        return Err(NbtError::Format(format!(
            "ListTag length cannot be negative: {count}"
        )));
    }
    match visitor.visit_list(element_type, count) {
        ValueResult::Halt => return Ok(ValueResult::Halt),
        ValueResult::Break => {
            skip_n(element_type, input, count, accounter)?;
            return Ok(visitor.visit_container_end());
        }
        ValueResult::Continue => {}
    }
    accounter.account_bytes_n(4, i64::from(count))?;
    let mut i = 0;
    while i < count {
        match visitor.visit_element(element_type, i) {
            EntryResult::Halt => return Ok(ValueResult::Halt),
            EntryResult::Break => {
                skip(element_type, input, accounter)?;
                break;
            }
            EntryResult::Skip => skip(element_type, input, accounter)?,
            EntryResult::Enter => match parse(element_type, input, visitor, accounter)? {
                ValueResult::Halt => return Ok(ValueResult::Halt),
                ValueResult::Break => break,
                ValueResult::Continue => {}
            },
        }
        i += 1;
    }
    let amount_to_skip = count - 1 - i;
    if amount_to_skip > 0 {
        skip_n(element_type, input, amount_to_skip, accounter)?;
    }
    Ok(visitor.visit_container_end())
}

/// `CompoundTag.TYPE.parseCompound`.
fn parse_compound(
    input: &mut impl DataInput,
    visitor: &mut (impl StreamTagVisitor + ?Sized),
    accounter: &mut NbtAccounter,
) -> Result<ValueResult, NbtError> {
    accounter.account_bytes(48)?;
    let mut type_id;
    loop {
        type_id = input.read_byte()?;
        if type_id == 0 {
            break;
        }
        match visitor.visit_entry_type(type_id) {
            EntryResult::Halt => return Ok(ValueResult::Halt),
            EntryResult::Break => {
                skip_string(input)?;
                skip(type_id, input, accounter)?;
                break;
            }
            EntryResult::Skip => {
                skip_string(input)?;
                skip(type_id, input, accounter)?;
                continue;
            }
            EntryResult::Enter => {}
        }
        let key = input.read_utf()?;
        accounter.account_bytes(28)?;
        accounter.account_bytes_n(2, key.len() as i64)?;
        match visitor.visit_entry(type_id, &key) {
            EntryResult::Halt => return Ok(ValueResult::Halt),
            EntryResult::Break => {
                skip(type_id, input, accounter)?;
                break;
            }
            EntryResult::Skip => skip(type_id, input, accounter)?,
            EntryResult::Enter => {
                accounter.account_bytes(36)?;
                // A BREAK from the value only ends that value (bytecode: `goto` the loop).
                if parse(type_id, input, visitor, accounter)? == ValueResult::Halt {
                    return Ok(ValueResult::Halt);
                }
            }
        }
    }
    if type_id != 0 {
        skip_entries(input, accounter)?;
    }
    Ok(visitor.visit_container_end())
}

/// `TagType.parseRoot`: a root payload whose type is already known.
///
/// # Errors
///
/// As [`parse`].
pub fn parse_root(
    type_id: i8,
    input: &mut impl DataInput,
    visitor: &mut (impl StreamTagVisitor + ?Sized),
    accounter: &mut NbtAccounter,
) -> Result<(), NbtError> {
    match visitor.visit_root_entry(type_id) {
        ValueResult::Continue => {
            parse(type_id, input, visitor, accounter)?;
        }
        ValueResult::Halt => {}
        ValueResult::Break => skip(type_id, input, accounter)?,
    }
    Ok(())
}

/// `NbtIo.parse`: a named root tag, as in files.
///
/// # Errors
///
/// As [`parse`].
pub fn parse_named_root(
    input: &mut impl DataInput,
    visitor: &mut (impl StreamTagVisitor + ?Sized),
    accounter: &mut NbtAccounter,
) -> Result<(), NbtError> {
    let type_id = input.read_byte()?;
    if type_id == 0 {
        if visitor.visit_root_entry(0) == ValueResult::Continue {
            visitor.visit_end();
        }
        return Ok(());
    }
    match visitor.visit_root_entry(type_id) {
        ValueResult::Halt => {}
        ValueResult::Break => {
            skip_string(input)?;
            skip(type_id, input, accounter)?;
        }
        ValueResult::Continue => {
            skip_string(input)?;
            parse(type_id, input, visitor, accounter)?;
        }
    }
    Ok(())
}

/// `Tag.accept(StreamTagVisitor)`: walks an in-memory tag. Lists are walked in their written
/// form, so the elements of a mixed list arrive wrapped in compounds.
pub fn accept(tag: &Tag, visitor: &mut (impl StreamTagVisitor + ?Sized)) -> ValueResult {
    match tag {
        Tag::End => visitor.visit_end(),
        Tag::Byte(v) => visitor.visit_byte(*v),
        Tag::Short(v) => visitor.visit_short(*v),
        Tag::Int(v) => visitor.visit_int(*v),
        Tag::Long(v) => visitor.visit_long(*v),
        Tag::Float(v) => visitor.visit_float(*v),
        Tag::Double(v) => visitor.visit_double(*v),
        Tag::ByteArray(v) => visitor.visit_byte_array(v),
        Tag::String(v) => visitor.visit_string(v),
        Tag::IntArray(v) => visitor.visit_int_array(v),
        Tag::LongArray(v) => visitor.visit_long_array(v),
        Tag::List(list) => accept_list(list, visitor),
        Tag::Compound(compound) => accept_compound(compound, visitor),
    }
}

fn accept_list(list: &ListTag, visitor: &mut (impl StreamTagVisitor + ?Sized)) -> ValueResult {
    let element_type = list.identify_raw_element_type();
    let size = i32::try_from(list.len()).expect("Java list size");
    match visitor.visit_list(element_type as i8, size) {
        ValueResult::Halt => return ValueResult::Halt,
        ValueResult::Break => return visitor.visit_container_end(),
        ValueResult::Continue => {}
    }
    for (i, element) in list.iter().enumerate() {
        let tag = wrap_if_needed(element_type, element);
        match visitor.visit_element(tag.id() as i8, i as i32) {
            EntryResult::Halt => return ValueResult::Halt,
            EntryResult::Break => return visitor.visit_container_end(),
            EntryResult::Skip => {}
            EntryResult::Enter => match accept(&tag, visitor) {
                ValueResult::Halt => return ValueResult::Halt,
                ValueResult::Break => return visitor.visit_container_end(),
                ValueResult::Continue => {}
            },
        }
    }
    visitor.visit_container_end()
}

fn accept_compound(
    compound: &CompoundTag,
    visitor: &mut (impl StreamTagVisitor + ?Sized),
) -> ValueResult {
    for (key, value) in compound.iter() {
        let type_id = value.id() as i8;
        match visitor.visit_entry_type(type_id) {
            EntryResult::Halt => return ValueResult::Halt,
            EntryResult::Break => return visitor.visit_container_end(),
            EntryResult::Skip => continue,
            EntryResult::Enter => {}
        }
        match visitor.visit_entry(type_id, key) {
            EntryResult::Halt => return ValueResult::Halt,
            EntryResult::Break => return visitor.visit_container_end(),
            EntryResult::Skip => continue,
            EntryResult::Enter => {}
        }
        match accept(value, visitor) {
            ValueResult::Halt => return ValueResult::Halt,
            ValueResult::Break => return visitor.visit_container_end(),
            ValueResult::Continue => {}
        }
    }
    visitor.visit_container_end()
}

/// `Tag.acceptAsRoot`.
pub fn accept_as_root(tag: &Tag, visitor: &mut (impl StreamTagVisitor + ?Sized)) {
    if visitor.visit_root_entry(tag.id() as i8) == ValueResult::Continue {
        accept(tag, visitor);
    }
}

/// `SkipAll`: continues through containers but skips every entry and element.
#[derive(Clone, Copy, Debug, Default)]
pub struct SkipAll;

impl StreamTagVisitor for SkipAll {
    fn visit_end(&mut self) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_string(&mut self, _: &JString) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_byte(&mut self, _: i8) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_short(&mut self, _: i16) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_int(&mut self, _: i32) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_long(&mut self, _: i64) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_float(&mut self, _: f32) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_double(&mut self, _: f64) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_byte_array(&mut self, _: &[i8]) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_int_array(&mut self, _: &[i32]) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_long_array(&mut self, _: &[i64]) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_list(&mut self, _: i8, _: i32) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_entry_type(&mut self, _: i8) -> EntryResult {
        EntryResult::Skip
    }
    fn visit_entry(&mut self, _: i8, _: &JString) -> EntryResult {
        EntryResult::Skip
    }
    fn visit_element(&mut self, _: i8, _: i32) -> EntryResult {
        EntryResult::Skip
    }
    fn visit_container_end(&mut self) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_root_entry(&mut self, _: i8) -> ValueResult {
        ValueResult::Continue
    }
}

/// A container `CollectToTag` is building.
#[derive(Debug)]
enum Builder {
    Root(Option<Tag>),
    List(ListTag),
    Compound { compound: CompoundTag, last_id: JString },
}

impl Builder {
    fn accept_value(&mut self, tag: Tag) {
        match self {
            Self::Root(result) => *result = Some(tag),
            Self::List(list) => list.add_and_unwrap(tag),
            Self::Compound { compound, last_id } => {
                compound.put(last_id.clone(), tag);
            }
        }
    }

    fn build(self) -> Option<Tag> {
        match self {
            Self::Root(result) => result,
            Self::List(list) => Some(Tag::List(list)),
            Self::Compound { compound, .. } => Some(Tag::Compound(compound)),
        }
    }
}

/// `CollectToTag`: builds the visited tag in memory.
#[derive(Debug)]
pub struct CollectToTag {
    stack: Vec<Builder>,
}

impl Default for CollectToTag {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectToTag {
    /// A collector with nothing collected yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: vec![Builder::Root(None)],
        }
    }

    /// The collected tag (`getResult`).
    #[must_use]
    pub fn result(self) -> Option<Tag> {
        self.stack.into_iter().next().and_then(Builder::build)
    }

    /// `depth()`: the number of open containers.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len() - 1
    }

    fn append(&mut self, tag: Tag) -> ValueResult {
        self.stack.last_mut().expect("root").accept_value(tag);
        ValueResult::Continue
    }

    fn enter_container_if_needed(&mut self, type_id: i8) {
        match type_id as u8 {
            id::LIST => self.stack.push(Builder::List(ListTag::new())),
            id::COMPOUND => self.stack.push(Builder::Compound {
                compound: CompoundTag::new(),
                last_id: JString::new(),
            }),
            _ => {}
        }
    }
}

impl StreamTagVisitor for CollectToTag {
    fn visit_end(&mut self) -> ValueResult {
        self.append(Tag::End)
    }
    fn visit_string(&mut self, value: &JString) -> ValueResult {
        self.append(Tag::String(value.clone()))
    }
    fn visit_byte(&mut self, value: i8) -> ValueResult {
        self.append(Tag::Byte(value))
    }
    fn visit_short(&mut self, value: i16) -> ValueResult {
        self.append(Tag::Short(value))
    }
    fn visit_int(&mut self, value: i32) -> ValueResult {
        self.append(Tag::Int(value))
    }
    fn visit_long(&mut self, value: i64) -> ValueResult {
        self.append(Tag::Long(value))
    }
    fn visit_float(&mut self, value: f32) -> ValueResult {
        self.append(Tag::Float(value))
    }
    fn visit_double(&mut self, value: f64) -> ValueResult {
        self.append(Tag::Double(value))
    }
    fn visit_byte_array(&mut self, value: &[i8]) -> ValueResult {
        self.append(Tag::ByteArray(value.to_vec()))
    }
    fn visit_int_array(&mut self, value: &[i32]) -> ValueResult {
        self.append(Tag::IntArray(value.to_vec()))
    }
    fn visit_long_array(&mut self, value: &[i64]) -> ValueResult {
        self.append(Tag::LongArray(value.to_vec()))
    }
    fn visit_list(&mut self, _: i8, _: i32) -> ValueResult {
        ValueResult::Continue
    }
    fn visit_element(&mut self, type_id: i8, _: i32) -> EntryResult {
        self.enter_container_if_needed(type_id);
        EntryResult::Enter
    }
    fn visit_entry_type(&mut self, _: i8) -> EntryResult {
        EntryResult::Enter
    }
    fn visit_entry(&mut self, type_id: i8, name: &JString) -> EntryResult {
        if let Some(Builder::Compound { last_id, .. }) = self.stack.last_mut() {
            *last_id = name.clone();
        }
        self.enter_container_if_needed(type_id);
        EntryResult::Enter
    }
    fn visit_container_end(&mut self) -> ValueResult {
        let container = self.stack.pop().expect("open container");
        if let Some(tag) = container.build() {
            self.append(tag);
        }
        ValueResult::Continue
    }
    fn visit_root_entry(&mut self, type_id: i8) -> ValueResult {
        self.enter_container_if_needed(type_id);
        ValueResult::Continue
    }
}

/// `FieldSelector`: a field to collect or skip, by path, type and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSelector {
    /// The names of the compounds leading to the field.
    pub path: Vec<JString>,
    /// The field's type id.
    pub type_id: i8,
    /// The field's name.
    pub name: JString,
}

impl FieldSelector {
    /// A field of the root compound.
    #[must_use]
    pub fn new(type_id: i8, name: impl Into<JString>) -> Self {
        Self {
            path: Vec::new(),
            type_id,
            name: name.into(),
        }
    }

    /// A field nested in the given compounds.
    #[must_use]
    pub fn nested(path: &[&str], type_id: i8, name: impl Into<JString>) -> Self {
        Self {
            path: path.iter().map(|&p| JString::from(p)).collect(),
            type_id,
            name: name.into(),
        }
    }
}

/// `FieldTree`: the selected fields per compound depth.
#[derive(Clone, Debug)]
struct FieldTree {
    depth: usize,
    selected_fields: HashMap<JString, i8>,
    fields_to_recurse: HashMap<JString, FieldTree>,
}

impl FieldTree {
    fn new(depth: usize) -> Self {
        Self {
            depth,
            selected_fields: HashMap::new(),
            fields_to_recurse: HashMap::new(),
        }
    }

    fn add_entry(&mut self, field: &FieldSelector) {
        if self.depth <= field.path.len() {
            let depth = self.depth;
            self.fields_to_recurse
                .entry(field.path[depth - 1].clone())
                .or_insert_with(|| FieldTree::new(depth + 1))
                .add_entry(field);
        } else {
            self.selected_fields.insert(field.name.clone(), field.type_id);
        }
    }

    fn is_selected(&self, type_id: i8, name: &JString) -> bool {
        self.selected_fields.get(name) == Some(&type_id)
    }
}

/// Follows the path of `stack` (a list of `FieldTree` path keys from the root).
fn frame<'a>(root: &'a FieldTree, path: &[JString]) -> &'a FieldTree {
    path.iter()
        .fold(root, |tree, key| &tree.fields_to_recurse[key])
}

fn frame_mut<'a>(root: &'a mut FieldTree, path: &[JString]) -> &'a mut FieldTree {
    path.iter().fold(root, |tree, key| {
        tree.fields_to_recurse.get_mut(key).expect("pushed frame")
    })
}

/// `CollectFields`: collects only the selected fields (and the compounds leading to them),
/// stopping once all were found.
#[derive(Debug)]
pub struct CollectFields {
    collect: CollectToTag,
    fields_to_get_count: usize,
    wanted_types: Vec<i8>,
    root: FieldTree,
    /// The `FieldTree` frames entered, as keys from the root (Java's stack of trees).
    stack: Vec<JString>,
}

impl CollectFields {
    /// `new CollectFields(wantedFields...)`.
    #[must_use]
    pub fn new(wanted_fields: &[FieldSelector]) -> Self {
        let mut root = FieldTree::new(1);
        let mut wanted_types = Vec::new();
        for field in wanted_fields {
            root.add_entry(field);
            wanted_types.push(field.type_id);
        }
        wanted_types.push(id::COMPOUND as i8);
        Self {
            collect: CollectToTag::new(),
            fields_to_get_count: wanted_fields.len(),
            wanted_types,
            root,
            stack: Vec::new(),
        }
    }

    /// `getMissingFieldCount`.
    #[must_use]
    pub fn missing_field_count(&self) -> usize {
        self.fields_to_get_count
    }

    /// The collected tag.
    #[must_use]
    pub fn result(self) -> Option<Tag> {
        self.collect.result()
    }

    fn current(&self) -> &FieldTree {
        frame(&self.root, &self.stack)
    }
}

impl StreamTagVisitor for CollectFields {
    fn visit_end(&mut self) -> ValueResult {
        self.collect.visit_end()
    }
    fn visit_string(&mut self, value: &JString) -> ValueResult {
        self.collect.visit_string(value)
    }
    fn visit_byte(&mut self, value: i8) -> ValueResult {
        self.collect.visit_byte(value)
    }
    fn visit_short(&mut self, value: i16) -> ValueResult {
        self.collect.visit_short(value)
    }
    fn visit_int(&mut self, value: i32) -> ValueResult {
        self.collect.visit_int(value)
    }
    fn visit_long(&mut self, value: i64) -> ValueResult {
        self.collect.visit_long(value)
    }
    fn visit_float(&mut self, value: f32) -> ValueResult {
        self.collect.visit_float(value)
    }
    fn visit_double(&mut self, value: f64) -> ValueResult {
        self.collect.visit_double(value)
    }
    fn visit_byte_array(&mut self, value: &[i8]) -> ValueResult {
        self.collect.visit_byte_array(value)
    }
    fn visit_int_array(&mut self, value: &[i32]) -> ValueResult {
        self.collect.visit_int_array(value)
    }
    fn visit_long_array(&mut self, value: &[i64]) -> ValueResult {
        self.collect.visit_long_array(value)
    }
    fn visit_list(&mut self, element_type: i8, size: i32) -> ValueResult {
        self.collect.visit_list(element_type, size)
    }
    fn visit_element(&mut self, type_id: i8, index: i32) -> EntryResult {
        self.collect.visit_element(type_id, index)
    }
    fn visit_root_entry(&mut self, type_id: i8) -> ValueResult {
        if type_id != id::COMPOUND as i8 {
            return ValueResult::Halt;
        }
        self.collect.visit_root_entry(type_id)
    }
    fn visit_entry_type(&mut self, type_id: i8) -> EntryResult {
        if self.collect.depth() > self.current().depth {
            return self.collect.visit_entry_type(type_id);
        }
        if self.fields_to_get_count == 0 {
            return EntryResult::Break;
        }
        if !self.wanted_types.contains(&type_id) {
            return EntryResult::Skip;
        }
        self.collect.visit_entry_type(type_id)
    }
    fn visit_entry(&mut self, type_id: i8, name: &JString) -> EntryResult {
        if self.collect.depth() > self.current().depth {
            return self.collect.visit_entry(type_id, name);
        }
        // `selectedFields().remove(id, type)`: removes only when mapped to that type.
        let current = frame_mut(&mut self.root, &self.stack);
        if current.selected_fields.get(name) == Some(&type_id) {
            current.selected_fields.remove(name);
            self.fields_to_get_count -= 1;
            return self.collect.visit_entry(type_id, name);
        }
        if type_id == id::COMPOUND as i8 && current.fields_to_recurse.contains_key(name) {
            self.stack.push(name.clone());
            return self.collect.visit_entry(type_id, name);
        }
        EntryResult::Skip
    }
    fn visit_container_end(&mut self) -> ValueResult {
        if self.collect.depth() == self.current().depth {
            // `stack.pop()` on Java's deque; the root frame is popped too, after which
            // `element()` would throw, which vanilla never reaches.
            self.stack.pop();
        }
        self.collect.visit_container_end()
    }
}

/// `SkipFields`: collects everything except the selected fields.
#[derive(Debug)]
pub struct SkipFields {
    collect: CollectToTag,
    root: FieldTree,
    stack: Vec<JString>,
}

impl SkipFields {
    /// `new SkipFields(wantedFields...)`.
    #[must_use]
    pub fn new(fields: &[FieldSelector]) -> Self {
        let mut root = FieldTree::new(1);
        for field in fields {
            root.add_entry(field);
        }
        Self {
            collect: CollectToTag::new(),
            root,
            stack: Vec::new(),
        }
    }

    /// The collected tag.
    #[must_use]
    pub fn result(self) -> Option<Tag> {
        self.collect.result()
    }
}

impl StreamTagVisitor for SkipFields {
    fn visit_end(&mut self) -> ValueResult {
        self.collect.visit_end()
    }
    fn visit_string(&mut self, value: &JString) -> ValueResult {
        self.collect.visit_string(value)
    }
    fn visit_byte(&mut self, value: i8) -> ValueResult {
        self.collect.visit_byte(value)
    }
    fn visit_short(&mut self, value: i16) -> ValueResult {
        self.collect.visit_short(value)
    }
    fn visit_int(&mut self, value: i32) -> ValueResult {
        self.collect.visit_int(value)
    }
    fn visit_long(&mut self, value: i64) -> ValueResult {
        self.collect.visit_long(value)
    }
    fn visit_float(&mut self, value: f32) -> ValueResult {
        self.collect.visit_float(value)
    }
    fn visit_double(&mut self, value: f64) -> ValueResult {
        self.collect.visit_double(value)
    }
    fn visit_byte_array(&mut self, value: &[i8]) -> ValueResult {
        self.collect.visit_byte_array(value)
    }
    fn visit_int_array(&mut self, value: &[i32]) -> ValueResult {
        self.collect.visit_int_array(value)
    }
    fn visit_long_array(&mut self, value: &[i64]) -> ValueResult {
        self.collect.visit_long_array(value)
    }
    fn visit_list(&mut self, element_type: i8, size: i32) -> ValueResult {
        self.collect.visit_list(element_type, size)
    }
    fn visit_element(&mut self, type_id: i8, index: i32) -> EntryResult {
        self.collect.visit_element(type_id, index)
    }
    fn visit_entry_type(&mut self, type_id: i8) -> EntryResult {
        self.collect.visit_entry_type(type_id)
    }
    fn visit_root_entry(&mut self, type_id: i8) -> ValueResult {
        self.collect.visit_root_entry(type_id)
    }
    fn visit_entry(&mut self, type_id: i8, name: &JString) -> EntryResult {
        let current = frame(&self.root, &self.stack);
        if current.is_selected(type_id, name) {
            return EntryResult::Skip;
        }
        if type_id == id::COMPOUND as i8 && current.fields_to_recurse.contains_key(name) {
            self.stack.push(name.clone());
        }
        self.collect.visit_entry(type_id, name)
    }
    fn visit_container_end(&mut self) -> ValueResult {
        if self.collect.depth() == frame(&self.root, &self.stack).depth {
            self.stack.pop();
        }
        self.collect.visit_container_end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{ByteBufInput, ByteBufOutput};
    use crate::io::write_any_tag;

    fn chunk() -> Tag {
        let mut level = CompoundTag::new();
        level.put_string("Status", "minecraft:full");
        level.put("Sections", Tag::List(ListTag::from_vec(vec![Tag::Int(1), Tag::Int(2)])));
        let mut root = CompoundTag::new();
        root.put_int("DataVersion", 5023);
        root.put("Level", Tag::Compound(level));
        root.put_string("Other", "x");
        Tag::Compound(root)
    }

    fn bytes(tag: &Tag) -> Vec<u8> {
        let mut buffer = Vec::new();
        write_any_tag(tag, &mut ByteBufOutput::new(&mut buffer)).unwrap();
        buffer
    }

    #[test]
    fn collect_to_tag_rebuilds_the_tag() {
        let tag = chunk();
        let mut collector = CollectToTag::new();
        accept_as_root(&tag, &mut collector);
        assert_eq!(collector.result(), Some(tag.clone()));

        let data = bytes(&tag);
        let mut input = ByteBufInput::new(&data[1..]);
        let mut collector = CollectToTag::new();
        parse_root(10, &mut input, &mut collector, &mut NbtAccounter::default_quota()).unwrap();
        assert_eq!(collector.result(), Some(tag));
    }

    #[test]
    fn collect_fields_reads_only_what_is_asked() {
        let data = bytes(&chunk());
        let mut visitor = CollectFields::new(&[
            FieldSelector::new(id::INT as i8, "DataVersion"),
            FieldSelector::nested(&["Level"], id::STRING as i8, "Status"),
        ]);
        let mut input = ByteBufInput::new(&data[1..]);
        parse_root(10, &mut input, &mut visitor, &mut NbtAccounter::default_quota()).unwrap();
        assert_eq!(visitor.missing_field_count(), 0);
        let Some(Tag::Compound(result)) = visitor.result() else {
            panic!("expected a compound");
        };
        assert_eq!(result.get_int("DataVersion"), Some(5023));
        let level = result.get_compound("Level").unwrap();
        assert_eq!(level.get_string("Status").unwrap(), "minecraft:full");
        assert!(!level.contains("Sections"));
        assert!(!result.contains("Other"));
    }

    #[test]
    fn skip_fields_drops_the_selected_fields() {
        let mut visitor = SkipFields::new(&[FieldSelector::nested(
            &["Level"],
            id::LIST as i8,
            "Sections",
        )]);
        accept_as_root(&chunk(), &mut visitor);
        let Some(Tag::Compound(result)) = visitor.result() else {
            panic!("expected a compound");
        };
        assert!(!result.get_compound("Level").unwrap().contains("Sections"));
        assert_eq!(result.get_string("Other").unwrap(), "x");
    }

    #[test]
    fn skip_all_consumes_everything() {
        let data = bytes(&chunk());
        let mut input = ByteBufInput::new(&data[1..]);
        parse_root(10, &mut input, &mut SkipAll, &mut NbtAccounter::default_quota()).unwrap();
        assert_eq!(input.position(), data.len() - 1);
    }
}
