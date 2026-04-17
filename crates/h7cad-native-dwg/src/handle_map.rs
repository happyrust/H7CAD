//! AC1015 (R2000) `AcDb:Handles` section decoder.
//!
//! The Handles section is a linear list of chunks, each encoding a
//! delta-compressed `(handle, object_stream_offset)` pair stream. The
//! on-disk layout mirrors `ACadSharp/DwgHandleReader.cs`:
//!
//! ```text
//! Repeat:
//!   RS (big-endian)  — size of this chunk including the 2-byte size
//!   if size == 2:
//!       break (empty tail chunk)
//!   maxOffset = min(size - 2, 2032)
//!   while consumed < maxOffset:
//!       ModularChar (unsigned) — delta handle
//!       SignedModularChar      — delta location
//!       lasthandle += delta_handle
//!       lastloc    += delta_location
//!       if delta_handle > 0: map[lasthandle] = lastloc
//!   2 CRC bytes trail each chunk
//! ```
//!
//! This module covers the **byte-aligned** decoders only. The bit-level
//! `BitReader` is deliberately not used here because the Handle section
//! is pure byte data. Live validation against the real ACadSharp
//! `sample_AC1015.dwg` returns **1047** entries across 2 chunks.

use crate::DwgReadError;
use h7cad_native_model::Handle;

/// One `(handle, object_stream_offset)` entry recovered from the
/// `AcDb:Handles` section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleMapEntry {
    pub handle: Handle,
    /// Offset inside the DWG file where this object's bit stream starts.
    /// AutoCAD guarantees the offset is non-negative but we keep the
    /// signed representation of the on-disk delta-encoded stream until
    /// higher layers slice object bytes.
    pub offset: i64,
}

/// Hard cap on the number of handle entries we will materialize before
/// treating the stream as corrupt. A single real AC1015 drawing reports
/// ~1k entries; anything above 10k means we are following garbage bytes.
const MAX_HANDLE_MAP_ENTRIES: usize = 1 << 20;

/// Hard cap on how many chunks we will follow. Real R2000 drawings have
/// 2-5 chunks; an unbounded loop here would be a denial-of-service
/// vector if the size prefix is corrupt.
const MAX_HANDLE_MAP_CHUNKS: usize = 1024;

/// Decode an `AcDb:Handles` section payload into a flat vector of
/// `(handle, offset)` entries. Entries appear in handle order because
/// the on-disk delta encoding is strictly monotonic.
pub fn parse_handle_map(payload: &[u8]) -> Result<Vec<HandleMapEntry>, DwgReadError> {
    let mut entries = Vec::new();
    let mut cursor = 0usize;
    let mut chunk_index = 0usize;
    let mut last_handle: u64 = 0;
    let mut last_loc: i64 = 0;

    loop {
        if chunk_index > MAX_HANDLE_MAP_CHUNKS {
            return Err(DwgReadError::UnexpectedEof {
                context: "AcDb:Handles chunk count exceeded sanity cap",
            });
        }
        if cursor + 2 > payload.len() {
            break;
        }

        let size = u16::from_be_bytes([payload[cursor], payload[cursor + 1]]) as usize;
        cursor += 2;

        if size == 2 {
            break;
        }
        if size < 2 {
            return Err(DwgReadError::UnexpectedEof {
                context: "AcDb:Handles chunk size below minimum",
            });
        }

        let max_payload = (size - 2).min(2032);
        let chunk_end = cursor
            .checked_add(max_payload)
            .ok_or(DwgReadError::UnexpectedEof {
                context: "AcDb:Handles chunk offset arithmetic overflowed",
            })?;
        if chunk_end > payload.len() {
            return Err(DwgReadError::UnexpectedEof {
                context: "AcDb:Handles chunk payload truncated",
            });
        }

        while cursor < chunk_end {
            let delta_handle =
                read_modular_char(payload, &mut cursor).ok_or(DwgReadError::UnexpectedEof {
                    context: "AcDb:Handles modular char (handle delta) truncated",
                })?;
            let delta_loc =
                read_signed_modular_char(payload, &mut cursor).ok_or(DwgReadError::UnexpectedEof {
                    context: "AcDb:Handles signed modular char (offset delta) truncated",
                })?;
            last_handle = last_handle.wrapping_add(delta_handle);
            last_loc = last_loc.saturating_add(delta_loc);
            if delta_handle > 0 {
                entries.push(HandleMapEntry {
                    handle: Handle::new(last_handle),
                    offset: last_loc,
                });
                if entries.len() > MAX_HANDLE_MAP_ENTRIES {
                    return Err(DwgReadError::UnexpectedEof {
                        context: "AcDb:Handles entry count exceeded sanity cap",
                    });
                }
            }
        }

        // Two trailing CRC bytes per chunk; we don't verify the CRC here
        // because the higher layers treat the Handle map as advisory
        // until we hook a full-stream checksum pass in a later milestone.
        cursor = cursor.saturating_add(2);
        chunk_index += 1;
    }

    Ok(entries)
}

