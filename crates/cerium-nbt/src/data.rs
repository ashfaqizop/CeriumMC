//! `DataInput` and `DataOutput`, in the two flavours vanilla reads and writes NBT through.
//!
//! - Files use Java's `DataInputStream`/`DataOutputStream` ([`StreamInput`],
//!   [`StreamOutput`]). End of input is an `EOFException` without a message, `skipBytes`
//!   quietly skips fewer bytes (or none, for a negative count), and floats are written with
//!   `floatToIntBits`, which turns every NaN into the canonical NaN.
//! - Packets use Netty's `ByteBufInputStream`/`ByteBufOutputStream` ([`ByteBufInput`],
//!   [`ByteBufOutput`]). A short read is an `EOFException` saying how much was missing,
//!   a negative `skipBytes` count throws, and floats are written with their raw bits, so NaN
//!   payloads survive.

use std::io::{Read, Write};

use cerium_java::{JString, mutf8};

use crate::NbtError;

/// `java.io.DataInput`, as far as NBT uses it.
pub trait DataInput {
    /// `readByte()`.
    ///
    /// # Errors
    ///
    /// At the end of the input.
    fn read_byte(&mut self) -> Result<i8, NbtError>;

    /// `readFully(len)`: exactly `len` bytes.
    ///
    /// # Errors
    ///
    /// When fewer bytes remain.
    fn read_fully(&mut self, len: usize) -> Result<Vec<u8>, NbtError>;

    /// `skipBytes(n)`: skips up to `n` bytes and returns how many were skipped.
    ///
    /// # Errors
    ///
    /// Only Netty's input throws, for a negative `n`.
    fn skip_bytes(&mut self, n: i32) -> Result<i32, NbtError>;

    /// `readShort()`.
    ///
    /// # Errors
    ///
    /// At the end of the input.
    fn read_short(&mut self) -> Result<i16, NbtError> {
        let bytes = self.read_array::<2>()?;
        Ok(i16::from_be_bytes(bytes))
    }

    /// `readUnsignedShort()`.
    ///
    /// # Errors
    ///
    /// At the end of the input.
    fn read_unsigned_short(&mut self) -> Result<u16, NbtError> {
        Ok(self.read_short()? as u16)
    }

    /// `readInt()`.
    ///
    /// # Errors
    ///
    /// At the end of the input.
    fn read_int(&mut self) -> Result<i32, NbtError> {
        Ok(i32::from_be_bytes(self.read_array::<4>()?))
    }

    /// `readLong()`.
    ///
    /// # Errors
    ///
    /// At the end of the input.
    fn read_long(&mut self) -> Result<i64, NbtError> {
        Ok(i64::from_be_bytes(self.read_array::<8>()?))
    }

    /// `readFloat()`: `intBitsToFloat(readInt())`, raw bits kept.
    ///
    /// # Errors
    ///
    /// At the end of the input.
    fn read_float(&mut self) -> Result<f32, NbtError> {
        Ok(f32::from_bits(self.read_int()? as u32))
    }

    /// `readDouble()`.
    ///
    /// # Errors
    ///
    /// At the end of the input.
    fn read_double(&mut self) -> Result<f64, NbtError> {
        Ok(f64::from_bits(self.read_long()? as u64))
    }

    /// `readUTF()`: a length-prefixed Modified UTF-8 string.
    ///
    /// # Errors
    ///
    /// At the end of the input, or for malformed Modified UTF-8.
    fn read_utf(&mut self) -> Result<JString, NbtError> {
        let len = self.read_unsigned_short()?;
        let bytes = self.read_fully(usize::from(len))?;
        Ok(mutf8::decode(&bytes)?)
    }

    /// Reads `N` bytes as the primitive reads do: Java's stream reads them one at a time,
    /// Netty checks that all are available first.
    ///
    /// # Errors
    ///
    /// At the end of the input.
    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], NbtError>;
}

/// `java.io.DataInputStream` over any reader: a file, a decompressor or a byte slice.
#[derive(Debug)]
pub struct StreamInput<R> {
    inner: R,
}

