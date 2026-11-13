//! HotSpot's x86_64 `Math.pow`: a transcription of OpenJDK's `stubGenerator_x86_64_pow.cpp`
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
//! `x = 2^k * mx` with `mx` in `[1, 2)`. `B ≈ 1/mx` comes from `RCPPS`, scaled by a short
//! approximation `LH` of `log2(e)` and rounded to 9 bits. The reduced argument
//! `r = B*mx - LH` is computed exactly in high and low parts, and
//! `log2(x) = k - log2(B) + p(r)` with `-log2(B)` from a table and `p` of degree 8. For `|x|`
//! in `[1 - 1/32, 1 + 1/16)` a slower, more accurate variant is used. Then
//! `exp2(y * log2(x))` uses an 8-bit table and a degree-5 polynomial. The main path is taken
//! when no overflow or underflow is possible; the many special cases of `pow` branch off.
//!
//! The stub's control flow is kept as is: each label is a [`Label`], each jump sets the next
//! label, and every register keeps its value across jumps (several paths re-enter the main
//! computation with registers prepared elsewhere). Registers the stub never reads before
//! writing start as zero.

use super::pow_tables::{
    COEFF_H, COEFF_POW, DOUBLE0, DOUBLE0DOT5, DOUBLE2, E_COEFF, HALFMASK, HIGHMASK_LOG_X,
    HIGHMASK_Y, HIGHSIGMASK, L_TBL_POW, LOG2_E, LOG2_POW, T_EXP,
};
use super::rcp::Vendor;
use super::sse::Xmm;

/// The stub's labels (`L_2TAG_PACKET_n_0_2` is `Ln`; `B1_5`, the exit, is a `return`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Label {
    Pow,
    L0,
    L1,
    L2,
    /// The fall-through of `L_2TAG_PACKET_2_0_2` when `y` is not small.
    L2Accurate,
    L3,
    L4,
    L5,
    L6,
    L7,
    L8,
    L9,
    L10,
    L11,
    L12,
    L13,
    L15,
    L16,
    L18,
    L19,
    L20,
    L21,
    L22,
    L23,
    L24,
    L25,
    L26,
    L27,
    L28,
    L29,
    L30,
    L31,
    L32,
    L33,
    L34,
    L35,
    L36,
    L37,
    L39,
    L40,
    L41,
    L42,
    L43,
    L44,
    L45,
    L46,
    L47,
    L48,
    L49,
    L51,
    L53,
    L54,
    L55,
}

/// `sarl`: arithmetic right shift of a 32-bit register.
fn sar(value: u32, shift: u32) -> u32 {
    (value.cast_signed() >> (shift & 31)).cast_unsigned()
}

/// `bsrl`: the index of the highest set bit (the stub only applies it to non-zero values).
fn bsr(value: u32) -> u32 {
    31 - value.leading_zeros()
}

/// A double with the given high 16 bits and zero low bits (`xorpd`, then `pinsrw …, 3`).
fn with_word3(word: u32) -> Xmm {
    let mut xmm = Xmm::default();
    xmm.pinsrw(word, 3);
    xmm
}

