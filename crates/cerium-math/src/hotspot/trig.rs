//! HotSpot's x86_64 `Math.sin` and `Math.cos`: a transcription of OpenJDK's
//! `stubGenerator_x86_64_sin.cpp` and `stubGenerator_x86_64_cos.cpp` (tag `jdk-25.0.1-ga`)
//! onto the [`Xmm`] model, with general-purpose registers as `u64` values.
//!
//! Original notice of the transcribed files:
//!
//! Copyright (c) 2016, 2024, Intel Corporation. All rights reserved.
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
//! `x ≈ N*pi/32 + r` with `|r| <= pi/64`, computed as `x - N*(P_1 + P_2 + P_3)`. With
//! `B = (N mod 64)*pi/32`, `sin(B + r)` is assembled from tabulated `sin(B)` and `cos(B)`
//! (`CTABLE`) and polynomials in `r`, using compensated sums. `cos` is `sin` shifted by a
//! quarter period (16 table entries). Arguments with `|x| >= 90112` first go through a
//! Payne–Hanek reduction with the bits of `2/pi` (`PI_INV_TABLE`) in integer
//! registers; that part is identical in both stubs, so it is transcribed once.
//!
//! Every instruction below mirrors one stub instruction, and labels match the sin stub's.
//! 32-bit register instructions (`…l`) zero the upper half of their destination, which [`lo`]
//! models. Neither stub uses `RCPPS`, so results are the same for every CPU vendor.

use super::constants::{
    CTABLE, NEG_ZERO, ONE, ONEHALF, P_1, P_2, P_3, PI_4, PI_INV_TABLE, PI32INV, SC_1, SC_2, SC_3,
    SC_4, SIGN_MASK,
};
use super::sse::Xmm;

/// `ALL_ONES` of the sin stub: the largest double below 1.
const ALL_ONES: [u32; 2] = [0xFFFF_FFFF, 0x3FEF_FFFF];

/// `Math.sin(x)` exactly as HotSpot computes it on x86_64.
#[allow(
    clippy::too_many_lines,
    reason = "a transcription of one assembly routine"
)]
pub(crate) fn sin(x: f64) -> f64 {
    let mut xmm0 = Xmm::from_f64(x);
    let mut rax = (x.to_bits() >> 32) & 0x7FFF_0000;
    rax = lo(rax.wrapping_sub(808_452_096));
    if rax > 281_346_048 {
        // L_2TAG_PACKET_0_0_1, still on the flags of the comparison above.
        if (rax as u32).cast_signed() > 281_346_048 {
            return large(x, 1_865_216, false); // L_2TAG_PACKET_1_0_1
        }
        rax >>= 20;
        if rax == 3325 {
            // Zero or subnormal.
            xmm0.mulsd(Xmm::load_low(&ALL_ONES, 0));
            return xmm0.low();
        }
        // L_2TAG_PACKET_2_0_1: tiny; the stub computes 2^-55 * (2^55 * x - x) in xmm3 only to
        // raise the floating-point flags, and returns x.
        return xmm0.low();
    }
    let mut xmm1 = Xmm::load_low(&PI32INV, 0);
    xmm1.mulsd(xmm0);
    let mut xmm5 = Xmm::load(&ONEHALF, 0);
    let mut xmm4 = Xmm::load_low(&SIGN_MASK, 0);
    xmm4.pand(xmm0);
    xmm5.por(xmm4);
    xmm1.addpd(xmm5);
    let edx = xmm1.cvttsd2sil();
    xmm1.cvtsi2sdl(edx);
    let mut xmm6 = Xmm::load(&P_2, 0);
    let mut xmm3 = Xmm::from_u64(0x3FB9_21FB_5440_0000);
    xmm5 = Xmm::load(&SC_4, 0);
    xmm4 = Xmm::pshufd(xmm0, 68);
    xmm3.mulsd(xmm1);
    xmm1 = Xmm::movddup(xmm1);
    let rdx = lo(u64::from(edx.cast_unsigned()) & 63) << 5;
    let c = (rdx / 4) as usize; // rax = Ctable + rdx, as a `juint` index
    xmm6.mulpd(xmm1);
    xmm1.mulsd(Xmm::load_low(&P_3, 0));
    xmm4.subsd(xmm3);
    let mut xmm7 = Xmm::load_low(&CTABLE, c + 2);
    xmm0.subsd(xmm3);
    xmm3 = Xmm::movddup(xmm4);
    xmm4.subsd(xmm6);
    xmm0 = Xmm::pshufd(xmm0, 68);
    let mut xmm2 = Xmm::load(&CTABLE, c);
    xmm5.mulpd(xmm0);
    xmm0.subpd(xmm6);
    xmm7.mulsd(xmm4);
    xmm3.subsd(xmm4);
    xmm5.mulpd(xmm0);
    xmm0.mulpd(xmm0);
    xmm3.subsd(xmm6);
    xmm6 = Xmm::load(&SC_2, 0);
    xmm1.subsd(xmm3);
    xmm3 = Xmm::load_low(&CTABLE, c + 6);
    xmm2.addsd(xmm3);
    xmm7.subsd(xmm2);
    xmm2.mulsd(xmm4);
    xmm6.mulpd(xmm0);
    xmm3.mulsd(xmm4);
    xmm2.mulpd(xmm0);
    xmm0.mulpd(xmm0);
    xmm5.addpd(Xmm::load(&SC_3, 0));
    xmm4.mulsd(Xmm::load_low(&CTABLE, c));
    xmm6.addpd(Xmm::load(&SC_1, 0));
    xmm5.mulpd(xmm0);
    xmm0 = xmm3;
    xmm3.addsd(Xmm::load_low(&CTABLE, c + 2));
    xmm1.mulpd(xmm7);
    xmm7 = xmm4;
    xmm4.addsd(xmm3);
    xmm6.addpd(xmm5);
    xmm5 = Xmm::load_low(&CTABLE, c + 2);
    xmm5.subsd(xmm3);
    xmm3.subsd(xmm4);
    xmm1.addsd(Xmm::load_low(&CTABLE, c + 4));
    xmm6.mulpd(xmm2);
    xmm5.addsd(xmm0);
    xmm3.addsd(xmm7);
    xmm1.addsd(xmm5);
    xmm1.addsd(xmm3);
    xmm1.addsd(xmm6);
    xmm6.unpckhpd(xmm6);
    xmm0 = xmm4;
    xmm1.addsd(xmm6);
    xmm0.addsd(xmm1);
    xmm0.low() // B1_4
}

