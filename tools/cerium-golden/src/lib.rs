//! Golden test cases recorded from the vanilla reference server.
//!
//! In CI, a Java harness (`parity/golden/`) is compiled against the official server jar. It
//! calls vanilla code with many inputs and records each call and its result. Rust tests
//! replay the calls against CeriumMC and require identical results.
//!
//! The recorded data is derived from Mojang's code, so it is never committed. Tests find
//! it through environment variables:
//!
//! - `CERIUM_GOLDEN_DIR`: the directory with the recorded files. When unset, golden tests
//!   are skipped.
//! - `CERIUM_REQUIRE_GOLDEN=1`: fail instead of skipping when the data is missing.
//!
//! # File format
//!
//! One case per line: `op arg… = result…`. Each value is a typed token:
//!
//! | Token | Meaning |
//! |---|---|
//! | `i:-12` | `int` |
//! | `l:-12` | `long` |
//! | `b:-12` | `byte` |
//! | `z:1` | `boolean` |
//! | `f:3f800000` | `float`, raw IEEE-754 bits in hex |
//! | `d:3ff0000000000000` | `double`, raw IEEE-754 bits in hex |
//! | `s:6869` | string, UTF-8 bytes in hex |
//! | `u:00680069` | Java string, UTF-16 code units in hex (4 digits each), lone surrogates allowed |
//! | `x:0a00` | bytes in hex |
//!
//! Lines starting with `#` are comments.

use std::fmt;
use std::path::PathBuf;

/// A typed value from a golden file.
#[derive(Debug, Clone)]
pub enum Value {
    /// A Java `int`.
    Int(i32),
    /// A Java `long`.
    Long(i64),
    /// A Java `byte`.
    Byte(i8),
    /// A Java `boolean`.
    Bool(bool),
    /// A Java `float`.
    Float(f32),
    /// A Java `double`.
    Double(f64),
    /// A Java `String`.
    Str(String),
    /// A Java `String` as UTF-16 code units, which may include unpaired surrogates.
    Utf16(Vec<u16>),
    /// Raw bytes.
    Bytes(Vec<u8>),
}

