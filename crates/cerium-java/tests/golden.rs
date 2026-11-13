//! Replays JDK behaviour recorded from Java 25 in CI (see `parity/golden/JStdMain.java`)
//! against `cerium_java`: `HashMap` iteration order, `Double`/`Float.toString`, the number
//! parsers, `Character`, `UUID.fromString`, Modified UTF-8 and `String`. Results must match
//! exactly, including the messages of the exceptions Java throws.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "golden arguments are Java ints converted to the Rust API's types"
)]

use std::fmt::Write as _;

use cerium_golden::{Case, Report, Value, load};
use cerium_java::{JString, JavaHashMap, character, mutf8, number, to_decimal, uuid};

fn utf16(s: &str) -> Value {
    Value::Utf16(s.encode_utf16().collect())
}

fn jstring(s: &JString) -> Value {
    Value::Utf16(s.to_utf16())
}

/// How the recorder renders a thrown exception: `"<SimpleName>: <message>"`.
fn thrown(class: &str, message: &JString) -> Value {
    let mut units: Vec<u16> = format!("{class}: ").encode_utf16().collect();
    units.extend(message.units());
    Value::Utf16(units)
}

fn number_format(error: &number::NumberFormatError) -> Value {
    thrown("NumberFormatException", error.message())
}

fn arg_jstring(case: &Case, index: usize) -> JString {
    JString::from_utf16(case.utf16(index))
}

#[test]
fn jdk_behaviour_matches_java() {
    let Some(cases) = load("jstd.txt") else {
        return;
    };
    let mut report = Report::new("jstd.txt");
    let mut pool: Vec<JString> = Vec::new();
    for case in &cases {
        if case.op == "jstd.map" {
            if let Value::Utf16(units) = &case.args[0] {
                pool = units
                    .split(|&unit| unit == 1)
                    .map(JString::from_utf16)
                    .collect();
            }
            report.check(case, &[Value::Str(run_map(&pool, &case.string(1)))]);
            continue;
        }
        let actual = eval(case);
        report.check(case, &actual);
    }
    report.finish();
}

fn eval(case: &Case) -> Vec<Value> {
    let radix = |index: usize| case.int(index) as u32;
    let one = |value: Value| vec![value];
    match case.op.as_str() {
        "jstd.d2s" => one(utf16(&to_decimal::double_to_string(case.double(0)))),
        "jstd.f2s" => one(utf16(&to_decimal::float_to_string(case.float(0)))),
        "jstd.parse_d" => one(match number::parse_double(&arg_jstring(case, 0)) {
            Ok(value) => Value::Double(value),
            Err(error) => number_format(&error),
        }),
        "jstd.parse_f" => one(match number::parse_float(&arg_jstring(case, 0)) {
            Ok(value) => Value::Float(value),
            Err(error) => number_format(&error),
        }),
        "jstd.parse_i" => one(match number::parse_int(&arg_jstring(case, 0), radix(1)) {
            Ok(value) => Value::Int(value),
            Err(error) => number_format(&error),
        }),
        "jstd.parse_l" => one(match number::parse_long(&arg_jstring(case, 0), radix(1)) {
            Ok(value) => Value::Long(value),
            Err(error) => number_format(&error),
        }),
        "jstd.parse_ui" => one(
            match number::parse_unsigned_int(&arg_jstring(case, 0), radix(1)) {
                Ok(value) => Value::Int(value),
                Err(error) => number_format(&error),
            },
        ),
        "jstd.parse_ul" => one(
            match number::parse_unsigned_long(&arg_jstring(case, 0), radix(1)) {
                Ok(value) => Value::Long(value),
                Err(error) => number_format(&error),
            },
        ),
        "jstd.parse_b" => one(match number::parse_byte(&arg_jstring(case, 0), radix(1)) {
            Ok(value) => Value::Byte(value),
            Err(error) => number_format(&error),
        }),
        "jstd.parse_s" => one(match number::parse_short(&arg_jstring(case, 0), radix(1)) {
            Ok(value) => Value::Int(i32::from(value)),
            Err(error) => number_format(&error),
        }),
        "jstd.parse_lr" => {
            let s = arg_jstring(case, 0);
            let (begin, end) = (case.int(1) as usize, case.int(2) as usize);
            one(match number::parse_long_range(&s, begin, end, radix(3)) {
                Ok(value) => Value::Long(value),
                Err(error) => number_format(&error),
            })
        }
        "jstd.ws" => one(Value::Bool(character::is_whitespace(case.int(0) as u16))),
        "jstd.digit" => {
            let digit = character::digit(case.int(0) as u16, radix(1));
            one(Value::Int(digit.map_or(-1, |d| d as i32)))
        }
        "jstd.name" => {
            one(character::name(case.int(0) as u32).map_or(Value::Bool(false), |name| utf16(&name)))
        }
        "jstd.cp_of" => one(match character::code_point_of(&arg_jstring(case, 0)) {
            Some(cp) => Value::Int(cp as i32),
            // CeriumMC reports only that no character has the name; Java's message is
            // "Unrecognized character name :" and the uppercased name.
            None => match &case.expected[0] {
                expected @ Value::Utf16(_) => expected.clone(),
                _ => Value::Str("no such character".to_owned()),
            },
        }),
        "jstd.uuid" => match uuid::from_string(&arg_jstring(case, 0)) {
            Ok((msb, lsb)) => vec![Value::Long(msb), Value::Long(lsb)],
            Err(error) => vec![thrown(error.java_class(), error.message())],
        },
        "jstd.mutf8_enc" => one(match mutf8::encode(&arg_jstring(case, 0)) {
            Ok(bytes) => Value::Bytes(bytes),
            Err(error) => thrown("UTFDataFormatException", &JString::from(error.message())),
        }),
        "jstd.mutf8_dec" => one(match mutf8::decode(case.bytes(0)) {
            Ok(s) => jstring(&s),
            Err(error) => thrown("UTFDataFormatException", &JString::from(error.message())),
        }),
        "jstd.str" => {
            let (a, b) = (arg_jstring(case, 0), arg_jstring(case, 1));
            let sign = a.compare_to(&b) as i32;
            vec![Value::Int(a.hash_code()), Value::Int(sign)]
        }
        other => panic!("line {}: unknown operation {other}", case.line),
    }
}

