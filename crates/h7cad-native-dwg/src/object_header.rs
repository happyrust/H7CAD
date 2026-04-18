//! M3-B brick 3a: AC1015 object header decoder.
//!
//! An object slice as produced by [`crate::ObjectStreamCursor::
//! object_slice_by_handle`] has the on-disk layout:
//!
//! ```text
//! [MS size]   ← byte-aligned, Modular Short prefix
//! [BS type]   ← bit stream begins here, object class number
//! [RL bits]   ← main_size_bits, absolute bit offset of the handle
//!               stream (pre-R2004 drawings carry this inline,
//!               AC1015 is the canonical case)
//! [H handle]  ← the object's own handle reference
//! [xdata][entity/object payload][handle references at bit RL]
//! ```
//!
//! This module decodes the first three fields (`BS type`, `RL bits`,
//! `H handle`) and hands a positioned [`BitReader`] back to the caller
//! so brick 3b can continue with xdata + class-routed payload
//! decoding without re-scanning the MS prefix.
//!
//! Scope rules observed here:
//!
//! * **AC1015 only.** R2007+ object framing also adds a `ModularChar`
//!   after the MS prefix (handle-stream bit count), and R2010+ removes
//!   the inline `RL` field. Those branches live in future bricks; this
//!   module refuses to carry that complexity so its behaviour stays
//!   obvious.
//! * **No class routing.** Returning the `object_type` is enough for
//!   brick 3b to dispatch; we never try to turn it into a typed enum
//!   here.
//! * **No CRC check.** The object_stream cursor deliberately excludes
//!   the trailing 2-byte CRC so this module never has to worry about
//!   it.

use crate::bit_reader::BitReader;
use crate::modular::read_modular_short;
use crate::DwgReadError;
use h7cad_native_model::Handle;

/// Upper bound on the `main_size_bits` field. AC1015 object bodies
/// are in the tens-of-kilobytes range at the extreme; anything over
/// 16 MiB worth of bits means we've followed a corrupt slice.
const MAX_MAIN_SIZE_BITS: u32 = 128 * 1024 * 1024; // 128 Mbits ≈ 16 MiB

/// Decoded AC1015 object header, i.e. the three fields that always
/// precede the object body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectHeader {
    /// `BS` — the object class number. For built-in types this maps
    /// to the ODA spec table (1 = TEXT, 17 = ARC, 19 = CIRCLE, …);
    /// values ≥ 500 are custom classes registered by the drawing's
    /// Classes section.
    pub object_type: i16,
    /// `RL` — the absolute bit position (measured from the first bit
    /// of the object body, i.e. right after the `MS` prefix) where
    /// the trailing handle-reference stream begins. Brick 3b will
    /// use this to split the merged data into the main + handle
    /// streams.
    pub main_size_bits: u32,
    /// `H` — the object's own handle, matching the `PendingDocument.
    /// handle_offsets` entry that routed us here.
    pub handle: Handle,
    /// The control-byte high nibble from the `H` read. For an
    /// "object owns itself" handle reference this is always
    /// [`HANDLE_CODE_HARD_OWNER`]; we surface it so brick 3b can
    /// reject malformed slices that claim some other code.
    pub handle_code: u8,
}

/// Handle reference code for "hard-owned self-handle", which is
/// always how an object advertises its own identity in AC1015. Any
/// other code in the header position is almost certainly a sign the
/// slice is mis-aligned.
pub const HANDLE_CODE_HARD_OWNER: u8 = 0x5;

fn parse_ac1015_object_header_internal(
    slice: &[u8],
) -> Result<(ObjectHeader, &'_ [u8], usize), DwgReadError> {
    let mut cursor = 0usize;
    let body_size = read_modular_short(slice, &mut cursor).ok_or(DwgReadError::UnexpectedEof {
        context: "object MS size prefix",
    })?;
    let body_size = usize::try_from(body_size).map_err(|_| DwgReadError::UnexpectedEof {
        context: "object MS size does not fit in usize",
    })?;
    let body_end = cursor
        .checked_add(body_size)
        .ok_or(DwgReadError::UnexpectedEof {
            context: "object body end overflows usize",
        })?;
    if body_end > slice.len() {
        return Err(DwgReadError::UnexpectedEof {
            context: "object body extends past slice",
        });
    }
    let body = &slice[cursor..body_end];
    let mut reader = BitReader::new(body);

    let object_type = reader.read_bit_short()?;

    let main_size_bits = reader.read_raw_u32_le()?;
    if main_size_bits > MAX_MAIN_SIZE_BITS {
        return Err(DwgReadError::UnexpectedEof {
            context: "object main_size_bits out of plausible range",
        });
    }

    let (handle_code, handle_value) = reader.read_handle()?;
    let header_end_bits = reader.position_in_bits();

    Ok((
        ObjectHeader {
            object_type,
            main_size_bits,
            handle: Handle::new(handle_value),
            handle_code,
        },
        body,
        header_end_bits,
    ))
}

