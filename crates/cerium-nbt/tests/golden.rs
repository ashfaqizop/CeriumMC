//! Replays what vanilla's `net.minecraft.nbt` did with random and malformed inputs, recorded in
//! CI by `parity/golden/NbtMain.java`, against `cerium_nbt`: written bytes (packet and file
//! forms), `toString`, the structure printer, `hashCode`, `sizeInBytes`, `copy()` order, quota
//! and depth accounting, exceptions with their crash-report details, the field visitors, and
//! SNBT parsing with its exact errors.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "golden arguments are Java ints converted to the Rust API's types"
)]

use cerium_brigadier::{Arg, Message};
use cerium_golden::{Case, Report, Value, load};
use cerium_java::JString;
use cerium_nbt::data::{ByteBufInput, ByteBufOutput, StreamInput, StreamOutput};
use cerium_nbt::print::{to_pretty_snbt, to_snbt};
use cerium_nbt::visitor::{CollectFields, FieldSelector, SkipFields, accept_as_root, parse_root};
use cerium_nbt::{NbtAccounter, NbtError, Tag, io, snbt};
use cerium_packrat::ParseFailure;

fn utf16(s: &str) -> Value {
    Value::Utf16(s.encode_utf16().collect())
}

fn jstring(s: &JString) -> Value {
    Value::Utf16(s.to_utf16())
}

fn network(tag: &Tag) -> Vec<u8> {
    let mut buffer = Vec::new();
    io::write_any_tag(tag, &mut ByteBufOutput::new(&mut buffer)).expect("writable");
    buffer
}

/// The recorder's exception tokens: class, message, and for a `ReportedNbtException` its
/// cause's class and message and the "Tag name"/"Tag type" crash-report details.
fn thrown(error: &NbtError) -> Vec<Value> {
    let message = |e: &NbtError| e.message().map_or(Value::Bool(false), |m| utf16(&m));
    let mut tokens = vec![utf16(error.java_class()), message(error)];
    if let NbtError::Reported {
        cause,
        tag_name,
        tag_type,
    } = error
    {
        tokens.push(utf16(cause.java_class()));
        tokens.push(message(cause));
        tokens.push(tag_name.as_ref().map_or_else(|| utf16("~none~"), jstring));
        tokens.push(utf16(tag_type));
    }
    tokens
}

#[test]
fn nbt_matches_vanilla() {
    let Some(cases) = load("nbt.txt") else {
        return;
    };
    let mut report = Report::new("nbt.txt");
    for case in &cases {
        let actual = match case.op.as_str() {
            "nbt.tag" => tag_case(case),
            "nbt.read" => read_case(case),
            "nbt.collect" => collect_case(case),
            "nbt.skip" => skip_case(case),
            "snbt.parse" => {
                let input = JString::from_utf16(case.utf16(0));
                snbt_result(snbt::parse_tag(&input))
            }
            "snbt.compound" => {
                let input = JString::from_utf16(case.utf16(0));
                snbt_result(snbt::parse_compound_fully(&input).map(Tag::Compound))
            }
            other => panic!("line {}: unknown operation {other}", case.line),
        };
        report.check(case, &actual);
    }
    report.finish();
}

fn tag_case(case: &Case) -> Vec<Value> {
    let input = case.bytes(0);
    let tag = io::read_any_tag(&mut ByteBufInput::new(input), &mut NbtAccounter::unlimited_heap())
        .expect("vanilla wrote it");
    let file = match &tag {
        Tag::Compound(compound) => {
            let mut output = StreamOutput::new(Vec::new());
            io::write_root(compound, &mut output).expect("writable");
            output.into_inner()
        }
        _ => Vec::new(),
    };
    vec![
        Value::Bytes(network(&tag)),
        Value::Bytes(file),
        jstring(&to_snbt(&tag)),
        jstring(&to_pretty_snbt(&tag)),
        Value::Int(tag.java_hash_code()),
        Value::Int(tag.size_in_bytes()),
        Value::Bytes(network(&tag.copy())),
    ]
}

