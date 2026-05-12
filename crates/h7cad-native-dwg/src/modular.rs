//! Byte-aligned modular integer decoders shared by the Handle section
//! and the object-stream prefix reader.
//!
//! These helpers live outside `BitReader` because the on-disk framing
//! is strictly byte-aligned: each consumer reads whole bytes (or pairs
//! of bytes for `ModularShort`) and must not drag bit-cursor state
//! across chunk boundaries. Keeping them here lets higher layers mix
//! the byte-aligned Handle stream with the bit-aligned object body
//! without accidental coupling.

/// Decode an unsigned modular character: 7 bits per byte, continuation
/// flagged by bit 7. Returns `None` on truncation or on a byte stream
/// that would shift past 63 bits of accumulated value (corruption
/// guard, not a legitimate on-disk case).
pub(crate) fn read_modular_char(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*cursor)?;
        *cursor += 1;
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

/// Decode a signed modular character. Framing matches the unsigned
/// form, but the terminator byte's bit 6 (`0x40`) flags a negative
/// value and the terminator payload is only the low 6 bits.
pub(crate) fn read_signed_modular_char(bytes: &[u8], cursor: &mut usize) -> Option<i64> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    loop {
        let byte = *bytes.get(*cursor)?;
        *cursor += 1;
        if byte & 0x80 != 0 {
            value |= ((byte & 0x7F) as u64) << shift;
            shift += 7;
            if shift > 63 {
                return None;
            }
        } else {
            let negative = byte & 0x40 != 0;
            value |= ((byte & 0x3F) as u64) << shift;
            return Some(if negative {
                -(value as i64)
            } else {
                value as i64
            });
        }
    }
}

/// Encode a u64 as an unsigned modular character. Pushes 7 bits of
/// payload per byte; bit 7 set on every byte except the terminator.
/// The terminator carries 7 bits of payload (its bit 7 is clear).
///
/// This is the byte-aligned inverse of [`read_modular_char`] and is
/// the F5.M3 brick used by the AC1015 `AcDb:Handles` section writer.
pub(crate) fn write_modular_char(value: u64, bytes: &mut Vec<u8>) {
    let mut remaining = value;
    while remaining > 0x7F {
        bytes.push(0x80 | (remaining as u8 & 0x7F));
        remaining >>= 7;
    }
    bytes.push(remaining as u8 & 0x7F);
}

/// Encode a u64 as a Modular Short. Two-byte little-endian chunks
/// with the high bit (`0x8000`) of each chunk acting as a continuation
/// flag; payload is the low 15 bits.
///
/// Byte-aligned inverse of [`read_modular_short`]. Used by the
/// AC1015 object slice composer (F5.M4.B) to emit the body-size
/// prefix.
pub(crate) fn write_modular_short(value: u64, bytes: &mut Vec<u8>) {
    let mut remaining = value;
    while remaining > 0x7FFF {
        let word = ((remaining as u16) & 0x7FFF) | 0x8000;
        bytes.extend_from_slice(&word.to_le_bytes());
        remaining >>= 15;
    }
    let word = (remaining as u16) & 0x7FFF;
    bytes.extend_from_slice(&word.to_le_bytes());
}

/// Encode an i64 as a signed modular character. Continuation bytes
/// carry 7 bits of payload (bit 7 set); the terminator carries 6 bits
/// of payload in bits 0..=5 and the sign in bit 6 (set = negative).
///
/// Byte-aligned inverse of [`read_signed_modular_char`].
pub(crate) fn write_signed_modular_char(value: i64, bytes: &mut Vec<u8>) {
    let negative = value < 0;
    let mut remaining = value.unsigned_abs();
    while remaining > 0x3F {
        bytes.push(0x80 | (remaining as u8 & 0x7F));
        remaining >>= 7;
    }
    let mut terminator = (remaining as u8) & 0x3F;
    if negative {
        terminator |= 0x40;
    }
    bytes.push(terminator);
}