/// `Math.cos(x)` exactly as HotSpot computes it on x86_64.
#[allow(
    clippy::too_many_lines,
    reason = "a transcription of one assembly routine"
)]
pub(crate) fn cos(x: f64) -> f64 {
    let mut xmm0 = Xmm::from_f64(x);
    let mut rax = (x.to_bits() >> 32) & 0x7FFF_0000;
    rax = lo(rax.wrapping_sub(808_452_096));
    if rax > 281_346_048 {
        // L_2TAG_PACKET_0_0_1, still on the flags of the comparison above.
        if (rax as u32).cast_signed() > 281_346_048 {
            return large(x, 1_865_232, true); // L_2TAG_PACKET_1_0_1
        }
        // Tiny, zero or subnormal: 1 - |x|.
        let eax = xmm0.pextrw(3) & 0x7FFF;
        xmm0.pinsrw(eax, 3);
        let mut xmm1 = Xmm::load_low(&ONE, 0);
        xmm1.subsd(xmm0);
        return xmm1.low();
    }
    let mut xmm1 = Xmm::load_low(&PI32INV, 0);
    xmm1.mulsd(xmm0);
    let mut xmm5 = Xmm::load(&ONEHALF, 0);
    let mut xmm4 = Xmm::load_low(&SIGN_MASK, 0);
    xmm4.pand(xmm0);
    xmm5.por(xmm4);
    xmm1.addpd(xmm5);
    let edx = xmm1.cvttsd2sil();
    xmm1.cvtsi2sdl(edx);
    let mut xmm2 = Xmm::load(&P_2, 0);
    let mut xmm3 = Xmm::load_low(&P_1, 0);
    xmm3.mulsd(xmm1);
    xmm1.unpcklpd(xmm1);
    // `addq rdx, 1865232` on the zero-extended 32-bit result, then `andq rdx, 63`.
    let rdx = (u64::from(edx.cast_unsigned()).wrapping_add(1_865_232) & 63) << 5;
    xmm4 = xmm0;
    xmm5 = Xmm::load(&SC_4, 0);
    let c = (rdx / 4) as usize;
    xmm2.mulpd(xmm1);
    xmm0.subsd(xmm3);
    xmm1.mulsd(Xmm::load_low(&P_3, 0));
    xmm4.subsd(xmm3);
    let mut xmm7 = Xmm::load_low(&CTABLE, c + 2);
    xmm0.unpcklpd(xmm0);
    xmm3 = xmm4;
    xmm4.subsd(xmm2);
    xmm5.mulpd(xmm0);
    xmm0.subpd(xmm2);
    let mut xmm6 = Xmm::load(&SC_2, 0);
    xmm7.mulsd(xmm4);
    xmm3.subsd(xmm4);
    xmm5.mulpd(xmm0);
    xmm0.mulpd(xmm0);
    xmm3.subsd(xmm2);
    xmm2 = Xmm::load(&CTABLE, c);
    xmm1.subsd(xmm3);
    xmm3 = Xmm::load_low(&CTABLE, c + 6);
    xmm2.addsd(xmm3);
    xmm7.subsd(xmm2);
    xmm2.mulsd(xmm4);
    xmm6.mulpd(xmm0);
    xmm3.mulsd(xmm4);
    xmm2.mulpd(xmm0);
    xmm0.mulpd(xmm0);
    xmm5.addpd(Xmm::load(&SC_3, 0));
    xmm4.mulsd(Xmm::load_low(&CTABLE, c));
    xmm6.addpd(Xmm::load(&SC_1, 0));
    xmm5.mulpd(xmm0);
    xmm0 = xmm3;
    xmm3.addsd(Xmm::load_low(&CTABLE, c + 2));
    xmm1.mulpd(xmm7);
    xmm7 = xmm4;
    xmm4.addsd(xmm3);
    xmm6.addpd(xmm5);
    xmm5 = Xmm::load_low(&CTABLE, c + 2);
    xmm5.subsd(xmm3);
    xmm3.subsd(xmm4);
    xmm1.addsd(Xmm::load_low(&CTABLE, c + 4));
    xmm6.mulpd(xmm2);
    xmm0.addsd(xmm5);
    xmm3.addsd(xmm7);
    xmm0.addsd(xmm1);
    xmm0.addsd(xmm3);
    xmm0.addsd(xmm6);
    xmm6.unpckhpd(xmm6);
    xmm0.addsd(xmm6);
    xmm0.addsd(xmm4);
    xmm0.low() // B1_4
}