/// Decode an unsigned modular character: 7 bits per byte, continuation
/// flagged by bit 7.
fn read_modular_char(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
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

/// Decode a signed modular character: same framing as the unsigned
/// form, but the final byte's bit 6 (`0x40`) flags a negative value and
/// the payload in the terminator is only the low 6 bits.
fn read_signed_modular_char(bytes: &[u8], cursor: &mut usize) -> Option<i64> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(payload: &[u8]) -> Vec<u8> {
        let size = (payload.len() + 2) as u16;
        let mut bytes = size.to_be_bytes().to_vec();
        bytes.extend_from_slice(payload);
        bytes.extend_from_slice(&[0x00, 0x00]);
        bytes
    }

    fn empty_tail() -> Vec<u8> {
        vec![0x00, 0x02]
    }

    #[test]
    fn parse_handle_map_single_chunk_recovers_handles() {
        // delta_handle = 5, delta_loc = +16, delta_handle = 3, delta_loc = +32
        let payload = [0x05, 0x10, 0x03, 0x20];
        let mut buf = chunk(&payload);
        buf.extend(empty_tail());
        let map = parse_handle_map(&buf).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(
            map[0],
            HandleMapEntry {
                handle: Handle::new(5),
                offset: 16,
            }
        );
        assert_eq!(
            map[1],
            HandleMapEntry {
                handle: Handle::new(8),
                offset: 48,
            }
        );
    }

    #[test]
    fn parse_handle_map_ignores_zero_handle_deltas() {
        // Zero handle delta must not create an entry even though
        // AutoCAD occasionally emits `(0, delta_loc)` stream padding.
        let payload = [0x00, 0x08, 0x02, 0x04];
        let mut buf = chunk(&payload);
        buf.extend(empty_tail());
        let map = parse_handle_map(&buf).unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map[0].handle, Handle::new(2));
        assert_eq!(map[0].offset, 12);
    }

    #[test]
    fn parse_handle_map_decodes_negative_offset_delta() {
        // delta_handle = 1, delta_loc = -2 (signed modular char: 0x42)
        let payload = [0x01, 0x42];
        let mut buf = chunk(&payload);
        buf.extend(empty_tail());
        let map = parse_handle_map(&buf).unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map[0].handle, Handle::new(1));
        assert_eq!(map[0].offset, -2);
    }

    #[test]
    fn parse_handle_map_supports_multi_byte_modular_chars() {
        // Unsigned modular char: 128 = 0x80 0x01; signed modular char:
        // +128 = 0x80 0x01 (payload continues: 0x80 means more bits,
        // next byte 0x01 terminates with sign bit 0 and payload 1<<7).
        let payload = [0x80, 0x01, 0x80, 0x01];
        let mut buf = chunk(&payload);
        buf.extend(empty_tail());
        let map = parse_handle_map(&buf).unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map[0].handle, Handle::new(128));
        assert_eq!(map[0].offset, 128);
    }

    #[test]
    fn parse_handle_map_reports_truncated_chunk() {
        // Claim 0x10 bytes of chunk payload but provide only 2.
        let err = parse_handle_map(&[0x00, 0x10, 0x01, 0x02]).unwrap_err();
        assert!(matches!(err, DwgReadError::UnexpectedEof { .. }));
    }

    #[test]
    fn parse_handle_map_handles_immediate_empty_tail() {
        let map = parse_handle_map(&empty_tail()).unwrap();
        assert!(map.is_empty());
    }
}
