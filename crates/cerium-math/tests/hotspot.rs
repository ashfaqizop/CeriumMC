//! Replays `java.lang.Math` results recorded from HotSpot (JDK 25.0.1, x86_64) in CI (see
//! `parity/golden/JMathMain.java`) against [`cerium_math::hotspot`], bit for bit.
//!
//! Unlike the other golden tests, NaN results must match exactly too: the ports model x86 NaN
//! rules, so they reproduce Java's NaN bit patterns on every platform. This covers the HotSpot
//! intrinsics ([`cerium_math::hotspot`]) and the `StrictMath` functions vanilla calls
//! ([`cerium_math::fdlibm`]).

use std::collections::BTreeMap;

use cerium_golden::{Value, cpu_vendor, load};
use cerium_math::fdlibm;
use cerium_math::hotspot::{self, Vendor};

/// How many mismatches are printed in full.
const MAX_SHOWN: usize = 25;

#[test]
fn hotspot_math_matches_the_jdk() {
    let Some(cases) = load("jmath.txt") else {
        return;
    };
    let vendor = recorded_vendor();
    let mut checked = 0_usize;
    let mut failures = Vec::new();
    let mut not_ported: BTreeMap<&str, usize> = BTreeMap::new();
    for case in &cases {
        let x = case.double(0);
        let actual = match case.op.as_str() {
            "jm.exp" => hotspot::exp(x),
            "jm.sin" => hotspot::sin(x),
            "jm.cos" => hotspot::cos(x),
            "jm.log" => vendor.log(x),
            "jm.log10" => vendor.log10(x),
            "jm.pow" => vendor.pow(x, case.double(1)),
            // StrictMath-backed (Java's FdLibm).
            "jm.asin" => fdlibm::asin(x),
            "jm.acos" => fdlibm::acos(x),
            "jm.log1p" => fdlibm::log1p(x),
            "jm.atan2" => fdlibm::atan2(x, case.double(1)),
            "jm.hypot" => fdlibm::hypot(x, case.double(1)),
            other => {
                *not_ported.entry(other).or_default() += 1;
                continue;
            }
        };
        checked += 1;
        let Value::Double(expected) = case.expected[0] else {
            panic!("line {}: expected a double result", case.line);
        };
        if actual.to_bits() != expected.to_bits() {
            failures.push(format!(
                "line {}: {} d:{:016x}\n    hotspot:  d:{:016x}\n    ceriummc: d:{:016x}",
                case.line,
                case.op,
                x.to_bits(),
                expected.to_bits(),
                actual.to_bits()
            ));
        }
    }
    for (op, count) in &not_ported {
        eprintln!("jmath.txt: {count} {op} cases skipped (not ported yet)");
    }
    let failed = failures.len();
    eprintln!("jmath.txt ({vendor:?} RCPPS): {checked} cases checked, {failed} mismatched");
    assert_ne!(
        checked, 0,
        "jmath.txt has no cases for the ported functions"
    );
    assert!(
        failures.is_empty(),
        "{failed} of {checked} HotSpot math cases differ:\n{}",
        failures[..failed.min(MAX_SHOWN)].join("\n")
    );
}

/// The CPU vendor of the machine that recorded the golden data.
fn recorded_vendor() -> Vendor {
    let name = cpu_vendor().expect("golden data was loaded");
    Vendor::from_cpuid(&name)
        .unwrap_or_else(|| panic!("golden data was recorded on an unknown CPU vendor {name:?}"))
}
