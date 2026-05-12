//! AC1015 object slice composer (F5.M4.B).
//!
//! Glues the M4.A object header writer together with caller-supplied
//! main-stream and handle-stream [`BitWriter`]s and produces a byte
//! sequence that round-trips through
//! [`crate::ObjectStreamCursor::object_slice_by_handle`] +
//! [`crate::split_ac1015_object_streams`].
//!
//! Wire format (mirrored from `object_stream.rs` and `object_header.rs`):
//!
//! ```text
//! [MS body_size]                       ← modular-short, byte aligned
//! [BS object_type][RL main_size_bits][H handle]   ← header bits
//! [main stream bits]                   ← `header.main_size_bits` − header_bit_count bits
//! [handle stream bits]                 ← remaining bits, padded to byte boundary
//! [0x00 0x00]                          ← CRC stub (reader does not validate)
//! ```
//!
//! `header.main_size_bits` is **the absolute bit position** within
//! the body where the handle stream begins (counted from the first
//! bit *after* the MS prefix). The composer therefore validates that
//! `main_size_bits == header_bit_count + main_stream.position_in_bits()`
//! up-front and refuses to emit slices that would not split cleanly
//! on the reader side. See
//! `docs/plans/2026-05-09-dwg-m4-object-stream-writer-plan.md` §2.2.

use crate::bit_reader::BitReader;
use crate::bit_writer::BitWriter;
use crate::modular::write_modular_short;
use crate::object_header::ObjectHeader;
use crate::writer::object_header::write_ac1015_object_header;
use crate::DwgWriteError;

/// Two-byte CRC trailer stub. The reader does not validate the
/// trailing CRC at this milestone (see the comment at the end of
/// `crate::handle_map::parse_handle_map`); the writer therefore
/// emits zeros so a real CRC pass added in a later milestone can be
/// dropped in without rewriting compose call sites.
pub const CRC_STUB: [u8; 2] = [0x00, 0x00];

/// Compose an AC1015 object slice from `header`, a fully-written
/// `main_stream`, and a fully-written `handle_stream`.
///
/// Both stream arguments are *finished* [`BitWriter`]s — i.e. all
/// bits have already been emitted, including any byte-padding the
/// caller wants. The composer walks them bit-by-bit (via
/// [`BitReader`]) so unaligned bit counts are preserved precisely
/// across the merge.
///
/// Returns [`DwgWriteError::InvalidValue`] when `header.main_size_bits`
/// does not match the actual `header_bit_count + main_stream` length;
/// this catches the most common composer bug (off-by-one in the
/// caller's bit accounting) before it becomes a reader-side panic.
pub fn compose_ac1015_object_slice(
    header: ObjectHeader,
    main_stream: &BitWriter,
    handle_stream: &BitWriter,
) -> Result<Vec<u8>, DwgWriteError> {
    // Probe the header bit count by writing the same header into a
    // throwaway `BitWriter`. This avoids hard-coding a constant that
    // would silently desync if M4.A's encoding ever changes.
    let header_bit_count = {
        let mut probe = BitWriter::new();
        write_ac1015_object_header(header, &mut probe)?;
        probe.position_in_bits()
    };

    let expected_main_size_bits = header_bit_count
        .checked_add(main_stream.position_in_bits())
        .ok_or_else(|| {
            DwgWriteError::InvalidValue(
                "object slice: header_bits + main_stream bit count overflowed usize".into(),
            )
        })?;
    if header.main_size_bits as usize != expected_main_size_bits {
        return Err(DwgWriteError::InvalidValue(format!(
            "object slice: header.main_size_bits = {} but header({header_bit_count}) + main({}) = {expected_main_size_bits}",
            header.main_size_bits,
            main_stream.position_in_bits()
        )));
    }

    // Build the body BitWriter: header → main bits → handle bits.
    let mut body = BitWriter::new();
    write_ac1015_object_header(header, &mut body)?;
    append_bit_stream(&mut body, main_stream)?;
    append_bit_stream(&mut body, handle_stream)?;
    body.align_to_byte();

    let body_bytes = body.into_bytes();
    let mut slice = Vec::with_capacity(4 + body_bytes.len() + CRC_STUB.len());
    write_modular_short(body_bytes.len() as u64, &mut slice);
    slice.extend_from_slice(&body_bytes);
    slice.extend_from_slice(&CRC_STUB);
    Ok(slice)
}

