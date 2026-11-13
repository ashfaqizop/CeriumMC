//! Bit-exact ports of Java's `StrictMath` functions that vanilla calls through `Math`:
//! [`asin`], [`acos`], [`atan`], [`atan2`] and [`log1p`].
//!
//! Java computes these with `java.lang.FdLibm`, a pure-Java port of fdlibm 5.3, using strict
//! IEEE double arithmetic. They are transcribed here from `FdLibm.java` of OpenJDK 25.0.1
//! (tag `jdk-25.0.1-ga`), operation by operation, so they give Java's exact results on every
//! platform. The `libm` crate is not used for them: it ports musl, which deviates from fdlibm
//! (for example, musl's `atan2` returns `±pi/2` directly when `|y/x| > 2^64`, where fdlibm's
//! result for negative `x` is one ULP higher).
//!
//! NaN results reproduce what Java produces on x86_64 bit for bit: an invalid operation gives
//! the x86 default NaN (`0xFFF8_0000_0000_0000`), and a NaN operand is returned quieted.
//!
//! Original notices:
//!
//! Copyright (c) 1998, 2024, Oracle and/or its affiliates. All rights reserved.
//! This code is free software; you can redistribute it and/or modify it under the terms of the
//! GNU General Public License version 2 only, as published by the Free Software Foundation.
//! Oracle designates this particular file as subject to the "Classpath" exception as provided
//! by Oracle in the LICENSE file that accompanied this code.
//!
//! Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved. Developed at SunSoft, a
//! Sun Microsystems, Inc. business. Permission to use, copy, modify, and distribute this
//! software is freely granted, provided that this notice is preserved.

/// `EXP_SIGNIF_BITS`: masks the sign out of a high word.
const EXP_SIGNIF_BITS: i32 = 0x7FFF_FFFF;
/// `EXP_BITS`: the high word of `+Inf`.
const EXP_BITS: i32 = 0x7FF0_0000;
const HUGE: f64 = 1.0e300;
const TWO54: f64 = 18_014_398_509_481_984.0; // 2^54
/// `Math.PI`.
const PI: f64 = std::f64::consts::PI;

/// The x86 "`QNaN` floating-point indefinite": the NaN x86 produces for invalid operations.
const DEFAULT_NAN: u64 = 0xFFF8_0000_0000_0000;
const QUIET: u64 = 0x0008_0000_0000_0000;

