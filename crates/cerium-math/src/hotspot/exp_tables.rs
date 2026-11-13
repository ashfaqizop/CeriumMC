//! Constant tables of OpenJDK's `stubGenerator_x86_64_exp.cpp` (tag `jdk-25.0.1-ga`),
//! extracted verbatim by `parity/tables/extract_stub_tables.py`. Do not edit by hand.
//!
//! Original notice of the source file:
//!
//!
//! Copyright (c) 2016, 2024, Intel Corporation. All rights reserved.
//! Copyright (C) 2021 THL A29 Limited, a Tencent company. All rights reserved.
//! Intel Math Library (LIBM) Source Code
//!
//! DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
//!
//! This code is free software; you can redistribute it and/or modify it
//! under the terms of the GNU General Public License version 2 only, as
//! published by the Free Software Foundation.
//!
//! This code is distributed in the hope that it will be useful, but WITHOUT
//! ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
//! FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
//! version 2 for more details (a copy is included in the LICENSE file that
//! accompanied this code).
//!
//! You should have received a copy of the GNU General Public License version
//! 2 along with this work; if not, write to the Free Software Foundation,
//! Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
//!
//! Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
//! or visit www.oracle.com if you need additional information or have any
//! questions.
//!
//!
#[rustfmt::skip]
pub(super) const CV: [u32; 24] = [
    0x652B_82FE, 0x4057_1547, 0x652B_82FE, 0x4057_1547,
    0xFEFA_0000, 0x3F86_2E42, 0xFEFA_0000, 0x3F86_2E42,
    0xBC9E_3B3A, 0x3D1C_F79A, 0xBC9E_3B3A, 0x3D1C_F79A,
    0xFFFF_FFFE, 0x3FDF_FFFF, 0xFFFF_FFFE, 0x3FDF_FFFF,
    0xE328_9860, 0x3F56_C15C, 0x555B_9E25, 0x3FA5_5555,
    0xC090_CF0F, 0x3F81_1115, 0x5554_8BA1, 0x3FC5_5555,
];

#[rustfmt::skip]
pub(super) const MMASK: [u32; 4] = [
    0xFFFF_FFC0, 0x0000_0000, 0xFFFF_FFC0, 0x0000_0000,
];

#[rustfmt::skip]
pub(super) const BIAS: [u32; 4] = [
    0x0000_FFC0, 0x0000_0000, 0x0000_FFC0, 0x0000_0000,
];

