//! AC1015 (R2000) object header writer (F5.M4.A).
//!
//! Inverse of [`crate::read_ac1015_object_header`]: writes the three
//! header fields (BS object type, RL main_size_bits, H handle) into a
//! [`BitWriter`] in the same order and bit framing the reader expects.
//!
//! This module is **scope-limited to the on-disk header itself**:
//!
//! - It does **not** prepend the modular-short body-size prefix; that
//!   is the job of the M4.B object slice composer (which knows the
//!   total body bit count after the main+handle streams have been
//!   written).
//! - It does **not** append the trailing two-byte CRC; that is also
//!   M4.B's responsibility, mirroring how the reader's
//!   [`crate::ObjectStreamCursor::object_slice_by_handle`] drops the
//!   trailing CRC before calling
//!   [`crate::read_ac1015_object_header`].
//! - It does **not** validate `main_size_bits` against any specific
//!   body length; the value is captured as-is so callers can stage
//!   header writes before the main stream is fully assembled.

use crate::bit_writer::BitWriter;
use crate::object_header::{ObjectHeader, HANDLE_CODE_HARD_OWNER};
use crate::DwgWriteError;

/// Encode an AC1015 object header into `writer` at its current bit
/// position. The written bit count matches the reader's expectation
/// (see `read_ac1015_object_header`'s
/// `reader_positioned_exactly_after_header` test).
///
/// `header.handle_code` is forwarded verbatim to
/// [`BitWriter::write_handle`]; the canonical value for a self-handle
/// is [`HANDLE_CODE_HARD_OWNER`] (`0x5`), but other codes are
/// permitted so callers can stage any pre-existing header value
/// during round-trip testing.
pub fn write_ac1015_object_header(
    header: ObjectHeader,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    writer.write_bit_short(header.object_type)?;
    writer.write_raw_u32_le(header.main_size_bits)?;
    writer.write_handle(header.handle_code, header.handle.value())?;
    Ok(())
}

/// Convenience: like [`write_ac1015_object_header`] but always uses
/// [`HANDLE_CODE_HARD_OWNER`] for the handle reference. AC1015
/// objects identify themselves with a hard-owned self-reference, so
/// this is the recommended entry point for the M4.B composer.
pub fn write_ac1015_object_self_header(
    object_type: i16,
    main_size_bits: u32,
    handle: h7cad_native_model::Handle,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    write_ac1015_object_header(
        ObjectHeader {
            object_type,
            main_size_bits,
            handle,
            handle_code: HANDLE_CODE_HARD_OWNER,
        },
        writer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_ac1015_object_header, BitWriter};
    use h7cad_native_model::Handle;

    /// Wrap `body_bits` (the BitWriter output for the header itself)
    /// in a synthetic AC1015 object slice: `[MS body_size][body bytes]`.
    /// Used by every round-trip test to feed the writer's bytes into
    /// the reader without reaching for the (much heavier) full-file
    /// pipeline.
    fn synth_slice(body_bytes: &[u8]) -> Vec<u8> {
        assert!(
            body_bytes.len() < 0x8000,
            "test helper only supports single-chunk MS"
        );
        let size = body_bytes.len() as u16;
        let mut buf = Vec::with_capacity(2 + body_bytes.len());
        buf.push((size & 0xFF) as u8);
        buf.push(((size >> 8) & 0xFF) as u8);
        buf.extend_from_slice(body_bytes);
        buf
    }

    #[test]
    fn object_header_round_trips_via_read_ac1015_object_header() {
        let header = ObjectHeader {
            object_type: 17,
            main_size_bits: 256,
            handle: Handle::new(0x2A),
            handle_code: HANDLE_CODE_HARD_OWNER,
        };
        let mut writer = BitWriter::new();
        write_ac1015_object_header(header, &mut writer).expect("header writes");
        let slice = synth_slice(writer.as_bytes());
        let (parsed, _reader) = read_ac1015_object_header(&slice).expect("reader recovers header");
        assert_eq!(parsed, header);
    }

    #[test]
    fn object_header_round_trips_for_multi_byte_handle() {
        let header = ObjectHeader {
            object_type: 42,
            main_size_bits: 512,
            handle: Handle::new(0x1234_5678),
            handle_code: HANDLE_CODE_HARD_OWNER,
        };
        let mut writer = BitWriter::new();
        write_ac1015_object_header(header, &mut writer).expect("header writes");
        let slice = synth_slice(writer.as_bytes());
        let (parsed, _reader) = read_ac1015_object_header(&slice).expect("reader recovers header");
        assert_eq!(parsed, header);
    }

    #[test]
    fn object_self_header_emits_canonical_hard_owner_code() {
        let mut writer = BitWriter::new();
        write_ac1015_object_self_header(1, 64, Handle::new(0x07), &mut writer)
            .expect("self header writes");
        let slice = synth_slice(writer.as_bytes());
        let (parsed, reader) = read_ac1015_object_header(&slice).expect("reader recovers header");
        assert_eq!(parsed.object_type, 1);
        assert_eq!(parsed.main_size_bits, 64);
        assert_eq!(parsed.handle, Handle::new(0x07));
        assert_eq!(parsed.handle_code, HANDLE_CODE_HARD_OWNER);
        // The reader's `reader_positioned_exactly_after_header` test
        // asserts 58 bits for the same fixture (BS=0x01, RL=0x80,
        // handle=0x07 with len=1). Mirror that expectation here so
        // any drift in the writer's bit count is caught immediately.
        assert_eq!(reader.position_in_bits(), 58);
    }

    #[test]
    fn object_header_propagates_handle_code_overflow_error() {
        // BitWriter::write_handle rejects codes above the 4-bit nibble.
        let header = ObjectHeader {
            object_type: 1,
            main_size_bits: 8,
            handle: Handle::new(0x01),
            handle_code: 0xFF, // out of range
        };
        let mut writer = BitWriter::new();
        let err = write_ac1015_object_header(header, &mut writer)
            .expect_err("oversized handle_code must be rejected");
        match err {
            DwgWriteError::InvalidValue(_) => {}
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }
}