/// `__HI(x)`: the high 32 bits as an `int`.
fn hi(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

/// `__LO(x)`: the low 32 bits as an `int`.
fn lo(x: f64) -> i32 {
    x.to_bits() as i32
}

/// `__LO(x, low)`: `x` with its low 32 bits replaced.
fn with_lo(x: f64, low: i32) -> f64 {
    f64::from_bits((x.to_bits() & 0xFFFF_FFFF_0000_0000) | u64::from(low.cast_unsigned()))
}

/// `__HI(x, high)`: `x` with its high 32 bits replaced.
fn with_hi(x: f64, high: i32) -> f64 {
    f64::from_bits((x.to_bits() & 0xFFFF_FFFF) | (u64::from(high.cast_unsigned()) << 32))
}

/// `(x - x) / (x - x)` as Java evaluates it on x86_64: a NaN `x` is returned quieted, and
/// anything else gives the default NaN.
fn invalid(x: f64) -> f64 {
    if x.is_nan() {
        f64::from_bits(x.to_bits() | QUIET)
    } else {
        f64::from_bits(DEFAULT_NAN)
    }
}

/// `x + y` as x86_64 evaluates it when at least one operand is NaN: the first NaN operand,
/// quieted.
fn add_nan(x: f64, y: f64) -> f64 {
    let nan = if x.is_nan() { x } else { y };
    f64::from_bits(nan.to_bits() | QUIET)
}

/// `StrictMath.asin(x)` (fdlibm `__ieee754_asin`).
#[must_use]
pub fn asin(x: f64) -> f64 {
    const PIO2_HI: f64 = f64::from_bits(0x3FF9_21FB_5444_2D18);
    const PIO2_LO: f64 = f64::from_bits(0x3C91_A626_3314_5C07);
    const PIO4_HI: f64 = f64::from_bits(0x3FE9_21FB_5444_2D18);
    let t;
    let hx = hi(x);
    let ix = hx & EXP_SIGNIF_BITS;
    if ix >= 0x3FF0_0000 {
        // |x| >= 1
        if ((ix - 0x3FF0_0000) | lo(x)) == 0 {
            return x * PIO2_HI + x * PIO2_LO; // asin(+-1) = +-pi/2 with inexact
        }
        return invalid(x); // asin(|x| > 1) is NaN
    } else if ix < 0x3FE0_0000 {
        // |x| < 0.5
        if ix < 0x3E40_0000 {
            // |x| < 2^-27: return x with inexact if x != 0
            if HUGE + x > 1.0 {
                return x;
            }
            t = 0.0;
        } else {
            t = x * x;
        }
        let w = r(t);
        return x + x * w;
    }
    // 1 > |x| >= 0.5
    let w = 1.0 - x.abs();
    let t = w * 0.5;
    let (p, q) = pq(t);
    let s = t.sqrt();
    let t = if ix >= 0x3FEF_3333 {
        // |x| > 0.975
        let w = p / q;
        PIO2_HI - (2.0 * (s + s * w) - PIO2_LO)
    } else {
        let w = with_lo(s, 0);
        let c = (t - w * w) / (s + w);
        let r = p / q;
        let p = 2.0 * s * r - (PIO2_LO - 2.0 * c);
        let q = PIO4_HI - 2.0 * w;
        PIO4_HI - (p - q)
    };
    if hx > 0 { t } else { -t }
}

/// The numerator `p` and denominator `q` of fdlibm's rational approximation `R(t) = p/q`,
/// shared by `asin` and `acos`.
fn pq(t: f64) -> (f64, f64) {
    const PS0: f64 = f64::from_bits(0x3FC5_5555_5555_5555);
    const PS1: f64 = f64::from_bits(0xBFD4_D612_03EB_6F7D);
    const PS2: f64 = f64::from_bits(0x3FC9_C155_0E88_4455);
    const PS3: f64 = f64::from_bits(0xBFA4_8228_B568_8F3B);
    const PS4: f64 = f64::from_bits(0x3F49_EFE0_7501_B288);
    const PS5: f64 = f64::from_bits(0x3F02_3DE1_0DFD_F709);
    const QS1: f64 = f64::from_bits(0xC003_3A27_1C8A_2D4B);
    const QS2: f64 = f64::from_bits(0x4000_2AE5_9C59_8AC8);
    const QS3: f64 = f64::from_bits(0xBFE6_066C_1B8D_0159);
    const QS4: f64 = f64::from_bits(0x3FB3_B8C5_B12E_9282);
    let p = t * (PS0 + t * (PS1 + t * (PS2 + t * (PS3 + t * (PS4 + t * PS5)))));
    let q = 1.0 + t * (QS1 + t * (QS2 + t * (QS3 + t * QS4)));
    (p, q)
}

/// `R(t) = p/q`.
fn r(t: f64) -> f64 {
    let (p, q) = pq(t);
    p / q
}

/// `StrictMath.acos(x)` (fdlibm `__ieee754_acos`).
#[must_use]
pub fn acos(x: f64) -> f64 {
    const PIO2_HI: f64 = f64::from_bits(0x3FF9_21FB_5444_2D18);
    const PIO2_LO: f64 = f64::from_bits(0x3C91_A626_3314_5C07);
    let hx = hi(x);
    let ix = hx & EXP_SIGNIF_BITS;
    if ix >= 0x3FF0_0000 {
        // |x| >= 1
        if ((ix - 0x3FF0_0000) | lo(x)) == 0 {
            // |x| == 1: acos(1) = 0, acos(-1) = pi
            return if hx > 0 { 0.0 } else { PI + 2.0 * PIO2_LO };
        }
        return invalid(x); // acos(|x| > 1) is NaN
    }
    if ix < 0x3FE0_0000 {
        // |x| < 0.5
        if ix <= 0x3C60_0000 {
            return PIO2_HI + PIO2_LO; // |x| < 2^-57
        }
        let z = x * x;
        let r = r(z);
        PIO2_HI - (x - (PIO2_LO - x * r))
    } else if hx < 0 {
        // x < -0.5
        #[allow(clippy::manual_midpoint, reason = "Java computes (1.0 + x) * 0.5")]
        let z = (1.0 + x) * 0.5;
        let (p, q) = pq(z);
        let s = z.sqrt();
        let r = p / q;
        let w = r * s - PIO2_LO;
        PI - 2.0 * (s + w)
    } else {
        // x > 0.5
        let z = (1.0 - x) * 0.5;
        let s = z.sqrt();
        let df = with_lo(s, 0);
        let c = (z - df * df) / (s + df);
        let (p, q) = pq(z);
        let r = p / q;
        let w = r * s + c;
        2.0 * (df + w)
    }
}

/// `StrictMath.atan(x)` (fdlibm `atan`).
#[must_use]
pub fn atan(x: f64) -> f64 {
    const ATANHI: [f64; 4] = [
        f64::from_bits(0x3FDD_AC67_0561_BB4F),
        f64::from_bits(0x3FE9_21FB_5444_2D18),
        f64::from_bits(0x3FEF_730B_D281_F69B),
        f64::from_bits(0x3FF9_21FB_5444_2D18),
    ];
    const ATANLO: [f64; 4] = [
        f64::from_bits(0x3C7A_2B7F_222F_65E2),
        f64::from_bits(0x3C81_A626_3314_5C07),
        f64::from_bits(0x3C70_0788_7AF0_CBBD),
        f64::from_bits(0x3C91_A626_3314_5C07),
    ];
    const AT: [f64; 11] = [
        f64::from_bits(0x3FD5_5555_5555_550D),
        f64::from_bits(0xBFC9_9999_9998_EBC4),
        f64::from_bits(0x3FC2_4924_9200_83FF),
        f64::from_bits(0xBFBC_71C6_FE23_1671),
        f64::from_bits(0x3FB7_45CD_C54C_206E),
        f64::from_bits(0xBFB3_B0F2_AF74_9A6D),
        f64::from_bits(0x3FB1_0D66_A0D0_3D51),
        f64::from_bits(0xBFAD_DE2D_52DE_FD9A),
        f64::from_bits(0x3FA9_7B4B_2476_0DEB),
        f64::from_bits(0xBFA2_B444_2C6A_6C2F),
        f64::from_bits(0x3F90_AD3A_E322_DA11),
    ];
    let hx = hi(x);
    let ix = hx & EXP_SIGNIF_BITS;
    let mut x = x;
    let id: Option<usize>;
    if ix >= 0x4410_0000 {
        // |x| >= 2^66
        if ix > EXP_BITS || (ix == EXP_BITS && lo(x) != 0) {
            return add_nan(x, x); // NaN
        }
        return if hx > 0 {
            ATANHI[3] + ATANLO[3]
        } else {
            -ATANHI[3] - ATANLO[3]
        };
    }
    if ix < 0x3FDC_0000 {
        // |x| < 0.4375
        if ix < 0x3E20_0000 && HUGE + x > 1.0 {
            return x; // |x| < 2^-29, with inexact
        }
        id = None;
    } else {
        x = x.abs();
        if ix < 0x3FF3_0000 {
            // |x| < 1.1875
            if ix < 0x3FE6_0000 {
                // 7/16 <= |x| < 11/16
                id = Some(0);
                x = (2.0 * x - 1.0) / (2.0 + x);
            } else {
                // 11/16 <= |x| < 19/16
                id = Some(1);
                x = (x - 1.0) / (x + 1.0);
            }
        } else if ix < 0x4003_8000 {
            // |x| < 2.4375
            id = Some(2);
            x = (x - 1.5) / (1.0 + 1.5 * x);
        } else {
            // 2.4375 <= |x| < 2^66
            id = Some(3);
            x = -1.0 / x;
        }
    }
    // end of argument reduction
    let z = x * x;
    let w = z * z;
    // break sum from i=0 to 10 aT[i]z**(i+1) into odd and even poly
    let s1 = z * (AT[0] + w * (AT[2] + w * (AT[4] + w * (AT[6] + w * (AT[8] + w * AT[10])))));
    let s2 = w * (AT[1] + w * (AT[3] + w * (AT[5] + w * (AT[7] + w * AT[9]))));
    match id {
        None => x - x * (s1 + s2),
        Some(id) => {
            let z = ATANHI[id] - ((x * (s1 + s2) - ATANLO[id]) - x);
            if hx < 0 { -z } else { z }
        }
    }
}

/// `StrictMath.atan2(y, x)` (fdlibm `__ieee754_atan2`).
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    const TINY: f64 = 1.0e-300;
    const PI_O_4: f64 = f64::from_bits(0x3FE9_21FB_5444_2D18);
    const PI_O_2: f64 = f64::from_bits(0x3FF9_21FB_5444_2D18);
    const PI_LO: f64 = f64::from_bits(0x3CA1_A626_3314_5C07);
    let hx = hi(x);
    let ix = hx & EXP_SIGNIF_BITS;
    let lx = lo(x);
    let hy = hi(y);
    let iy = hy & EXP_SIGNIF_BITS;
    let ly = lo(y);
    if x.is_nan() || y.is_nan() {
        return add_nan(x, y);
    }
    if ((hx.wrapping_sub(0x3FF0_0000)) | lx) == 0 {
        return atan(y); // x = 1.0
    }
    let m = ((hy >> 31) & 1) | ((hx >> 30) & 2); // 2*sign(x) + sign(y)

    // when y = 0
    if (iy | ly) == 0 {
        match m {
            0 | 1 => return y,      // atan(+/-0, +anything) = +/-0
            2 => return PI + TINY,  // atan(+0, -anything) = pi
            _ => return -PI - TINY, // atan(-0, -anything) = -pi
        }
    }
    // when x = 0
    if (ix | lx) == 0 {
        return if hy < 0 {
            -PI_O_2 - TINY
        } else {
            PI_O_2 + TINY
        };
    }
    // when x is INF
    if ix == EXP_BITS {
        if iy == EXP_BITS {
            return match m {
                0 => PI_O_4 + TINY,        // atan(+INF, +INF)
                1 => -PI_O_4 - TINY,       // atan(-INF, +INF)
                2 => 3.0 * PI_O_4 + TINY,  // atan(+INF, -INF)
                _ => -3.0 * PI_O_4 - TINY, // atan(-INF, -INF)
            };
        }
        return match m {
            0 => 0.0,        // atan(+..., +INF)
            1 => -0.0,       // atan(-..., +INF)
            2 => PI + TINY,  // atan(+..., -INF)
            _ => -PI - TINY, // atan(-..., -INF)
        };
    }
    // when y is INF
    if iy == EXP_BITS {
        return if hy < 0 {
            -PI_O_2 - TINY
        } else {
            PI_O_2 + TINY
        };
    }

    // compute y/x
    let k = (iy - ix) >> 20;
    let z = if k > 60 {
        PI_O_2 + 0.5 * PI_LO // |y/x| > 2^60
    } else if hx < 0 && k < -60 {
        0.0 // |y|/x < -2^60
    } else {
        atan((y / x).abs()) // safe to do y/x
    };
    match m {
        0 => z,                // atan(+, +)
        1 => -z,               // atan(-, +)
        2 => PI - (z - PI_LO), // atan(+, -)
        _ => (z - PI_LO) - PI, // atan(-, -)
    }
}

