//! HotSpot's x86_64 `Math.exp`: a transcription of OpenJDK's `stubGenerator_x86_64_exp.cpp`
//! (tag `jdk-25.0.1-ga`) onto the [`Xmm`] model.
//!
//! Original notice of the transcribed file:
//!
//! Copyright (c) 2016, 2024, Intel Corporation. All rights reserved.
//! Copyright (C) 2021 THL A29 Limited, a Tencent company. All rights reserved.
//! Intel Math Library (LIBM) Source Code
//!
//! This code is free software; you can redistribute it and/or modify it under the terms of the
//! GNU General Public License version 2 only, as published by the Free Software Foundation.
//!
//! This code is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
//! without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
//! See the GNU General Public License version 2 for more details.
//!
//! # Algorithm (from the original description)
//!
//! With `K = 64`: `x = m*log(2)/K + y` with `y` in `[-log(2)/K, log(2)/K]` and `m = n*K + j`.
//! Then `e^x = 2^n * T[j] * (1 + P(y))`, where `T[j] = 2^(j/K)` is tabulated in high and low
//! parts and `P` is a minimax polynomial for `e^y - 1`. Near overflow and underflow, `2^n` is
//! applied in two steps.
//!
//! Every instruction below mirrors one stub instruction, and labels match the stub's. `exp`
//! does not use `RCPPS`, so its results are the same for every CPU vendor.

use super::constants::{ONE, SHIFTER, ZERO};
use super::exp_tables::{ALLONES, BIAS, CV, EBIAS, INF, MMASK, TBL_ADDR, XMAX, XMIN};
use super::sse::Xmm;

/// `Math.exp(x)` exactly as HotSpot computes it on x86_64.
#[allow(
    clippy::too_many_lines,
    reason = "a transcription of one assembly routine"
)]
pub(crate) fn exp(x: f64) -> f64 {
    // The stub spills the argument to [rsp + 8]; [rsp + 12] is its high word.
    let low_word = x.to_bits() as u32;
    let high_word = (x.to_bits() >> 32) as u32;
    let mut xmm0 = Xmm::from_f64(x);
    xmm0.unpcklpd(xmm0);
    let mut xmm1 = Xmm::load(&CV, 0);
    let mut xmm2 = Xmm::load(&CV, 4);
    let mut xmm3 = Xmm::load(&CV, 8);
    let mut xmm6 = Xmm::load(&SHIFTER, 0);
    let mut eax = xmm0.pextrw(3);
    eax &= 0x7FFF;
    let mut edx = 16_527_u32.wrapping_sub(eax);
    eax = eax.wrapping_sub(15_504);
    edx |= eax;
    if edx >= 0x8000_0000 {
        return special(x, xmm0, low_word, high_word); // L_2TAG_PACKET_0_0_2
    }
    xmm1.mulpd(xmm0);
    xmm1.addpd(xmm6);
    let mut xmm7 = xmm1;
    xmm1.subpd(xmm6);
    xmm2.mulpd(xmm1);
    let mut xmm4 = Xmm::load(&CV, 16);
    xmm3.mulpd(xmm1);
    let mut xmm5 = Xmm::load(&CV, 20);
    xmm0.subpd(xmm2);
    eax = xmm7.low_u32();
    let mut ecx = eax;
    ecx &= 0x3F;
    ecx <<= 4;
    eax = (eax.cast_signed() >> 6).cast_unsigned();
    edx = eax;
    xmm6 = Xmm::load(&MMASK, 0);
    xmm7.pand(xmm6);
    xmm6 = Xmm::load(&BIAS, 0);
    xmm7.paddq(xmm6);
    xmm7.psllq(46);
    xmm0.subpd(xmm3);
    xmm2 = Xmm::load(&TBL_ADDR, (ecx / 4) as usize);
    xmm4.mulpd(xmm0);
    xmm6 = xmm0;
    xmm1 = xmm0;
    xmm6.mulpd(xmm6);
    xmm0.mulpd(xmm6);
    xmm5.addpd(xmm4);
    xmm0.mulsd(xmm6);
    xmm6.mulpd(Xmm::load(&CV, 12));
    xmm1.addsd(xmm2);
    xmm2.unpckhpd(xmm2);
    xmm0.mulpd(xmm5);
    xmm1.addsd(xmm0);
    xmm2.por(xmm7);
    xmm0.unpckhpd(xmm0);
    xmm0.addsd(xmm1);
    xmm0.addsd(xmm6);
    edx = edx.wrapping_add(894);
    if edx <= 1916 {
        xmm0.mulsd(xmm2);
        xmm0.addsd(xmm2);
        return xmm0.low(); // B1_5
    }

    // L_2TAG_PACKET_1_0_2: the result is near overflow or underflow; scale in two steps.
    xmm3 = Xmm::default();
    xmm4 = Xmm::load(&ALLONES, 0);
    let n = eax.cast_signed();
    edx = (-1022_i32).wrapping_sub(n).cast_unsigned();
    xmm5 = Xmm::from_u32(edx);
    xmm4.psllq_by(xmm5);
    ecx = eax;
    eax = (eax.cast_signed() >> 1).cast_unsigned();
    xmm3.pinsrw(eax, 3);
    xmm6 = Xmm::load(&EBIAS, 0);
    xmm3.psllq(4);
    xmm2.psubd(xmm3);
    xmm0.mulsd(xmm2);
    if edx.cast_signed() > 52 {
        // L_2TAG_PACKET_2_0_2
        xmm3.paddd(xmm6);
        xmm0.addpd(xmm2);
        xmm0.mulsd(xmm3);
        return xmm0.low(); // L_2TAG_PACKET_6_0_2
    }
    xmm4.pand(xmm2);
    xmm3.paddd(xmm6);
    xmm2.subsd(xmm4);
    xmm0.addsd(xmm2);
    if ecx.cast_signed() >= 1023 {
        // L_2TAG_PACKET_3_0_2: both of its exits (overflow or not) return xmm0 unchanged.
        xmm0.addsd(xmm4);
        xmm0.mulsd(xmm3);
        return xmm0.low();
    }
    ecx = xmm0.pextrw(3);
    ecx &= 0x8000;
    edx |= ecx;
    if edx == 0 {
        // L_2TAG_PACKET_4_0_2
        xmm0.addsd(xmm4);
        xmm0.mulsd(xmm3);
        return xmm0.low();
    }
    xmm6 = xmm0;
    xmm0.addsd(xmm4);
    xmm0.mulsd(xmm3);
    ecx = xmm0.pextrw(3);
    ecx &= 0x7FF0;
    if ecx != 0 {
        return xmm0.low(); // B1_5
    }

    // L_2TAG_PACKET_5_0_2: the result is subnormal; round it once, from the unscaled parts.
    xmm6.mulsd(xmm3);
    xmm4.mulsd(xmm3);
    xmm0 = xmm6;
    xmm6.pxor(xmm4);
    xmm6.psrad(31);
    xmm6 = Xmm::pshufd(xmm6, 85);
    xmm0.psllq(1);
    xmm0.psrlq(1);
    xmm0.pxor(xmm6);
    xmm6.psrlq(63);
    xmm0.paddq(xmm6);
    xmm0.paddq(xmm4);
    xmm0.low() // L_2TAG_PACKET_6_0_2
}

