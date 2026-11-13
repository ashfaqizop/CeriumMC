//! `Double.toString(double)` and `Float.toString(float)` of JDK 25, transcribed from
//! `jdk.internal.math.DoubleToDecimal`, `FloatToDecimal`, `ToDecimal` and `MathUtils`
//! (Raffaello Giulietti's Schubfach algorithm; GPL-2.0 with the Classpath Exception).
//!
//! Java picks the shortest decimal that rounds back to the value, but never fewer than two
//! significant digits (`4.9E-324`, where Rust prints `5e-324`). Values from `1e-3` up to
//! (excluding) `1e7` are printed plainly with at least one fractional digit (`100.0`), and
//! all others in computerized scientific notation (`1.0E7`, `1.0E-4`).

use crate::to_decimal_table::G;

/// `MathUtils.K_MIN`: the exponent of the first entry of [`G`].
const K_MIN: i32 = -324;
const Q_10: i32 = 41;
const C_10: i64 = 661_971_961_083;
const A_10: i64 = -274_743_187_321;
const Q_2: i32 = 38;
const C_2: i64 = 913_124_641_741;
const MASK_28: u32 = (1 << 28) - 1;

/// The first powers of ten, up to `10^17`.
const POW10: [i64; 18] = {
    let mut table = [1i64; 18];
    let mut i = 1;
    while i < table.len() {
        table[i] = table[i - 1] * 10;
        i += 1;
    }
    table
};

fn multiply_high(a: i64, b: i64) -> i64 {
    ((i128::from(a) * i128::from(b)) >> 64) as i64
}

/// `floor(log10(2^e))`.
fn flog10pow2(e: i32) -> i32 {
    ((i64::from(e) * C_10) >> Q_10) as i32
}

/// `floor(log10(3/4 * 2^e))`.
fn flog10_three_quarters_pow2(e: i32) -> i32 {
    ((i64::from(e) * C_10 + A_10) >> Q_10) as i32
}

/// `floor(log2(10^e))`.
fn flog2pow10(e: i32) -> i32 {
    ((i64::from(e) * C_2) >> Q_2) as i32
}

fn g1(k: i32) -> i64 {
    G[(k - K_MIN) as usize].0 as i64
}

fn g0(k: i32) -> i64 {
    G[(k - K_MIN) as usize].1 as i64
}

fn pow10(e: i32) -> i64 {
    POW10[e as usize]
}

/// `ToDecimal.y`: the fraction `a / 10^8` as a 28-bit fixed-point number, used to peel off
/// one decimal digit per multiplication by 10.
fn y(a: i32) -> u32 {
    let scaled = i64::from(a + 1) << 28;
    ((multiply_high(scaled, 193_428_131_138_340_668) as u64 >> 20) as i64 - 1) as u32
}

fn put_digit(out: &mut String, d: u32) {
    out.push(char::from(b'0' + d as u8));
}

/// `ToDecimal.put8Digits`: the 8 digits of `m` (`0 <= m < 10^8`), leading zeros included.
fn put_8_digits(out: &mut String, m: i32) {
    let mut y = y(m);
    for _ in 0..8 {
        let t = 10 * y;
        put_digit(out, t >> 28);
        y = t & MASK_28;
    }
}

/// `ToDecimal.removeTrailingZeroes`: drops trailing zeros but keeps the one right after '.'.
fn remove_trailing_zeroes(out: &mut String) {
    while out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.push('0');
    }
}

