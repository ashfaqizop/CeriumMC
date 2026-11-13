//! Replays `java.lang.Math` helper results recorded from Java 25 in CI (see
//! `parity/golden/JLangMain.java`) against [`cerium_math::java`]: results must match bit for
//! bit (NaN payloads included), and the functions must return `None` exactly where Java throws.

use cerium_golden::{Case, Value, load};
use cerium_math::java;

/// How many mismatches are printed in full.
const MAX_SHOWN: usize = 25;

#[test]
fn java_lang_math_matches_java() {
    let Some(cases) = load("jlang.txt") else {
        return;
    };
    let mut failures = Vec::new();
    for case in &cases {
        let actual = eval(case);
        if !same(&case.expected[0], &actual) {
            let args: Vec<String> = case.args.iter().map(ToString::to_string).collect();
            failures.push(format!(
                "line {}: {} {}\n    java:     {}\n    ceriummc: {actual}",
                case.line,
                case.op,
                args.join(" "),
                case.expected[0]
            ));
        }
    }
    let failed = failures.len();
    eprintln!("jlang.txt: {} cases checked, {failed} mismatched", cases.len());
    assert!(
        failures.is_empty(),
        "{failed} of {} java.lang.Math cases differ:\n{}",
        cases.len(),
        failures[..failed.min(MAX_SHOWN)].join("\n")
    );
}

/// Exact equality: floats compare by their bits, so NaN payloads and signed zeros count.
fn same(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
        (Value::Double(a), Value::Double(b)) => a.to_bits() == b.to_bits(),
        _ => expected == actual,
    }
}

/// The result Java records when it throws: the exception's simple class name.
fn thrown(exception: &str) -> Value {
    Value::Str(exception.to_owned())
}

fn arithmetic<T>(result: Option<T>, wrap: fn(T) -> Value) -> Value {
    result.map_or_else(|| thrown("ArithmeticException"), wrap)
}

fn illegal<T>(result: Option<T>, wrap: fn(T) -> Value) -> Value {
    result.map_or_else(|| thrown("IllegalArgumentException"), wrap)
}

fn eval(c: &Case) -> Value {
    use Value::{Double, Float, Int, Long};

    match c.op.as_str() {
        "jl.round_f" => Int(java::round_f32(c.float(0))),
        "jl.round_d" => Long(java::round_f64(c.double(0))),
        "jl.signum_f" => Float(java::signum_f32(c.float(0))),
        "jl.signum_d" => Double(java::signum_f64(c.double(0))),
        "jl.copy_sign_f" => Float(java::copy_sign_f32(c.float(0), c.float(1))),
        "jl.copy_sign_d" => Double(java::copy_sign_f64(c.double(0), c.double(1))),
        "jl.to_radians" => Double(java::to_radians(c.double(0))),
        "jl.to_degrees" => Double(java::to_degrees(c.double(0))),
        "jl.next_up_f" => Float(java::next_up_f32(c.float(0))),
        "jl.next_up_d" => Double(java::next_up_f64(c.double(0))),
        "jl.next_down_f" => Float(java::next_down_f32(c.float(0))),
        "jl.next_down_d" => Double(java::next_down_f64(c.double(0))),
        "jl.clamp_li" => illegal(java::clamp_i64_to_i32(c.long(0), c.int(1), c.int(2)), Int),
        "jl.clamp_l" => illegal(java::clamp_i64(c.long(0), c.long(1), c.long(2)), Long),
        "jl.clamp_d" => illegal(java::clamp_f64(c.double(0), c.double(1), c.double(2)), Double),
        "jl.clamp_f" => illegal(java::clamp_f32(c.float(0), c.float(1), c.float(2)), Float),
        "jl.to_int_exact" => arithmetic(java::to_int_exact(c.long(0)), Int),
        "jl.abs_exact_i" => arithmetic(java::abs_exact_i32(c.int(0)), Int),
        "jl.abs_exact_l" => arithmetic(java::abs_exact_i64(c.long(0)), Long),
        "jl.negate_exact_i" => arithmetic(java::negate_exact_i32(c.int(0)), Int),
        "jl.negate_exact_l" => arithmetic(java::negate_exact_i64(c.long(0)), Long),
        "jl.subtract_exact_i" => arithmetic(java::subtract_exact_i32(c.int(0), c.int(1)), Int),
        "jl.subtract_exact_l" => arithmetic(java::subtract_exact_i64(c.long(0), c.long(1)), Long),
        "jl.floor_div_exact_i" => arithmetic(java::floor_div_exact_i32(c.int(0), c.int(1)), Int),
        "jl.floor_div_exact_l" => {
            arithmetic(java::floor_div_exact_i64(c.long(0), c.long(1)), Long)
        }
        "jl.pow_exact_i" => arithmetic(java::pow_exact_i32(c.int(0), c.int(1)), Int),
        "jl.pow_exact_l" => arithmetic(java::pow_exact_i64(c.long(0), c.int(1)), Long),
        other => panic!("line {}: unknown operation {other}", c.line),
    }
}
