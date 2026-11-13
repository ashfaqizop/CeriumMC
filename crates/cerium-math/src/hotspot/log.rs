//! HotSpot's x86_64 `Math.log` and `Math.log10`: a transcription of OpenJDK's
//! `stubGenerator_x86_64_log.cpp` (tag `jdk-25.0.1-ga`) onto the [`Xmm`] model.
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
//! `x = 2^k * mx` with `mx` in `[1, 2)`. `B ≈ 1/mx` comes from `RCPPS`, rounded to 7 bits.
//! The reduced argument `r = B*mx - 1` is computed exactly in high and low parts. The result
//! is `k*log(2) - log(B) + p(r)`, with `-log(B)` read from a table and `p` a polynomial.
//!
//! Every instruction below mirrors one stub instruction, and labels match the stub's. Register
//! upper lanes the stub never reads are modelled as zero.

use super::log_tables::{
    COEFF, COEFF_LOG10, HIGHSIGMASK_LOG10, L_TBL, L_TBL_LOG10, LOG2, LOG2_LOG10, LOG10_E,
};
use super::rcp::Vendor;
use super::sse::Xmm;

/// `Math.log(x)` exactly as HotSpot computes it on x86_64, with `RCPPS` from `vendor`.
#[allow(clippy::too_many_lines, reason = "a transcription of one assembly routine")]
pub(crate) fn log(x: f64, vendor: Vendor) -> f64 {
    let stack0 = x;
    let mut xmm0 = Xmm::from_f64(x);
    let mut xmm2 = Xmm::from_u64(0x3FF0_0000_0000_0000);
    let mut xmm3 = Xmm::from_u64(0x77F0_0000_0000_0000);
    let mut xmm4 = Xmm::from_u32(32_768);
    let mut xmm5 = Xmm::from_u64(0xFFFF_E000_0000_0000);
    let mut xmm1 = xmm0;
    let mut xmm6;
    let mut xmm7 = Xmm::default();
    let mut eax = xmm0.pextrw(3);
    xmm0.por(xmm2);
    let mut ecx: u32 = 16_352;
    xmm0.psrlq(27);
    xmm0.psrld(2);
    xmm0.rcpps(vendor);
    xmm1.psllq(12);
    xmm6 = Xmm::pshufd(xmm5, 228);
    xmm1.psrlq(12);
    eax = eax.wrapping_sub(16);
    if eax >= 32_736 {
        // L_2TAG_PACKET_0_0_2: zero, denormal, negative, infinite or NaN.
        xmm0 = Xmm::from_f64(stack0);
        xmm1 = Xmm::from_f64(stack0);
        eax = eax.wrapping_add(16);
        if eax >= 32_768 {
            // L_2TAG_PACKET_2_0_2: negative (sign bit set).
            let edx = xmm1.low_u32();
            xmm1.psrlq(32);
            let ecx2 = xmm1.low_u32().wrapping_add(xmm1.low_u32());
            if ecx2 >= 0xFFE0_0000 {
                // L_2TAG_PACKET_5_0_2: -Inf or NaN.
                if ecx2 > 0xFFE0_0000 || edx > 0 {
                    return nan_or_self(xmm0); // L_2TAG_PACKET_4_0_2
                }
                return invalid(); // L_2TAG_PACKET_6_0_2: -Inf
            }
            if edx | ecx2 == 0 {
                return negative_infinity(); // L_2TAG_PACKET_7_0_2: -0.0
            }
            return invalid(); // L_2TAG_PACKET_6_0_2
        }
        if eax >= 16 {
            return nan_or_self(xmm0); // L_2TAG_PACKET_4_0_2: +Inf or NaN
        }
        // L_2TAG_PACKET_3_0_2: +0.0 or a denormal.
        xmm1 = Xmm::default();
        xmm1.addsd(xmm0);
        let edx = xmm1.low_u32();
        xmm1.psrlq(32);
        if edx | xmm1.low_u32() == 0 {
            return negative_infinity(); // L_2TAG_PACKET_7_0_2
        }
        xmm1 = Xmm::default();
        xmm1.pinsrw(18_416, 3);
        xmm0.mulsd(xmm1);
        xmm1 = xmm0;
        eax = xmm0.pextrw(3);
        xmm0.por(xmm2);
        xmm0.psrlq(27);
        ecx = 18_416;
        xmm0.psrld(2);
        xmm0.rcpps(vendor);
        xmm1.psllq(12);
        xmm6 = Xmm::pshufd(xmm5, 228);
        xmm1.psrlq(12);
    }

    // L_2TAG_PACKET_1_0_2: the main path.
    xmm0.paddd(xmm4);
    xmm1.por(xmm3);
    let mut edx = xmm0.low_u32();
    xmm0.psllq(29);
    xmm5.pand(xmm1);
    xmm0.pand(xmm6);
    xmm1.subsd(xmm5);
    xmm5.mulpd(xmm0);
    eax &= 32_752;
    eax = eax.wrapping_sub(ecx);
    xmm7.cvtsi2sdl(eax.cast_signed());
    xmm1.mulsd(xmm0);
    xmm6 = Xmm::load_low(&LOG2, 0);
    xmm3 = Xmm::load(&COEFF, 0);
    xmm5.subsd(xmm2);
    edx &= 16_711_680;
    edx >>= 12;
    xmm0 = Xmm::load(&L_TBL, (edx / 4) as usize);
    xmm4 = Xmm::load(&COEFF, 4);
    xmm1.addsd(xmm5);
    xmm2 = Xmm::load(&COEFF, 8);
    xmm6.mulsd(xmm7);
    xmm5 = Xmm::movddup(xmm1);
    xmm7.mulsd(Xmm::load_low(&LOG2, 2));
    xmm3.mulsd(xmm1);
    xmm0.addsd(xmm6);
    xmm4.mulpd(xmm5);
    xmm5.mulpd(xmm5);
    xmm6 = Xmm::movddup(xmm0);
    xmm0.addsd(xmm1);
    xmm4.addpd(xmm2);
    xmm3.mulpd(xmm5);
    xmm6.subsd(xmm0);
    xmm4.mulsd(xmm1);
    xmm2 = Xmm::pshufd(xmm0, 238);
    xmm1.addsd(xmm6);
    xmm5.mulsd(xmm5);
    xmm7.addsd(xmm2);
    xmm4.addpd(xmm3);
    xmm1.addsd(xmm7);
    xmm4.mulpd(xmm5);
    xmm1.addsd(xmm4);
    xmm5 = Xmm::pshufd(xmm4, 238);
    xmm1.addsd(xmm5);
    xmm0.addsd(xmm1);
    xmm0.low()
}