/// Append every bit (in the original MSB-first order) from `source`
/// into `target`. Walks bit-by-bit via [`BitReader`] so unaligned
/// trailing bits in `source` survive the copy.
fn append_bit_stream(target: &mut BitWriter, source: &BitWriter) -> Result<(), DwgWriteError> {
    let total_bits = source.position_in_bits();
    if total_bits == 0 {
        return Ok(());
    }
    let bytes = source.as_bytes();
    let mut reader = BitReader::new(bytes);
    for _ in 0..total_bits {
        let bit = reader.read_bit().map_err(|e| {
            DwgWriteError::InvalidValue(format!(
                "object slice: bit-stream walk failed at position {}: {e}",
                reader.position_in_bits()
            ))
        })?;
        target.write_bit(bit)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object_header::HANDLE_CODE_HARD_OWNER;
    use crate::{split_ac1015_object_streams, BitWriter};
    use h7cad_native_model::Handle;

    fn build_header(main_size_bits: u32) -> ObjectHeader {
        ObjectHeader {
            object_type: 19, // CIRCLE — arbitrary, stand-in entity type
            main_size_bits,
            handle: Handle::new(0x42),
            handle_code: HANDLE_CODE_HARD_OWNER,
        }
    }

    /// Returns the bit count of an object header for the supplied
    /// `header` value. Used by the tests that need to construct
    /// `main_size_bits` directly without duplicating the M4.A bit
    /// accounting.
    fn header_bit_count(header: ObjectHeader) -> usize {
        let mut probe = BitWriter::new();
        write_ac1015_object_header(header, &mut probe).expect("header writes");
        probe.position_in_bits()
    }

    #[test]
    fn round_trips_through_split_ac1015_object_streams() {
        // Main stream: write a single bit_short value the test can
        // verify on the reader side.
        let mut main = BitWriter::new();
        main.write_bit_short(7).expect("main writes");
        let main_bits = main.position_in_bits();

        // Handle stream: write a single handle reference.
        let mut handle_stream = BitWriter::new();
        handle_stream
            .write_handle(0x4, 0x77)
            .expect("handle stream writes");

        // header_bit_count must include the actual main stream length
        // so the reader's split point lines up.
        let header = build_header(0); // placeholder
        let main_size_bits = (header_bit_count(header) + main_bits) as u32;
        let header = build_header(main_size_bits);

        let slice =
            compose_ac1015_object_slice(header, &main, &handle_stream).expect("compose succeeds");
        let (parsed_header, mut parsed_main, mut parsed_handle) =
            split_ac1015_object_streams(&slice).expect("reader splits streams");

        assert_eq!(parsed_header, header);
        assert_eq!(parsed_main.read_bit_short().unwrap(), 7);
        assert_eq!(parsed_handle.read_handle().unwrap(), (0x4, 0x77));
    }

    #[test]
    fn round_trips_with_empty_main_and_handle_streams() {
        // Edge case: zero-content streams. The `main_size_bits`
        // therefore equals `header_bit_count` and the handle stream
        // is empty. The reader's `split_ac1015_object_streams` should
        // produce both readers with `bits_remaining == 0`.
        let header = build_header(0);
        let main_size_bits = header_bit_count(header) as u32;
        let header = build_header(main_size_bits);

        let main = BitWriter::new();
        let handle_stream = BitWriter::new();

        let slice =
            compose_ac1015_object_slice(header, &main, &handle_stream).expect("compose succeeds");
        let (parsed_header, parsed_main, parsed_handle) =
            split_ac1015_object_streams(&slice).expect("reader splits empty streams");

        assert_eq!(parsed_header, header);
        assert_eq!(parsed_main.bits_remaining(), 0);
        // handle reader trails by whatever remaining bytes-aligned
        // padding sits between the main split point and the body's
        // byte-aligned end (padding bits don't carry data).
        assert!(parsed_handle.bits_remaining() < 8);
    }

    #[test]
    fn rejects_main_size_bits_inconsistent_with_streams() {
        let mut main = BitWriter::new();
        main.write_bit_short(1).unwrap();
        let actual_bits = (header_bit_count(build_header(0)) + main.position_in_bits()) as u32;
        // Lie about main_size_bits: claim it is 8 bits longer than
        // the actual main stream supplies.
        let header = build_header(actual_bits + 8);
        let handle_stream = BitWriter::new();
        let err = compose_ac1015_object_slice(header, &main, &handle_stream)
            .expect_err("inconsistent main_size_bits must be rejected");
        match err {
            DwgWriteError::InvalidValue(msg) => assert!(
                msg.contains("main_size_bits"),
                "InvalidValue should mention main_size_bits: `{msg}`"
            ),
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }
}