/// The low 32 bits, zero-extended: the result of a 32-bit register instruction.
fn lo(value: u64) -> u64 {
    value & 0xFFFF_FFFF
}

/// `L_2TAG_PACKET_1_0_1`: large, infinite or NaN arguments. `quarter` is the stub's octant
/// offset constant, and `wide` selects the cos stub's 64-bit conversions.
fn large(x: f64, quarter: u64, wide: bool) -> f64 {
    if Xmm::from_f64(x).pextrw(3) & 0x7FF0 == 0x7FF0 {
        // L_2TAG_PACKET_3_0_1: x * -0.0 gives the default NaN for infinities and quiets NaNs.
        let mut xmm0 = Xmm::from_f64(x);
        xmm0.mulsd(Xmm::load_low(&NEG_ZERO, 0));
        return xmm0.low();
    }
    let (xmm0, xmm6, rax) = reduce(x);
    finish(xmm0, xmm6, rax, quarter, wide)
}

/// Subtracts the 192-bit value `(high, middle, low)` from `top:0:0` (`subq`, `sbbq`, `sbbq`
/// from zeroed registers), returning the new `(r9, r10, r8)`.
fn negate(top: u64, high: u64, middle: u64, low: u64) -> (u64, u64, u64) {
    let (r8, borrow1) = 0_u64.overflowing_sub(low);
    let (r10, borrow2) = sub_with_borrow(0, middle, borrow1);
    let (r9, _) = sub_with_borrow(top, high, borrow2);
    (r9, r10, r8)
}

/// `sbbq`: `a - b - borrow`, with the borrow out.
fn sub_with_borrow(a: u64, b: u64, borrow: bool) -> (u64, bool) {
    let (difference, borrow_a) = a.overflowing_sub(b);
    let (difference, borrow_b) = difference.overflowing_sub(u64::from(borrow));
    (difference, borrow_a || borrow_b)
}