impl<R: Read> StreamInput<R> {
    /// Wraps `inner`.
    pub fn new(inner: R) -> Self {
        Self { inner }
    }

    /// The wrapped reader.
    pub fn into_inner(self) -> R {
        self.inner
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), NbtError> {
        self.inner.read_exact(buf).map_err(|error| {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                NbtError::Eof { message: None }
            } else {
                NbtError::Io(error.to_string())
            }
        })
    }
}

impl<R: Read> DataInput for StreamInput<R> {
    fn read_byte(&mut self) -> Result<i8, NbtError> {
        let mut byte = [0];
        self.fill(&mut byte)?;
        Ok(byte[0] as i8)
    }

    fn read_fully(&mut self, len: usize) -> Result<Vec<u8>, NbtError> {
        // Read in bounded steps, so a huge declared length fails at the end of the input
        // instead of allocating it up front.
        let mut bytes = Vec::with_capacity(len.min(1 << 16));
        let mut chunk = [0u8; 8192];
        while bytes.len() < len {
            let n = (len - bytes.len()).min(chunk.len());
            self.fill(&mut chunk[..n])?;
            bytes.extend_from_slice(&chunk[..n]);
        }
        Ok(bytes)
    }

    fn skip_bytes(&mut self, n: i32) -> Result<i32, NbtError> {
        // `DataInputStream.skipBytes`: stops quietly at the end of the input.
        let Ok(n) = u64::try_from(n) else {
            return Ok(0);
        };
        let skipped = std::io::copy(&mut (&mut self.inner).take(n), &mut std::io::sink())
            .map_err(|error| NbtError::Io(error.to_string()))?;
        Ok(skipped as i32)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], NbtError> {
        let mut bytes = [0u8; N];
        self.fill(&mut bytes)?;
        Ok(bytes)
    }
}

/// Netty's `ByteBufInputStream` over the readable bytes of a packet.
#[derive(Debug)]
pub struct ByteBufInput<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> ByteBufInput<'a> {
    /// Reads `bytes` from the start.
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// How many bytes were consumed (the buffer's reader index).
    #[must_use]
    pub fn position(&self) -> usize {
        self.pos
    }

    fn available(&self) -> usize {
        self.bytes.len() - self.pos
    }

    /// `checkAvailable(fieldSize)`.
    fn check_available(&self, field_size: usize) -> Result<(), NbtError> {
        if field_size > self.available() {
            return Err(NbtError::Eof {
                message: Some(format!(
                    "fieldSize is too long! Length is {field_size}, but maximum is {}",
                    self.available()
                )),
            });
        }
        Ok(())
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], NbtError> {
        self.check_available(n)?;
        let bytes = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(bytes)
    }
}

impl DataInput for ByteBufInput<'_> {
    fn read_byte(&mut self) -> Result<i8, NbtError> {
        Ok(self.take(1)?[0] as i8)
    }

    fn read_fully(&mut self, len: usize) -> Result<Vec<u8>, NbtError> {
        Ok(self.take(len)?.to_vec())
    }

    fn skip_bytes(&mut self, n: i32) -> Result<i32, NbtError> {
        // `Math.min(available(), n)`, then `ByteBuf.skipBytes`, which rejects a negative count.
        let available = i32::try_from(self.available()).unwrap_or(i32::MAX);
        let n = available.min(n);
        if n < 0 {
            return Err(NbtError::IllegalArgument(format!(
                "minimumReadableBytes : {n} (expected: >= 0)"
            )));
        }
        self.pos += n as usize;
        Ok(n)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], NbtError> {
        Ok(self.take(N)?.try_into().expect("N bytes"))
    }
}

