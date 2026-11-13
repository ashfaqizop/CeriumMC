//! Bit-exact ports of HotSpot's x86_64 `java.lang.Math` intrinsics.
//!
//! On x86_64, the JDK that vanilla ships with (25.0.1) computes `Math.log`, `log10`, `exp`,
//! `sin`, `cos`, `tan` and `pow` with hand-written Intel LIBM assembly stubs, not with
//! `StrictMath` (fdlibm). Their results are neither correctly rounded nor equal to any portable
//! libm, so vanilla's outputs (Gaussian noise, physics, world generation) depend on them.
//!
//! Each function here transcribes one OpenJDK stub, instruction by instruction, onto a small
//! model of the SSE registers (`sse`). The model follows x86 rules for NaNs and rounding, so
//! the results are identical on every CPU and architecture CeriumMC runs on.
//!
//! The stubs start from `RCPPS`, whose exact output differs between CPU vendors, so vanilla's
//! results depend on whether its server runs on an Intel or an AMD CPU. CeriumMC reproduces
//! either: [`set_vendor`] chooses, once at startup, and the free functions ([`log()`], …) use
//! that choice. [`Vendor`]'s methods take the vendor explicitly.
//!
//! The transcribed stubs are Copyright Intel Corporation and others, GPL-2.0-only; each module
//! keeps the original notice.

#[allow(dead_code, reason = "shared by stubs that are not ported yet")]
mod constants;
mod exp;
mod exp_tables;
mod log;
mod log_tables;
mod rcp;
mod rcp_amd;
mod rcp_intel;
mod sse;

use std::sync::atomic::{AtomicU8, Ordering};

pub use rcp::Vendor;

/// The process-wide vendor: `0` for Intel, `1` for AMD.
static VENDOR: AtomicU8 = AtomicU8::new(0);

/// Chooses whose `RCPPS` the free functions reproduce, for the whole process.
///
/// Set it once at startup, before any world generation or simulation, to the vendor of the
/// CPU that the vanilla server being matched ran on. The default is [`Vendor::Intel`].
pub fn set_vendor(vendor: Vendor) {
    VENDOR.store(vendor as u8, Ordering::Relaxed);
}

/// The vendor chosen with [`set_vendor`].
#[must_use]
pub fn vendor() -> Vendor {
    match VENDOR.load(Ordering::Relaxed) {
        1 => Vendor::Amd,
        _ => Vendor::Intel,
    }
}

/// `Math.exp(x)`, exactly as HotSpot computes it on x86_64 (the same for every vendor).
#[must_use]
pub fn exp(x: f64) -> f64 {
    exp::exp(x)
}

/// `Math.log(x)`, exactly as HotSpot computes it on x86_64, for the [`vendor`].
#[must_use]
pub fn log(x: f64) -> f64 {
    vendor().log(x)
}

/// `Math.log10(x)`, exactly as HotSpot computes it on x86_64, for the [`vendor`].
#[must_use]
pub fn log10(x: f64) -> f64 {
    vendor().log10(x)
}

impl Vendor {
    /// `Math.log(x)`, exactly as HotSpot computes it on an x86_64 CPU from this vendor.
    #[must_use]
    pub fn log(self, x: f64) -> f64 {
        log::log(x, self)
    }

    /// `Math.log10(x)`, exactly as HotSpot computes it on an x86_64 CPU from this vendor.
    #[must_use]
    pub fn log10(self, x: f64) -> f64 {
        log::log10(x, self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VENDORS: [Vendor; 2] = [Vendor::Intel, Vendor::Amd];

    #[test]
    fn the_default_vendor_is_intel() {
        assert_eq!(Vendor::default(), Vendor::Intel);
        assert_eq!(vendor(), Vendor::Intel);
    }

    #[test]
    fn special_values() {
        for vendor in VENDORS {
            for f in [Vendor::log, Vendor::log10] {
                let bits = |x: u64| f(vendor, f64::from_bits(x)).to_bits();
                assert_eq!(f(vendor, 0.0), f64::NEG_INFINITY);
                assert_eq!(f(vendor, -0.0), f64::NEG_INFINITY);
                assert_eq!(f(vendor, f64::INFINITY), f64::INFINITY);
                // Invalid inputs give the x86 default NaN; NaN inputs are returned quieted.
                assert_eq!(bits((-1.0f64).to_bits()), 0xFFF8_0000_0000_0000);
                assert_eq!(bits(0xFFF0_0000_0000_0000), 0xFFF8_0000_0000_0000);
                assert_eq!(bits(0x7FF0_0000_0000_0001), 0x7FF8_0000_0000_0001);
                assert_eq!(bits(0xFFF0_0000_0000_0001), 0xFFF8_0000_0000_0001);
            }
        }
    }

    #[test]
    fn exp_special_values() {
        assert_eq!(exp(0.0), 1.0);
        assert_eq!(exp(-0.0), 1.0);
        assert_eq!(exp(f64::INFINITY), f64::INFINITY);
        assert_eq!(exp(f64::NEG_INFINITY).to_bits(), 0);
        assert_eq!(exp(1000.0), f64::INFINITY);
        assert_eq!(exp(-1000.0).to_bits(), 0);
        assert_eq!(exp(f64::MIN_POSITIVE), 1.0);
        let nan = f64::from_bits(0xFFF0_0000_0000_0001);
        assert_eq!(exp(nan).to_bits(), 0xFFF8_0000_0000_0001);
    }

    #[test]
    fn exp_close_to_the_true_exponential() {
        let close = |a: f64, b: f64| (a - b).abs() <= a.abs() * 4.0 * f64::EPSILON;
        let points = [-700.0, -1.5, -1e-10, 1e-10, 0.5, 1.0, 2.0, 709.7];
        for x in points {
            assert!(close(exp(x), libm::exp(x)), "exp {x}");
        }
    }

    #[test]
    fn close_to_the_true_logarithm() {
        // The stubs are accurate to well under 1 ULP; the golden test checks exact bits.
        let close = |a: f64, b: f64| (a - b).abs() <= a.abs() * 4.0 * f64::EPSILON;
        for vendor in VENDORS {
            for x in [
                1e-310,
                1e-300,
                0.1,
                0.5,
                0.75,
                1.5,
                2.0,
                3.0,
                1e10,
                1e300,
                f64::MAX,
            ] {
                assert!(close(vendor.log(x), libm::log(x)), "log {x}");
                assert!(close(vendor.log10(x), libm::log10(x)), "log10 {x}");
            }
        }
    }
}