/// The Payne–Hanek reduction shared by both stubs, from `L_2TAG_PACKET_1_0_1` (after the
/// infinity and NaN check) to `L_2TAG_PACKET_12_0_1`. Returns `xmm0` and `xmm6` (the reduced
/// argument as a high and low part) and `rax` (the octant bits).
#[allow(
    clippy::too_many_lines,
    reason = "a transcription of one assembly routine"
)]
fn reduce(x: f64) -> (Xmm, Xmm, u64) {
    let table = |offset: u64| u64::from(PI_INV_TABLE[(offset / 4) as usize]);
    let xmm0 = Xmm::from_f64(x);
    let mut rcx = u64::from(xmm0.pextrw(3)) & 0x7FF0;
    rcx = lo(rcx.wrapping_sub(16_224));
    rcx >>= 7;
    rcx &= 0xFFFC;
    // The stub adds the address of PI_INV_TABLE; this is the byte offset into it.
    let offset = rcx;
    let mut rax = x.to_bits();
    let mut r10 = table(offset + 20);
    let mut r8 = table(offset + 24);
    let mut rdx = lo(rax);
    rax >>= 21;
    rax = lo(rax) | 0x8000_0000;
    rax >>= 11;
    let mut r9 = lo(r10);
    r10 = r10.wrapping_mul(rdx);
    r9 = r9.wrapping_mul(rax);
    r8 = r8.wrapping_mul(rax);
    let mut rsi = table(offset + 16);
    let mut rdi = table(offset + 12);
    let mut r11 = lo(r10);
    r10 >>= 32;
    r9 = r9.wrapping_add(r10);
    r11 = r11.wrapping_add(r8);
    r8 = lo(r11);
    r11 >>= 32;
    r9 = r9.wrapping_add(r11);
    r10 = lo(rsi);
    rsi = rsi.wrapping_mul(rdx);
    r10 = r10.wrapping_mul(rax);
    r11 = lo(rdi);
    rdi = rdi.wrapping_mul(rdx);
    let mut rbx = lo(rsi);
    rsi >>= 32;
    r9 = r9.wrapping_add(rbx);
    rbx = lo(r9);
    r9 >>= 32;
    r10 = r10.wrapping_add(rsi);
    r10 = r10.wrapping_add(r9);
    rbx <<= 32;
    r8 |= rbx;
    r11 = r11.wrapping_mul(rax);
    r9 = table(offset + 8);
    rsi = table(offset + 4);
    rbx = lo(rdi);
    rdi >>= 32;
    r10 = r10.wrapping_add(rbx);
    rbx = lo(r10);
    r10 >>= 32;
    r11 = r11.wrapping_add(rdi);
    r11 = r11.wrapping_add(r10);
    rdi = r9;
    r9 = r9.wrapping_mul(rdx);
    rdi = rdi.wrapping_mul(rax);
    r10 = lo(r9);
    r9 >>= 32;
    r11 = r11.wrapping_add(r10);
    r10 = lo(r11);
    r11 >>= 32;
    rdi = rdi.wrapping_add(r9);
    rdi = rdi.wrapping_add(r11);
    r9 = rsi;
    rsi = rsi.wrapping_mul(rdx);
    r9 = r9.wrapping_mul(rax);
    r10 <<= 32;
    r10 |= rbx;
    rax = table(offset);
    r11 = lo(rsi);
    rsi >>= 32;
    rdi = rdi.wrapping_add(r11);
    r11 = lo(rdi);
    rdi >>= 32;
    r9 = r9.wrapping_add(rsi);
    r9 = r9.wrapping_add(rdi);
    rdx = rdx.wrapping_mul(rax);
    rbx = u64::from(xmm0.pextrw(3));
    rcx = offset; // subq rcx, PI_INV_TABLE
    rcx = lo(rcx << 3); // three `addl rcx, rcx`
    rcx = lo(rcx + 19);
    rsi = 0x8000 & rbx;
    rbx = lo(rbx >> 4);
    rbx &= 0x7FF;
    rbx = lo(rbx.wrapping_sub(1023));
    rcx = lo(rcx.wrapping_sub(rbx));
    r9 = r9.wrapping_add(rdx);
    rdx = lo(rcx + 32);
    if (rcx as u32).cast_signed() >= 1 {
        rcx = lo(rcx.wrapping_neg());
        rcx = lo(rcx.wrapping_add(29));
        r9 = lo(lo(r9) << (rcx & 31));
        rdi = lo(r9);
        r9 &= 0x1FFF_FFFF;
        if r9 & 0x1000_0000 == 0 {
            r9 = lo(r9) >> (rcx & 31);
            rbx = 0;
            r9 = (r9 << 32) | r11;
        } else {
            // L_2TAG_PACKET_5_0_1: the fraction is at least 1/2; negate it.
            r9 = lo(r9) >> (rcx & 31);
            rbx = lo(0x2000_0000 >> (rcx & 31));
            r9 = (r9 << 32) | r11;
            rbx <<= 32;
            rdi = lo(rdi + 0x2000_0000);
            (r9, r10, r8) = negate(rbx, r9, r10, r8);
            rbx = 0x8000;
        }
    } else {
        // L_2TAG_PACKET_4_0_1
        rcx = lo(rcx.wrapping_neg());
        r9 = (r9 << 32) | r11;
        r9 <<= rcx & 63;
        rdi = r9;
        if r9 & 0x8000_0000 == 0 {
            r9 = lo(r9) >> (rcx & 31);
            rbx = 0;
            rdi >>= 3;
        } else {
            // L_2TAG_PACKET_13_0_1: the fraction is at least 1/2; negate it.
            r9 = lo(r9) >> (rcx & 31);
            rbx = 0x1_0000_0000_u64 >> (rcx & 63);
            (r9, r10, r8) = negate(rbx, r9, r10, r8);
            rbx = 0x8000;
            rdi >>= 3;
            rdi = lo(rdi + 0x2000_0000);
        }
    }

    // L_2TAG_PACKET_7_0_1: normalize the fraction `r9:r10:r8`.
    if r9 == 0 {
        // L_2TAG_PACKET_8_0_1: shift whole words.
        rdx = lo(rdx + 64);
        r9 = r10;
        r10 = r8;
        r8 = 0;
        if r9 == 0 {
            rdx = lo(rdx + 64);
            r9 = r10;
            r10 = r8;
            if r9 == 0 {
                return (Xmm::default(), Xmm::default(), rax); // to L_2TAG_PACKET_12_0_1
            }
        }
    }
    // L_2TAG_PACKET_9_0_1
    let r11 = 63 - u64::from(r9.leading_zeros()); // bsrq
    let shift = 29_i32 - r11 as i32; // `subl rcx, r11`, whose flags select the branch
    rcx = lo(u64::from(shift.cast_unsigned()));
    if shift > 0 {
        r9 <<= rcx & 63;
        rax = r10;
        r10 <<= rcx & 63;
        rdx = lo(rdx + rcx);
        rcx = lo(rcx.wrapping_neg());
        rcx = lo(rcx + 64);
        rax >>= rcx & 63;
        r8 >>= rcx & 63;
        r9 |= rax;
        r10 |= r8;
    } else if shift < 0 {
        // L_2TAG_PACKET_10_0_1
        rcx = lo(rcx.wrapping_neg());
        r10 >>= rcx & 63;
        rax = r9;
        r9 >>= rcx & 63;
        rdx = lo(rdx.wrapping_sub(rcx));
        rcx = lo(rcx.wrapping_neg());
        rcx = lo(rcx + 64);
        rax <<= rcx & 63;
        r10 |= rax;
    }

    // L_2TAG_PACKET_11_0_1: convert to doubles and multiply by pi/4.
    let mut xmm0 = xmm0;
    xmm0.cvtsi2sdq(r9.cast_signed());
    r10 >>= 1;
    let mut xmm3 = Xmm::default();
    xmm3.cvtsi2sdq(r10.cast_signed());
    let mut xmm4 = Xmm::default();
    rdx = lo(rdx << 4);
    rdx = lo(rdx.wrapping_neg());
    rdx = lo(rdx + 16_368);
    rdx |= rsi;
    rdx = lo(rdx ^ rbx);
    xmm4.pinsrw(rdx as u32, 3);
    let mut xmm2 = Xmm::load_low(&PI_4, 0);
    let mut xmm6 = Xmm::load_low(&PI_4, 2);
    let mut xmm5 = Xmm::default();
    rdx = lo(rdx.wrapping_sub(1008));
    xmm5.pinsrw(rdx as u32, 3);
    xmm0.mulsd(xmm4);
    rsi = lo(rsi << 16);
    rsi = u64::from(((rsi as u32).cast_signed() >> 31).cast_unsigned());
    xmm3.mulsd(xmm5);
    let mut xmm1 = xmm0;
    xmm0.mulsd(xmm2);
    rdi = lo(rdi) >> 29;
    xmm1.addsd(xmm3);
    xmm3.mulsd(xmm2);
    rdi = lo(rdi + rsi);
    rdi = lo(rdi ^ rsi);
    xmm6.mulsd(xmm1);
    rax = lo(rdi);
    xmm6.addsd(xmm3);
    xmm2 = xmm0;
    xmm0.addsd(xmm6);
    xmm2.subsd(xmm0);
    xmm6.addsd(xmm2);
    (xmm0, xmm6, rax)
}

