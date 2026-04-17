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

/// Decode a Modular Short: little-endian 2-byte chunks, 15 bits of
/// payload per chunk, bit `0x8000` of each word is the continuation
/// flag. ACadSharp's `ReadModularShort` uses the same encoding and is
/// the reference implementation.
///
/// Returns `None` on truncation or if the accumulated shift would
/// exceed 60 bits (we cap at 4 chunks worth of payload; real AC1015
/// object sizes never approach that scale).
///
/// Currently only exercised by the in-module tests; the production
/// call site lands in M3-B brick 2b (`object_stream::ObjectStreamCursor`).
#[allow(dead_code)]
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
}