/// `java.io.DataOutput`, as far as NBT uses it.
pub trait DataOutput {
    /// `write(bytes)`.
    ///
    /// # Errors
    ///
    /// When the underlying writer fails.
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), NbtError>;

    /// `writeFloat(v)`.
    ///
    /// # Errors
    ///
    /// When the underlying writer fails.
    fn write_float(&mut self, v: f32) -> Result<(), NbtError>;

    /// `writeDouble(v)`.
    ///
    /// # Errors
    ///
    /// When the underlying writer fails.
    fn write_double(&mut self, v: f64) -> Result<(), NbtError>;

    /// `writeUTF(s)`.
    ///
    /// # Errors
    ///
    /// When the encoding exceeds 65535 bytes (nothing is written then), or the writer fails.
    fn write_utf(&mut self, s: &JString) -> Result<(), NbtError> {
        let bytes = mutf8::encode(s)?;
        self.write_bytes(&bytes)
    }

    /// `writeByte(v)`.
    ///
    /// # Errors
    ///
    /// When the underlying writer fails.
    fn write_byte(&mut self, v: i8) -> Result<(), NbtError> {
        self.write_bytes(&v.to_be_bytes())
    }

    /// `writeShort(v)`.
    ///
    /// # Errors
    ///
    /// When the underlying writer fails.
    fn write_short(&mut self, v: i16) -> Result<(), NbtError> {
        self.write_bytes(&v.to_be_bytes())
    }

    /// `writeInt(v)`.
    ///
    /// # Errors
    ///
    /// When the underlying writer fails.
    fn write_int(&mut self, v: i32) -> Result<(), NbtError> {
        self.write_bytes(&v.to_be_bytes())
    }

    /// `writeLong(v)`.
    ///
    /// # Errors
    ///
    /// When the underlying writer fails.
    fn write_long(&mut self, v: i64) -> Result<(), NbtError> {
        self.write_bytes(&v.to_be_bytes())
    }
}

impl<T: DataInput + ?Sized> DataInput for &mut T {
    fn read_byte(&mut self) -> Result<i8, NbtError> {
        (**self).read_byte()
    }

    fn read_fully(&mut self, len: usize) -> Result<Vec<u8>, NbtError> {
        (**self).read_fully(len)
    }

    fn skip_bytes(&mut self, n: i32) -> Result<i32, NbtError> {
        (**self).skip_bytes(n)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], NbtError> {
        (**self).read_array()
    }
}

impl<T: DataOutput + ?Sized> DataOutput for &mut T {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), NbtError> {
        (**self).write_bytes(bytes)
    }

    fn write_float(&mut self, v: f32) -> Result<(), NbtError> {
        (**self).write_float(v)
    }

    fn write_double(&mut self, v: f64) -> Result<(), NbtError> {
        (**self).write_double(v)
    }

    fn write_utf(&mut self, s: &JString) -> Result<(), NbtError> {
        (**self).write_utf(s)
    }
}

/// `java.io.DataOutputStream` over any writer.
#[derive(Debug)]
pub struct StreamOutput<W> {
    inner: W,
}

impl<W: Write> StreamOutput<W> {
    /// Wraps `inner`.
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    /// The wrapped writer.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> DataOutput for StreamOutput<W> {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), NbtError> {
        self.inner
            .write_all(bytes)
            .map_err(|error| NbtError::Io(error.to_string()))
    }

    /// `writeInt(Float.floatToIntBits(v))`: every NaN becomes `0x7FC00000`.
    fn write_float(&mut self, v: f32) -> Result<(), NbtError> {
        let bits = if v.is_nan() { 0x7FC0_0000 } else { v.to_bits() };
        self.write_bytes(&bits.to_be_bytes())
    }

    /// `writeLong(Double.doubleToLongBits(v))`: every NaN becomes the canonical NaN.
    fn write_double(&mut self, v: f64) -> Result<(), NbtError> {
        let bits = if v.is_nan() { 0x7FF8_0000_0000_0000 } else { v.to_bits() };
        self.write_bytes(&bits.to_be_bytes())
    }
}

/// Netty's `ByteBufOutputStream`, appending to a packet buffer.
#[derive(Debug)]
pub struct ByteBufOutput<'a> {
    buffer: &'a mut Vec<u8>,
}

