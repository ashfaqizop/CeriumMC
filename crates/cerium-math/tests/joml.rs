//! Replays `org.joml.Math` results recorded in CI with the JOML jar the server bundles (see
//! `parity/golden/JomlMain.java`) against [`cerium_math::joml`], bit for bit.

use cerium_golden::{Case, Value, load};
use cerium_math::joml;

/// How many mismatches are printed in full.
const MAX_SHOWN: usize = 25;

#[test]
fn joml_math_matches_joml() {
    let Some(cases) = load("joml.txt") else {
        return;
    };
    let mut failures = Vec::new();
    for case in &cases {
        let actual = eval(case);
        let Value::Float(expected) = case.expected[0] else {
            panic!("line {}: expected a float result", case.line);
        };
        if actual.to_bits() != expected.to_bits() {
            let args: Vec<String> = case.args.iter().map(ToString::to_string).collect();
            failures.push(format!(
                "line {}: {} {}\n    joml:     f:{:08x}\n    ceriummc: f:{:08x}",
                case.line,
                case.op,
                args.join(" "),
                expected.to_bits(),
                actual.to_bits()
            ));
        }
    }
    let failed = failures.len();
    eprintln!(
        "joml.txt: {} cases checked, {failed} mismatched",
        cases.len()
    );
    assert!(
        failures.is_empty(),
        "{failed} of {} org.joml.Math cases differ:\n{}",
        cases.len(),
        failures[..failed.min(MAX_SHOWN)].join("\n")
    );
}

fn eval(c: &Case) -> f32 {
    match c.op.as_str() {
        "joml.sin" => joml::sin(c.float(0)),
        "joml.sqrt" => joml::sqrt(c.float(0)),
        "joml.invsqrt" => joml::invsqrt(c.float(0)),
        "joml.abs" => joml::abs(c.float(0)),
        "joml.max" => joml::max(c.float(0), c.float(1)),
        "joml.cos_from_sin" => joml::cos_from_sin(c.float(0), c.float(1)),
        other => panic!("line {}: unknown operation {other}", c.line),
    }
}
