//! Printing tags as SNBT: `StringTagVisitor` ([`to_snbt`], what `Tag.toString()` returns) and
//! `SnbtPrinterTagVisitor` ([`to_pretty_snbt`], the indented form of structure files), plus
//! `StringTag`'s quoting and escaping.
//!
//! Output is a [`JString`], since string values may contain unpaired surrogates. Compound keys
//! are sorted by `String.compareTo` (UTF-16 order), and numbers use Java's `toString`.

use cerium_java::to_decimal::{double_to_string, float_to_string};
use cerium_java::{JString, JStringBuilder};

use crate::tag::{CompoundTag, ListTag, Tag};

/// `SnbtGrammar.escapeControlCharacters`: the escape letter(s) for a control character.
#[must_use]
pub fn escape_control_character(c: u16) -> Option<String> {
    Some(match c {
        0x08 => "b".to_owned(),
        0x09 => "t".to_owned(),
        0x0A => "n".to_owned(),
        0x0C => "f".to_owned(),
        0x0D => "r".to_owned(),
        0x00..=0x1F => format!("x{c:02X}"),
        _ => return None,
    })
}

const BACKSLASH: u16 = b'\\' as u16;
const DOUBLE_QUOTE: u16 = b'"' as u16;
const SINGLE_QUOTE: u16 = b'\'' as u16;

/// `StringTag.quoteAndEscape(input, builder)`: quotes with `"` unless the first quote
/// character inside is `"`, in which case `'`, escaping that quote, backslashes and control
/// characters.
pub fn quote_and_escape_into(input: &JString, out: &mut JStringBuilder) {
    let quote_mark_index = out.len();
    out.push_unit(u16::from(b' '));
    let mut quote = 0u16;
    for c in input.units() {
        if c == BACKSLASH {
            out.push_str("\\\\");
        } else if c != DOUBLE_QUOTE && c != SINGLE_QUOTE {
            if let Some(escaped) = escape_control_character(c) {
                out.push_unit(BACKSLASH).push_str(&escaped);
            } else {
                out.push_unit(c);
            }
        } else {
            if quote == 0 {
                quote = if c == DOUBLE_QUOTE {
                    SINGLE_QUOTE
                } else {
                    DOUBLE_QUOTE
                };
            }
            if quote == c {
                out.push_unit(BACKSLASH);
            }
            out.push_unit(c);
        }
    }
    if quote == 0 {
        quote = DOUBLE_QUOTE;
    }
    out.set_unit(quote_mark_index, quote);
    out.push_unit(quote);
}

/// `StringTag.quoteAndEscape(input)`.
#[must_use]
pub fn quote_and_escape(input: &JString) -> JString {
    let mut out = JStringBuilder::new();
    quote_and_escape_into(input, &mut out);
    out.build()
}

/// `StringTag.escapeWithoutQuotes(input)`: escapes both quotes, backslashes and control
/// characters.
#[must_use]
pub fn escape_without_quotes(input: &JString) -> JString {
    let mut out = JStringBuilder::new();
    for c in input.units() {
        if c == DOUBLE_QUOTE || c == SINGLE_QUOTE || c == BACKSLASH {
            out.push_unit(BACKSLASH).push_unit(c);
        } else if let Some(escaped) = escape_control_character(c) {
            out.push_unit(BACKSLASH).push_str(&escaped);
        } else {
            out.push_unit(c);
        }
    }
    out.build()
}

fn is_ascii_letter(c: u16) -> bool {
    matches!(c, 0x41..=0x5A | 0x61..=0x7A)
}

fn is_simple_char(c: u16) -> bool {
    is_ascii_letter(c) || matches!(c, 0x30..=0x39 | 0x2E | 0x5F | 0x2B | 0x2D)
}

/// `StringTagVisitor.UNQUOTED_KEY_MATCH`: `[A-Za-z._]+[A-Za-z0-9._+-]*`.
fn is_unquoted_key(key: &JString) -> bool {
    let mut units = key.units();
    units
        .next()
        .is_some_and(|first| is_ascii_letter(first) || first == 0x2E || first == 0x5F)
        && units.all(is_simple_char)
}

