//! The MD5 message digest (RFC 1321).
//!
//! MD5 is cryptographically broken. CeriumMC uses it only where vanilla does, for
//! compatibility: deriving random seeds from names and building offline-mode UUIDs.

/// Per-step left-rotation amounts.
#[rustfmt::skip]
const SHIFTS: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// `floor(2^32 * |sin(i + 1)|)` for `i` in `0..64`.
#[rustfmt::skip]
const SINES: [u32; 64] = [
    0xD76A_A478, 0xE8C7_B756, 0x2420_70DB, 0xC1BD_CEEE,
    0xF57C_0FAF, 0x4787_C62A, 0xA830_4613, 0xFD46_9501,
    0x6980_98D8, 0x8B44_F7AF, 0xFFFF_5BB1, 0x895C_D7BE,
    0x6B90_1122, 0xFD98_7193, 0xA679_438E, 0x49B4_0821,
    0xF61E_2562, 0xC040_B340, 0x265E_5A51, 0xE9B6_C7AA,
    0xD62F_105D, 0x0244_1453, 0xD8A1_E681, 0xE7D3_FBC8,
    0x21E1_CDE6, 0xC337_07D6, 0xF4D5_0D87, 0x455A_14ED,
    0xA9E3_E905, 0xFCEF_A3F8, 0x676F_02D9, 0x8D2A_4C8A,
    0xFFFA_3942, 0x8771_F681, 0x6D9D_6122, 0xFDE5_380C,
    0xA4BE_EA44, 0x4BDE_CFA9, 0xF6BB_4B60, 0xBEBF_BC70,
    0x289B_7EC6, 0xEAA1_27FA, 0xD4EF_3085, 0x0488_1D05,
    0xD9D4_D039, 0xE6DB_99E5, 0x1FA2_7CF8, 0xC4AC_5665,
    0xF429_2244, 0x432A_FF97, 0xAB94_23A7, 0xFC93_A039,
    0x655B_59C3, 0x8F0C_CC92, 0xFFEF_F47D, 0x8584_5DD1,
    0x6FA8_7E4F, 0xFE2C_E6E0, 0xA301_4314, 0x4E08_11A1,
    0xF753_7E82, 0xBD3A_F235, 0x2AD7_D2BB, 0xEB86_D391,
];

const INITIAL_STATE: [u32; 4] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476];

/// Computes the 16-byte MD5 digest of `data`.
#[must_use]
pub fn digest(data: &[u8]) -> [u8; 16] {
    let mut state = INITIAL_STATE;
    let mut blocks = data.chunks_exact(64);
    for block in &mut blocks {
        compress(&mut state, block);
    }

    // Padding: 0x80, zeros, then the message length in bits (little-endian u64).
    let rest = blocks.remainder();
    let mut tail = [0u8; 128];
    tail[..rest.len()].copy_from_slice(rest);
    tail[rest.len()] = 0x80;
    let tail_len = if rest.len() < 56 { 64 } else { 128 };
    let bit_len = (data.len() as u64).wrapping_mul(8);
    tail[tail_len - 8..tail_len].copy_from_slice(&bit_len.to_le_bytes());
    for block in tail[..tail_len].chunks_exact(64) {
        compress(&mut state, block);
    }

    let mut out = [0u8; 16];
    for (bytes, word) in out.chunks_exact_mut(4).zip(state) {
        bytes.copy_from_slice(&word.to_le_bytes());
    }
    out
}

/// Processes one 64-byte block.
#[allow(clippy::many_single_char_names, reason = "names follow RFC 1321")]
fn compress(state: &mut [u32; 4], block: &[u8]) {
    let mut words = [0u32; 16];
    for (word, bytes) in words.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }

    let [mut a, mut b, mut c, mut d] = *state;
    for (i, (&sine, &shift)) in SINES.iter().zip(&SHIFTS).enumerate() {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let rotated = a
            .wrapping_add(f)
            .wrapping_add(sine)
            .wrapping_add(words[g])
            .rotate_left(shift);
        a = d;
        d = c;
        c = b;
        b = b.wrapping_add(rotated);
    }

    for (word, value) in state.iter_mut().zip([a, b, c, d]) {
        *word = word.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(data: &[u8]) -> String {
        let mut out = String::new();
        for byte in digest(data) {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    #[test]
    #[rustfmt::skip]
    fn rfc_1321_test_suite() {
        let cases: [(&str, &str); 7] = [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            ("abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"),
            (
                "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f",
            ),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(hex(input.as_bytes()), expected, "MD5({input:?})");
        }
    }

    #[test]
    fn padding_boundaries() {
        assert_eq!(hex(&[b'x'; 55]), "04364420e25c512fd958a70738aa8f72");
        assert_eq!(hex(&[b'x'; 56]), "668a72d5ba17f08e62dabcafad6db14b");
        assert_eq!(hex(&[b'x'; 64]), "c1bb4f81d892b2d57947682aeb252456");
        assert_eq!(hex(b"minecraft:test"), "1a2b64f5d3bdb4612e96d0a422fe68a7");
    }
}
