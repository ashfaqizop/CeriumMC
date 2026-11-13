//! SNBT parsing: `SnbtGrammar`, `SnbtOperations` and `TagParser`, rule for rule.
//!
//! The grammar is built on [`cerium_packrat`] exactly as vanilla builds it, so it accepts the
//! same inputs and reports the same error (translation key, arguments and cursor) for every
//! malformed one. Like vanilla's, it is generic over the values it builds ([`SnbtOps`], Java's
//! `DynamicOps`); [`NbtOps`] builds [`Tag`]s.
//!
//! One documented difference: vanilla's grammar returns the same `CompoundTag`/`ListTag`
//! instance for every empty `{}`/`[]` it parses (and shares them across parses), so mutating
//! one parsed empty container would change the others. CeriumMC's values are owned, so they
//! never alias; vanilla's callers copy parsed tags before changing them.

use std::rc::Rc;

use cerium_brigadier::{Arg, CommandSyntaxException, Message, StringReader};
use cerium_java::{JString, JStringBuilder, character, number, uuid};
use cerium_packrat::terms::{
    self, GreedyPredicateParseRule, NumberRunParseRule, UnquotedStringParseRule, alternative, cut,
    fail, marker, optional, positive_lookahead, sequence,
};
use cerium_packrat::{Atom, Dictionary, Grammar, ParseFailure, ParseState};

use crate::tag::{CompoundTag, ListTag, Tag};

/// The parts of `DynamicOps<T>` the SNBT grammar uses.
pub trait SnbtOps: 'static {
    /// The values built.
    type Value: Clone + 'static;

    /// `createByte`.
    fn create_byte(&self, value: i8) -> Self::Value;
    /// `createShort`.
    fn create_short(&self, value: i16) -> Self::Value;
    /// `createInt`.
    fn create_int(&self, value: i32) -> Self::Value;
    /// `createLong`.
    fn create_long(&self, value: i64) -> Self::Value;
    /// `createFloat`.
    fn create_float(&self, value: f32) -> Self::Value;
    /// `createDouble`.
    fn create_double(&self, value: f64) -> Self::Value;
    /// `createBoolean`.
    fn create_boolean(&self, value: bool) -> Self::Value;
    /// `createString`.
    fn create_string(&self, value: JString) -> Self::Value;
    /// `emptyMap`.
    fn empty_map(&self) -> Self::Value;
    /// `emptyList`.
    fn empty_list(&self) -> Self::Value;
    /// `createMap(Map)`: the entries in the map's iteration order.
    fn create_map(&self, entries: Vec<(JString, Self::Value)>) -> Self::Value;
    /// `createList`.
    fn create_list(&self, values: Vec<Self::Value>) -> Self::Value;
    /// `createByteList`.
    fn create_byte_list(&self, values: Vec<i8>) -> Self::Value;
    /// `createIntList`.
    fn create_int_list(&self, values: Vec<i32>) -> Self::Value;
    /// `createLongList`.
    fn create_long_list(&self, values: Vec<i64>) -> Self::Value;
    /// `getBooleanValue(value).result()`.
    fn boolean_value(&self, value: &Self::Value) -> Option<bool>;
    /// `getNumberValue(value).result().map(Number::doubleValue)`.
    fn number_value(&self, value: &Self::Value) -> Option<f64>;
    /// `getStringValue(value).result()`.
    fn string_value(&self, value: &Self::Value) -> Option<JString>;
}

/// `NbtOps`, as far as SNBT uses it.
#[derive(Clone, Copy, Debug, Default)]
pub struct NbtOps;

impl SnbtOps for NbtOps {
    type Value = Tag;

    fn create_byte(&self, value: i8) -> Tag {
        Tag::Byte(value)
    }
    fn create_short(&self, value: i16) -> Tag {
        Tag::Short(value)
    }
    fn create_int(&self, value: i32) -> Tag {
        Tag::Int(value)
    }
    fn create_long(&self, value: i64) -> Tag {
        Tag::Long(value)
    }
    fn create_float(&self, value: f32) -> Tag {
        Tag::Float(value)
    }
    fn create_double(&self, value: f64) -> Tag {
        Tag::Double(value)
    }
    fn create_boolean(&self, value: bool) -> Tag {
        Tag::boolean(value)
    }
    fn create_string(&self, value: JString) -> Tag {
        Tag::String(value)
    }
    fn empty_map(&self) -> Tag {
        Tag::Compound(CompoundTag::new())
    }
    fn empty_list(&self) -> Tag {
        Tag::List(ListTag::new())
    }
    fn create_map(&self, entries: Vec<(JString, Tag)>) -> Tag {
        let mut compound = CompoundTag::new();
        for (key, value) in entries {
            compound.put(key, value);
        }
        Tag::Compound(compound)
    }
    fn create_list(&self, values: Vec<Tag>) -> Tag {
        Tag::List(ListTag::from_vec(values))
    }
    fn create_byte_list(&self, values: Vec<i8>) -> Tag {
        Tag::ByteArray(values)
    }
    fn create_int_list(&self, values: Vec<i32>) -> Tag {
        Tag::IntArray(values)
    }
    fn create_long_list(&self, values: Vec<i64>) -> Tag {
        Tag::LongArray(values)
    }
    /// `getNumberValue(input).map(v -> v.doubleValue() != 0.0)`.
    fn boolean_value(&self, value: &Tag) -> Option<bool> {
        value.as_double().map(|v| v != 0.0)
    }
    fn number_value(&self, value: &Tag) -> Option<f64> {
        value.as_double()
    }
    fn string_value(&self, value: &Tag) -> Option<JString> {
        value.as_string().cloned()
    }
}

/// The `snbt.parser.*` and related messages.
mod errors {
    use cerium_brigadier::{Arg, Message};

    fn key(key: &'static str) -> Message {
        Message::translatable(key)
    }

    fn with(key: &'static str, arg: impl Into<Arg>) -> Message {
        Message::translatable_with(key, vec![arg.into()])
    }