/// `StrictMath.log1p(x)` (fdlibm `log1p`).
#[must_use]
pub fn log1p(x: f64) -> f64 {
    const LN2_HI: f64 = f64::from_bits(0x3FE6_2E42_FEE0_0000);
    const LN2_LO: f64 = f64::from_bits(0x3DEA_39EF_3579_3C76);
    const LP1: f64 = f64::from_bits(0x3FE5_5555_5555_5593);
    const LP2: f64 = f64::from_bits(0x3FD9_9999_9997_FA04);
    const LP3: f64 = f64::from_bits(0x3FD2_4924_9422_9359);
    const LP4: f64 = f64::from_bits(0x3FCC_71C5_1D8E_78AF);
    const LP5: f64 = f64::from_bits(0x3FC7_4664_96CB_03DE);
    const LP6: f64 = f64::from_bits(0x3FC3_9A09_D078_C69F);
    const LP7: f64 = f64::from_bits(0x3FC2_F112_DF3E_5244);
    let mut f = 0.0;
    let mut c = 0.0;
    let mut hu = 0;
    let hx = hi(x);
    let ax = hx & EXP_SIGNIF_BITS;

    let mut k = 1;
    if hx < 0x3FDA_827A {
        // x < 0.41422
        if ax >= 0x3FF0_0000 {
            // x <= -1.0
            return if x == -1.0 {
                f64::NEG_INFINITY
            } else {
                f64::NAN
            };
        }
        if ax < 0x3E20_0000 {
            // |x| < 2^-29
            return if TWO54 + x > 0.0 && ax < 0x3C90_0000 {
                x // |x| < 2^-54
            } else {
                x - x * x * 0.5
            };
        }
        if hx > 0 || hx <= 0xBFD2_BEC3_u32.cast_signed() {
            // -0.2929 < x < 0.41422
            k = 0;
            f = x;
            hu = 1;
        }
    }
    if hx >= EXP_BITS {
        return if x.is_nan() { add_nan(x, x) } else { x + x };
    }
    if k != 0 {
        let mut u;
        if hx < 0x4340_0000 {
            u = 1.0 + x;
            hu = hi(u);
            k = (hu >> 20) - 1023;
            // correction term
            c = if k > 0 { 1.0 - (u - x) } else { x - (u - 1.0) };
            c /= u;
        } else {
            u = x;
            hu = hi(u);
            k = (hu >> 20) - 1023;
            c = 0.0;
        }
        hu &= 0x000F_FFFF;
        if hu < 0x6_A09E {
            u = with_hi(u, hu | 0x3FF0_0000); // normalize u
        } else {
            k += 1;
            u = with_hi(u, hu | 0x3FE0_0000); // normalize u/2
            hu = (0x0010_0000 - hu) >> 2;
        }
        f = u - 1.0;
    }
    let hfsq = 0.5 * f * f;
    let kf = f64::from(k);
    if hu == 0 {
        // |f| < 2^-20
        if f == 0.0 {
            if k == 0 {
                return 0.0;
            }
            c += kf * LN2_LO;
            return kf * LN2_HI + c;
        }
        // Java's literal 0.66666666666666666 rounds to these bits.
        let two_thirds = f64::from_bits(0x3FE5_5555_5555_5555);
        let r = hfsq * (1.0 - two_thirds * f);
        return if k == 0 {
            f - r
        } else {
            kf * LN2_HI - ((r - (kf * LN2_LO + c)) - f)
        };
    }
    let s = f / (2.0 + f);
    let z = s * s;
    let r = z * (LP1 + z * (LP2 + z * (LP3 + z * (LP4 + z * (LP5 + z * (LP6 + z * LP7))))));
    if k == 0 {
        f - (hfsq - s * (hfsq + r))
    } else {
        kf * LN2_HI - ((hfsq - (s * (hfsq + r) + (kf * LN2_LO + c))) - f)
    }
}