impl<'a> ByteBufOutput<'a> {
    /// Appends to `buffer`.
    pub fn new(buffer: &'a mut Vec<u8>) -> Self {
        Self { buffer }
    }
}

impl DataOutput for ByteBufOutput<'_> {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), NbtError> {
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    /// `ByteBuf.writeFloat`: the raw bits.
    fn write_float(&mut self, v: f32) -> Result<(), NbtError> {
        self.write_bytes(&v.to_bits().to_be_bytes())
    }

    /// `ByteBuf.writeDouble`: the raw bits.
    fn write_double(&mut self, v: f64) -> Result<(), NbtError> {
        self.write_bytes(&v.to_bits().to_be_bytes())
    }
}

/// `NbtIo.StringFallbackDataOutput`: writes `""` instead of a string too long for Modified
/// UTF-8 (vanilla logs the failure). Used when saving NBT to files.
#[derive(Debug)]
pub struct StringFallbackOutput<O> {
    inner: O,
}

impl<O: DataOutput> StringFallbackOutput<O> {
    /// Wraps `inner`.
    pub fn new(inner: O) -> Self {
        Self { inner }
    }

    /// The wrapped output.
    pub fn into_inner(self) -> O {
        self.inner
    }
}

impl<O: DataOutput> DataOutput for StringFallbackOutput<O> {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), NbtError> {
        self.inner.write_bytes(bytes)
    }

    fn write_float(&mut self, v: f32) -> Result<(), NbtError> {
        self.inner.write_float(v)
    }

    fn write_double(&mut self, v: f64) -> Result<(), NbtError> {
        self.inner.write_double(v)
    }

    fn write_utf(&mut self, s: &JString) -> Result<(), NbtError> {
        match self.inner.write_utf(s) {
            Err(NbtError::Utf(_)) => self.inner.write_utf(&JString::new()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_of_input_differs_by_stream() {
        let mut stream = StreamInput::new(&[1u8][..]);
        assert_eq!(stream.read_int(), Err(NbtError::Eof { message: None }));
        let mut netty = ByteBufInput::new(&[1, 2]);
        let error = netty.read_int().unwrap_err();
        assert_eq!(
            error.message().as_deref(),
            Some("fieldSize is too long! Length is 4, but maximum is 2")
        );
        // Netty checks before consuming.
        assert_eq!(netty.read_short(), Ok(0x0102));
    }

    #[test]
    fn skipping_differs_by_stream() {
        let mut stream = StreamInput::new(&[0u8; 4][..]);
        assert_eq!(stream.skip_bytes(-3), Ok(0));
        assert_eq!(stream.skip_bytes(10), Ok(4));
        let mut netty = ByteBufInput::new(&[0; 4]);
        assert_eq!(netty.skip_bytes(10), Ok(4));
        assert_eq!(netty.skip_bytes(-1).unwrap_err().java_class(), "IllegalArgumentException");
    }

    #[test]
    fn nan_payloads_differ_by_stream() {
        let nan = f32::from_bits(0x7FC0_1234);
        let mut file = StreamOutput::new(Vec::new());
        file.write_float(nan).unwrap();
        assert_eq!(file.into_inner(), [0x7F, 0xC0, 0, 0]);
        let mut packet = Vec::new();
        ByteBufOutput::new(&mut packet).write_float(nan).unwrap();
        assert_eq!(packet, [0x7F, 0xC0, 0x12, 0x34]);
    }

    #[test]
    fn the_fallback_writes_an_empty_string() {
        let long = JString::from("\u{800}".repeat(30_000).as_str());
        let mut plain = StreamOutput::new(Vec::new());
        assert_eq!(plain.write_utf(&long).unwrap_err().java_class(), "UTFDataFormatException");
        let mut fallback = StringFallbackOutput::new(StreamOutput::new(Vec::new()));
        fallback.write_utf(&long).unwrap();
        assert_eq!(fallback.into_inner().into_inner(), [0, 0]);
    }
}