/// The keys of a compound in `Entry.comparingByKey()` order.
fn sorted_entries(compound: &CompoundTag) -> Vec<(&JString, &Tag)> {
    let mut entries: Vec<_> = compound.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
}

/// `Tag.toString()`: compact SNBT from `StringTagVisitor`.
#[must_use]
pub fn to_snbt(tag: &Tag) -> JString {
    let mut out = JStringBuilder::new();
    write_compact(tag, &mut out);
    out.build()
}

fn push_joined<T>(
    out: &mut JStringBuilder,
    items: &[T],
    mut item: impl FnMut(&mut JStringBuilder, &T),
) {
    for (i, value) in items.iter().enumerate() {
        if i != 0 {
            out.push_unit(u16::from(b','));
        }
        item(out, value);
    }
}

fn write_compact(tag: &Tag, out: &mut JStringBuilder) {
    match tag {
        Tag::String(s) => quote_and_escape_into(s, out),
        Tag::Byte(v) => {
            out.push_str(&format!("{v}b"));
        }
        Tag::Short(v) => {
            out.push_str(&format!("{v}s"));
        }
        Tag::Int(v) => {
            out.push_str(&v.to_string());
        }
        Tag::Long(v) => {
            out.push_str(&format!("{v}L"));
        }
        Tag::Float(v) => {
            out.push_str(&float_to_string(*v)).push_str("f");
        }
        Tag::Double(v) => {
            out.push_str(&double_to_string(*v)).push_str("d");
        }
        Tag::ByteArray(data) => {
            out.push_str("[B;");
            push_joined(out, data, |out, v| {
                out.push_str(&format!("{v}B"));
            });
            out.push_str("]");
        }
        Tag::IntArray(data) => {
            out.push_str("[I;");
            push_joined(out, data, |out, v| {
                out.push_str(&v.to_string());
            });
            out.push_str("]");
        }
        Tag::LongArray(data) => {
            out.push_str("[L;");
            push_joined(out, data, |out, v| {
                out.push_str(&format!("{v}L"));
            });
            out.push_str("]");
        }
        Tag::List(list) => {
            out.push_str("[");
            for (i, element) in list.iter().enumerate() {
                if i != 0 {
                    out.push_str(",");
                }
                write_compact(element, out);
            }
            out.push_str("]");
        }
        Tag::Compound(compound) => {
            out.push_str("{");
            for (i, (key, value)) in sorted_entries(compound).into_iter().enumerate() {
                if i != 0 {
                    out.push_str(",");
                }
                // `handleKeyEscape`. Java's `equalsIgnoreCase` also matches some non-ASCII
                // spellings of true/false, but those fail the unquoted-key pattern anyway.
                if !key.equals_ignore_ascii_case("true")
                    && !key.equals_ignore_ascii_case("false")
                    && is_unquoted_key(key)
                {
                    out.push_jstring(key);
                } else {
                    quote_and_escape_into(key, out);
                }
                out.push_str(":");
                write_compact(value, out);
            }
            out.push_str("}");
        }
        Tag::End => {
            out.push_str("END");
        }
    }
}

/// `SnbtPrinterTagVisitor.KEY_ORDER`: the keys that structure files list first, by path.
fn key_order(path: &str) -> Option<&'static [&'static str]> {
    match path {
        "{}" => Some(&[
            "DataVersion",
            "author",
            "size",
            "data",
            "entities",
            "palette",
            "palettes",
        ]),
        "{}.data.[].{}" => Some(&["pos", "state", "nbt"]),
        "{}.entities.[].{}" => Some(&["blockPos", "pos"]),
        _ => None,
    }
}

/// `SnbtPrinterTagVisitor.NO_INDENTATION`: the paths printed on one line.
fn no_indentation(path: &str) -> bool {
    matches!(
        path,
        "{}.size.[]" | "{}.data.[].{}" | "{}.palette.[].{}" | "{}.entities.[].{}"
    )
}