fn read_case(case: &Case) -> Vec<Value> {
    let mode = case.int(0);
    let mut accounter = NbtAccounter::new(case.long(1), case.int(2));
    let bytes = case.bytes(3);
    let (result, consumed) = if mode == 0 {
        let mut input = ByteBufInput::new(bytes);
        let result = io::read_any_tag(&mut input, &mut accounter);
        (result, input.position())
    } else {
        let mut input = StreamInput::new(bytes);
        let result = match mode {
            1 => io::read_any_tag(&mut input, &mut accounter),
            2 => io::read(&mut input, &mut accounter).map(Tag::Compound),
            _ => io::read_unnamed_tag(&mut input, &mut accounter),
        };
        (result, bytes.len() - input.into_inner().len())
    };
    // Java ran out of memory allocating a huge declared size; CeriumMC must fail instead.
    if case.expected.first() == Some(&utf16("OutOfMemoryError")) {
        return match result {
            Err(_) => case.expected.clone(),
            Ok(tag) => vec![Value::Bytes(network(&tag))],
        };
    }
    match result {
        Ok(tag) => vec![
            Value::Bytes(network(&tag)),
            Value::Long(accounter.usage()),
            Value::Int(accounter.depth()),
            Value::Int(consumed as i32),
        ],
        Err(error) => thrown(&error),
    }
}

/// Parses the recorder's selectors: `type:path/.../name` joined by U+0001.
fn selectors(case: &Case, index: usize) -> Vec<FieldSelector> {
    let text = String::from_utf16(case.utf16(index)).expect("ASCII selectors");
    text.split('\u{1}')
        .map(|selector| {
            let (type_id, path) = selector.split_once(':').expect("type:path");
            let mut parts: Vec<&str> = path.split('/').collect();
            let name = parts.pop().expect("a name");
            FieldSelector {
                path: parts
                    .into_iter()
                    .filter(|p| !p.is_empty())
                    .map(JString::from)
                    .collect(),
                type_id: type_id.parse().expect("type id"),
                name: JString::from(name),
            }
        })
        .collect()
}

fn collect_case(case: &Case) -> Vec<Value> {
    let bytes = case.bytes(0);
    let mut visitor = CollectFields::new(&selectors(case, 1));
    let mut input = StreamInput::new(&bytes[1..]);
    let parsed = parse_root(
        bytes[0] as i8,
        &mut input,
        &mut visitor,
        &mut NbtAccounter::unlimited_heap(),
    );
    match parsed {
        Ok(()) => {
            let missing = visitor.missing_field_count() as i32;
            let result = visitor.result().unwrap_or(Tag::End);
            vec![Value::Bytes(network(&result)), Value::Int(missing)]
        }
        Err(error) => thrown(&error),
    }
}

fn skip_case(case: &Case) -> Vec<Value> {
    let tag = io::read_any_tag(
        &mut ByteBufInput::new(case.bytes(0)),
        &mut NbtAccounter::unlimited_heap(),
    )
    .expect("vanilla wrote it");
    let mut visitor = SkipFields::new(&selectors(case, 1));
    accept_as_root(&tag, &mut visitor);
    vec![Value::Bytes(network(&visitor.result().unwrap_or(Tag::End)))]
}

fn arg(value: &Arg) -> Value {
    match value {
        Arg::Str(s) => jstring(s),
        Arg::Int(v) => Value::Int(*v),
        Arg::Long(v) => Value::Long(*v),
        Arg::Float(v) => Value::Float(*v),
        Arg::Double(v) => Value::Double(*v),
        Arg::Bool(v) => utf16(&format!("?Boolean:{v}")),
    }
}

fn snbt_result(result: Result<Tag, ParseFailure>) -> Vec<Value> {
    match result {
        Ok(tag) => vec![Value::Bytes(network(&tag))],
        Err(ParseFailure::Syntax(error)) => match error.message() {
            Message::Translatable { key, args } => {
                let mut tokens = vec![Value::Str((*key).to_owned()), Value::Int(error.cursor())];
                tokens.extend(args.iter().map(arg));
                tokens
            }
            Message::Literal(text) => {
                let mut literal = String::from("literal:");
                literal.push_str(&text.to_string_lossy());
                vec![utf16(&literal)]
            }
        },
        Err(ParseFailure::NoErrors) => vec![
            utf16("IllegalStateException"),
            utf16("Failed to parse: "),
        ],
    }
}
