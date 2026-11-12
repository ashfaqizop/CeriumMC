//! Just enough JSON for the probe: reading fields out of a status response and writing
//! flat report objects. This is not a general-purpose JSON library.

use std::fmt::{Display, Write};

/// Returns the raw value of `key` inside the top-level `"version"` object of a status
/// response. String values are returned without their quotes and are not unescaped.
pub(crate) fn version_field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let object = object_value(json, "version")?;
    scalar_value(object, key)
}

/// Finds `"key"` followed by `:` and returns the text after the colon.
fn after_key<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let mut rest = json;
    while let Some(index) = rest.find(&needle) {
        rest = &rest[index + needle.len()..];
        if let Some(value) = rest.trim_start().strip_prefix(':') {
            return Some(value.trim_start());
        }
    }
    None
}

/// Returns the `{ ... }` object stored under `key`, braces included.
fn object_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let value = after_key(json, key)?;
    if !value.starts_with('{') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, c) in value.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&value[..=index]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Returns a string (without quotes) or number stored under `key`.
fn scalar_value<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let value = after_key(json, key)?;
    if let Some(string) = value.strip_prefix('"') {
        let end = string.find('"')?;
        return Some(&string[..end]);
    }
    let end = value
        .find(|c: char| !(c.is_ascii_digit() || c == '-'))
        .unwrap_or(value.len());
    if end > 0 { Some(&value[..end]) } else { None }
}

/// Writes a flat JSON object one field at a time.
#[derive(Debug, Default)]
pub(crate) struct ObjectWriter {
    out: String,
}

impl ObjectWriter {
    /// Adds a string field.
    pub(crate) fn string(&mut self, key: &str, value: &str) -> &mut Self {
        self.key(key);
        push_string(&mut self.out, value);
        self
    }

    /// Adds a number field.
    pub(crate) fn number(&mut self, key: &str, value: impl Display) -> &mut Self {
        self.key(key);
        let _ = write!(self.out, "{value}");
        self
    }

    /// Adds a number field, or `null` when there is no value.
    pub(crate) fn opt_number(&mut self, key: &str, value: Option<impl Display>) -> &mut Self {
        match value {
            Some(value) => self.number(key, value),
            None => self.null(key),
        }
    }

    /// Adds a string field, or `null` when there is no value.
    pub(crate) fn opt_string(&mut self, key: &str, value: Option<&str>) -> &mut Self {
        match value {
            Some(value) => self.string(key, value),
            None => self.null(key),
        }
    }

    /// Returns the finished object.
    pub(crate) fn finish(&self) -> String {
        format!("{{{}}}", self.out)
    }

    fn null(&mut self, key: &str) -> &mut Self {
        self.key(key);
        self.out.push_str("null");
        self
    }

    fn key(&mut self, key: &str) {
        if !self.out.is_empty() {
            self.out.push(',');
        }
        push_string(&mut self.out, key);
        self.out.push(':');
    }
}

/// Appends `value` as a JSON string literal.
fn push_string(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = r#"{"description":{"text":"say \"version\": hi"},
        "players":{"max":20,"online":1,"sample":[{"name":"Steve","id":"x"}]},
        "version":{"name":"26.3","protocol":777},"enforcesSecureChat":false}"#;

    #[test]
    fn reads_version_name_and_protocol() {
        assert_eq!(version_field(STATUS, "name"), Some("26.3"));
        assert_eq!(version_field(STATUS, "protocol"), Some("777"));
    }

    #[test]
    fn missing_version_object_yields_none() {
        assert_eq!(version_field(r#"{"players":{}}"#, "name"), None);
    }

    #[test]
    fn writes_escaped_flat_objects() {
        let mut object = ObjectWriter::default();
        object
            .string("label", "vanilla \"26.3\"\n")
            .number("time_ms", 1234_u128)
            .opt_number("rss_kib", None::<u64>)
            .opt_string("name", Some("a\\b"));
        assert_eq!(
            object.finish(),
            r#"{"label":"vanilla \"26.3\"\n","time_ms":1234,"rss_kib":null,"name":"a\\b"}"#
        );
    }

    #[test]
    fn control_characters_are_unicode_escaped() {
        let mut out = String::new();
        push_string(&mut out, "\u{1}");
        assert_eq!(out, "\"\\u0001\"");
    }
}