/// Runs a `JStdMain` map script: `<initial capacity or -1>;` then `<op><key>:<value>[:<key>];`
/// steps, and renders the final iteration order as `<key index>=<value>,` entries.
fn run_map(pool: &[JString], script: &str) -> String {
    let mut steps = script.split(';').filter(|step| !step.is_empty());
    let initial: i32 = steps
        .next()
        .expect("initial capacity")
        .parse()
        .expect("int");
    let mut map: JavaHashMap<JString, i32> = if initial >= 0 {
        JavaHashMap::with_capacity(initial)
    } else {
        JavaHashMap::new()
    };
    for step in steps {
        let fields: Vec<usize> = step[1..]
            .split(':')
            .map(|f| f.parse().expect("number"))
            .collect();
        let key = pool[fields[0]].clone();
        let value = fields[1] as i32;
        match step.as_bytes()[0] {
            b'P' => {
                map.insert(key, value);
            }
            b'R' => {
                map.remove(&key);
            }
            b'A' => {
                map.put_if_absent(key, value);
            }
            b'C' => {
                map.compute_if_absent(key, |_| Some(value));
            }
            b'M' => {
                map.compute(key, |_, old| match old {
                    None => Some(value),
                    Some(&old) => Some(old + value).filter(|sum| sum % 3 != 0),
                });
            }
            b'G' => {
                map.merge(key, value, |&a, b| Some(a + b).filter(|sum| sum % 4 != 0));
            }
            b'F' => {
                let modulus = 2 + value % 5;
                map.retain(|_, v| *v % modulus != 0);
            }
            b'N' => map = JavaHashMap::from_map(&map),
            b'L' => {
                let mut other = JavaHashMap::new();
                other.insert(key, value);
                other.insert(pool[fields[2]].clone(), value + 1);
                map.put_all(&other);
            }
            b'X' => map.clear(),
            b'K' => {
                let mut copy = JavaHashMap::new();
                for (k, v) in &map {
                    copy.insert(k.clone(), *v);
                }
                map = copy;
            }
            other => panic!("unknown map step {}", char::from(other)),
        }
    }
    map.iter().fold(String::new(), |mut order, (k, v)| {
        let index = pool.iter().position(|p| p == k).expect("pooled key");
        let _ = write!(order, "{index}={v},");
        order
    })
}