/// Decode the AC1015 object header from a slice produced by
/// [`crate::ObjectStreamCursor::object_slice_by_handle`].
///
/// On success returns the parsed header together with a
/// [`BitReader`] that has already consumed `[BS type][RL bits][H
/// handle]` and sits at the xdata/payload that follows.
///
/// Failure modes:
///
/// * [`DwgReadError::UnexpectedEof`] when any of the MS header,
///   body bytes, BS/RL/H fields, or their encoding would read past
///   the slice.
/// * [`DwgReadError::UnexpectedEof`] with a specific context when
///   the `MS` field decodes to a body larger than the slice or
///   when `main_size_bits` is obviously out of range.
pub fn read_ac1015_object_header(slice: &[u8]) -> Result<(ObjectHeader, BitReader<'_>), DwgReadError> {
    let (header, body, header_end_bits) = parse_ac1015_object_header_internal(slice)?;
    let mut reader = BitReader::new(body);
    reader.set_position_in_bits(header_end_bits)?;
    Ok((header, reader))
}

/// Decode the AC1015 object header and split the remaining body into
/// the R2000 main + handle sub-readers.
///
/// `main_size_bits` is measured from the first bit of the object body,
/// i.e. immediately after the modular-short body-size prefix. The
/// returned `main_reader` is restricted to `[header_end_bits,
/// main_size_bits)` and the returned `handle_reader` is restricted to
/// `[main_size_bits, body_bits)`.
pub fn split_ac1015_object_streams(
    slice: &[u8],
) -> Result<(ObjectHeader, BitReader<'_>, BitReader<'_>), DwgReadError> {
    let (header, body, header_end_bits) = parse_ac1015_object_header_internal(slice)?;
    let body_bits = body.len() * 8;
    let main_end_bits = header.main_size_bits as usize;
    if main_end_bits < header_end_bits || main_end_bits > body_bits {
        return Err(DwgReadError::UnexpectedEof {
            context: "object main_size_bits outside body range",
        });
    }
    let main_reader = BitReader::from_bit_range(body, header_end_bits, main_end_bits)?;
    let handle_reader = BitReader::from_bit_range(body, main_end_bits, body_bits)?;
    Ok((header, main_reader, handle_reader))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic AC1015 object slice with an MS prefix of the
    /// given `body_size`, then append the provided body bytes as-is.
    /// `body_size` is encoded as a single-chunk ModularShort and must
    /// be < 0x8000.
    fn synth_slice(body: &[u8]) -> Vec<u8> {
        assert!(body.len() < 0x8000, "test helper only supports single-chunk MS");
        let size = body.len() as u16;
        let mut buf = Vec::with_capacity(2 + body.len());
        buf.push((size & 0xFF) as u8);
        buf.push(((size >> 8) & 0xFF) as u8);
        buf.extend_from_slice(body);
        buf
    }

    /// Pack a list of (bits, count) pairs MSB-first into a byte
    /// buffer padded with zeros. Useful for writing the bit layout by
    /// hand without a full BitWriter.
    fn pack_bits(fields: &[(u64, u8)]) -> Vec<u8> {
        let total_bits: usize = fields.iter().map(|(_, n)| *n as usize).sum();
        let byte_count = (total_bits + 7) / 8;
        let mut out = vec![0u8; byte_count];
        let mut cursor = 0usize;
        for (value, count) in fields {
            for bit in (0..*count).rev() {
                let b = ((value >> bit) & 1) as u8;
                if b == 1 {
                    let byte_idx = cursor / 8;
                    let bit_idx = 7 - (cursor % 8);
                    out[byte_idx] |= 1 << bit_idx;
                }
                cursor += 1;
            }
        }
        out
    }

    /// Convenience: pack a BS 42 (using the 8-bit unsigned prefix),
    /// a raw LE u32 of `main_size_bits`, and a handle control byte
    /// with the given code nibble + length nibble, then length raw
    /// bytes. Pads out to a byte boundary.
    ///
    /// Returns the packed body bytes, ready to go inside the MS
    /// prefix of a synthetic slice.
    fn synth_body(bs_value: u8, main_size_bits: u32, handle_code: u8, handle_bytes: &[u8]) -> Vec<u8> {
        let mut fields: Vec<(u64, u8)> = Vec::new();
        // BS prefix `01` (next byte is unsigned short) + bs_value.
        fields.push((0b01, 2));
        fields.push((bs_value as u64, 8));
        // RL is 4 raw LE bytes: emit low-to-high 8 bits at a time,
        // MSB-first within each byte.
        for shift in 0..4 {
            fields.push((((main_size_bits >> (shift * 8)) & 0xFF) as u64, 8));
        }
        // Handle control byte: (code << 4) | len.
        let len = handle_bytes.len() as u8;
        assert!(len <= 8, "handle length must fit in 4 bits");
        let control = (handle_code << 4) | (len & 0x0F);
        fields.push((control as u64, 8));
        for byte in handle_bytes {
            fields.push((*byte as u64, 8));
        }
        pack_bits(&fields)
    }

    #[test]
    fn decodes_well_formed_header() {
        let body = synth_body(0x11, 0x100, HANDLE_CODE_HARD_OWNER, &[0x2A]);
        let slice = synth_slice(&body);
        let (header, _reader) = read_ac1015_object_header(&slice).unwrap();
        assert_eq!(header.object_type, 17);
        assert_eq!(header.main_size_bits, 0x100);
        assert_eq!(header.handle.value(), 0x2A);
        assert_eq!(header.handle_code, HANDLE_CODE_HARD_OWNER);
    }

    #[test]
    fn decodes_large_handle() {
        // Handle 0x1234_5678 packs as 4 big-endian bytes under an
        // len=4 control byte.
        let handle_bytes = [0x12, 0x34, 0x56, 0x78];
        let body = synth_body(0x2A, 0x200, HANDLE_CODE_HARD_OWNER, &handle_bytes);
        let slice = synth_slice(&body);
        let (header, _reader) = read_ac1015_object_header(&slice).unwrap();
        assert_eq!(header.object_type, 42);
        assert_eq!(header.handle.value(), 0x1234_5678);
    }

    #[test]
    fn rejects_empty_slice() {
        assert!(read_ac1015_object_header(&[]).is_err());
    }

    #[test]
    fn rejects_truncated_ms_prefix() {
        let err = read_ac1015_object_header(&[0x80]).unwrap_err();
        match err {
            DwgReadError::UnexpectedEof { context } => {
                assert!(context.contains("MS size"), "context was {context}");
            }
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn rejects_body_size_larger_than_slice() {
        // MS says body is 100 bytes, but only 4 follow.
        let mut slice = Vec::new();
        slice.extend_from_slice(&[0x64, 0x00]); // MS = 100
        slice.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        let err = read_ac1015_object_header(&slice).unwrap_err();
        match err {
            DwgReadError::UnexpectedEof { context } => {
                assert!(context.contains("past slice"), "context was {context}");
            }
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn rejects_truncated_bs_field() {
        // MS claims a 1-byte body, which is too small to hold BS + RL + H.
        let slice = synth_slice(&[0xFF]);
        assert!(read_ac1015_object_header(&slice).is_err());
    }

    #[test]
    fn rejects_implausible_main_size_bits() {
        // Craft a body that decodes a huge RL: BS `10` (literal 0)
        // then RL = 0xFFFF_FFFF then handle control 0x50 (code=5,
        // len=0 → value 0).
        let mut fields: Vec<(u64, u8)> = Vec::new();
        fields.push((0b10, 2)); // BS = 0
        for _ in 0..4 {
            fields.push((0xFF, 8));
        }
        fields.push((0x50, 8));
        let body = pack_bits(&fields);
        let slice = synth_slice(&body);
        let err = read_ac1015_object_header(&slice).unwrap_err();
        match err {
            DwgReadError::UnexpectedEof { context } => {
                assert!(
                    context.contains("main_size_bits"),
                    "context was {context}"
                );
            }
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn reader_positioned_exactly_after_header() {
        // Layout: 2 bits BS prefix + 8 bits BS payload + 32 RL raw
        // bits + 8 handle-control bits + N*8 handle bytes. With a
        // 1-byte handle that is 2+8+32+8+8 = 58 bits total; the
        // reader must report exactly that position regardless of
        // byte alignment, so brick 3b can continue without having
        // to re-derive the offset.
        let body = synth_body(0x01, 0x80, HANDLE_CODE_HARD_OWNER, &[0x07]);
        let slice = synth_slice(&body);
        let (_header, reader) = read_ac1015_object_header(&slice).unwrap();
        assert_eq!(reader.position_in_bits(), 58);
    }

    #[test]
    fn split_streams_bounds_follow_main_size_bits() {
        let body = synth_body(0x11, 64, HANDLE_CODE_HARD_OWNER, &[0x2A]);
        let slice = synth_slice(&body);
        let (header, main, handle) = split_ac1015_object_streams(&slice).unwrap();
        assert_eq!(header.object_type, 17);
        assert_eq!(main.position_in_bits(), 58);
        assert_eq!(main.bits_remaining(), 6);
        assert_eq!(handle.position_in_bits(), 64);
        assert_eq!(handle.bits_remaining(), body.len() * 8 - 64);
    }

    #[test]
    fn split_streams_rejects_main_size_before_header_end() {
        let body = synth_body(0x11, 8, HANDLE_CODE_HARD_OWNER, &[0x2A]);
        let slice = synth_slice(&body);
        let err = split_ac1015_object_streams(&slice).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }
}
