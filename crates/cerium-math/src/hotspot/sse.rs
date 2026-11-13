//! A minimal model of the x86 SSE registers and instructions used by HotSpot's math stubs.
//!
//! The stubs are transcribed instruction by instruction onto this model, so each method has
//! the exact semantics of the x86 instruction it is named after. An [`Xmm`] holds two 64-bit
//! lanes; `[0]` is the low lane, which scalar (`…sd`) instructions operate on.

use super::rcp::{self, Vendor};

/// A 128-bit XMM register as two 64-bit lanes: `[low, high]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Xmm(pub(crate) [u64; 2]);

impl Xmm {
    /// `movq`/`movsd` load of a double: the value in the low lane, zero in the high lane.
    pub(crate) fn from_f64(value: f64) -> Self {
        Self([value.to_bits(), 0])
    }

    /// `movdq xmm, r64`: zero-extended into the low lane.
    pub(crate) fn from_u64(value: u64) -> Self {
        Self([value, 0])
    }

    /// `movdl xmm, r32`: zero-extended into the low lane.
    pub(crate) fn from_u32(value: u32) -> Self {
        Self([u64::from(value), 0])
    }

    /// `movdqu xmm, [table + offset]`: two consecutive 64-bit words from a `juint` table.
    pub(crate) fn load(table: &[u32], index: usize) -> Self {
        Self([qword(table, index), qword(table, index + 2)])
    }

    /// `movq xmm, [table + offset]`: one 64-bit word, high lane zeroed.
    pub(crate) fn load_low(table: &[u32], index: usize) -> Self {
        Self([qword(table, index), 0])
    }

    /// The low lane as a double.
    pub(crate) fn low(self) -> f64 {
        f64::from_bits(self.0[0])
    }

    /// `movdl r32, xmm`: the low 32 bits.
    pub(crate) fn low_u32(self) -> u32 {
        self.0[0] as u32
    }

    /// `pextrw r32, xmm, index`: one 16-bit word, zero-extended.
    pub(crate) fn pextrw(self, index: u32) -> u32 {
        let lane = self.0[(index / 4) as usize];
        ((lane >> ((index % 4) * 16)) & 0xFFFF) as u32
    }

    /// `pinsrw xmm, r32, index`: replace one 16-bit word.
    pub(crate) fn pinsrw(&mut self, value: u32, index: u32) {
        let lane = &mut self.0[(index / 4) as usize];
        let shift = (index % 4) * 16;
        *lane = (*lane & !(0xFFFF << shift)) | (u64::from(value & 0xFFFF) << shift);
    }

    /// `por`.
    pub(crate) fn por(&mut self, other: Self) {
        self.0[0] |= other.0[0];
        self.0[1] |= other.0[1];
    }

    /// `pand`/`andpd`.
    pub(crate) fn pand(&mut self, other: Self) {
        self.0[0] &= other.0[0];
        self.0[1] &= other.0[1];
    }

    /// `psrlq xmm, imm`: logical right shift of each 64-bit lane.
    pub(crate) fn psrlq(&mut self, shift: u32) {
        self.0 = self.0.map(|lane| lane.checked_shr(shift).unwrap_or(0));
    }

    /// `psllq xmm, imm`: left shift of each 64-bit lane.
    pub(crate) fn psllq(&mut self, shift: u32) {
        self.0 = self.0.map(|lane| lane.checked_shl(shift).unwrap_or(0));
    }

    /// The four 32-bit elements, lowest first.
    fn dwords(self) -> [u32; 4] {
        let [low, high] = self.0;
        [low as u32, (low >> 32) as u32, high as u32, (high >> 32) as u32]
    }

    fn from_dwords(d: [u32; 4]) -> Self {
        let low = u64::from(d[0]) | (u64::from(d[1]) << 32);
        let high = u64::from(d[2]) | (u64::from(d[3]) << 32);
        Self([low, high])
    }

    /// `psrld xmm, imm`: logical right shift of each 32-bit element.
    pub(crate) fn psrld(&mut self, shift: u32) {
        *self = Self::from_dwords(self.dwords().map(|d| d.checked_shr(shift).unwrap_or(0)));
    }

    /// `paddd`: wrapping add of each 32-bit element.
    pub(crate) fn paddd(&mut self, other: Self) {
        let a = self.dwords();
        let b = other.dwords();
        *self = Self::from_dwords([
            a[0].wrapping_add(b[0]),
            a[1].wrapping_add(b[1]),
            a[2].wrapping_add(b[2]),
            a[3].wrapping_add(b[3]),
        ]);
    }