/// `Math.log10(x)` exactly as HotSpot computes it on x86_64, with `RCPPS` from `vendor`.
#[allow(clippy::too_many_lines, reason = "a transcription of one assembly routine")]
pub(crate) fn log10(x: f64, vendor: Vendor) -> f64 {
    let stack0 = x;
    let mut xmm0 = Xmm::from_f64(x);
    let mut xmm2 = Xmm::default();
    xmm2.pinsrw(16_368, 3);
    let mut xmm7 = Xmm::from_u32(1_054_736_384);
    let mut xmm3 = Xmm::default();
    xmm3.pinsrw(30_704, 3);
    let mut xmm1 = xmm0;
    let mut xmm4 = Xmm::from_u32(32_768);
    let mut xmm5 = Xmm::load(&HIGHSIGMASK_LOG10, 0);
    let mut eax = xmm0.pextrw(3);
    xmm0.por(xmm2);
    let mut ecx: u32 = 16_352;
    xmm0.psrlq(27);
    xmm2 = Xmm::load(&LOG10_E, 0);
    xmm0.psrld(2);
    xmm0.rcpps(vendor);
    xmm1.psllq(12);
    let mut xmm6 = Xmm::pshufd(xmm5, 78);
    xmm1.psrlq(12);
    eax = eax.wrapping_sub(16);
    if eax >= 32_736 {
        // L_2TAG_PACKET_0_0_2: zero, denormal, negative, infinite or NaN.
        xmm0 = Xmm::from_f64(stack0);
        xmm1 = Xmm::from_f64(stack0);
        eax = eax.wrapping_add(16);
        if eax >= 32_768 {
            // L_2TAG_PACKET_2_0_2: negative (sign bit set).
            let edx = xmm1.low_u32();
            xmm1.psrlq(32);
            let ecx2 = xmm1.low_u32().wrapping_add(xmm1.low_u32());
            if ecx2 >= 0xFFE0_0000 {
                // L_2TAG_PACKET_5_0_2: -Inf or NaN.
                if ecx2 > 0xFFE0_0000 || edx > 0 {
                    return nan_or_self(xmm0); // L_2TAG_PACKET_4_0_2
                }
                return invalid(); // L_2TAG_PACKET_6_0_2: -Inf
            }
            if edx | ecx2 == 0 {
                return negative_infinity(); // L_2TAG_PACKET_7_0_2: -0.0
            }
            return invalid(); // L_2TAG_PACKET_6_0_2
        }
        if eax >= 16 {
            return nan_or_self(xmm0); // L_2TAG_PACKET_4_0_2: +Inf or NaN
        }
        // L_2TAG_PACKET_3_0_2: +0.0 or a denormal.
        xmm1 = Xmm::default();
        xmm1.addsd(xmm0);
        let edx = xmm1.low_u32();
        xmm1.psrlq(32);
        if edx | xmm1.low_u32() == 0 {
            return negative_infinity(); // L_2TAG_PACKET_7_0_2
        }
        xmm1 = Xmm::default();
        xmm1.pinsrw(18_416, 3);
        xmm0.mulsd(xmm1);
        xmm2 = Xmm::default();
        xmm2.pinsrw(16_368, 3);
        xmm1 = xmm0;
        eax = xmm0.pextrw(3);
        xmm0.por(xmm2);
        ecx = 18_416;
        xmm0.psrlq(27);
        xmm2 = Xmm::load(&LOG10_E, 0);
        xmm0.psrld(2);
        xmm0.rcpps(vendor);
        xmm1.psllq(12);
        xmm6 = Xmm::pshufd(xmm5, 78);
        xmm1.psrlq(12);
    }

    // L_2TAG_PACKET_1_0_2: the main path.
    xmm0.mulss(xmm7);
    xmm1.por(xmm3);
    xmm5.pand(xmm1);
    xmm0.paddd(xmm4);
    xmm1.subsd(xmm5);
    let mut edx = xmm0.low_u32();
    xmm0.psllq(29);
    xmm0.pand(xmm6);
    eax &= 32_752;
    eax = eax.wrapping_sub(ecx);
    xmm7.cvtsi2sdl(eax.cast_signed());
    xmm5.mulpd(xmm0);
    xmm1.mulsd(xmm0);
    xmm6 = Xmm::load_low(&LOG2_LOG10, 0);
    xmm3 = Xmm::load(&COEFF_LOG10, 0);
    xmm5.subsd(xmm2);
    edx &= 16_711_680;
    edx >>= 12;
    xmm0 = Xmm::load(&L_TBL_LOG10, ((edx - 1504) / 4) as usize);
    xmm4 = Xmm::load(&COEFF_LOG10, 4);
    xmm1.addsd(xmm5);
    xmm2 = Xmm::load(&COEFF_LOG10, 8);
    xmm6.mulsd(xmm7);
    xmm5 = Xmm::pshufd(xmm1, 68);
    xmm7.mulsd(Xmm::load_low(&LOG2_LOG10, 2));
    xmm3.mulsd(xmm1);
    xmm0.addsd(xmm6);
    xmm4.mulpd(xmm5);
    xmm6 = Xmm::load_low(&LOG10_E, 2);
    xmm5.mulpd(xmm5);
    xmm4.addpd(xmm2);
    xmm3.mulpd(xmm5);
    xmm2 = Xmm::pshufd(xmm0, 228);
    xmm0.addsd(xmm1);
    xmm4.mulsd(xmm1);
    xmm2.subsd(xmm0);
    xmm6.mulsd(xmm1);
    xmm1.addsd(xmm2);
    xmm2 = Xmm::pshufd(xmm0, 238);
    xmm5.mulsd(xmm5);
    xmm7.addsd(xmm2);
    xmm1.addsd(xmm6);
    xmm4.addpd(xmm3);
    xmm1.addsd(xmm7);
    xmm4.mulpd(xmm5);
    xmm1.addsd(xmm4);
    xmm5 = Xmm::pshufd(xmm4, 238);
    xmm1.addsd(xmm5);
    xmm0.addsd(xmm1);
    xmm0.low()
}

/// `L_2TAG_PACKET_4_0_2`: `x + x`, which returns `+Inf` unchanged and quiets a NaN.
fn nan_or_self(mut xmm0: Xmm) -> f64 {
    xmm0.addsd(xmm0);
    xmm0.low()
}

/// `L_2TAG_PACKET_6_0_2`: `0.0 * +Inf`, the x86 default NaN.
fn invalid() -> f64 {
    let mut xmm1 = Xmm::default();
    let mut xmm0 = Xmm::default();
    xmm1.pinsrw(32_752, 3);
    xmm0.mulsd(xmm1);
    xmm0.low()
}

/// `L_2TAG_PACKET_7_0_2`: `-1.0 / 0.0`.
fn negative_infinity() -> f64 {
    let xmm1 = Xmm::default();
    let mut xmm0 = Xmm::default();
    xmm0.pinsrw(49_136, 3);
    xmm0.divsd(xmm1);
    xmm0.low()
}