/// The digits of a 9-digit significand `h.m` (`h` one digit, `m` eight digits) placed for
/// decimal exponent `e`, in the plain or the scientific layout. `low` appends the digits
/// after `m` (if any) and removes trailing zeros.
fn layout(out: &mut String, h: i32, m: i32, e: i32, low: impl FnOnce(&mut String)) {
    if 0 < e && e <= 7 {
        // Plain format without leading zeroes: digits of h.m with the point after e digits.
        put_digit(out, h as u32);
        let mut y = y(m);
        let mut i = 1;
        while i < e {
            let t = 10 * y;
            put_digit(out, t >> 28);
            y = t & MASK_28;
            i += 1;
        }
        out.push('.');
        while i <= 8 {
            let t = 10 * y;
            put_digit(out, t >> 28);
            y = t & MASK_28;
            i += 1;
        }
        low(out);
    } else if -3 < e && e <= 0 {
        // Plain format with leading zeroes.
        out.push_str("0.");
        for _ in e..0 {
            out.push('0');
        }
        put_digit(out, h as u32);
        put_8_digits(out, m);
        low(out);
    } else {
        // Computerized scientific notation.
        put_digit(out, h as u32);
        out.push('.');
        put_8_digits(out, m);
        low(out);
        exponent(out, e - 1);
    }
}

/// `exponent`: `E`, a minus sign if negative, and one to three digits.
fn exponent(out: &mut String, mut e: i32) {
    out.push('E');
    if e < 0 {
        out.push('-');
        e = -e;
    }
    out.push_str(&e.to_string());
}

/// `Double.toString(v)`.
#[must_use]
pub fn double_to_string(v: f64) -> String {
    const P: i32 = 53;
    const Q_MIN: i32 = -1074;
    const C_MIN: i64 = 1 << (P - 1);
    const BQ_MASK: i32 = (1 << 11) - 1;
    const T_MASK: i64 = (1 << (P - 1)) - 1;
    const C_TINY: i64 = 3;

    let bits = v.to_bits() as i64;
    let t = bits & T_MASK;
    let bq = ((bits as u64) >> (P - 1)) as i32 & BQ_MASK;
    if bq < BQ_MASK {
        let mut out = String::with_capacity(24);
        if bits < 0 {
            out.push('-');
        }
        if bq != 0 {
            // Normal value. Here mq = -q.
            let mq = -Q_MIN + 1 - bq;
            let c = C_MIN | t;
            // The fast path for integers that need no rounding.
            if 0 < mq && mq < P {
                let f = c >> mq;
                if f << mq == c {
                    double_chars(&mut out, f, 0);
                    return out;
                }
            }
            double_decimal(&mut out, -mq, c, 0);
            return out;
        }
        if t != 0 {
            // Subnormal value.
            if t < C_TINY {
                double_decimal(&mut out, Q_MIN, 10 * t, -1);
            } else {
                double_decimal(&mut out, Q_MIN, t, 0);
            }
            return out;
        }
        return if bits == 0 { "0.0" } else { "-0.0" }.to_owned();
    }
    if t != 0 {
        "NaN".to_owned()
    } else if bits > 0 {
        "Infinity".to_owned()
    } else {
        "-Infinity".to_owned()
    }
}

/// `DoubleToDecimal.toDecimal(index, q, c, dk)`: the decimal for `c * 2^q`.
fn double_decimal(out: &mut String, q: i32, c: i64, dk: i32) {
    const C_MIN: i64 = 1 << 52;
    const Q_MIN: i32 = -1074;
    let out_bit = c & 1;
    let cb = c << 2;
    let cbr = cb + 2;
    let (cbl, k) = if c != C_MIN || q == Q_MIN {
        // Regular spacing.
        (cb - 2, flog10pow2(q))
    } else {
        // Irregular spacing.
        (cb - 1, flog10_three_quarters_pow2(q))
    };
    let h = q + flog2pow10(-k) + 2;
    let (g1, g0) = (g1(k), g0(k));
    let vb = rop(g1, g0, cb << h);
    let vbl = rop(g1, g0, cbl << h);
    let vbr = rop(g1, g0, cbr << h);

    let s = vb >> 2;
    if s >= 100 {
        // Try the shorter decimals sp10 and tp10, multiples of 10 around s.
        let sp10 = 10 * multiply_high(s, 115_292_150_460_684_698 << 4);
        let tp10 = sp10 + 10;
        let upin = vbl + out_bit <= sp10 << 2;
        let wpin = (tp10 << 2) + out_bit <= vbr;
        if upin != wpin {
            double_chars(out, if upin { sp10 } else { tp10 }, k);
            return;
        }
    }
    let t = s + 1;
    let uin = vbl + out_bit <= s << 2;
    let win = (t << 2) + out_bit <= vbr;
    if uin != win {
        // Exactly one of s and t rounds back.
        double_chars(out, if uin { s } else { t }, k + dk);
        return;
    }
    // Both round back: pick the closer, or the even one on a tie.
    let cmp = vb - ((s + t) << 1);
    let f = if cmp < 0 || cmp == 0 && s & 1 == 0 {
        s
    } else {
        t
    };
    double_chars(out, f, k + dk);
}

