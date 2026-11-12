//! Minimal Minecraft wire format: `VarInt`s, strings and uncompressed packet frames.
//!
//! This is a deliberately small, independent implementation used for testing, so that
//! the probe can check CeriumMC's own protocol code from the outside.

use std::io::{self, Read, Write};

/// Maximum encoded length of a `VarInt`.
pub(crate) const MAX_VARINT_LEN: usize = 5;

/// Largest frame vanilla accepts: the length prefix is at most a 21-bit `VarInt`.
pub(crate) const MAX_FRAME_LEN: usize = (1 << 21) - 1;

/// Largest string vanilla's status response may carry, in UTF-16 code units.
const MAX_STRING_CHARS: usize = 32_767;

/// Appends `value` to `buf` as a `VarInt`: 7 bits per byte, least-significant group first.
pub(crate) fn write_varint(buf: &mut Vec<u8>, value: i32) {
    let mut rest = value.cast_unsigned();
    loop {
        let low = rest.to_le_bytes()[0] & 0x7F;
        rest >>= 7;
        if rest == 0 {
            buf.push(low);
            return;
        }
        buf.push(low | 0x80);
    }
}

/// Reads one `VarInt`, rejecting encodings longer than [`MAX_VARINT_LEN`] bytes.
pub(crate) fn read_varint(reader: &mut impl Read) -> io::Result<i32> {
    let mut value: u32 = 0;
    for group in 0..MAX_VARINT_LEN {
        let mut byte = [0u8; 1];
        reader.read_exact(&mut byte)?;
        value |= u32::from(byte[0] & 0x7F) << (7 * group);
        if byte[0] & 0x80 == 0 {
            return Ok(value.cast_signed());
        }
    }
    Err(invalid("VarInt is longer than 5 bytes"))
}

/// Appends a string: its UTF-8 byte length as a `VarInt`, then the bytes.
pub(crate) fn write_string(buf: &mut Vec<u8>, value: &str) -> io::Result<()> {
    let len = i32::try_from(value.len()).map_err(|_| invalid("string is too long"))?;
    write_varint(buf, len);
    buf.extend_from_slice(value.as_bytes());
    Ok(())
}

/// Reads a string written by [`write_string`], with vanilla's length limit.
pub(crate) fn read_string(reader: &mut impl Read) -> io::Result<String> {
    let len =
        usize::try_from(read_varint(reader)?).map_err(|_| invalid("negative string length"))?;
    if len > MAX_STRING_CHARS * 3 {
        return Err(invalid("string length exceeds the protocol limit"));
    }
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| invalid("string is not valid UTF-8"))
}

/// Writes one uncompressed packet: `VarInt` frame length, `VarInt` packet id, payload.
pub(crate) fn write_packet(writer: &mut impl Write, id: i32, payload: &[u8]) -> io::Result<()> {
    let mut body = Vec::with_capacity(MAX_VARINT_LEN + payload.len());
    write_varint(&mut body, id);
    body.extend_from_slice(payload);
    if body.len() > MAX_FRAME_LEN {
        return Err(invalid("packet is larger than the maximum frame"));
    }
    let len = i32::try_from(body.len()).map_err(|_| invalid("packet is too large"))?;
    let mut frame = Vec::with_capacity(MAX_VARINT_LEN + body.len());
    write_varint(&mut frame, len);
    frame.extend_from_slice(&body);
    writer.write_all(&frame)?;
    writer.flush()
}

/// Reads one uncompressed packet and returns its id and payload.
pub(crate) fn read_packet(reader: &mut impl Read) -> io::Result<(i32, Vec<u8>)> {
    let len =
        usize::try_from(read_varint(reader)?).map_err(|_| invalid("negative frame length"))?;
    if len == 0 || len > MAX_FRAME_LEN {
        return Err(invalid("frame length is out of range"));
    }
    let mut frame = vec![0u8; len];
    reader.read_exact(&mut frame)?;
    let mut cursor = frame.as_slice();
    let id = read_varint(&mut cursor)?;
    Ok((id, cursor.to_vec()))
}

/// Builds an [`io::ErrorKind::InvalidData`] error.
pub(crate) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known-good encodings from the protocol documentation.
    const VECTORS: &[(i32, &[u8])] = &[
        (0, &[0x00]),
        (1, &[0x01]),
        (2, &[0x02]),
        (127, &[0x7F]),
        (128, &[0x80, 0x01]),
        (255, &[0xFF, 0x01]),
        (25_565, &[0xDD, 0xC7, 0x01]),
        (2_097_151, &[0xFF, 0xFF, 0x7F]),
        (i32::MAX, &[0xFF, 0xFF, 0xFF, 0xFF, 0x07]),
        (-1, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]),
        (i32::MIN, &[0x80, 0x80, 0x80, 0x80, 0x08]),
    ];

    #[test]
    fn varint_matches_known_encodings() {
        for &(value, encoded) in VECTORS {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            assert_eq!(buf, encoded, "encoding {value}");
            let mut cursor = encoded;
            assert_eq!(read_varint(&mut cursor).unwrap(), value, "decoding {value}");
            assert_eq!(cursor, b"", "decoding {value} left bytes behind");
        }
    }

    #[test]
    fn varint_rejects_six_byte_encodings() {
        let mut cursor: &[u8] = &[0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        assert!(read_varint(&mut cursor).is_err());
    }

    #[test]
    fn varint_rejects_truncated_input() {
        let mut cursor: &[u8] = &[0x80, 0x80];
        assert!(read_varint(&mut cursor).is_err());
    }

    #[test]
    fn packets_round_trip() {
        let mut wire = Vec::new();
        let mut payload = Vec::new();
        write_string(&mut payload, "héllo, wörld").unwrap();
        write_packet(&mut wire, 0x2A, &payload).unwrap();

        let mut cursor = wire.as_slice();
        let (id, body) = read_packet(&mut cursor).unwrap();
        assert_eq!(id, 0x2A);
        assert_eq!(read_string(&mut body.as_slice()).unwrap(), "héllo, wörld");
        assert_eq!(cursor, b"");
    }

    #[test]
    fn empty_frames_are_rejected() {
        let mut cursor: &[u8] = &[0x00];
        assert!(read_packet(&mut cursor).is_err());
    }
}