/// `L_2TAG_PACKET_12_0_1` (sin) and `L_2TAG_PACKET_11_0_1` (cos): evaluates the reduced
/// argument `xmm0 + xmm6` in octant `rax`. The two stubs differ only in `quarter` and in
/// cos's 64-bit conversions (`wide`).
#[allow(
    clippy::too_many_lines,
    reason = "a transcription of one assembly routine"
)]
fn finish(mut xmm0: Xmm, mut xmm6: Xmm, rax: u64, quarter: u64, wide: bool) -> f64 {
    let mut xmm1 = Xmm::load_low(&PI32INV, 0);
    xmm1.mulsd(xmm0);
    let mut xmm5 = Xmm::load_low(&ONEHALF, 0);
    let mut xmm4 = Xmm::load_low(&SIGN_MASK, 0);
    xmm4.pand(xmm0);
    xmm5.por(xmm4);
    xmm1.addpd(xmm5);
    let rdx = if wide {
        let n = xmm1.cvttsd2siq();
        xmm1.cvtsi2sdq(n);
        n.cast_unsigned()
    } else {
        let n = xmm1.cvttsd2sil();
        xmm1.cvtsi2sdl(n);
        u64::from(n.cast_unsigned())
    };
    let mut xmm3 = Xmm::load_low(&P_1, 0);
    let mut xmm2 = Xmm::load(&P_2, 0);
    xmm3.mulsd(xmm1);
    xmm1.unpcklpd(xmm1);
    let rax = lo(rax << 3);
    let mut rdx = lo(rdx.wrapping_add(quarter));
    xmm4 = xmm0;
    rdx = lo(rdx.wrapping_add(rax));
    rdx &= 63;
    xmm5 = Xmm::load(&SC_4, 0);
    rdx = lo(rdx << 5);
    let c = (rdx / 4) as usize;
    xmm2.mulpd(xmm1);
    xmm0.subsd(xmm3);
    xmm1.mulsd(Xmm::load_low(&P_3, 0));
    xmm4.subsd(xmm3);
    let mut xmm7 = Xmm::load_low(&CTABLE, c + 2);
    xmm0.unpcklpd(xmm0);
    xmm3 = xmm4;
    xmm4.subsd(xmm2);
    xmm5.mulpd(xmm0);
    xmm0.subpd(xmm2);
    xmm7.mulsd(xmm4);
    xmm3.subsd(xmm4);
    xmm5.mulpd(xmm0);
    xmm0.mulpd(xmm0);
    xmm3.subsd(xmm2);
    xmm2 = Xmm::load(&CTABLE, c);
    xmm1.subsd(xmm3);
    xmm3 = Xmm::load_low(&CTABLE, c + 6);
    xmm2.addsd(xmm3);
    xmm7.subsd(xmm2);
    xmm1.subsd(xmm6);
    xmm6 = Xmm::load(&SC_2, 0);
    xmm2.mulsd(xmm4);
    xmm6.mulpd(xmm0);
    xmm3.mulsd(xmm4);
    xmm2.mulpd(xmm0);
    xmm0.mulpd(xmm0);
    xmm5.addpd(Xmm::load(&SC_3, 0));
    xmm4.mulsd(Xmm::load_low(&CTABLE, c));
    xmm6.addpd(Xmm::load(&SC_1, 0));
    xmm5.mulpd(xmm0);
    xmm0 = xmm3;
    xmm3.addsd(Xmm::load_low(&CTABLE, c + 2));
    xmm1.mulpd(xmm7);
    xmm7 = xmm4;
    xmm4.addsd(xmm3);
    xmm6.addpd(xmm5);
    xmm5 = Xmm::load_low(&CTABLE, c + 2);
    xmm5.subsd(xmm3);
    xmm3.subsd(xmm4);
    xmm1.addsd(Xmm::load_low(&CTABLE, c + 4));
    xmm6.mulpd(xmm2);
    xmm5.addsd(xmm0);
    xmm3.addsd(xmm7);
    xmm1.addsd(xmm5);
    xmm1.addsd(xmm3);
    xmm1.addsd(xmm6);
    xmm6.unpckhpd(xmm6);
    xmm0 = xmm4;
    xmm1.addsd(xmm6);
    xmm0.addsd(xmm1);
    xmm0.low() // B1_4
}
