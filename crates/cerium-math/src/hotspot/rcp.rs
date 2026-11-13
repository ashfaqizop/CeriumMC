//! The x86 `RCPPS` instruction (approximate reciprocal), as measured on real CPUs.
//!
//! `RCPPS` guarantees only a relative error of at most 1.5 × 2^-12, so its exact output is
//! implementation-defined, and Intel and AMD CPUs return different bits. HotSpot's `Math.log`,
//! `log10` and `pow` stubs start from it, which is why vanilla's `Math.pow` can differ by an
//! ULP between an Intel and an AMD server.
//!
//! The stubs only ever pass `RCPPS` a float `1.m` (exponent field 127) built from the top 23
//! mantissa bits of the argument, or a zero or denormal from register lanes whose result is
//! discarded. The tables in `rcp_intel` and `rcp_amd` hold the exact output for all 2^23
//! values `1.m`, measured with `tools/cerium-rcp` (CPU-measured data, not derived from any
//! Minecraft or JDK code). Every other input is handled by the architectural rules.

use super::{rcp_amd, rcp_intel};

/// Whose `RCPPS` implementation to reproduce.
///
/// HotSpot's results depend on the CPU the vanilla server runs on. CeriumMC reproduces either
/// vendor exactly, on any CPU, so a server can match the vanilla servers it replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Vendor {
    /// Intel CPUs (`GenuineIntel`). The default.
    #[default]
    Intel,
    /// AMD CPUs (`AuthenticAMD`), Zen 3 and later.
    Amd,
}

impl Vendor {
    /// The vendor for a CPUID vendor string, such as `GenuineIntel` or `AuthenticAMD`.
    #[must_use]
    pub fn from_cpuid(vendor: &str) -> Option<Self> {
        match vendor {
            "GenuineIntel" => Some(Self::Intel),
            "AuthenticAMD" => Some(Self::Amd),
            _ => None,
        }
    }

    /// The measured `RCPPS(1.m)` runs: `(first mantissa, output bits)`, sorted by mantissa.
    fn runs(self) -> &'static [(u32, u32)] {
        match self {
            Self::Intel => &rcp_intel::RUNS,
            Self::Amd => &rcp_amd::RUNS,
        }
    }
}

const SIGN: u32 = 0x8000_0000;
const MANTISSA: u32 = 0x007F_FFFF;
const INFINITY: u32 = 0x7F80_0000;
const QUIET: u32 = 0x0040_0000;

/// `RCPPS` on one 32-bit float, given and returned as raw bits.
pub(crate) fn rcpps(bits: u32, vendor: Vendor) -> u32 {
    let sign = bits & SIGN;
    let exponent = (bits >> 23) & 0xFF;
    let mantissa = bits & MANTISSA;
    match exponent {
        // Zeros and denormals are treated as zero: the result is an infinity of the same sign.
        0 => sign | INFINITY,
        // Infinities give zeros of the same sign; NaNs are returned quieted.
        0xFF if mantissa == 0 => sign,
        0xFF => bits | QUIET,
        _ => {
            // `RCPPS(2^e × 1.m) = 2^-e × RCPPS(1.m)`; results below the normal range are
            // flushed to zero. Only `e = 0` occurs in HotSpot's stubs.
            let unit = unit_reciprocal(mantissa, vendor);
            let result_exponent = ((unit >> 23) & 0xFF) as i32 - (exponent as i32 - 127);
            if result_exponent <= 0 {
                sign
            } else {
                sign | ((result_exponent as u32) << 23) | (unit & MANTISSA)
            }
        }
    }
}

/// The measured `RCPPS` of the float `1.m`.
fn unit_reciprocal(mantissa: u32, vendor: Vendor) -> u32 {
    let runs = vendor.runs();
    // The first run starts at mantissa 0, so the partition point is at least 1.
    runs[runs.partition_point(|&(start, _)| start <= mantissa) - 1].1
}

#[cfg(test)]
mod tests {
    use super::*;

    const VENDORS: [Vendor; 2] = [Vendor::Intel, Vendor::Amd];

    #[test]
    fn tables_cover_every_mantissa_in_order() {
        for vendor in VENDORS {
            let runs = vendor.runs();
            assert_eq!(runs[0].0, 0, "{vendor:?}");
            assert!(runs.is_sorted_by(|a, b| a.0 < b.0), "{vendor:?}");
            assert!(runs.iter().all(|&(start, _)| start <= MANTISSA), "{vendor:?}");
        }
    }

    #[test]
    fn unit_reciprocals_are_within_the_architectural_error_bound() {
        // |RCPPS(x) - 1/x| <= 1.5 * 2^-12 * |1/x| for every input, on every vendor.
        for vendor in VENDORS {
            for mantissa in (0..=MANTISSA).step_by(97).chain([MANTISSA]) {
                let x = 0x3F80_0000 | mantissa;
                let approx = f32::from_bits(rcpps(x, vendor));
                let error = (f64::from(approx) * f64::from(f32::from_bits(x)) - 1.0).abs();
                assert!(error <= 1.5 / 4096.0, "{vendor:?} m={mantissa:#x}: {error}");
            }
        }
    }

    #[test]
    fn special_inputs() {
        for vendor in VENDORS {
            assert_eq!(rcpps(0x0000_0000, vendor), 0x7F80_0000);
            assert_eq!(rcpps(0x8000_0000, vendor), 0xFF80_0000);
            assert_eq!(rcpps(0x0000_0001, vendor), 0x7F80_0000);
            assert_eq!(rcpps(0x807F_FFFF, vendor), 0xFF80_0000);
            assert_eq!(rcpps(0x7F80_0000, vendor), 0x0000_0000);
            assert_eq!(rcpps(0xFF80_0000, vendor), 0x8000_0000);
            assert_eq!(rcpps(0x7F80_0001, vendor), 0x7FC0_0001);
            assert_eq!(rcpps(0xFFC0_0000, vendor), 0xFFC0_0000);
        }
    }

    #[test]
    fn scaling_by_powers_of_two() {
        for vendor in VENDORS {
            let unit = rcpps(0x3FC0_0000, vendor); // 1.5
            assert_eq!(rcpps(0x40C0_0000, vendor), unit - (2 << 23)); // 6.0 = 1.5 * 2^2
            assert_eq!(rcpps(0xBEC0_0000, vendor), SIGN | (unit + (2 << 23))); // -0.375
            assert_eq!(rcpps(0x7F40_0000, vendor), 0); // 1.5 * 2^127 underflows
        }
    }
}