/// `Math.pow(x, y)` exactly as HotSpot computes it on x86_64, with `RCPPS` from `vendor`.
#[allow(
    clippy::too_many_lines,
    reason = "a transcription of one assembly routine"
)]
pub(crate) fn pow(x: f64, y: f64, vendor: Vendor) -> f64 {
    let x_bits = x.to_bits(); // [rsp + 8]
    let y_bits = y.to_bits(); // [rsp + 16]
    let y_word3 = (y_bits >> 48) as u32; // the 16-bit word at [rsp + 22]
    let qword = |table: &[u32]| Xmm::load_low(table, 0).0[0];

    let mut xmm0 = Xmm::from_f64(x);
    let mut xmm1 = Xmm::from_f64(y);
    if y_bits == qword(&DOUBLE2) {
        xmm0.mulsd(xmm0);
        return xmm0.low();
    }
    // B1_2
    if y_bits == qword(&DOUBLE0DOT5) && x_bits.cast_signed() >= qword(&DOUBLE0).cast_signed() {
        xmm0.sqrtsd();
        return xmm0.low();
    }

    let mut xmm2 = Xmm::default();
    let mut xmm3 = Xmm::default();
    let mut xmm4 = Xmm::default();
    let mut xmm5 = Xmm::default();
    let mut xmm6 = Xmm::default();
    let mut xmm7 = Xmm::default();
    let (mut eax, mut ecx, mut edx, mut r8): (u32, u32, u32, u32) = (0, 0, 0, 0);

    let mut label = Label::Pow;
    loop {
        label = match label {
            Label::Pow => {
                eax = xmm0.pextrw(3);
                xmm2 = Xmm::from_u64(0x3FF0_0000_0000_0000);
                r8 = 1_069_088_768;
                xmm7 = Xmm::from_u32(r8);
                xmm1 = Xmm::from_u64(0x77F0_0000_0000_0000);
                xmm3 = xmm0;
                edx = 0x7FF0 & eax;
                edx = edx.wrapping_sub(16_368);
                ecx = edx;
                edx = sar(edx, 31);
                ecx = ecx.wrapping_add(edx);
                ecx ^= edx;
                xmm0.por(xmm2);
                xmm6 = Xmm::load(&HIGHSIGMASK, 0);
                xmm0.psrlq(27);
                xmm2 = Xmm::load_low(&LOG2_E, 0);
                xmm0.psrld(2);
                ecx = ecx.wrapping_add(16);
                ecx = bsr(ecx);
                xmm0.rcpps(vendor);
                xmm3.psllq(12);
                xmm4 = Xmm::from_u32(8192);
                xmm3.psrlq(12);
                eax = eax.wrapping_sub(16);
                if eax >= 32_736 {
                    Label::L0
                } else {
                    r8 = 0;
                    Label::L1
                }
            }
            Label::L1 => {
                xmm0.mulss(xmm7);
                edx = u32::MAX;
                ecx = ecx.wrapping_sub(4);
                edx <<= ecx & 31;
                xmm5 = Xmm::from_u64(u64::from(edx) << 32);
                xmm3.por(xmm1);
                eax = eax.wrapping_sub(16_351);
                if eax <= 1 {
                    Label::L2
                } else {
                    xmm0.paddd(xmm4);
                    xmm5.pand(xmm3);
                    edx = xmm0.low_u32();
                    xmm0.psllq(29);
                    Label::L3
                }
            }
            Label::L3 => {
                xmm3.subsd(xmm5);
                xmm0.pand(xmm6);
                eax = eax.wrapping_sub(1);
                eax = sar(eax, 4);
                xmm7.cvtsi2sdl(eax.cast_signed());
                xmm5.mulpd(xmm0);
                Label::L4
            }
            Label::L4 => {
                xmm3.mulsd(xmm0);
                xmm1 = Xmm::load(&COEFF_POW, 0);
                xmm5.subsd(xmm2);
                xmm4 = Xmm::load(&COEFF_POW, 4);
                ecx = eax;
                eax = sar(eax, 31);
                ecx = ecx.wrapping_add(eax);
                eax ^= ecx;
                eax = eax.wrapping_add(1);
                eax = bsr(eax);
                xmm5.unpcklpd(xmm3);
                xmm6 = Xmm::load(&COEFF_POW, 8);
                xmm3.addsd(xmm5);
                edx &= 0x00FF_C000;
                edx >>= 10;
                xmm5.addpd(Xmm::load(&L_TBL_POW, ((edx - 3648) / 4) as usize));
                xmm0 = Xmm::load(&COEFF_POW, 12);
                xmm2 = Xmm::pshufd(xmm3, 68);
                xmm3.mulsd(xmm3);
                xmm1.mulpd(xmm2);
                xmm4.mulpd(xmm2);
                xmm5.addsd(xmm7);
                xmm2.mulsd(xmm3);
                xmm6.addpd(xmm1);
                xmm3.mulsd(xmm3);
                xmm0.addpd(xmm4);
                xmm1 = Xmm::from_f64(y);
                ecx = (ecx & 0xFFFF_0000) | y_word3;
                xmm7 = Xmm::pshufd(xmm5, 238);
                xmm4 = Xmm::load_low(&HIGHMASK_Y, 0);
                xmm6.mulpd(xmm2);
                xmm3 = Xmm::pshufd(xmm3, 68);
                xmm0.mulpd(xmm2);
                eax <<= 4;
                eax = eax.wrapping_sub(15_872);
                ecx &= 0x7FF0;
                eax = eax.wrapping_add(ecx);
                xmm3.mulpd(xmm6);
                if eax >= 624 {
                    Label::L5
                } else {
                    xmm6 = with_word3(17_080);
                    xmm2 = xmm1;
                    xmm4.pand(xmm1);
                    xmm1.subsd(xmm4);
                    xmm4.mulsd(xmm5);
                    xmm0.addsd(xmm7);
                    xmm1.mulsd(xmm5);
                    xmm7 = xmm6;
                    xmm6.addsd(xmm4);
                    xmm3.addpd(xmm0);
                    edx = xmm6.low_u32();
                    xmm6.subsd(xmm7);
                    xmm0 = Xmm::pshufd(xmm3, 238);
                    xmm4.subsd(xmm6);
                    xmm0.addsd(xmm3);
                    ecx = edx;
                    edx &= 0xFF;
                    edx = edx.wrapping_add(edx);
                    xmm5 = Xmm::load(&T_EXP, (edx * 2) as usize);
                    xmm4.addsd(xmm1);
                    xmm2.mulsd(xmm0);
                    xmm7 = Xmm::load(&E_COEFF, 0);
                    xmm3 = Xmm::load(&E_COEFF, 4);
                    ecx <<= 12;
                    ecx ^= r8;
                    ecx &= 0xFFF0_0000;
                    xmm6 = Xmm::from_u32(ecx);
                    xmm2.addsd(xmm4);
                    xmm1 = Xmm::from_u64(0x3FE6_2E42_FEFA_39EF);
                    xmm0 = Xmm::pshufd(xmm2, 68);
                    xmm4 = Xmm::pshufd(xmm2, 68);
                    xmm1.mulsd(xmm2);
                    xmm6 = Xmm::pshufd(xmm6, 17);
                    xmm0.mulpd(xmm0);
                    xmm7.mulpd(xmm4);
                    xmm5.paddd(xmm6);
                    xmm1.mulsd(xmm5);
                    xmm6 = Xmm::pshufd(xmm5, 238);
                    xmm0.mulsd(xmm0);
                    xmm3.addpd(xmm7);
                    xmm1.addsd(xmm6);
                    xmm0.mulpd(xmm3);
                    xmm3 = Xmm::pshufd(xmm0, 238);
                    xmm0.mulsd(xmm5);
                    xmm3.mulsd(xmm5);
                    xmm0.addsd(xmm1);
                    xmm0.addsd(xmm3);
                    xmm0.addsd(xmm5);
                    return xmm0.low(); // B1_5
                }
            }
            Label::L0 => {
                eax = eax.wrapping_add(16);
                edx = 0x7FF0 & eax;
                if edx == 0x7FF0 {
                    Label::L6
                } else if eax & 0x8000 != 0 {
                    Label::L7
                } else {
                    Label::L8
                }
            }
            Label::L8 | Label::L10 => {
                xmm0 = Xmm::from_f64(x);
                xmm3 = Xmm::from_f64(x);
                edx = xmm3.low_u32();
                xmm3.psrlq(32);
                ecx = xmm3.low_u32();
                edx |= ecx;
                if edx == 0 {
                    Label::L9
                } else {
                    // A subnormal x: scale it by 2^52 and redo the start of the main path.
                    xmm3 = with_word3(18_416);
                    xmm0.mulsd(xmm3);
                    xmm2 = with_word3(16_368);
                    xmm3 = xmm0;
                    eax = xmm0.pextrw(3);
                    xmm0.por(xmm2);
                    ecx = 18_416;
                    xmm0.psrlq(27);
                    xmm2 = Xmm::load_low(&LOG2_E, 0);
                    xmm0.psrld(2);
                    xmm0.rcpps(vendor);
                    xmm3.psllq(12);
                    xmm6 = Xmm::load(&HIGHSIGMASK, 0);
                    xmm3.psrlq(12);
                    xmm0.mulss(xmm7);
                    edx = 0xFFFF_FC00;
                    xmm5 = Xmm::from_u32(edx);
                    xmm3.por(xmm1);
                    xmm0.paddd(xmm4);
                    xmm5.psllq(32);
                    edx = xmm0.low_u32();
                    xmm0.psllq(29);
                    xmm5.pand(xmm3);
                    r8 = if label == Label::L10 { 0x8000_0000 } else { 0 };
                    xmm0.pand(xmm6);
                    xmm3.subsd(xmm5);
                    eax &= 0x7FF0;
                    eax = eax.wrapping_sub(18_416);
                    eax = sar(eax, 4);
                    xmm7.cvtsi2sdl(eax.cast_signed());
                    xmm5.mulpd(xmm0);
                    Label::L4
                }
            }
            Label::L5 => {
                if eax.cast_signed() < 0 {
                    Label::L11
                } else if eax >= 752 {
                    Label::L12
                } else {
                    xmm0.addsd(xmm7);
                    xmm2 = Xmm::load_low(&HALFMASK, 0);
                    xmm3.addpd(xmm0);
                    xmm6 = with_word3(17_080);
                    xmm0 = Xmm::pshufd(xmm3, 238);
                    xmm0.addsd(xmm3);
                    xmm3 = xmm5;
                    xmm5.addsd(xmm0);
                    xmm4 = xmm2;
                    xmm3.subsd(xmm5);
                    xmm7 = xmm5;
                    xmm5.pand(xmm2);
                    xmm2 = xmm1;
                    xmm4.pand(xmm1);
                    xmm7.subsd(xmm5);
                    xmm0.addsd(xmm3);
                    xmm1.subsd(xmm4);
                    xmm4.mulsd(xmm5);
                    xmm0.addsd(xmm7);
                    xmm2.mulsd(xmm0);
                    xmm7 = xmm6;
                    xmm1.mulsd(xmm5);
                    xmm6.addsd(xmm4);
                    eax = xmm6.low_u32();
                    xmm6.subsd(xmm7);
                    xmm2.addsd(xmm1);
                    xmm7 = Xmm::load(&E_COEFF, 0);
                    xmm3 = Xmm::load(&E_COEFF, 4);
                    xmm4.subsd(xmm6);
                    edx = xmm6.pextrw(3);
                    ecx = eax;
                    eax &= 0xFF;
                    eax = eax.wrapping_add(eax);
                    xmm5 = Xmm::load(&T_EXP, (eax * 2) as usize);
                    xmm2.addsd(xmm4);
                    ecx = sar(ecx, 8);
                    eax = ecx;
                    ecx = sar(ecx, 1);
                    eax = eax.wrapping_sub(ecx);
                    ecx <<= 20;
                    ecx ^= r8;
                    xmm6 = Xmm::from_u32(ecx);
                    xmm1 = Xmm::load_low(&E_COEFF, 8);
                    edx &= 0x7FFF;
                    if edx > 16_529 {
                        Label::L12
                    } else {
                        xmm0 = Xmm::pshufd(xmm2, 68);
                        xmm4 = Xmm::pshufd(xmm2, 68);
                        xmm0.mulpd(xmm0);
                        xmm7.mulpd(xmm4);
                        xmm6 = Xmm::pshufd(xmm6, 17);
                        xmm1.mulsd(xmm2);
                        xmm0.mulsd(xmm0);
                        xmm5.paddd(xmm6);
                        xmm3.addpd(xmm7);
                        xmm1.mulsd(xmm5);
                        xmm6 = Xmm::pshufd(xmm5, 238);
                        xmm0.mulpd(xmm3);
                        xmm1.addsd(xmm6);
                        xmm3 = Xmm::pshufd(xmm0, 238);
                        xmm0.mulsd(xmm5);
                        xmm3.mulsd(xmm5);
                        eax <<= 4;
                        eax = eax.wrapping_add(16_368);
                        xmm4 = with_word3(eax);
                        xmm0.addsd(xmm1);
                        xmm0.addsd(xmm3);
                        xmm1 = xmm0;
                        xmm0.addsd(xmm5);
                        xmm0.mulsd(xmm4);
                        eax = xmm0.pextrw(3) & 0x7FF0;
                        if eax == 0 {
                            Label::L13
                        } else {
                            // L_2TAG_PACKET_14_0_2 (overflow) and B1_5 both return xmm0.
                            return xmm0.low();
                        }
                    }
                }
            }
            Label::L6 => {
                xmm1 = Xmm::from_f64(y);
                xmm0 = Xmm::from_f64(x);
                xmm2 = xmm0;
                eax = xmm2.low_u32();
                xmm2.psrlq(20);
                edx = xmm2.low_u32();
                eax |= edx;
                if eax == 0 {
                    Label::L15
                } else {
                    eax = xmm1.low_u32();
                    xmm1.psrlq(32);
                    edx = xmm1.low_u32();
                    ecx = edx;
                    edx = edx.wrapping_add(edx);
                    eax |= edx;
                    if eax == 0 {
                        Label::L16
                    } else {
                        xmm0.addsd(xmm0);
                        return xmm0.low();
                    }
                }
            }
            Label::L18 => {
                xmm0 = Xmm::from_f64(y);
                xmm0.addpd(xmm0);
                return xmm0.low();
            }
            Label::L15 => {
                eax = xmm1.low_u32();
                xmm2 = xmm1;
                xmm1.psrlq(32);
                edx = xmm1.low_u32();
                ecx = edx;
                edx = edx.wrapping_add(edx);
                eax |= edx;
                if eax == 0 {
                    Label::L19
                } else if xmm2.pextrw(3) & 0x7FF0 != 0x7FF0 {
                    Label::L20
                } else {
                    eax = xmm2.low_u32();
                    xmm2.psrlq(20);
                    edx = xmm2.low_u32();
                    eax |= edx;
                    if eax == 0 { Label::L20 } else { Label::L18 }
                }
            }
            Label::L20 => {
                eax = xmm0.pextrw(3);
                if eax & 0x8000 != 0 {
                    Label::L21
                } else if ecx & 0x8000_0000 != 0 {
                    Label::L22
                } else {
                    return xmm0.low();
                }
            }
            Label::L23 => {
                xmm1 = Xmm::from_f64(y);
                eax = xmm1.low_u32();
                if eax & 1 != 0 {
                    Label::L24
                } else if eax & 2 != 0 {
                    Label::L25
                } else {
                    Label::L24
                }
            }
            Label::L21 => {
                ecx >>= 20;
                ecx &= 0x7FF;
                if ecx > 1075 {
                    Label::L24
                } else if ecx == 1075 {
                    Label::L26
                } else if ecx > 1074 {
                    Label::L23
                } else if ecx < 1023 {
                    Label::L24
                } else {
                    xmm1 = Xmm::from_f64(y);
                    xmm3 = with_word3(17_208);
                    xmm4 = xmm3;
                    xmm3.addsd(xmm1);
                    xmm4.subsd(xmm3);
                    xmm1.addsd(xmm4);
                    if xmm1.pextrw(3) & 0x7FF0 != 0 || xmm3.low_u32() & 1 == 0 {
                        Label::L24
                    } else {
                        Label::L25
                    }
                }
            }
            Label::L25 => {
                xmm1 = Xmm::from_f64(y);
                if xmm1.pextrw(3) & 0x8000 != 0 {
                    Label::L27
                } else {
                    return xmm0.low();
                }
            }
            Label::L27 => return with_word3(0x8000).low(),
            Label::L24 => {
                xmm1 = Xmm::from_f64(y);
                if xmm1.pextrw(3) & 0x8000 != 0 {
                    Label::L22
                } else {
                    return with_word3(0x7FF0).low();
                }
            }
            Label::L26 => {
                xmm1 = Xmm::from_f64(y);
                if xmm1.low_u32() & 1 == 0 {
                    Label::L24
                } else {
                    Label::L25
                }
            }
            Label::L28 => {
                eax = xmm1.low_u32();
                xmm1.psrlq(20);
                edx = xmm1.low_u32();
                eax |= edx;
                if eax == 0 {
                    Label::L29
                } else {
                    xmm0 = Xmm::from_f64(y);
                    xmm0.addsd(xmm0);
                    return xmm0.low();
                }
            }
            Label::L29 => {
                xmm0 = Xmm::from_f64(x);
                eax = xmm0.pextrw(3);
                if eax == 49_136 {
                    ecx = xmm0.low_u32();
                    xmm0.psrlq(20);
                    edx = xmm0.low_u32();
                    ecx |= edx;
                    if ecx == 0 {
                        return with_word3(32_760).low();
                    }
                }
                Label::L30
            }
            Label::L30 => {
                xmm1 = Xmm::from_f64(y);
                eax &= 0x7FF0;
                eax = eax.wrapping_sub(16_368);
                edx = xmm1.pextrw(3);
                xmm0 = Xmm::default();
                eax ^= edx;
                eax &= 0x8000;
                if eax == 0 {
                    Label::L31
                } else {
                    return xmm0.low();
                }
            }
            Label::L32 => {
                eax = xmm1.low_u32();
                if edx > 17_184 {
                    Label::L33
                } else if eax & 1 != 0 {
                    Label::L34
                } else if eax & 2 == 0 {
                    Label::L35
                } else {
                    Label::L36
                }
            }
            Label::L33 => {
                if eax & 1 == 0 {
                    Label::L35
                } else {
                    Label::L36
                }
            }
            Label::L7 => {
                xmm2 = Xmm::from_f64(x);
                eax = xmm2.low_u32();
                xmm2.psrlq(31);
                ecx = xmm2.low_u32();
                eax |= ecx;
                if eax == 0 {
                    Label::L9
                } else {
                    xmm1 = Xmm::from_f64(y);
                    edx = xmm1.pextrw(3);
                    eax = xmm1.low_u32();
                    xmm2 = xmm1;
                    xmm2.psrlq(32);
                    ecx = xmm2.low_u32();
                    ecx = ecx.wrapping_add(ecx);
                    ecx |= eax;
                    if ecx == 0 {
                        Label::L37
                    } else {
                        edx &= 0x7FF0;
                        if edx == 0x7FF0 {
                            Label::L28
                        } else if edx > 17_200 {
                            Label::L35
                        } else if edx >= 17_184 {
                            Label::L32
                        } else if edx < 16_368 {
                            Label::L34
                        } else {
                            xmm2 = with_word3(17_208);
                            xmm4 = xmm2;
                            xmm2.addsd(xmm1);
                            xmm4.subsd(xmm2);
                            xmm1.addsd(xmm4);
                            if xmm1.pextrw(3) & 0x7FFF != 0 {
                                Label::L34
                            } else if xmm2.low_u32() & 1 == 0 {
                                Label::L35
                            } else {
                                Label::L36
                            }
                        }
                    }
                }
            }
            Label::L36 | Label::L35 => {
                // A negative x with an integer y: odd (L36, negate the result) or even (L35).
                // The RCPPS result from the start of the main path is still in xmm0.
                let odd = label == Label::L36;
                xmm1 = with_word3(30_704);
                xmm2 = Xmm::load_low(&LOG2_E, 0);
                xmm4 = Xmm::from_f64(x);
                eax = xmm4.pextrw(3);
                edx = 8192;
                xmm4 = Xmm::from_u32(edx);
                eax &= 0x7FFF;
                eax = eax.wrapping_sub(16);
                if eax.cast_signed() < 0 {
                    if odd { Label::L10 } else { Label::L8 }
                } else {
                    edx = eax;
                    edx &= 0x7FF0;
                    edx = edx.wrapping_sub(16_368);
                    ecx = edx;
                    edx = sar(edx, 31);
                    ecx = ecx.wrapping_add(edx);
                    ecx ^= edx;
                    ecx = ecx.wrapping_add(16);
                    ecx = bsr(ecx);
                    r8 = if odd { 0x8000_0000 } else { 0 };
                    Label::L1
                }
            }
            Label::L34 => {
                // A negative finite x with a non-integer y: 0 * Inf, the x86 default NaN.
                xmm1 = with_word3(0x7FF0);
                xmm0 = Xmm::default();
                xmm0.mulsd(xmm1);
                return xmm0.low();
            }
            Label::L16 | Label::L19 | Label::L37 | Label::L39 => return with_word3(16_368).low(),
            Label::L22 => return 0.0,
            Label::L11 => {
                eax = eax.wrapping_add(384);
                if eax.cast_signed() < 0 {
                    Label::L37 // L_2TAG_PACKET_38_0_2
                } else {
                    xmm5.mulsd(xmm1);
                    xmm0.addsd(xmm7);
                    r8 >>= 31;
                    xmm3.addpd(xmm0);
                    xmm0 = Xmm::pshufd(xmm3, 238);
                    xmm3.addsd(xmm0);
                    xmm4 = Xmm::load_low(&LOG2_POW, (r8 * 2) as usize);
                    xmm1.mulsd(xmm3);
                    eax = 16_368;
                    r8 <<= 15;
                    eax |= r8;
                    xmm0 = with_word3(eax);
                    xmm5.addsd(xmm1);
                    xmm5.mulsd(xmm4);
                    xmm0.addsd(xmm5);
                    return xmm0.low();
                }
            }
            Label::L9 => {
                xmm1 = Xmm::from_f64(y);
                xmm2 = xmm1;
                if xmm1.pextrw(3) & 0x7FF0 == 0x7FF0 {
                    eax = xmm2.low_u32();
                    xmm2.psrlq(20);
                    edx = xmm2.low_u32();
                    eax |= edx;
                    if eax == 0 { Label::L40 } else { Label::L18 }
                } else {
                    Label::L40
                }
            }
            Label::L40 => {
                eax = xmm1.low_u32();
                xmm1.psrlq(32);
                edx = xmm1.low_u32();
                ecx = edx;
                edx = edx.wrapping_add(edx);
                eax |= edx;
                if eax == 0 {
                    Label::L39
                } else {
                    edx >>= 21;
                    if edx > 1075 {
                        Label::L41
                    } else if edx == 1075 {
                        Label::L42
                    } else if edx < 1023 {
                        Label::L41
                    } else {
                        xmm1 = Xmm::from_f64(y);
                        xmm3 = with_word3(17_208);
                        xmm4 = xmm3;
                        xmm3.addsd(xmm1);
                        xmm4.subsd(xmm3);
                        xmm1.addsd(xmm4);
                        if xmm1.pextrw(3) & 0x7FF0 != 0 || xmm3.low_u32() & 1 == 0 {
                            Label::L41
                        } else {
                            Label::L43
                        }
                    }
                }
            }
            Label::L43 => {
                xmm0 = Xmm::from_f64(x);
                if ecx & 0x8000_0000 != 0 {
                    Label::L44
                } else {
                    return xmm0.low();
                }
            }
            Label::L42 => {
                xmm1 = Xmm::from_f64(y);
                if xmm1.low_u32() & 1 != 0 {
                    Label::L43
                } else {
                    Label::L41
                }
            }
            Label::L41 => {
                if ecx & 0x8000_0000 == 0 {
                    Label::L22
                } else {
                    xmm0 = Xmm::default();
                    Label::L44
                }
            }
            Label::L44 => {
                xmm1 = with_word3(16_368);
                xmm1.divsd(xmm0);
                return xmm1.low();
            }
            Label::L12 => {
                xmm2 = Xmm::from_f64(x);
                xmm6 = Xmm::from_f64(y);
                eax = xmm2.pextrw(3);
                edx = xmm6.pextrw(3);
                ecx = 0x7FF0 & edx;
                if ecx == 0x7FF0 {
                    Label::L45
                } else {
                    eax &= 0x7FF0;
                    eax = eax.wrapping_sub(16_368);
                    edx ^= eax;
                    if edx & 0x8000 != 0 {
                        Label::L46
                    } else {
                        Label::L47
                    }
                }
            }
            Label::L47 => {
                // Overflow: a huge value squared, with the sign from r8.
                eax = 32_736;
                xmm0.pinsrw(eax, 3);
                r8 >>= 16;
                eax |= r8;
                xmm1.pinsrw(eax, 3);
                xmm0.mulsd(xmm1);
                return xmm0.low(); // L_2TAG_PACKET_14_0_2
            }
            Label::L46 => {
                // Underflow: a tiny value squared, with the sign from r8.
                xmm0.pinsrw(16, 3);
                xmm0.mulsd(xmm0);
                if r8 & 0x8000_0000 != 0 {
                    xmm2 = Xmm::from_u64(0x8000_0000_0000_0000);
                    xmm0.pxor(xmm2);
                }
                Label::L48
            }
            Label::L48 => return xmm0.low(),
            Label::L13 => {
                // The result is subnormal: recompute it with a single rounding.
                ecx = xmm5.pextrw(3);
                edx = xmm4.pextrw(3);
                eax = u32::MAX;
                ecx &= 0x7FF0;
                ecx = ecx.wrapping_sub(16_368);
                edx &= 0x7FF0;
                edx = edx.wrapping_add(ecx);
                ecx = (-31_i32).cast_unsigned();
                edx = sar(edx, 4);
                let less_equal = ecx.cast_signed() <= edx.cast_signed();
                ecx = ecx.wrapping_sub(edx);
                if less_equal {
                    Label::L49
                } else if ecx > 20 {
                    Label::L48 // L_2TAG_PACKET_50_0_2
                } else {
                    eax <<= ecx & 31;
                    Label::L49
                }
            }
            Label::L49 => {
                xmm0 = Xmm::from_u32(eax);
                xmm0.psllq(32);
                xmm0.pand(xmm5);
                xmm5.subsd(xmm0);
                xmm5.addsd(xmm1);
                xmm0.mulsd(xmm4);
                xmm5.mulsd(xmm4);
                xmm0.addsd(xmm5);
                Label::L48 // L_2TAG_PACKET_50_0_2
            }
            Label::L2 => {
                // |x| close to 1: the more accurate logarithm.
                ecx = (ecx & 0xFFFF_0000) | y_word3;
                edx = 0x8000_0000;
                xmm1 = Xmm::from_u32(edx);
                xmm7 = Xmm::default();
                xmm0.paddd(xmm4);
                edx = xmm0.low_u32();
                xmm0.psllq(29);
                xmm1.paddq(xmm3);
                xmm5.pand(xmm1);
                ecx &= 0x7FF0;
                if ecx.cast_signed() < 16_560 {
                    Label::L3
                } else {
                    Label::L2Accurate
                }
            }
            Label::L2Accurate => {
                xmm0.pand(xmm6);
                xmm3.subsd(xmm5);
                eax = eax.wrapping_add(16_351);
                eax >>= 4;
                eax = eax.wrapping_sub(1022);
                xmm7.cvtsi2sdl(eax.cast_signed());
                xmm5.mulpd(xmm0);
                xmm4 = Xmm::load_low(&COEFF_H, 0);
                xmm3.mulsd(xmm0);
                xmm6 = Xmm::load_low(&COEFF_H, 0);
                xmm5.subsd(xmm2);
                xmm1 = Xmm::load_low(&COEFF_H, 2);
                xmm2 = Xmm::pshufd(xmm3, 68);
                xmm5.unpcklpd(xmm3);
                xmm3.addsd(xmm5);
                xmm0 = Xmm::load_low(&COEFF_H, 2);
                edx &= 0x00FF_C000;
                edx >>= 10;
                xmm7.addpd(Xmm::load(&L_TBL_POW, ((edx - 3648) / 4) as usize));
                xmm4.mulsd(xmm5);
                xmm0.mulsd(xmm5);
                xmm6.mulsd(xmm2);
                xmm1.mulsd(xmm2);
                xmm2 = xmm5;
                xmm4.mulsd(xmm5);
                xmm5.addsd(xmm0);
                xmm0 = xmm7;
                xmm2.addsd(xmm3);
                xmm7.addsd(xmm5);
                xmm6.mulsd(xmm2);
                xmm0.subsd(xmm7);
                xmm2 = xmm7;
                xmm7.addsd(xmm4);
                xmm0.addsd(xmm5);
                xmm2.subsd(xmm7);
                xmm4.addsd(xmm2);
                xmm2 = Xmm::pshufd(xmm5, 238);
                xmm5 = xmm7;
                xmm7.addsd(xmm2);
                xmm4.addsd(xmm0);
                xmm0 = Xmm::load(&COEFF_POW, 0);
                xmm5.subsd(xmm7);
                xmm6.addsd(xmm4);
                xmm4 = xmm7;
                xmm5.addsd(xmm2);
                xmm7.addsd(xmm1);
                xmm2 = Xmm::load(&COEFF_POW, 16);
                xmm4.subsd(xmm7);
                xmm6.addsd(xmm5);
                xmm4.addsd(xmm1);
                xmm5 = Xmm::pshufd(xmm7, 238);
                xmm1 = xmm7;
                xmm7.addsd(xmm5);
                xmm1.subsd(xmm7);
                xmm1.addsd(xmm5);
                xmm5 = Xmm::load(&COEFF_POW, 20);
                xmm3 = Xmm::pshufd(xmm3, 68);
                xmm6.addsd(xmm4);
                xmm6.addsd(xmm1);
                xmm1 = Xmm::load(&COEFF_POW, 8);
                xmm0.mulpd(xmm3);
                xmm2.mulpd(xmm3);
                xmm4 = Xmm::pshufd(xmm3, 68);
                xmm3.mulpd(xmm3);
                xmm0.addpd(xmm1);
                xmm5.addpd(xmm2);
                xmm4.mulsd(xmm3);
                xmm2 = Xmm::load_low(&HIGHMASK_LOG_X, 0);
                xmm3.mulpd(xmm3);
                xmm1 = Xmm::from_f64(y);
                ecx = (ecx & 0xFFFF_0000) | y_word3;
                xmm0.mulpd(xmm4);
                eax = xmm7.pextrw(3);
                xmm5.mulpd(xmm4);
                xmm0.mulpd(xmm3);
                xmm4 = Xmm::load_low(&HIGHMASK_Y, 2);
                xmm2.pand(xmm7);
                xmm5.addsd(xmm6);
                xmm7.subsd(xmm2);
                xmm5.addpd(xmm0);
                eax &= 0x7FF0;
                eax = eax.wrapping_sub(16_368);
                ecx &= 0x7FF0;
                if ecx == 0x7FF0 {
                    Label::L45
                } else {
                    ecx = ecx.wrapping_add(eax);
                    if ecx >= 16_576 {
                        Label::L51
                    } else {
                        xmm0 = Xmm::pshufd(xmm5, 238);
                        xmm4.pand(xmm1);
                        xmm3 = xmm1;
                        xmm5.addsd(xmm0);
                        xmm1.subsd(xmm4);
                        xmm6 = with_word3(17_080);
                        xmm7.addsd(xmm5);
                        xmm4.mulsd(xmm2);
                        xmm1.mulsd(xmm2);
                        xmm5 = xmm6;
                        xmm3.mulsd(xmm7);
                        xmm6.addsd(xmm4);
                        xmm1.addsd(xmm3);
                        xmm7 = Xmm::load(&E_COEFF, 0);
                        edx = xmm6.low_u32();
                        xmm6.subsd(xmm5);
                        xmm3 = Xmm::load(&E_COEFF, 4);
                        xmm2 = Xmm::load_low(&E_COEFF, 8);
                        xmm4.subsd(xmm6);
                        ecx = edx;
                        edx &= 0xFF;
                        edx = edx.wrapping_add(edx);
                        xmm5 = Xmm::load(&T_EXP, (edx * 2) as usize);
                        xmm4.addsd(xmm1);
                        edx = xmm6.pextrw(3);
                        ecx >>= 8;
                        eax = ecx;
                        ecx >>= 1;
                        eax = eax.wrapping_sub(ecx);
                        ecx <<= 20;
                        xmm6 = Xmm::from_u32(ecx);
                        xmm0 = Xmm::pshufd(xmm4, 68);
                        xmm1 = Xmm::pshufd(xmm4, 68);
                        xmm0.mulpd(xmm0);
                        xmm7.mulpd(xmm1);
                        xmm6 = Xmm::pshufd(xmm6, 17);
                        xmm2.mulsd(xmm4);
                        edx &= 0x7FFF;
                        if edx > 16_529 {
                            Label::L12
                        } else {
                            xmm0.mulsd(xmm0);
                            xmm5.paddd(xmm6);
                            xmm3.addpd(xmm7);
                            xmm2.mulsd(xmm5);
                            xmm6 = Xmm::pshufd(xmm5, 238);
                            xmm0.mulpd(xmm3);
                            xmm2.addsd(xmm6);
                            xmm3 = Xmm::pshufd(xmm0, 238);
                            eax = eax.wrapping_add(1023);
                            eax <<= 20;
                            eax |= r8;
                            xmm4 = Xmm::from_u32(eax);
                            xmm0.mulsd(xmm5);
                            xmm3.mulsd(xmm5);
                            xmm0.addsd(xmm2);
                            xmm4.psllq(32);
                            xmm0.addsd(xmm3);
                            xmm1 = xmm0;
                            xmm0.addsd(xmm5);
                            xmm0.mulsd(xmm4);
                            eax = xmm0.pextrw(3) & 0x7FF0;
                            if eax == 0 {
                                Label::L13
                            } else {
                                // L_2TAG_PACKET_14_0_2 (overflow) and B1_5 both return xmm0.
                                return xmm0.low();
                            }
                        }
                    }
                }
            }
            Label::L45 => {
                xmm0 = Xmm::from_f64(x);
                xmm2 = with_word3(49_136);
                xmm2.addsd(xmm0);
                eax = xmm2.pextrw(3);
                if eax == 0 {
                    return with_word3(32_760).low();
                }
                Label::L53
            }
            Label::L53 => {
                xmm1 = Xmm::from_f64(y);
                edx = xmm1.low_u32();
                xmm3 = xmm1;
                xmm3.psrlq(20);
                ecx = xmm3.low_u32();
                ecx |= edx;
                if ecx == 0 {
                    Label::L54
                } else {
                    xmm1.addsd(xmm1);
                    return xmm1.low();
                }
            }
            Label::L51 => {
                eax = xmm1.pextrw(3);
                ecx = xmm2.pextrw(3);
                eax ^= ecx;
                if eax & 0x8000 == 0 {
                    Label::L47
                } else {
                    Label::L46
                }
            }
            Label::L54 => {
                eax = xmm0.pextrw(3) & 0x7FF0;
                edx = xmm1.pextrw(3);
                xmm0 = Xmm::default();
                eax = eax.wrapping_sub(16_368);
                eax ^= edx;
                if eax & 0x8000 == 0 {
                    Label::L55
                } else {
                    return xmm0.low();
                }
            }
            Label::L31 | Label::L55 => {
                xmm0.pinsrw(0x7FF0, 3);
                return xmm0.low();
            }
        };
    }
}