    pub(super) fn number_parse_failure(message: cerium_java::JString) -> Message {
        with("snbt.parser.number_parse_failure", message)
    }
    pub(super) fn expected_hex_escape(length: &str) -> Message {
        with("snbt.parser.expected_hex_escape", length)
    }
    pub(super) fn invalid_codepoint(codepoint: &str) -> Message {
        with("snbt.parser.invalid_codepoint", codepoint)
    }
    pub(super) fn no_such_operation(operation: cerium_java::JString) -> Message {
        with("snbt.parser.no_such_operation", operation)
    }
    pub(super) fn expected_integer_type() -> Message {
        key("snbt.parser.expected_integer_type")
    }
    pub(super) fn expected_float_type() -> Message {
        key("snbt.parser.expected_float_type")
    }
    pub(super) fn expected_non_negative_number() -> Message {
        key("snbt.parser.expected_non_negative_number")
    }
    pub(super) fn invalid_character_name() -> Message {
        key("snbt.parser.invalid_character_name")
    }
    pub(super) fn invalid_array_element_type() -> Message {
        key("snbt.parser.invalid_array_element_type")
    }
    pub(super) fn invalid_unquoted_start() -> Message {
        key("snbt.parser.invalid_unquoted_start")
    }
    pub(super) fn expected_unquoted_string() -> Message {
        key("snbt.parser.expected_unquoted_string")
    }
    pub(super) fn invalid_string_contents() -> Message {
        key("snbt.parser.invalid_string_contents")
    }
    pub(super) fn expected_binary_numeral() -> Message {
        key("snbt.parser.expected_binary_numeral")
    }
    pub(super) fn underscore_not_allowed() -> Message {
        key("snbt.parser.underscore_not_allowed")
    }
    pub(super) fn expected_decimal_numeral() -> Message {
        key("snbt.parser.expected_decimal_numeral")
    }
    pub(super) fn expected_hex_numeral() -> Message {
        key("snbt.parser.expected_hex_numeral")
    }
    pub(super) fn empty_key() -> Message {
        key("snbt.parser.empty_key")
    }
    pub(super) fn leading_zero_not_allowed() -> Message {
        key("snbt.parser.leading_zero_not_allowed")
    }
    pub(super) fn infinity_not_allowed() -> Message {
        key("snbt.parser.infinity_not_allowed")
    }
    pub(super) fn expected_string_uuid() -> Message {
        key("snbt.parser.expected_string_uuid")
    }
    pub(super) fn expected_number_or_boolean() -> Message {
        key("snbt.parser.expected_number_or_boolean")
    }
    pub(super) fn trailing() -> Message {
        key("argument.nbt.trailing")
    }
    pub(super) fn expected_compound() -> Message {
        key("argument.nbt.expected.compound")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sign {
    Plus,
    Minus,
}

impl Sign {
    fn append(self, out: &mut JStringBuilder) {
        if self == Self::Minus {
            out.push_str("-");
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeSuffix {
    Float,
    Double,
    Byte,
    Short,
    Int,
    Long,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SignedPrefix {
    Signed,
    Unsigned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IntegerSuffix {
    signed: Option<SignedPrefix>,
    kind: Option<TypeSuffix>,
}

const EMPTY_SUFFIX: IntegerSuffix = IntegerSuffix {
    signed: None,
    kind: None,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Base {
    Binary,
    Decimal,
    Hex,
}

#[derive(Clone, Debug)]
struct Signed {
    sign: Sign,
    value: JString,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrayPrefix {
    Byte,
    Int,
    Long,
}

/// A parsed integer, in the width its suffix (or array) asks for.
#[derive(Clone, Copy, Debug)]
enum Number {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
}

impl Number {
    /// `Number.longValue()`; `byteValue`/`intValue` narrow it like Java.
    fn long_value(self) -> i64 {
        match self {
            Self::Byte(v) => i64::from(v),
            Self::Short(v) => i64::from(v),
            Self::Int(v) => i64::from(v),
            Self::Long(v) => v,
        }
    }
}

#[derive(Clone, Debug)]
struct IntegerLiteral {
    sign: Sign,
    base: Base,
    digits: JString,
    suffix: IntegerSuffix,
}

fn needs_underscore_removal(contents: &JString) -> bool {
    contents.index_of(u16::from(b'_'), 0).is_some()
}

fn clean_and_append(out: &mut JStringBuilder, contents: &JString) {
    for unit in contents.units() {
        if unit != u16::from(b'_') {
            out.push_unit(unit);
        }
    }
}

impl IntegerLiteral {
    fn signed_or_default(&self) -> SignedPrefix {
        self.suffix.signed.unwrap_or(match self.base {
            Base::Binary | Base::Hex => SignedPrefix::Unsigned,
            Base::Decimal => SignedPrefix::Signed,
        })
    }

    fn cleanup_digits(&self) -> JString {
        if self.sign != Sign::Minus && !needs_underscore_removal(&self.digits) {
            return self.digits.clone();
        }
        let mut out = JStringBuilder::new();
        self.sign.append(&mut out);
        clean_and_append(&mut out, &self.digits);
        out.build()
    }

    /// `IntegerLiteral.create(ops, type, state)`, as a number (vanilla builds it through the
    /// ops, or `JavaOps` for array elements).
    fn number(&self, kind: TypeSuffix, state: &mut ParseState<'_>) -> Option<Number> {
        let is_signed = self.signed_or_default() == SignedPrefix::Signed;
        if !is_signed && self.sign == Sign::Minus {
            state.error_here(errors::expected_non_negative_number());
            return None;
        }
        let digits = self.cleanup_digits();
        let radix = match self.base {
            Base::Binary => 2,
            Base::Decimal => 10,
            Base::Hex => 16,
        };
        let parsed = match (is_signed, kind) {
            (true, TypeSuffix::Byte) => number::parse_byte(&digits, radix).map(Number::Byte),
            (true, TypeSuffix::Short) => number::parse_short(&digits, radix).map(Number::Short),
            (true, TypeSuffix::Int) => number::parse_int(&digits, radix).map(Number::Int),
            (true, TypeSuffix::Long) => number::parse_long(&digits, radix).map(Number::Long),
            (false, TypeSuffix::Byte) => {
                number::parse_unsigned_byte(&digits, radix).map(Number::Byte)
            }
            (false, TypeSuffix::Short) => parse_unsigned_short(&digits, radix).map(Number::Short),
            (false, TypeSuffix::Int) => number::parse_unsigned_int(&digits, radix).map(Number::Int),
            (false, TypeSuffix::Long) => {
                number::parse_unsigned_long(&digits, radix).map(Number::Long)
            }
            (_, TypeSuffix::Float | TypeSuffix::Double) => {
                state.error_here(errors::expected_integer_type());
                return None;
            }
        };
        match parsed {
            Ok(number) => Some(number),
            Err(error) => {
                state.error_here(errors::number_parse_failure(error.message().clone()));
                None
            }
        }
    }

    /// `IntegerLiteral.create(ops, state)`: the suffix's type, or `int`.
    fn create<O: SnbtOps>(&self, ops: &O, state: &mut ParseState<'_>) -> Option<O::Value> {
        let kind = self.suffix.kind.unwrap_or(TypeSuffix::Int);
        Some(match self.number(kind, state)? {
            Number::Byte(v) => ops.create_byte(v),
            Number::Short(v) => ops.create_short(v),
            Number::Int(v) => ops.create_int(v),
            Number::Long(v) => ops.create_long(v),
        })
    }
}

/// `SnbtGrammar.parseUnsignedShort`: `Integer.parseInt`, then a 16-bit range check.
fn parse_unsigned_short(s: &JString, radix: u32) -> Result<i16, number::NumberFormatError> {
    let parse = number::parse_int(s, radix)?;
    if parse >> 16 == 0 {
        Ok(parse as i16)
    } else {
        // Reuse Guava's wording, which is the same: "out of range: <value>".
        Err(
            number::parse_unsigned_byte(&JString::from(parse.to_string().as_str()), 10)
                .expect_err("a value at least 2^16 is out of byte range"),
        )
    }
}

impl ArrayPrefix {
    fn allows(self, kind: TypeSuffix) -> bool {
        match self {
            Self::Byte => kind == TypeSuffix::Byte,
            Self::Int => matches!(kind, TypeSuffix::Int | TypeSuffix::Byte | TypeSuffix::Short),
            Self::Long => matches!(
                kind,
                TypeSuffix::Long | TypeSuffix::Byte | TypeSuffix::Short | TypeSuffix::Int
            ),
        }
    }

    fn default_type(self) -> TypeSuffix {
        match self {
            Self::Byte => TypeSuffix::Byte,
            Self::Int => TypeSuffix::Int,
            Self::Long => TypeSuffix::Long,
        }
    }

    fn create_empty<O: SnbtOps>(self, ops: &O) -> O::Value {
        match self {
            Self::Byte => ops.create_byte_list(Vec::new()),
            Self::Int => ops.create_int_list(Vec::new()),
            Self::Long => ops.create_long_list(Vec::new()),
        }
    }

    /// `ArrayPrefix.create(ops, entries, state)`.
    fn create<O: SnbtOps>(
        self,
        ops: &O,
        entries: &[IntegerLiteral],
        state: &mut ParseState<'_>,
    ) -> Option<O::Value> {
        let mut values = Vec::with_capacity(entries.len());
        for entry in entries {
            // `buildNumber`: the element's type must be allowed in this array.
            let kind = match entry.suffix.kind {
                None => self.default_type(),
                Some(kind) if self.allows(kind) => kind,
                Some(_) => {
                    state.error_here(errors::invalid_array_element_type());
                    return None;
                }
            };
            values.push(entry.number(kind, state)?.long_value());
        }
        Some(match self {
            Self::Byte => ops.create_byte_list(values.into_iter().map(|v| v as i8).collect()),
            Self::Int => ops.create_int_list(values.into_iter().map(|v| v as i32).collect()),
            Self::Long => ops.create_long_list(values),
        })
    }
}

/// `SnbtGrammar.canStartNumber`.
fn can_start_number(c: u16) -> bool {
    matches!(c, 0x2B | 0x2D | 0x2E | 0x30..=0x39)
}

/// `HexFormat.fromHexDigits`: the digits as an `int` (8 digits wrap).
fn from_hex_digits(hex: &JString) -> i32 {
    hex.units().fold(0i32, |value, unit| {
        let digit = char::from(unit as u8).to_digit(16).expect("hex digit") as i32;
        value.wrapping_shl(4) | digit
    })
}

/// `Character.toString(codePoint)` for a valid code point: one unit, or a surrogate pair.
fn code_point_string(code_point: u32) -> JString {
    match char::from_u32(code_point) {
        Some(c) => JString::from(c.to_string().as_str()),
        // A surrogate code point is a single (unpaired) unit.
        None => JString::from_utf16(&[code_point as u16]),
    }
}

/// `SnbtGrammar.joinList`.
fn join_list(list: &[JString]) -> JString {
    let mut out = JStringBuilder::new();
    for part in list {
        out.push_jstring(part);
    }
    out.build()
}

/// `createFloat(ops, sign, whole, fraction, exponent, typeSuffix, state)`.
fn create_float<O: SnbtOps>(
    ops: &O,
    sign: Sign,
    whole: Option<&JString>,
    fraction: Option<&JString>,
    exponent: Option<&Signed>,
    type_suffix: Option<TypeSuffix>,
    state: &mut ParseState<'_>,
) -> Option<O::Value> {
    let mut result = JStringBuilder::new();
    sign.append(&mut result);
    if let Some(whole) = whole {
        clean_and_append(&mut result, whole);
    }
    if let Some(fraction) = fraction {
        result.push_str(".");
        clean_and_append(&mut result, fraction);
    }
    if let Some(exponent) = exponent {
        result.push_str("e");
        exponent.sign.append(&mut result);
        clean_and_append(&mut result, &exponent.value);
    }
    let contents = result.build();
    match type_suffix {
        Some(TypeSuffix::Float) => match number::parse_float(&contents) {
            Ok(value) if value.is_finite() => Some(ops.create_float(value)),
            Ok(_) => {
                state.error_here(errors::infinity_not_allowed());
                None
            }
            Err(error) => {
                state.error_here(errors::number_parse_failure(error.message().clone()));
                None
            }
        },
        None | Some(TypeSuffix::Double) => match number::parse_double(&contents) {
            Ok(value) if value.is_finite() => Some(ops.create_double(value)),
            Ok(_) => {
                state.error_here(errors::infinity_not_allowed());
                None
            }
            Err(error) => {
                state.error_here(errors::number_parse_failure(error.message().clone()));
                None
            }
        },
        Some(_) => {
            state.error_here(errors::expected_float_type());
            None
        }
    }
}

fn is_decimal_digit(c: u16) -> bool {
    matches!(c, 0x30..=0x39)
}

fn is_hex_digit(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66)
}

/// `SnbtOperations.BUILTIN_IDS`: the suggestions where an unquoted string may start.
fn builtin_ids() -> cerium_packrat::Suggestions {
    cerium_packrat::Suggestions::Values(
        ["false", "true", "bool", "uuid"]
            .into_iter()
            .map(JString::from)
            .collect(),
    )
}

/// `SnbtGrammar.createParser(ops)`.
#[allow(
    clippy::too_many_lines,
    reason = "a rule-for-rule transcription of vanilla's grammar"
)]
pub fn create_parser<O: SnbtOps>(ops: O) -> Grammar<O::Value> {
    let ops = Rc::new(ops);
    let true_value = ops.create_boolean(true);
    let false_value = ops.create_boolean(false);
    let empty_map_value = ops.empty_map();
    let empty_list = ops.empty_list();
    let mut rules = Dictionary::new();

    let sign: Atom<Sign> = Atom::of("sign");
    let sign_term = alternative(vec![
        sequence(vec![terms::character('+'), marker(sign, Sign::Plus)]),
        sequence(vec![terms::character('-'), marker(sign, Sign::Minus)]),
    ]);
    rules.put_simple(sign, sign_term, move |scope| scope.get_or_throw(sign));

    let integer_suffix: Atom<IntegerSuffix> = Atom::of("integer_suffix");
    let suffix = |signed: Option<SignedPrefix>, kind: TypeSuffix| IntegerSuffix {
        signed,
        kind: Some(kind),
    };
    let width_alternatives = |signed: Option<SignedPrefix>| {
        alternative(vec![
            sequence(vec![
                terms::characters('b', 'B'),
                marker(integer_suffix, suffix(signed, TypeSuffix::Byte)),
            ]),
            sequence(vec![
                terms::characters('s', 'S'),
                marker(integer_suffix, suffix(signed, TypeSuffix::Short)),
            ]),
            sequence(vec![
                terms::characters('i', 'I'),
                marker(integer_suffix, suffix(signed, TypeSuffix::Int)),
            ]),
            sequence(vec![
                terms::characters('l', 'L'),
                marker(integer_suffix, suffix(signed, TypeSuffix::Long)),
            ]),
        ])
    };
    let integer_suffix_term = alternative(vec![
        sequence(vec![
            terms::characters('u', 'U'),
            width_alternatives(Some(SignedPrefix::Unsigned)),
        ]),
        sequence(vec![
            terms::characters('s', 'S'),
            width_alternatives(Some(SignedPrefix::Signed)),
        ]),
        sequence(vec![
            terms::characters('b', 'B'),
            marker(integer_suffix, suffix(None, TypeSuffix::Byte)),
        ]),
        sequence(vec![
            terms::characters('s', 'S'),
            marker(integer_suffix, suffix(None, TypeSuffix::Short)),
        ]),
        sequence(vec![
            terms::characters('i', 'I'),
            marker(integer_suffix, suffix(None, TypeSuffix::Int)),
        ]),
        sequence(vec![
            terms::characters('l', 'L'),
            marker(integer_suffix, suffix(None, TypeSuffix::Long)),
        ]),
    ]);
    rules.put_simple(integer_suffix, integer_suffix_term, move |scope| {
        scope.get_or_throw(integer_suffix)
    });

    let binary_numeral: Atom<JString> = Atom::of("binary_numeral");
    rules.put(
        binary_numeral,
        Rc::new(NumberRunParseRule::new(
            errors::expected_binary_numeral(),
            errors::underscore_not_allowed(),
            |c| matches!(c, 0x30 | 0x31 | 0x5F),
        )),
    );
    let decimal_numeral: Atom<JString> = Atom::of("decimal_numeral");
    rules.put(
        decimal_numeral,
        Rc::new(NumberRunParseRule::new(
            errors::expected_decimal_numeral(),
            errors::underscore_not_allowed(),
            |c| is_decimal_digit(c) || c == 0x5F,
        )),
    );
    let hex_numeral: Atom<JString> = Atom::of("hex_numeral");
    rules.put(
        hex_numeral,
        Rc::new(NumberRunParseRule::new(
            errors::expected_hex_numeral(),
            errors::underscore_not_allowed(),
            |c| is_hex_digit(c) || c == 0x5F,
        )),
    );

    let integer_literal: Atom<IntegerLiteral> = Atom::of("integer_literal");
    let integer_literal_term = sequence(vec![
        optional(rules.named(sign)),
        alternative(vec![
            sequence(vec![
                terms::character('0'),
                cut(),
                alternative(vec![
                    sequence(vec![
                        terms::characters('x', 'X'),
                        cut(),
                        rules.named(hex_numeral),
                    ]),
                    sequence(vec![
                        terms::characters('b', 'B'),
                        rules.named(binary_numeral),
                    ]),
                    sequence(vec![
                        rules.named(decimal_numeral),
                        cut(),
                        fail(errors::leading_zero_not_allowed()),
                    ]),
                    marker(decimal_numeral, JString::from("0")),
                ]),
            ]),
            rules.named(decimal_numeral),
        ]),
        optional(rules.named(integer_suffix)),
    ]);
    let integer_literal_rule =
        rules.put_simple(integer_literal, integer_literal_term, move |scope| {
            let suffix = scope
                .get_or_default(integer_suffix, EMPTY_SUFFIX)
                .expect("a set integer suffix");
            let sign = scope.get_or_default(sign, Sign::Plus).expect("a set sign");
            let (base, digits) = if let Some(decimal) = scope.get(decimal_numeral) {
                (Base::Decimal, decimal)
            } else if let Some(hex) = scope.get(hex_numeral) {
                (Base::Hex, hex)
            } else {
                (Base::Binary, scope.get_or_throw(binary_numeral))
            };
            IntegerLiteral {
                sign,
                base,
                digits,
                suffix,
            }
        });

    let float_type_suffix: Atom<TypeSuffix> = Atom::of("float_type_suffix");
    let float_type_suffix_term = alternative(vec![
        sequence(vec![
            terms::characters('f', 'F'),
            marker(float_type_suffix, TypeSuffix::Float),
        ]),
        sequence(vec![
            terms::characters('d', 'D'),
            marker(float_type_suffix, TypeSuffix::Double),
        ]),
    ]);
    rules.put_simple(float_type_suffix, float_type_suffix_term, move |scope| {
        scope.get_or_throw(float_type_suffix)
    });

    let float_exponent_part: Atom<Signed> = Atom::of("float_exponent_part");
    let float_exponent_term = sequence(vec![
        terms::characters('e', 'E'),
        optional(rules.named(sign)),
        rules.named(decimal_numeral),
    ]);
    rules.put_simple(float_exponent_part, float_exponent_term, move |scope| {
        Signed {
            sign: scope.get_or_default(sign, Sign::Plus).expect("a set sign"),
            value: scope.get_or_throw(decimal_numeral),
        }
    });

    let float_whole_part: Atom<JString> = Atom::of("float_whole_part");
    let float_fraction_part: Atom<JString> = Atom::of("float_fraction_part");
    let float_literal: Atom<O::Value> = Atom::of("float_literal");
    let float_literal_term = sequence(vec![
        optional(rules.named(sign)),
        alternative(vec![
            sequence(vec![
                rules.named_with_alias(decimal_numeral, float_whole_part),
                terms::character('.'),
                cut(),
                optional(rules.named_with_alias(decimal_numeral, float_fraction_part)),
                optional(rules.named(float_exponent_part)),
                optional(rules.named(float_type_suffix)),
            ]),
            sequence(vec![
                terms::character('.'),
                cut(),
                rules.named_with_alias(decimal_numeral, float_fraction_part),
                optional(rules.named(float_exponent_part)),
                optional(rules.named(float_type_suffix)),
            ]),
            sequence(vec![
                rules.named_with_alias(decimal_numeral, float_whole_part),
                rules.named(float_exponent_part),
                cut(),
                optional(rules.named(float_type_suffix)),
            ]),
            sequence(vec![
                rules.named_with_alias(decimal_numeral, float_whole_part),
                optional(rules.named(float_exponent_part)),
                rules.named(float_type_suffix),
            ]),
        ]),
    ]);
    let float_ops = Rc::clone(&ops);
    rules.put_complex(float_literal, float_literal_term, move |state| {
        let scope = state.scope();
        let whole_sign = scope.get_or_default(sign, Sign::Plus).expect("a set sign");
        let whole = scope.get(float_whole_part);
        let fraction = scope.get(float_fraction_part);
        let exponent = scope.get(float_exponent_part);
        let type_suffix = scope.get(float_type_suffix);
        create_float(
            &*float_ops,
            whole_sign,
            whole.as_ref(),
            fraction.as_ref(),
            exponent.as_ref(),
            type_suffix,
            state,
        )
    });

    let hex_rule = |size: usize| {
        Rc::new(GreedyPredicateParseRule::new(
            size,
            size,
            errors::expected_hex_escape(&size.to_string()),
            is_hex_digit,
        ))
    };
    let string_hex_2: Atom<JString> = Atom::of("string_hex_2");
    rules.put(string_hex_2, hex_rule(2));
    let string_hex_4: Atom<JString> = Atom::of("string_hex_4");
    rules.put(string_hex_4, hex_rule(4));
    let string_hex_8: Atom<JString> = Atom::of("string_hex_8");
    rules.put(string_hex_8, hex_rule(8));
    let string_unicode_name: Atom<JString> = Atom::of("string_unicode_name");
    // `GreedyPatternParseRule` with `[-a-zA-Z0-9 ]+`: the longest run of those characters.
    rules.put(
        string_unicode_name,
        Rc::new(GreedyPredicateParseRule::new(
            1,
            usize::MAX,
            errors::invalid_character_name(),
            |c| matches!(c, 0x2D | 0x20 | 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A),
        )),
    );

    let string_escape_sequence: Atom<JString> = Atom::of("string_escape_sequence");
    let plain_escape = |c: char, value: &str| {
        sequence(vec![
            terms::character(c),
            marker(string_escape_sequence, JString::from(value)),
        ])
    };
    let escape_term = alternative(vec![
        plain_escape('b', "\u{8}"),
        plain_escape('s', " "),
        plain_escape('t', "\t"),
        plain_escape('n', "\n"),
        plain_escape('f', "\u{C}"),
        plain_escape('r', "\r"),
        plain_escape('\\', "\\"),
        plain_escape('\'', "'"),
        plain_escape('"', "\""),
        sequence(vec![terms::character('x'), rules.named(string_hex_2)]),
        sequence(vec![terms::character('u'), rules.named(string_hex_4)]),
        sequence(vec![terms::character('U'), rules.named(string_hex_8)]),
        sequence(vec![
            terms::character('N'),
            terms::character('{'),
            rules.named(string_unicode_name),
            terms::character('}'),
        ]),
    ]);
    rules.put_complex(string_escape_sequence, escape_term, move |state| {
        let scope = state.scope();
        if let Some(plain) = scope.get_any(&[string_escape_sequence]) {
            return Some(plain);
        }
        if let Some(hex) = scope.get_any(&[string_hex_2, string_hex_4, string_hex_8]) {
            let code_point = from_hex_digits(&hex);
            if !character::is_valid_code_point(code_point) {
                let formatted = format!("U+{:08X}", code_point as u32);
                state.error_here(errors::invalid_codepoint(&formatted));
                return None;
            }
            return Some(code_point_string(code_point as u32));
        }
        let name = scope.get_or_throw(string_unicode_name);
        match character::code_point_of(&name) {
            Some(code_point) => Some(code_point_string(code_point)),
            None => {
                state.error_here(errors::invalid_character_name());
                None
            }
        }
    });

    let string_plain_contents: Atom<JString> = Atom::of("string_plain_contents");
    rules.put(
        string_plain_contents,
        Rc::new(GreedyPredicateParseRule::new(
            1,
            usize::MAX,
            errors::invalid_string_contents(),
            |c| !matches!(c, 0x22 | 0x27 | 0x5C),
        )),
    );
    let string_chunks: Atom<Vec<JString>> = Atom::of("string_chunks");
    let string_contents: Atom<JString> = Atom::of("string_contents");

    let single_quoted_string_chunk: Atom<JString> = Atom::of("single_quoted_string_chunk");
    let single_chunk_term = alternative(vec![
        rules.named_with_alias(string_plain_contents, string_contents),
        sequence(vec![
            terms::character('\\'),
            rules.named_with_alias(string_escape_sequence, string_contents),
        ]),
        sequence(vec![
            terms::character('"'),
            marker(string_contents, JString::from("\"")),
        ]),
    ]);
    let single_chunk_rule = rules.put_simple(
        single_quoted_string_chunk,
        single_chunk_term,
        move |scope| scope.get_or_throw(string_contents),
    );
    let single_quoted_string_contents: Atom<JString> = Atom::of("single_quoted_string_contents");
    rules.put_simple(
        single_quoted_string_contents,
        terms::repeated(&single_chunk_rule, string_chunks, 0),
        move |scope| join_list(&terms::list(scope, string_chunks).expect("chunks")),
    );

    let double_quoted_string_chunk: Atom<JString> = Atom::of("double_quoted_string_chunk");
    let double_chunk_term = alternative(vec![
        rules.named_with_alias(string_plain_contents, string_contents),
        sequence(vec![
            terms::character('\\'),
            rules.named_with_alias(string_escape_sequence, string_contents),
        ]),
        sequence(vec![
            terms::character('\''),
            marker(string_contents, JString::from("'")),
        ]),
    ]);
    let double_chunk_rule = rules.put_simple(
        double_quoted_string_chunk,
        double_chunk_term,
        move |scope| scope.get_or_throw(string_contents),
    );
    let double_quoted_string_contents: Atom<JString> = Atom::of("double_quoted_string_contents");
    rules.put_simple(
        double_quoted_string_contents,
        terms::repeated(&double_chunk_rule, string_chunks, 0),
        move |scope| join_list(&terms::list(scope, string_chunks).expect("chunks")),
    );

    let quoted_string_literal: Atom<JString> = Atom::of("quoted_string_literal");
    let quoted_term = alternative(vec![
        sequence(vec![
            terms::character('"'),
            cut(),
            optional(rules.named_with_alias(double_quoted_string_contents, string_contents)),
            terms::character('"'),
        ]),
        sequence(vec![
            terms::character('\''),
            optional(rules.named_with_alias(single_quoted_string_contents, string_contents)),
            terms::character('\''),
        ]),
    ]);
    rules.put_simple(quoted_string_literal, quoted_term, move |scope| {
        scope.get_or_throw(string_contents)
    });

    let unquoted_string: Atom<JString> = Atom::of("unquoted_string");
    rules.put(
        unquoted_string,
        Rc::new(UnquotedStringParseRule::new(
            1,
            errors::expected_unquoted_string(),
        )),
    );

    let literal: Atom<O::Value> = Atom::of("literal");
    let argument_list: Atom<Vec<O::Value>> = Atom::of("arguments");
    let literal_forward = rules.forward(literal);
    rules.put_simple(
        argument_list,
        terms::repeated_with_trailing_separator(
            &literal_forward,
            argument_list,
            terms::character(','),
        ),
        move |scope| terms::list(scope, argument_list).expect("arguments"),
    );

    let unquoted_string_or_builtin: Atom<O::Value> = Atom::of("unquoted_string_or_builtin");
    let builtin_term = sequence(vec![
        rules.named(unquoted_string),
        optional(sequence(vec![
            terms::character('('),
            rules.named(argument_list),
            terms::character(')'),
        ])),
    ]);
    let builtin_ops = Rc::clone(&ops);
    rules.put_complex(unquoted_string_or_builtin, builtin_term, move |state| {
        let scope = state.scope();
        let contents = scope.get_or_throw(unquoted_string);
        if contents.is_empty() || can_start_number(contents.char_at(0)) {
            state.error_here_with(builtin_ids(), errors::invalid_unquoted_start());
            return None;
        }
        if let Some(arguments) = scope.get(argument_list) {
            return run_builtin(&*builtin_ops, &contents, &arguments, state);
        }
        // Unquoted strings are ASCII, so ASCII case folding is Java's `equalsIgnoreCase`.
        if contents.equals_ignore_ascii_case("true") {
            Some(true_value.clone())
        } else if contents.equals_ignore_ascii_case("false") {
            Some(false_value.clone())
        } else {
            Some(builtin_ops.create_string(contents))
        }
    });

    let map_key: Atom<JString> = Atom::of("map_key");
    let map_key_term = alternative(vec![
        rules.named(quoted_string_literal),
        rules.named(unquoted_string),
    ]);
    rules.put_simple(map_key, map_key_term, move |scope| {
        scope
            .get_any(&[quoted_string_literal, unquoted_string])
            .expect("a map key")
    });

    let map_entry: Atom<(JString, O::Value)> = Atom::of("map_entry");
    let map_entry_term = sequence(vec![
        rules.named(map_key),
        terms::character(':'),
        rules.named(literal),
    ]);
    let map_entry_rule = rules.put_complex(map_entry, map_entry_term, move |state| {
        let key = state.scope().get_or_throw(map_key);
        if key.is_empty() {
            state.error_here(errors::empty_key());
            return None;
        }
        Some((key, state.scope().get_or_throw(literal)))
    });
    let map_entries: Atom<Vec<(JString, O::Value)>> = Atom::of("map_entries");
    rules.put_simple(
        map_entries,
        terms::repeated_with_trailing_separator(
            &map_entry_rule,
            map_entries,
            terms::character(','),
        ),
        move |scope| terms::list(scope, map_entries).expect("entries"),
    );

    let map_literal: Atom<O::Value> = Atom::of("map_literal");
    let map_ops = Rc::clone(&ops);
    let map_literal_term = sequence(vec![
        terms::character('{'),
        rules.named(map_entries),
        terms::character('}'),
    ]);
    rules.put_simple(map_literal, map_literal_term, move |scope| {
        let entries = terms::list(scope, map_entries).expect("entries");
        if entries.is_empty() {
            return empty_map_value.clone();
        }
        map_ops.create_map(keep_last(entries))
    });

    let list_entries: Atom<Vec<O::Value>> = Atom::of("list_entries");
    rules.put_simple(
        list_entries,
        terms::repeated_with_trailing_separator(
            &literal_forward,
            list_entries,
            terms::character(','),
        ),
        move |scope| terms::list(scope, list_entries).expect("entries"),
    );

    let array_prefix: Atom<ArrayPrefix> = Atom::of("array_prefix");
    let array_prefix_term = alternative(vec![
        sequence(vec![
            terms::character('B'),
            marker(array_prefix, ArrayPrefix::Byte),
        ]),
        sequence(vec![
            terms::character('L'),
            marker(array_prefix, ArrayPrefix::Long),
        ]),
        sequence(vec![
            terms::character('I'),
            marker(array_prefix, ArrayPrefix::Int),
        ]),
    ]);
    rules.put_simple(array_prefix, array_prefix_term, move |scope| {
        scope.get_or_throw(array_prefix)
    });

    let int_array_entries: Atom<Vec<IntegerLiteral>> = Atom::of("int_array_entries");
    rules.put_simple(
        int_array_entries,
        terms::repeated_with_trailing_separator(
            &integer_literal_rule,
            int_array_entries,
            terms::character(','),
        ),
        move |scope| terms::list(scope, int_array_entries).expect("entries"),
    );

    let list_literal: Atom<O::Value> = Atom::of("list_literal");
    let list_literal_term = sequence(vec![
        terms::character('['),
        alternative(vec![
            sequence(vec![
                rules.named(array_prefix),
                terms::character(';'),
                rules.named(int_array_entries),
            ]),
            rules.named(list_entries),
        ]),
        terms::character(']'),
    ]);
    let list_ops = Rc::clone(&ops);
    rules.put_complex(list_literal, list_literal_term, move |state| {
        let scope = state.scope();
        if let Some(array_type) = scope.get(array_prefix) {
            let entries = terms::list(scope, int_array_entries).expect("entries");
            if entries.is_empty() {
                return Some(array_type.create_empty(&*list_ops));
            }
            return array_type.create(&*list_ops, &entries, state);
        }
        let entries = terms::list(scope, list_entries).expect("entries");
        if entries.is_empty() {
            Some(empty_list.clone())
        } else {
            Some(list_ops.create_list(entries))
        }
    });

    let number_lookahead = terms::terminal_characters(&[], can_start_number);
    let literal_term = alternative(vec![
        sequence(vec![
            positive_lookahead(number_lookahead),
            alternative(vec![
                rules.named_with_alias(float_literal, literal),
                rules.named(integer_literal),
            ]),
        ]),
        sequence(vec![
            positive_lookahead(terms::characters('"', '\'')),
            cut(),
            rules.named(quoted_string_literal),
        ]),
        sequence(vec![
            positive_lookahead(terms::character('{')),
            cut(),
            rules.named_with_alias(map_literal, literal),
        ]),
        sequence(vec![
            positive_lookahead(terms::character('[')),
            cut(),
            rules.named_with_alias(list_literal, literal),
        ]),
        rules.named_with_alias(unquoted_string_or_builtin, literal),
    ]);
    let literal_ops = Rc::clone(&ops);
    let literal_rule = rules.put_complex(literal, literal_term, move |state| {
        let scope = state.scope();
        if let Some(quoted) = scope.get(quoted_string_literal) {
            return Some(literal_ops.create_string(quoted));
        }
        if let Some(integer) = scope.get(integer_literal) {
            return integer.create(&*literal_ops, state);
        }
        Some(scope.get_or_throw(literal))
    });
    Grammar::new(rules, literal_rule)
}

/// `ImmutableMap.Builder.buildKeepingLast()`: each key once, at its first position, with its
/// last value.
fn keep_last<V>(entries: Vec<(JString, V)>) -> Vec<(JString, V)> {
    let mut result: Vec<(JString, V)> = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        if let Some(existing) = result.iter_mut().find(|(k, _)| *k == key) {
            existing.1 = value;
        } else {
            result.push((key, value));
        }
    }
    result
}

/// `SnbtOperations.BUILTIN_OPERATIONS`: `bool(x)` and `uuid(x)`.
fn run_builtin<O: SnbtOps>(
    ops: &O,
    id: &JString,
    arguments: &[O::Value],
    state: &mut ParseState<'_>,
) -> Option<O::Value> {
    if arguments.len() == 1 && *id == "bool" {
        let argument = &arguments[0];
        let result = ops
            .boolean_value(argument)
            .or_else(|| ops.number_value(argument).map(|n| n != 0.0));
        let Some(result) = result else {
            state.error_here(errors::expected_number_or_boolean());
            return None;
        };
        return Some(ops.create_boolean(result));
    }
    if arguments.len() == 1 && *id == "uuid" {
        let parsed = ops
            .string_value(&arguments[0])
            .and_then(|text| uuid::from_string(&text).ok());
        let Some((msb, lsb)) = parsed else {
            state.error_here(errors::expected_string_uuid());
            return None;
        };
        // `UUIDUtil.uuidToIntArray`.
        let ints = vec![
            (msb >> 32) as i32,
            msb as i32,
            (lsb >> 32) as i32,
            lsb as i32,
        ];
        return Some(ops.create_int_list(ints));
    }
    // `BuiltinKey.toString()`: `<id>/<argument count>`.
    let mut key = JStringBuilder::new();
    key.push_jstring(id)
        .push_str(&format!("/{}", arguments.len()));
    state.error_here(errors::no_such_operation(key.build()));
    None
}

/// Why SNBT failed to parse.
pub type SnbtError = ParseFailure;

thread_local! {
    static NBT_GRAMMAR: Rc<Grammar<Tag>> = Rc::new(create_parser(NbtOps));
}

/// `TagParser.create(NbtOps.INSTANCE)`'s grammar, built once per thread.
#[must_use]
pub fn nbt_grammar() -> Rc<Grammar<Tag>> {
    NBT_GRAMMAR.with(Rc::clone)
}

fn syntax(message: Message, reader: &StringReader) -> ParseFailure {
    ParseFailure::Syntax(CommandSyntaxException::with_context(message, reader))
}

/// `TagParser.parseFully(reader)`: a value, then only whitespace.
///
/// # Errors
///
/// The grammar's error, or `argument.nbt.trailing` after the value.
pub fn parse_fully<T: Clone + 'static>(
    grammar: &Grammar<T>,
    reader: &mut StringReader,
) -> Result<T, ParseFailure> {
    let result = grammar.parse_for_commands(reader)?;
    reader.skip_whitespace();
    if reader.can_read() {
        return Err(syntax(errors::trailing(), reader));
    }
    Ok(result)
}

/// `TagParser.parseCompoundFully(input)`.
///
/// # Errors
///
/// As [`parse_fully`], or `argument.nbt.expected.compound` for another type of tag.
pub fn parse_compound_fully(input: &JString) -> Result<CompoundTag, ParseFailure> {
    let mut reader = StringReader::new(input.clone());
    match parse_fully(&nbt_grammar(), &mut reader)? {
        Tag::Compound(compound) => Ok(compound),
        _ => Err(syntax(errors::expected_compound(), &reader)),
    }
}

/// `TagParser.parseCompoundAsArgument(reader)`: a compound at the reader's cursor, leaving
/// the cursor after it.
///
/// # Errors
///
/// The grammar's error, or `argument.nbt.expected.compound`.
pub fn parse_compound_as_argument(reader: &mut StringReader) -> Result<CompoundTag, ParseFailure> {
    match nbt_grammar().parse_for_commands(reader)? {
        Tag::Compound(compound) => Ok(compound),
        _ => Err(syntax(errors::expected_compound(), reader)),
    }
}

/// Parses SNBT into a tag (`NBT_OPS_PARSER.parseFully(input)`).
///
/// # Errors
///
/// As [`parse_fully`].
pub fn parse_tag(input: &JString) -> Result<Tag, ParseFailure> {
    let mut reader = StringReader::new(input.clone());
    parse_fully(&nbt_grammar(), &mut reader)
}

/// The key and arguments of a parse failure's message, for tests and logs.
#[must_use]
pub fn failure_key(failure: &ParseFailure) -> Option<(&'static str, &[Arg])> {
    match failure {
        ParseFailure::Syntax(error) => match error.message() {
            Message::Translatable { key, args } => Some((key, args)),
            Message::Literal(_) => None,
        },
        ParseFailure::NoErrors => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Tag, ParseFailure> {
        parse_tag(&JString::from(s))
    }

    fn error(s: &str) -> (&'static str, usize) {
        let failure = parse(s).unwrap_err();
        let ParseFailure::Syntax(exception) = &failure else {
            panic!("{s}: no syntax error");
        };
        (
            failure_key(&failure).unwrap().0,
            exception.cursor() as usize,
        )
    }

    #[test]
    fn numbers() {
        assert_eq!(parse("1b"), Ok(Tag::Byte(1)));
        assert_eq!(parse("-32768s"), Ok(Tag::Short(i16::MIN)));
        assert_eq!(parse("0xFFub"), Ok(Tag::Byte(-1)));
        assert_eq!(parse("0b101"), Ok(Tag::Int(5)));
        assert_eq!(parse("1_000L"), Ok(Tag::Long(1000)));
        assert_eq!(parse("1.5f"), Ok(Tag::Float(1.5)));
        assert_eq!(parse(".5"), Ok(Tag::Double(0.5)));
        assert_eq!(parse("1e3"), Ok(Tag::Double(1000.0)));
        assert_eq!(parse("2d"), Ok(Tag::Double(2.0)));
        assert_eq!(error("012"), ("snbt.parser.leading_zero_not_allowed", 1));
        assert_eq!(
            error("-0x1"),
            ("snbt.parser.expected_non_negative_number", 4)
        );
        assert_eq!(error("1e999"), ("snbt.parser.infinity_not_allowed", 5));
    }

    #[test]
    fn strings_and_builtins() {
        assert_eq!(parse("abc"), Ok(Tag::string("abc")));
        assert_eq!(parse("TRUE"), Ok(Tag::Byte(1)));
        assert_eq!(parse("'it\\'s'"), Ok(Tag::string("it's")));
        assert_eq!(
            parse("\"\\u00e9\\N{SNOWMAN}\""),
            Ok(Tag::string("é\u{2603}"))
        );
        assert_eq!(parse("bool(2)"), Ok(Tag::Byte(1)));
        assert_eq!(
            parse("uuid(\"f81d4fae-7dec-11d0-a765-00a0c91e6bf6\")"),
            Ok(Tag::IntArray(vec![
                0xF81D_4FAE_u32 as i32,
                0x7DEC_11D0,
                0xA765_00A0_u32 as i32,
                0xC91E_6BF6_u32 as i32
            ]))
        );
        assert_eq!(error("nope(1)").0, "snbt.parser.no_such_operation");
        assert_eq!(
            parse("\"\\uD800\"").map(|t| t.as_string().unwrap().len()),
            Ok(1)
        );
    }

    #[test]
    fn containers() {
        let parsed = parse("{a: 1, b: [1, 'x'], c: [B; 1b, 2b], a: 3}").unwrap();
        let compound = parsed.as_compound().unwrap();
        assert_eq!(compound.get_int("a"), Some(3));
        assert_eq!(compound.get_list("b").unwrap().len(), 2);
        assert_eq!(compound.get_byte_array("c"), Some(&[1i8, 2][..]));
        assert_eq!(parse("[I;]"), Ok(Tag::IntArray(Vec::new())));
        assert_eq!(error("[B; 1s]").0, "snbt.parser.invalid_array_element_type");
        assert_eq!(error("{\"\": 1}").0, "snbt.parser.empty_key");
        assert_eq!(error("{a: 1} x"), ("argument.nbt.trailing", 7));
    }
}