/// Decode a Modular Short: little-endian 2-byte chunks, 15 bits of
/// payload per chunk, bit `0x8000` of each word is the continuation
/// flag. ACadSharp's `ReadModularShort` uses the same encoding and is
/// the reference implementation.
///
/// Returns `None` on truncation or if the accumulated shift would
/// exceed 60 bits (we cap at 4 chunks worth of payload; real AC1015
/// object sizes never approach that scale).
pub(crate) fn read_modular_short(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    loop {
        let lo = *bytes.get(*cursor)?;
        let hi = *bytes.get(*cursor + 1)?;
        *cursor += 2;
        let word = u16::from_le_bytes([lo, hi]);
        value |= ((word & 0x7FFF) as u64) << shift;
        if word & 0x8000 == 0 {
            return Some(value);
        }
        shift += 15;
        if shift > 60 {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_modular_char_single_byte_terminator() {
        let mut cursor = 0;
        let value = read_modular_char(&[0x05], &mut cursor).unwrap();
        assert_eq!(value, 5);
        assert_eq!(cursor, 1);
    }

    #[test]
    fn read_modular_char_multi_byte_continuation() {
        // 0x80 continuation, 0x01 terminator: value = 1 << 7 = 128
        let mut cursor = 0;
        let value = read_modular_char(&[0x80, 0x01], &mut cursor).unwrap();
        assert_eq!(value, 128);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn read_modular_char_reports_truncation() {
        let mut cursor = 0;
        let result = read_modular_char(&[0x80], &mut cursor);
        assert!(result.is_none());
    }

    #[test]
    fn read_signed_modular_char_positive_terminator() {
        // Low 6 bits of 0x02 carry payload 2; bit 6 clear → positive.
        let mut cursor = 0;
        let value = read_signed_modular_char(&[0x02], &mut cursor).unwrap();
        assert_eq!(value, 2);
        assert_eq!(cursor, 1);
    }

    #[test]
    fn read_signed_modular_char_negative_terminator() {
        // 0x42 = 0b0100_0010: bit 6 set → negative, low 6 bits = 2.
        let mut cursor = 0;
        let value = read_signed_modular_char(&[0x42], &mut cursor).unwrap();
        assert_eq!(value, -2);
        assert_eq!(cursor, 1);
    }

    #[test]
    fn read_signed_modular_char_multi_byte_positive() {
        // 0x80 continuation contributes 0 at shift 0; terminator 0x01
        // at shift 7 contributes 1 << 7 = 128.
        let mut cursor = 0;
        let value = read_signed_modular_char(&[0x80, 0x01], &mut cursor).unwrap();
        assert_eq!(value, 128);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn read_modular_short_single_chunk() {
        // Word 0x0005 terminates immediately (bit 0x8000 clear) → 5.
        let mut cursor = 0;
        let value = read_modular_short(&[0x05, 0x00], &mut cursor).unwrap();
        assert_eq!(value, 5);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn read_modular_short_max_single_chunk_payload() {
        // Word 0x7FFF (= 32767) is the largest value that fits in one
        // chunk without triggering continuation.
        let mut cursor = 0;
        let value = read_modular_short(&[0xFF, 0x7F], &mut cursor).unwrap();
        assert_eq!(value, 0x7FFF);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn read_modular_short_two_chunk_continuation() {
        // First word 0x8001 (continuation + payload 1), second word
        // 0x0002 (terminator + payload 2): value = 1 | (2 << 15) =
        // 0x10001 = 65537.
        let mut cursor = 0;
        let bytes = [0x01, 0x80, 0x02, 0x00];
        let value = read_modular_short(&bytes, &mut cursor).unwrap();
        assert_eq!(value, 1 | (2 << 15));
        assert_eq!(cursor, 4);
    }

    #[test]
    fn read_modular_short_reports_truncation() {
        let mut cursor = 0;
        let result = read_modular_short(&[0x01, 0x80, 0x00], &mut cursor);
        assert!(result.is_none());
    }

    #[test]
    fn read_modular_short_reports_odd_single_byte() {
        let mut cursor = 0;
        let result = read_modular_short(&[0x01], &mut cursor);
        assert!(result.is_none());
    }

    #[test]
    fn write_modular_char_round_trips_single_byte_values() {
        for value in [0u64, 1, 5, 0x3F, 0x7F] {
            let mut bytes = Vec::new();
            write_modular_char(value, &mut bytes);
            assert_eq!(bytes.len(), 1, "value {value} should fit in one byte");
            let mut cursor = 0;
            let read_back = read_modular_char(&bytes, &mut cursor).unwrap();
            assert_eq!(read_back, value, "round-trip for {value}");
            assert_eq!(cursor, bytes.len(), "cursor consumed all bytes");
        }
    }

    #[test]
    fn write_modular_char_round_trips_multi_byte_values() {
        for value in [128u64, 255, 1024, 0xFFFF, 0xFFFF_FFFF, u64::MAX] {
            let mut bytes = Vec::new();
            write_modular_char(value, &mut bytes);
            let mut cursor = 0;
            let read_back = read_modular_char(&bytes, &mut cursor).unwrap();
            assert_eq!(read_back, value, "round-trip for {value:#x}");
            assert_eq!(cursor, bytes.len(), "cursor consumed all bytes");
        }
    }

    #[test]
    fn write_modular_char_at_byte_boundary_emits_two_bytes() {
        // 0x80 is the smallest value that does NOT fit in a single
        // 7-bit terminator; the writer must emit `0x80 0x01`.
        let mut bytes = Vec::new();
        write_modular_char(0x80, &mut bytes);
        assert_eq!(bytes, [0x80, 0x01]);
    }

    #[test]
    fn write_signed_modular_char_round_trips_small_values() {
        for value in [0i64, 1, -1, 5, -2, 0x3F, -0x3F] {
            let mut bytes = Vec::new();
            write_signed_modular_char(value, &mut bytes);
            let mut cursor = 0;
            let read_back = read_signed_modular_char(&bytes, &mut cursor).unwrap();
            assert_eq!(read_back, value, "round-trip for {value}");
            assert_eq!(cursor, bytes.len(), "cursor consumed all bytes");
        }
    }

    #[test]
    fn write_signed_modular_char_round_trips_multi_byte_values() {
        for value in [128i64, -128, 0x4000, -0x4000, i64::MAX, i64::MIN + 1] {
            let mut bytes = Vec::new();
            write_signed_modular_char(value, &mut bytes);
            let mut cursor = 0;
            let read_back = read_signed_modular_char(&bytes, &mut cursor).unwrap();
            assert_eq!(read_back, value, "round-trip for {value}");
            assert_eq!(cursor, bytes.len(), "cursor consumed all bytes");
        }
    }

    #[test]
    fn write_modular_short_round_trips_single_chunk_values() {
        for value in [0u64, 1, 5, 0x7FFE, 0x7FFF] {
            let mut bytes = Vec::new();
            write_modular_short(value, &mut bytes);
            assert_eq!(bytes.len(), 2, "value {value} should fit in one chunk");
            let mut cursor = 0;
            let read_back = read_modular_short(&bytes, &mut cursor).unwrap();
            assert_eq!(read_back, value, "round-trip for {value}");
            assert_eq!(cursor, bytes.len());
        }
    }

    #[test]
    fn write_modular_short_round_trips_multi_chunk_values() {
        for value in [0x8000u64, 0xFFFF, 0x10001, 0x7FFF_FFFF, 0xFFFF_FFFF] {
            let mut bytes = Vec::new();
            write_modular_short(value, &mut bytes);
            let mut cursor = 0;
            let read_back = read_modular_short(&bytes, &mut cursor).unwrap();
            assert_eq!(read_back, value, "round-trip for {value:#x}");
            assert_eq!(cursor, bytes.len());
        }
    }

    #[test]
    fn write_modular_short_at_chunk_boundary_emits_two_chunks() {
        // 0x8000 is the smallest value that does not fit in a single
        // 15-bit terminator; expect a continuation chunk.
        let mut bytes = Vec::new();
        write_modular_short(0x8000, &mut bytes);
        assert_eq!(bytes.len(), 4);
        // First chunk: payload 0 + continuation bit set → 0x8000 LE = [0x00, 0x80].
        assert_eq!(&bytes[0..2], &[0x00, 0x80]);
        // Second chunk: terminator carrying payload 1 (1 << 15 → 0x8000 = 32768).
        assert_eq!(&bytes[2..4], &[0x01, 0x00]);
    }

    #[test]
    fn write_signed_modular_char_at_terminator_boundary_emits_two_bytes() {
        // 64 = 0x40 is the smallest absolute value that does not fit
        // in the terminator's 6-bit payload (which tops out at 0x3F).
        // The continuation byte carries `0x80 | 0x40 = 0xC0` and the
        // terminator carries `0` (with the sign bit added for
        // negatives). Reader path verified directly above this test.
        let mut bytes = Vec::new();
        write_signed_modular_char(0x40, &mut bytes);
        assert_eq!(bytes, [0xC0, 0x00]);
        let mut bytes = Vec::new();
        write_signed_modular_char(-0x40, &mut bytes);
        assert_eq!(bytes, [0xC0, 0x40]);
    }
}