/// `DoubleToDecimal.rop`: rounds `g * cp` to odd, as a fixed-point number.
fn rop(g1: i64, g0: i64, cp: i64) -> i64 {
    const MASK_63: i64 = i64::MAX;
    let x1 = multiply_high(g0, cp);
    let y0 = g1.wrapping_mul(cp);
    let y1 = multiply_high(g1, cp);
    let z = ((y0 as u64 >> 1) as i64).wrapping_add(x1);
    let vbp = y1.wrapping_add((z as u64 >> 63) as i64);
    vbp | (((z & MASK_63).wrapping_add(MASK_63)) as u64 >> 63) as i64
}

/// `DoubleToDecimal.toChars`: the digits of `f * 10^e`.
fn double_chars(out: &mut String, f: i64, e: i32) {
    const H: i32 = 17;
    let mut len = flog10pow2(64 - f.leading_zeros() as i32);
    if f >= pow10(len) {
        len += 1;
    }
    // Normalize to 17 digits: f is then h (1 digit), m (8 digits) and l (8 digits).
    let f = f * pow10(H - len);
    let e = e + len;
    let hm = (multiply_high(f, 193_428_131_138_340_668) as u64 >> 20) as i64;
    let l = (f - 100_000_000 * hm) as i32;
    let h = ((hm * 1_441_151_881) as u64 >> 57) as i32;
    let m = (hm - 100_000_000 * i64::from(h)) as i32;
    layout(out, h, m, e, |out| {
        if l != 0 {
            put_8_digits(out, l);
        }
        remove_trailing_zeroes(out);
    });
}

/// `Float.toString(v)`.
#[must_use]
pub fn float_to_string(v: f32) -> String {
    const P: i32 = 24;
    const Q_MIN: i32 = -149;
    const C_MIN: i32 = 1 << (P - 1);
    const BQ_MASK: i32 = (1 << 8) - 1;
    const T_MASK: i32 = (1 << (P - 1)) - 1;
    const C_TINY: i32 = 8;

    let bits = v.to_bits() as i32;
    let t = bits & T_MASK;
    let bq = ((bits as u32) >> (P - 1)) as i32 & BQ_MASK;
    if bq < BQ_MASK {
        let mut out = String::with_capacity(16);
        if bits < 0 {
            out.push('-');
        }
        if bq != 0 {
            // Normal value. Here mq = -q.
            let mq = -Q_MIN + 1 - bq;
            let c = C_MIN | t;
            if 0 < mq && mq < P {
                let f = c >> mq;
                if f << mq == c {
                    float_chars(&mut out, f, 0);
                    return out;
                }
            }
            float_decimal(&mut out, -mq, c, 0);
            return out;
        }
        if t != 0 {
            // Subnormal value.
            if t < C_TINY {
                float_decimal(&mut out, Q_MIN, 10 * t, -1);
            } else {
                float_decimal(&mut out, Q_MIN, t, 0);
            }
            return out;
        }
        return if bits == 0 { "0.0" } else { "-0.0" }.to_owned();
    }
    if t != 0 {
        "NaN".to_owned()
    } else if bits > 0 {
        "Infinity".to_owned()
    } else {
        "-Infinity".to_owned()
    }
}