/// `SnbtPrinterTagVisitor.SIMPLE_VALUE`: `[A-Za-z0-9._+-]+`.
fn is_simple_value(key: &JString) -> bool {
    !key.is_empty() && key.units().all(is_simple_char)
}

/// `new SnbtPrinterTagVisitor().visit(tag)`: SNBT indented by four spaces, as structure
/// files are saved, with their key order and one-line paths.
#[must_use]
pub fn to_pretty_snbt(tag: &Tag) -> JString {
    let mut path = Vec::new();
    Pretty {
        indentation: "    ",
        depth: 0,
    }
    .visit(tag, &mut path)
}

/// One `SnbtPrinterTagVisitor`: an indentation, a depth, and the shared path.
struct Pretty<'a> {
    indentation: &'a str,
    depth: usize,
}

impl Pretty<'_> {
    fn visit(&self, tag: &Tag, path: &mut Vec<JString>) -> JString {
        let mut out = JStringBuilder::new();
        match tag {
            Tag::String(s) => quote_and_escape_into(s, &mut out),
            Tag::Byte(v) => {
                out.push_str(&format!("{v}b"));
            }
            Tag::Short(v) => {
                out.push_str(&format!("{v}s"));
            }
            Tag::Int(v) => {
                out.push_str(&v.to_string());
            }
            Tag::Long(v) => {
                out.push_str(&format!("{v}L"));
            }
            Tag::Float(v) => {
                out.push_str(&float_to_string(*v)).push_str("f");
            }
            Tag::Double(v) => {
                out.push_str(&double_to_string(*v)).push_str("d");
            }
            Tag::ByteArray(data) => array(&mut out, "B", data, |v| format!("{v}B")),
            Tag::IntArray(data) => array(&mut out, "I", data, ToString::to_string),
            Tag::LongArray(data) => array(&mut out, "L", data, |v| format!("{v}L")),
            Tag::List(list) => self.list(list, path, &mut out),
            Tag::Compound(compound) => self.compound(compound, path, &mut out),
            // `visitEnd` leaves the result empty.
            Tag::End => {}
        }
        out.build()
    }

    fn path_string(path: &[JString]) -> String {
        path.iter()
            .map(JString::to_string_lossy)
            .collect::<Vec<_>>()
            .join(".")
    }

    fn list(&self, list: &ListTag, path: &mut Vec<JString>, out: &mut JStringBuilder) {
        if list.is_empty() {
            out.push_str("[]");
            return;
        }
        out.push_str("[");
        path.push(JString::from("[]"));
        let indentation = if no_indentation(&Self::path_string(path)) {
            ""
        } else {
            self.indentation
        };
        if !indentation.is_empty() {
            out.push_str("\n");
        }
        let child = Pretty {
            indentation,
            depth: self.depth + 1,
        };
        for (i, element) in list.iter().enumerate() {
            out.push_str(&indentation.repeat(self.depth + 1));
            out.push_jstring(&child.visit(element, path));
            if i != list.len() - 1 {
                out.push_str(",")
                    .push_str(if indentation.is_empty() { " " } else { "\n" });
            }
        }
        if !indentation.is_empty() {
            out.push_str("\n").push_str(&indentation.repeat(self.depth));
        }
        out.push_str("]");
        path.pop();
    }

    fn compound(&self, compound: &CompoundTag, path: &mut Vec<JString>, out: &mut JStringBuilder) {
        if compound.is_empty() {
            out.push_str("{}");
            return;
        }
        out.push_str("{");
        path.push(JString::from("{}"));
        let path_string = Self::path_string(path);
        let indentation = if no_indentation(&path_string) {
            ""
        } else {
            self.indentation
        };
        if !indentation.is_empty() {
            out.push_str("\n");
        }
        let keys = Self::keys(compound, &path_string);
        let child = Pretty {
            indentation,
            depth: self.depth + 1,
        };
        for (i, key) in keys.iter().enumerate() {
            let value = compound.get_j(key).expect("listed key");
            path.push(key.clone());
            out.push_str(&indentation.repeat(self.depth + 1));
            // `handleEscapePretty`.
            if is_simple_value(key) {
                out.push_jstring(key);
            } else {
                quote_and_escape_into(key, out);
            }
            out.push_str(": ").push_jstring(&child.visit(value, path));
            path.pop();
            if i != keys.len() - 1 {
                out.push_str(",")
                    .push_str(if indentation.is_empty() { " " } else { "\n" });
            }
        }
        if !indentation.is_empty() {
            out.push_str("\n").push_str(&indentation.repeat(self.depth));
        }
        out.push_str("}");
        path.pop();
    }

    /// `getKeys`: the path's preferred keys first (when present), then the rest sorted.
    fn keys(compound: &CompoundTag, path: &str) -> Vec<JString> {
        let mut rest: Vec<JString> = compound.keys().cloned().collect();
        let mut keys = Vec::with_capacity(rest.len());
        if let Some(order) = key_order(path) {
            for key in order {
                if let Some(index) = rest.iter().position(|k| k == key) {
                    keys.push(rest.swap_remove(index));
                }
            }
        }
        rest.sort();
        keys.extend(rest);
        keys
    }
}