/// `L_2TAG_PACKET_0_0_2`: tiny, huge, infinite or NaN arguments. `xmm0` holds `x` in both
/// lanes.
fn special(x: f64, mut xmm0: Xmm, low_word: u32, high_word: u32) -> f64 {
    let eax = high_word & 0x7FFF_FFFF;
    if eax < 1_083_179_008 {
        // |x| < 2^-54 (or subnormal): e^x rounds to 1 + x.
        xmm0.addsd(Xmm::load_low(&ONE, 0));
        return xmm0.low();
    }
    // L_2TAG_PACKET_8_0_2
    if eax < 2_146_435_072 {
        let mut xmm0 = if high_word >= 0x8000_0000 {
            Xmm::load_low(&XMIN, 0) // L_2TAG_PACKET_10_0_2: underflow to +0.0
        } else {
            Xmm::load_low(&XMAX, 0) // overflow to +Inf (L_2TAG_PACKET_7_0_2)
        };
        xmm0.mulsd(xmm0);
        return xmm0.low();
    }
    // L_2TAG_PACKET_9_0_2: infinite or NaN.
    if eax > 2_146_435_072 || low_word != 0 {
        // L_2TAG_PACKET_11_0_2: NaN, returned quieted.
        let mut xmm0 = Xmm::from_f64(x);
        xmm0.addsd(xmm0);
        return xmm0.low();
    }
    if high_word == 2_146_435_072 {
        Xmm::load_low(&INF, 0).low()
    } else {
        Xmm::load_low(&ZERO, 0).low() // L_2TAG_PACKET_12_0_2: e^-Inf = +0.0
    }
}