/// `FloatToDecimal.toDecimal(index, q, c, dk)`: the decimal for `c * 2^q`.
fn float_decimal(out: &mut String, q: i32, c: i32, dk: i32) {
    const C_MIN: i32 = 1 << 23;
    const Q_MIN: i32 = -149;
    let out_bit = c & 1;
    let cb = i64::from(c << 2);
    let cbr = cb + 2;
    let (cbl, k) = if c != C_MIN || q == Q_MIN {
        (cb - 2, flog10pow2(q))
    } else {
        (cb - 1, flog10_three_quarters_pow2(q))
    };
    let h = q + flog2pow10(-k) + 33;
    let g = g1(k) + 1;
    let vb = float_rop(g, cb << h);
    let vbl = float_rop(g, cbl << h);
    let vbr = float_rop(g, cbr << h);

    let s = vb >> 2;
    if s >= 100 {
        let sp10 = 10 * ((i64::from(s) * 1_717_986_919) as u64 >> 34) as i32;
        let tp10 = sp10 + 10;
        let upin = vbl + out_bit <= sp10 << 2;
        let wpin = (tp10 << 2) + out_bit <= vbr;
        if upin != wpin {
            float_chars(out, if upin { sp10 } else { tp10 }, k);
            return;
        }
    }
    let t = s + 1;
    let uin = vbl + out_bit <= s << 2;
    let win = (t << 2) + out_bit <= vbr;
    if uin != win {
        float_chars(out, if uin { s } else { t }, k + dk);
        return;
    }
    let cmp = vb - ((s + t) << 1);
    let f = if cmp < 0 || cmp == 0 && s & 1 == 0 {
        s
    } else {
        t
    };
    float_chars(out, f, k + dk);
}

/// `FloatToDecimal.rop`.
fn float_rop(g: i64, cp: i64) -> i32 {
    const MASK_32: i64 = (1 << 32) - 1;
    let x1 = multiply_high(g, cp);
    let vbp = (x1 as u64 >> 31) as i64;
    (vbp | (((x1 & MASK_32) + MASK_32) as u64 >> 32) as i64) as i32
}

/// `FloatToDecimal.toChars`: the digits of `f * 10^e`.
fn float_chars(out: &mut String, f: i32, e: i32) {
    const H: i32 = 9;
    let mut len = flog10pow2(32 - f.leading_zeros() as i32);
    if i64::from(f) >= pow10(len) {
        len += 1;
    }
    // Normalize to 9 digits: h (1 digit) and l (8 digits).
    let f = f * pow10(H - len) as i32;
    let e = e + len;
    let h = ((i64::from(f) * 1_441_151_881) as u64 >> 57) as i32;
    let l = f - 100_000_000 * h;
    layout(out, h, l, e, remove_trailing_zeroes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_print_like_java() {
        let cases: [(f64, &str); 16] = [
            (0.0, "0.0"),
            (-0.0, "-0.0"),
            (1.0, "1.0"),
            (100.0, "100.0"),
            (0.1, "0.1"),
            (0.001, "0.001"),
            (0.0001, "1.0E-4"),
            (1e7, "1.0E7"),
            (9_999_999.0, "9999999.0"),
            (1.0 / 3.0, "0.3333333333333333"),
            (f64::MIN_POSITIVE * 0.5, "1.1125369292536007E-308"),
            (f64::from_bits(1), "4.9E-324"),
            (f64::MAX, "1.7976931348623157E308"),
            (2e23, "2.0E23"),
            (f64::NAN, "NaN"),
            (f64::NEG_INFINITY, "-Infinity"),
        ];
        for (value, java) in cases {
            assert_eq!(double_to_string(value), java, "{value:e}");
        }
    }

    #[test]
    fn floats_print_like_java() {
        let cases: [(f32, &str); 10] = [
            (0.0, "0.0"),
            (1.0, "1.0"),
            (0.1, "0.1"),
            (1.0e-5, "1.0E-5"),
            (123_456_790.0, "1.2345679E8"),
            (f32::from_bits(1), "1.4E-45"),
            (f32::MAX, "3.4028235E38"),
            (0.3, "0.3"),
            (f32::INFINITY, "Infinity"),
            (-2.5, "-2.5"),
        ];
        for (value, java) in cases {
            assert_eq!(float_to_string(value), java, "{value:e}");
        }
    }
}