/// An array as `SnbtPrinterTagVisitor` prints it: `[B; 1B, 2B]`.
fn array<T>(out: &mut JStringBuilder, prefix: &str, data: &[T], item: impl Fn(&T) -> String) {
    out.push_str("[").push_str(prefix).push_str(";");
    for (i, value) in data.iter().enumerate() {
        out.push_str(" ").push_str(&item(value));
        if i != data.len() - 1 {
            out.push_str(",");
        }
    }
    out.push_str("]");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compound(entries: &[(&str, Tag)]) -> CompoundTag {
        let mut compound = CompoundTag::new();
        for (key, tag) in entries {
            compound.put(*key, tag.clone());
        }
        compound
    }

    #[test]
    fn quoting_picks_the_other_quote() {
        let q = |s: &str| quote_and_escape(&JString::from(s)).to_string_lossy();
        assert_eq!(q("plain"), "\"plain\"");
        assert_eq!(q("say \"hi\""), "'say \"hi\"'");
        assert_eq!(q("it's \"x\""), "\"it's \\\"x\\\"\"");
        assert_eq!(q("a\\b\n\u{1}"), "\"a\\\\b\\n\\x01\"");
    }

    #[test]
    fn compact_snbt_matches_tostring() {
        let tag = Tag::Compound(compound(&[
            ("b", Tag::Byte(-1)),
            ("a key", Tag::Float(1.0)),
            ("true", Tag::Double(0.0001)),
            ("Z", Tag::LongArray(vec![1, -2])),
            (
                "list",
                Tag::List(ListTag::from_vec(vec![Tag::Short(3), Tag::End])),
            ),
        ]));
        assert_eq!(
            to_snbt(&tag).to_string_lossy(),
            "{Z:[L;1L,-2L],\"a key\":1.0f,b:-1b,list:[3s,END],\"true\":1.0E-4d}"
        );
    }

    #[test]
    fn pretty_snbt_follows_structure_layout() {
        let pos = Tag::List(ListTag::from_vec(vec![
            Tag::Int(1),
            Tag::Int(2),
            Tag::Int(3),
        ]));
        let block = compound(&[("state", Tag::Int(0)), ("pos", pos.clone())]);
        let tag = Tag::Compound(compound(&[
            ("size", pos),
            (
                "data",
                Tag::List(ListTag::from_vec(vec![Tag::Compound(block)])),
            ),
            ("DataVersion", Tag::Int(5023)),
            ("zz", Tag::ByteArray(vec![1, 2])),
        ]));
        let expected = "{\n    DataVersion: 5023,\n    size: [1, 2, 3],\n    data: [\n        \
                        {pos: [1, 2, 3], state: 0}\n    ],\n    zz: [B; 1B, 2B]\n}";
        assert_eq!(to_pretty_snbt(&tag).to_string_lossy(), expected);
    }
}