#[rustfmt::skip]
pub(super) const TBL_ADDR: [u32; 256] = [
    0x0000_0000, 0x0000_0000, 0x0000_0000, 0x0000_0000,
    0x0E03_754D, 0x3CAD_7BBF, 0x3E77_8060, 0x0000_2C9A,
    0x3567_F613, 0x3C8C_D252, 0xD315_8574, 0x0000_59B0,
    0x61E6_C861, 0x3C60_F74E, 0x1875_9BC8, 0x0000_8745,
    0x5D83_7B6C, 0x3C97_9AA6, 0x6CF9_890F, 0x0000_B558,
    0x702F_9CD1, 0x3C3E_BE3D, 0x32D3_D1A2, 0x0000_E3EC,
    0x1E63_BCD8, 0x3CA3_516E, 0xD012_5B50, 0x0001_1301,
    0x26F0_387B, 0x3CA4_C554, 0xAEA9_2DDF, 0x0001_429A,
    0x6252_3FB6, 0x3CA9_5153, 0x3C7D_517A, 0x0001_72B8,
    0x3F13_53BF, 0x3C8B_898C, 0xEB6F_CB75, 0x0001_A35B,
    0x3E3A_2F5F, 0x3C9A_ECF7, 0x3168_B9AA, 0x0001_D487,
    0x44A6_C38D, 0x3C8A_6F41, 0x8862_8CD6, 0x0002_063B,
    0xE3A8_A894, 0x3C96_8EFD, 0x6E75_6238, 0x0002_387A,
    0x981F_E7F2, 0x3C80_472B, 0x65E2_7CDD, 0x0002_6B45,
    0x6D09_AB31, 0x3C82_F7E1, 0xF51F_DEE1, 0x0002_9E9D,
    0x720C_0AB3, 0x3C8B_3782, 0xA6E4_030B, 0x0002_D285,
    0x4DB0_ABB6, 0x3C83_4D75, 0x0A31_B715, 0x0003_06FE,
    0x5DD3_F84A, 0x3C8F_DD39, 0xB264_16FF, 0x0003_3C08,
    0xCC18_7D29, 0x3CA1_2F8C, 0x373A_A9CA, 0x0003_71A7,
    0x738B_5E8B, 0x3CA7_D229, 0x34E5_9FF6, 0x0003_A7DB,
    0xA72A_4C6D, 0x3C85_9F48, 0x4C12_3422, 0x0003_DEA6,
    0x259D_9205, 0x3CA8_B846, 0x21F7_2E29, 0x0004_160A,
    0x60C2_AC12, 0x3C43_63ED, 0x6061_892D, 0x0004_4E08,
    0xDAA1_0379, 0x3C6E_CCE1, 0xB5C1_3CD0, 0x0004_86A2,
    0xBB7A_AFB0, 0x3C76_90CE, 0xD536_2A27, 0x0004_BFDA,
    0x9B28_2A09, 0x3CA0_83CC, 0x769D_2CA6, 0x0004_F9B2,
    0xC1AA_E707, 0x3CA5_09B0, 0x569D_4F81, 0x0005_342B,
    0x18FD_D78E, 0x3C93_3505, 0x36B5_27DA, 0x0005_6F47,
    0xE21C_5409, 0x3C90_63E1, 0xDD48_5429, 0x0005_AB07,
    0x2B64_C035, 0x3C94_32E6, 0x15AD_2148, 0x0005_E76F,
    0x99F0_8C0A, 0x3CA0_1284, 0xB03A_5584, 0x0006_247E,
    0x0073_DC06, 0x3C99_F087, 0x8255_2224, 0x0006_6238,
    0x0DA0_5571, 0x3C99_8D4D, 0x667F_3BCC, 0x0006_A09E,
    0x86CE_4786, 0x3CA5_2BB9, 0x3C65_1A2E, 0x0006_DFB2,
    0x206F_0DAB, 0x3CA3_2092, 0xE8EC_5F73, 0x0007_1F75,
    0x8E17_A7A6, 0x3CA0_6122, 0x5642_67C8, 0x0007_5FEB,
    0x461E_9F86, 0x3CA2_44AC, 0x73EB_0186, 0x0007_A114,
    0xABD6_6C55, 0x3C65_EBE1, 0x36CF_4E62, 0x0007_E2F3,
    0xBBFF_67D0, 0x3C96_FE9F, 0x994C_CE12, 0x0008_2589,
    0x14C8_01DF, 0x3C95_1F14, 0x9B44_92EC, 0x0008_68D9,
    0xC1F0_EAB4, 0x3C8D_B72F, 0x422A_A0DB, 0x0008_ACE5,
    0x59F3_5F44, 0x3C7B_F683, 0x9915_7736, 0x0008_F1AE,
    0x9C06_283C, 0x3CA3_60BA, 0xB0CD_C5E4, 0x0009_3737,
    0x20F9_62AA, 0x3C95_E8D1, 0x9FDE_4E4F, 0x0009_7D82,
    0x2B91_CE27, 0x3C71_AFFC, 0x82A3_F090, 0x0009_C491,
    0x589A_2EBD, 0x3C9B_6D34, 0x7B5D_E564, 0x000A_0C66,
    0x9AB8_9880, 0x3C95_277C, 0xB23E_255C, 0x000A_5503,
    0x6E73_5AB3, 0x3C84_6984, 0x5579_FDBF, 0x000A_9E6B,
    0x92CB_3387, 0x3C8C_1A77, 0x995A_D3AD, 0x000A_E89F,
    0xDC2D_1D96, 0x3CA2_2466, 0xB84F_15FA, 0x000B_33A2,
    0xB195_05AE, 0x3CA1_112E, 0xF2FB_5E46, 0x000B_7F76,
    0x0A5F_DDCD, 0x3C74_FFD7, 0x904B_C1D2, 0x000B_CC1E,
    0x30AF_0CB3, 0x3C73_6EAE, 0xDD85_529C, 0x000C_199B,
    0xD109_59AC, 0x3C84_E08F, 0x2E57_D14B, 0x000C_67F1,
    0x6C92_1968, 0x3C67_6B2C, 0xDCEF_9069, 0x000C_B720,
    0x36DF_99B3, 0x3C93_7009, 0x4A07_897B, 0x000D_072D,
    0xA63D_07A7, 0x3C74_A385, 0xDCFB_A487, 0x000D_5818,
    0xD5C1_92AC, 0x3C8E_5A50, 0x03DB_3285, 0x000D_A9E6,
    0x1C4A_9792, 0x3C98_BB73, 0x337B_9B5E, 0x000D_FC97,
    0x603A_88D3, 0x3C74_B604, 0xE78B_3FF6, 0x000E_502E,
    0x9209_4926, 0x3C91_6F27, 0xA2A4_90D9, 0x000E_A4AF,
    0x41AA_2008, 0x3C8E_C3BC, 0xEE61_5A27, 0x000E_FA1B,
    0x31D1_85EE, 0x3C8A_64A9, 0x5B6E_4540, 0x000F_5076,
    0x4D91_CD9D, 0x3C77_893B, 0x819E_90D8, 0x000F_A7C1,
];

#[rustfmt::skip]
pub(super) const ALLONES: [u32; 4] = [
    0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF, 0xFFFF_FFFF,
];

#[rustfmt::skip]
pub(super) const EBIAS: [u32; 4] = [
    0x0000_0000, 0x3FF0_0000, 0x0000_0000, 0x3FF0_0000,
];

#[rustfmt::skip]
pub(super) const XMAX: [u32; 2] = [
    0xFFFF_FFFF, 0x7FEF_FFFF,
];

#[rustfmt::skip]
pub(super) const XMIN: [u32; 2] = [
    0x0000_0000, 0x0010_0000,
];

#[rustfmt::skip]
pub(super) const INF: [u32; 2] = [
    0x0000_0000, 0x7FF0_0000,
];