    /// `pshufd dst, src, imm`: each destination element selects a source element.
    pub(crate) fn pshufd(source: Self, imm: u32) -> Self {
        let d = source.dwords();
        let pick = |slot: u32| d[((imm >> (slot * 2)) & 3) as usize];
        Self::from_dwords([pick(0), pick(1), pick(2), pick(3)])
    }

    /// `rcpps`: approximate reciprocal of each 32-bit float, as the chosen CPU vendor computes it.
    pub(crate) fn rcpps(&mut self, vendor: Vendor) {
        *self = Self::from_dwords(self.dwords().map(|bits| rcp::rcpps(bits, vendor)));
    }

    /// `mulss`: multiply the low 32-bit floats with x86 NaN rules; other elements unchanged.
    pub(crate) fn mulss(&mut self, other: Self) {
        const QUIET: u32 = 0x0040_0000;
        let mut d = self.dwords();
        let (a, b) = (d[0], other.dwords()[0]);
        let (x, y) = (f32::from_bits(a), f32::from_bits(b));
        d[0] = if x.is_nan() {
            a | QUIET
        } else if y.is_nan() {
            b | QUIET
        } else {
            let product = x * y;
            if product.is_nan() { 0xFFC0_0000 } else { product.to_bits() }
        };
        *self = Self::from_dwords(d);
    }

    /// `addsd`.
    pub(crate) fn addsd(&mut self, other: Self) {
        self.0[0] = x86(self.0[0], other.0[0], |a, b| a + b);
    }

    /// `subsd`.
    pub(crate) fn subsd(&mut self, other: Self) {
        self.0[0] = x86(self.0[0], other.0[0], |a, b| a - b);
    }

    /// `mulsd`.
    pub(crate) fn mulsd(&mut self, other: Self) {
        self.0[0] = x86(self.0[0], other.0[0], |a, b| a * b);
    }

    /// `divsd`.
    pub(crate) fn divsd(&mut self, other: Self) {
        self.0[0] = x86(self.0[0], other.0[0], |a, b| a / b);
    }

    /// `addpd`.
    pub(crate) fn addpd(&mut self, other: Self) {
        self.0[0] = x86(self.0[0], other.0[0], |a, b| a + b);
        self.0[1] = x86(self.0[1], other.0[1], |a, b| a + b);
    }

    /// `mulpd`.
    pub(crate) fn mulpd(&mut self, other: Self) {
        self.0[0] = x86(self.0[0], other.0[0], |a, b| a * b);
        self.0[1] = x86(self.0[1], other.0[1], |a, b| a * b);
    }

    /// `movddup`: duplicate the low lane (`movdqu` + `movlhps` without SSE3; same result).
    pub(crate) fn movddup(source: Self) -> Self {
        Self([source.0[0], source.0[0]])
    }

    /// `cvtsi2sdl xmm, r32`: the low lane becomes the signed integer as a double.
    pub(crate) fn cvtsi2sdl(&mut self, value: i32) {
        self.0[0] = f64::from(value).to_bits();
    }
}

/// The x86 "QNaN floating-point indefinite": the NaN SSE produces for invalid operations.
const DEFAULT_NAN: u64 = 0xFFF8_0000_0000_0000;

/// Applies a double operation with x86 SSE NaN rules, so results are bit-identical on every
/// CPU: if the first operand is NaN, the result is that NaN quieted; otherwise, if the second
/// is NaN, that NaN quieted; otherwise an invalid operation gives [`DEFAULT_NAN`]. (Other
/// architectures, such as ARM, produce a positive default NaN instead.)
fn x86(a: u64, b: u64, op: impl Fn(f64, f64) -> f64) -> u64 {
    const QUIET: u64 = 0x0008_0000_0000_0000;
    let (x, y) = (f64::from_bits(a), f64::from_bits(b));
    if x.is_nan() {
        return a | QUIET;
    }
    if y.is_nan() {
        return b | QUIET;
    }
    let result = op(x, y);
    if result.is_nan() { DEFAULT_NAN } else { result.to_bits() }
}

/// Reads the little-endian 64-bit word at `juint` index `index` of a stub table.
fn qword(table: &[u32], index: usize) -> u64 {
    u64::from(table[index]) | (u64::from(table[index + 1]) << 32)
}