impl PartialEq for Value {
    /// Floats compare by their bits, except that any NaN equals any NaN: x86 and ARM
    /// produce different NaN bit patterns, and Java doesn't canonicalize them either.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Long(a), Self::Long(b)) => a == b,
            (Self::Byte(a), Self::Byte(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => {
                (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
            }
            (Self::Double(a), Self::Double(b)) => {
                (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
            }
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Utf16(a), Self::Utf16(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(v) => write!(f, "i:{v}"),
            Self::Long(v) => write!(f, "l:{v}"),
            Self::Byte(v) => write!(f, "b:{v}"),
            Self::Bool(v) => write!(f, "z:{}", u8::from(*v)),
            Self::Float(v) => write!(f, "f:{:08x} ({v:?})", v.to_bits()),
            Self::Double(v) => write!(f, "d:{:016x} ({v:?})", v.to_bits()),
            Self::Str(v) => write!(f, "s:{v:?}"),
            Self::Utf16(v) => write!(f, "u:{:?}", String::from_utf16_lossy(v)),
            Self::Bytes(v) => {
                f.write_str("x:")?;
                v.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
            }
        }
    }
}

impl Value {
    /// Parses one typed token.
    ///
    /// # Errors
    ///
    /// Returns a description of the problem if the token is malformed.
    pub fn parse(token: &str) -> Result<Self, String> {
        let (kind, raw) = token
            .split_once(':')
            .ok_or_else(|| format!("token {token:?} has no type prefix"))?;
        let bad = |error: &dyn fmt::Display| format!("bad {kind} token {raw:?}: {error}");
        Ok(match kind {
            "i" => Self::Int(raw.parse().map_err(|e| bad(&e))?),
            "l" => Self::Long(raw.parse().map_err(|e| bad(&e))?),
            "b" => Self::Byte(raw.parse().map_err(|e| bad(&e))?),
            "z" => Self::Bool(raw == "1"),
            "f" => {
                let bits = u32::from_str_radix(raw, 16).map_err(|e| bad(&e))?;
                Self::Float(f32::from_bits(bits))
            }
            "d" => {
                let bits = u64::from_str_radix(raw, 16).map_err(|e| bad(&e))?;
                Self::Double(f64::from_bits(bits))
            }
            "s" => Self::Str(decode_hex_string(raw).map_err(|e| bad(&e))?),
            "u" => Self::Utf16(decode_hex_units(raw).map_err(|e| bad(&e))?),
            "x" => Self::Bytes(decode_hex_bytes(raw).map_err(|e| bad(&e))?),
            _ => return Err(format!("unknown token type in {token:?}")),
        })
    }
}

fn decode_hex_bytes(raw: &str) -> Result<Vec<u8>, String> {
    if !raw.len().is_multiple_of(2) {
        return Err("odd number of hex digits".to_owned());
    }
    (0..raw.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&raw[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| e.to_string())
}

fn decode_hex_string(raw: &str) -> Result<String, String> {
    String::from_utf8(decode_hex_bytes(raw)?).map_err(|e| e.to_string())
}

fn decode_hex_units(raw: &str) -> Result<Vec<u16>, String> {
    if !raw.len().is_multiple_of(4) {
        return Err("hex digit count is not a multiple of 4".to_owned());
    }
    (0..raw.len())
        .step_by(4)
        .map(|i| u16::from_str_radix(&raw[i..i + 4], 16))
        .collect::<Result<Vec<u16>, _>>()
        .map_err(|e| e.to_string())
}

/// One recorded call.
#[derive(Debug, Clone)]
pub struct Case {
    /// Line number in the golden file, for error messages.
    pub line: usize,
    /// The operation name.
    pub op: String,
    /// The arguments.
    pub args: Vec<Value>,
    /// The results vanilla produced.
    pub expected: Vec<Value>,
}

/// Generates a typed accessor for `Copy` case arguments.
macro_rules! accessor {
    ($name:ident, $variant:ident, $ty:ty) => {
        #[doc = concat!("Argument `index` as `", stringify!($ty), "`.")]
        ///
        /// # Panics
        ///
        /// Panics if the argument is missing or has another type.
        #[must_use]
        pub fn $name(&self, index: usize) -> $ty {
            match self.args.get(index) {
                Some(Value::$variant(value)) => *value,
                other => panic!(
                    "line {}: {} argument {index} should be {}, found {other:?}",
                    self.line,
                    self.op,
                    stringify!($variant)
                ),
            }
        }
    };
}

impl Case {
    accessor!(int, Int, i32);
    accessor!(long, Long, i64);
    accessor!(byte, Byte, i8);
    accessor!(bool, Bool, bool);
    accessor!(float, Float, f32);
    accessor!(double, Double, f64);

    /// Argument `index` as UTF-16 code units (a `u:` token).
    ///
    /// # Panics
    ///
    /// Panics if the argument is missing or has another type.
    #[must_use]
    pub fn utf16(&self, index: usize) -> &[u16] {
        match self.args.get(index) {
            Some(Value::Utf16(value)) => value,
            other => panic!(
                "line {}: {} argument {index} should be Utf16, found {other:?}",
                self.line, self.op
            ),
        }
    }

    /// Argument `index` as bytes (an `x:` token).
    ///
    /// # Panics
    ///
    /// Panics if the argument is missing or has another type.
    #[must_use]
    pub fn bytes(&self, index: usize) -> &[u8] {
        match self.args.get(index) {
            Some(Value::Bytes(value)) => value,
            other => panic!(
                "line {}: {} argument {index} should be Bytes, found {other:?}",
                self.line, self.op
            ),
        }
    }

    /// Argument `index` as a `String`.
    ///
    /// # Panics
    ///
    /// Panics if the argument is missing or has another type.
    #[must_use]
    pub fn string(&self, index: usize) -> String {
        match self.args.get(index) {
            Some(Value::Str(value)) => value.clone(),
            other => panic!(
                "line {}: {} argument {index} should be Str, found {other:?}",
                self.line, self.op
            ),
        }
    }
}

/// Parses golden-file text into cases.
///
/// # Errors
///
/// Returns a description of the first malformed line.
pub fn parse(text: &str) -> Result<Vec<Case>, String> {
    let mut cases = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (call, results) = line.split_once(" = ").unwrap_or((line, ""));
        let mut tokens = call.split_whitespace();
        let op = tokens
            .next()
            .ok_or_else(|| format!("line {line_number}: missing op"))?
            .to_owned();
        let args = parse_values(tokens, line_number)?;
        let expected = parse_values(results.split_whitespace(), line_number)?;
        cases.push(Case {
            line: line_number,
            op,
            args,
            expected,
        });
    }
    Ok(cases)
}

fn parse_values<'a>(
    tokens: impl Iterator<Item = &'a str>,
    line_number: usize,
) -> Result<Vec<Value>, String> {
    tokens
        .map(Value::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("line {line_number}: {e}"))
}

/// Loads `file` from `CERIUM_GOLDEN_DIR`. Returns `None` (after printing a note) when golden
/// data isn't configured, so the calling test can return early.
///
/// # Panics
///
/// Panics if the data is required but missing, or if the file can't be read or parsed.
#[must_use]
pub fn load(file: &str) -> Option<Vec<Case>> {
    let required = std::env::var("CERIUM_REQUIRE_GOLDEN").is_ok_and(|v| v == "1");
    let Some(dir) = std::env::var_os("CERIUM_GOLDEN_DIR") else {
        assert!(
            !required,
            "CERIUM_REQUIRE_GOLDEN=1 but CERIUM_GOLDEN_DIR is not set"
        );
        eprintln!("skipping golden test {file}: CERIUM_GOLDEN_DIR is not set");
        return None;
    };
    let path = PathBuf::from(dir).join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let cases = parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_ne!(cases.len(), 0, "{} contains no cases", path.display());
    Some(cases)
}

/// The CPUID vendor string (`GenuineIntel`, `AuthenticAMD`, …) of the machine that recorded
/// the golden data, from `cpu-vendor.txt` (written by `ci/cpu-vendor.sh`). HotSpot's
/// `Math.log`, `log10` and `pow` results depend on it. Returns `None` when golden data isn't
/// configured.
///
/// # Panics
///
/// Panics if golden data is configured but `cpu-vendor.txt` can't be read.
#[must_use]
pub fn cpu_vendor() -> Option<String> {
    let dir = std::env::var_os("CERIUM_GOLDEN_DIR")?;
    let path = PathBuf::from(dir).join("cpu-vendor.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    Some(text.trim().to_owned())
}

/// Collects mismatches between vanilla's results and CeriumMC's, and reports them together.
#[derive(Debug)]
pub struct Report {
    file: String,
    checked: usize,
    failures: Vec<String>,
}

/// How many mismatches are printed in full.
const MAX_SHOWN: usize = 25;

impl Report {
    /// Starts a report for `file`.
    #[must_use]
    pub fn new(file: &str) -> Self {
        Self {
            file: file.to_owned(),
            checked: 0,
            failures: Vec::new(),
        }
    }

    /// Compares CeriumMC's `actual` results with the case's expected results.
    pub fn check(&mut self, case: &Case, actual: &[Value]) {
        self.checked += 1;
        if case.expected.as_slice() != actual {
            let args = join(&case.args);
            let expected = join(&case.expected);
            let actual = join(actual);
            self.failures.push(format!(
                "line {}: {} {args}\n    vanilla:  {expected}\n    ceriummc: {actual}",
                case.line, case.op
            ));
        }
    }

    /// Prints a summary.
    ///
    /// # Panics
    ///
    /// Panics if any case mismatched.
    pub fn finish(self) {
        let failed = self.failures.len();
        eprintln!(
            "golden {}: {} cases checked, {failed} mismatched",
            self.file, self.checked
        );
        if failed > 0 {
            let shown = self.failures[..failed.min(MAX_SHOWN)].join("\n");
            panic!(
                "{failed} of {} golden cases in {} differ from vanilla:\n{shown}",
                self.checked, self.file
            );
        }
    }
}

fn join(values: &[Value]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_token_type() {
        let text = "# comment\nop i:-3 l:9 b:-1 z:1 f:3f800000 d:4000000000000000 s:6869 \
            u:0068d800 x:0aff = i:7\n";
        let cases = parse(text).unwrap();
        assert_eq!(cases.len(), 1);
        let case = &cases[0];
        assert_eq!(case.line, 2);
        assert_eq!(case.op, "op");
        assert_eq!(case.int(0), -3);
        assert_eq!(case.long(1), 9);
        assert_eq!(case.byte(2), -1);
        assert!(case.bool(3));
        assert_eq!(case.float(4).to_bits(), 1.0_f32.to_bits());
        assert_eq!(case.double(5).to_bits(), 2.0_f64.to_bits());
        assert_eq!(case.string(6), "hi");
        assert_eq!(case.utf16(7), [0x68, 0xD800]);
        assert_eq!(case.bytes(8), [0x0A, 0xFF]);
        assert_eq!(case.expected, [Value::Int(7)]);
    }

    #[test]
    fn any_nan_equals_any_nan() {
        let x86_nan = Value::Float(f32::from_bits(0xFFC0_0000));
        let arm_nan = Value::Float(f32::from_bits(0x7FC0_0000));
        assert_eq!(x86_nan, arm_nan);
        assert_ne!(Value::Float(0.0), Value::Float(-0.0));
    }

    #[test]
    fn report_collects_mismatches() {
        let cases = parse("op i:1 = i:2\n").unwrap();
        let mut report = Report::new("test");
        report.check(&cases[0], &[Value::Int(2)]);
        assert_eq!(report.failures, Vec::<String>::new());
        report.check(&cases[0], &[Value::Int(3)]);
        assert_eq!(report.failures.len(), 1);
    }
}
