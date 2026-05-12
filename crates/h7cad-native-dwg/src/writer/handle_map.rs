//! AC1015 (R2000) `AcDb:Handles` section payload writer (F5.M3).
//!
//! Inverse of [`crate::parse_handle_map`]. The on-disk wire format
//! (mirrored from `ACadSharp/DwgHandleReader.cs` and validated by
//! `crate::handle_map`'s own unit tests) is:
//!
//! ```text
//! Repeat:
//!   RS (big-endian)  — chunk size including the 2-byte size header
//!   ModularChar (unsigned)  — delta handle
//!   SignedModularChar       — delta location
//!   ...                     — until consumed >= max_payload
//!   2 CRC bytes             — 0x00 0x00 stub at this milestone
//! 0x00 0x02                  — empty terminator chunk
//! ```
//!
//! The reader does **not** verify the CRC bytes (see comment at the
//! end of `parse_handle_map`); the writer therefore emits zero bytes
//! at this milestone. A real CRC pass lands once a workspace-wide
//! checksum policy is decided in F5.M5/M6.
//!
//! Single-chunk strategy at this milestone: the entire encoded entry
//! stream is packed into a single chunk whose payload size must not
//! exceed the documented [`MAX_CHUNK_PAYLOAD`] of 2032 bytes. Inputs
//! that would overflow this budget return
//! [`DwgWriteError::SectionTooLarge`]. Multi-chunk emission lands in
//! a follow-on slice once entity body writers (F5.M5) start producing
//! payloads dense enough to need it.

use crate::modular::{write_modular_char, write_signed_modular_char};
use crate::{DwgWriteError, HandleMapEntry};

/// Per-AutoCAD-spec maximum payload bytes per AC1015 Handle chunk
/// (excluding the 2-byte size header and 2-byte CRC trailer).
pub const MAX_CHUNK_PAYLOAD: usize = 2032;

/// Two-byte empty terminator chunk: a chunk size of `0x0002`
/// (== 2 bytes, no payload, no CRC) signals end-of-stream to the
/// reader. The writer always appends this regardless of the entry
/// count, including the zero-entry case.
pub const EMPTY_TERMINATOR: [u8; 2] = [0x00, 0x02];

/// Encode a sorted `(handle, offset)` list as the AC1015
/// `AcDb:Handles` section payload.
///
/// The reader assumes monotonic delta encoding, so this writer sorts
/// `entries` by `handle` ascending before emitting. Duplicate handles
/// are rejected with [`DwgWriteError::InvalidDocument`] — there is no
/// well-defined wire representation for two entries sharing the same
/// handle, and silently dropping one would let writer/reader drift.
pub fn write_ac1015_handle_map_payload(
    entries: &[HandleMapEntry],
) -> Result<Vec<u8>, DwgWriteError> {
    if entries.is_empty() {
        // Zero-entry case: just the terminator. This matches the
        // empty-payload behavior the reader's `parse_handle_map`
        // exercises when given two bytes `0x00 0x02`.
        return Ok(EMPTY_TERMINATOR.to_vec());
    }

    let sorted = sort_and_validate(entries)?;

    let mut entry_stream = Vec::new();
    let mut last_handle: u64 = 0;
    let mut last_loc: i64 = 0;
    for entry in &sorted {
        // `delta_handle` is unsigned; `sort_and_validate` guarantees
        // strict monotonicity so the subtraction is bounded.
        let delta_handle = entry
            .handle
            .value()
            .checked_sub(last_handle)
            .ok_or_else(|| {
                DwgWriteError::InvalidDocument(format!(
                    "handle map entry {:#x} below predecessor {:#x}",
                    entry.handle.value(),
                    last_handle
                ))
            })?;
        // `delta_loc` is signed; we use `checked_sub` to guard the
        // (vanishingly unlikely) i64 underflow case.
        let delta_loc = entry.offset.checked_sub(last_loc).ok_or_else(|| {
            DwgWriteError::InvalidDocument(format!(
                "handle map offset delta overflow: {} - {} not representable as i64",
                entry.offset, last_loc
            ))
        })?;
        write_modular_char(delta_handle, &mut entry_stream);
        write_signed_modular_char(delta_loc, &mut entry_stream);
        last_handle = entry.handle.value();
        last_loc = entry.offset;
    }

    if entry_stream.len() > MAX_CHUNK_PAYLOAD {
        return Err(DwgWriteError::SectionTooLarge {
            section: "AcDb:Handles",
            bytes: entry_stream.len(),
            limit: MAX_CHUNK_PAYLOAD,
        });
    }

    let chunk_size = entry_stream.len() + 2; // include size header in count
    let chunk_size_u16 = u16::try_from(chunk_size).map_err(|_| DwgWriteError::SectionTooLarge {
        section: "AcDb:Handles",
        bytes: chunk_size,
        limit: u16::MAX as usize,
    })?;

    let mut payload = Vec::with_capacity(2 + entry_stream.len() + 2 + EMPTY_TERMINATOR.len());
    payload.extend_from_slice(&chunk_size_u16.to_be_bytes());
    payload.extend_from_slice(&entry_stream);
    // CRC stub: reader does not validate at this milestone.
    payload.extend_from_slice(&[0x00, 0x00]);
    payload.extend_from_slice(&EMPTY_TERMINATOR);
    Ok(payload)
}

/// Sort `entries` by handle ascending and reject duplicates. The
/// returned `Vec` is owned so the caller can iterate without mutating
/// the input slice's order.
fn sort_and_validate(entries: &[HandleMapEntry]) -> Result<Vec<HandleMapEntry>, DwgWriteError> {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|entry| entry.handle.value());
    for window in sorted.windows(2) {
        if window[0].handle == window[1].handle {
            return Err(DwgWriteError::InvalidDocument(format!(
                "duplicate handle {:#x} in handle map",
                window[0].handle.value()
            )));
        }
    }
    Ok(sorted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_handle_map;
    use h7cad_native_model::Handle;

    #[test]
    fn empty_entries_round_trip_through_parse_handle_map() {
        let payload = write_ac1015_handle_map_payload(&[]).expect("empty payload writes");
        assert_eq!(payload, EMPTY_TERMINATOR.to_vec());
        let parsed = parse_handle_map(&payload).expect("empty payload parses");
        assert!(parsed.is_empty());
    }

    #[test]
    fn five_entries_round_trip_through_parse_handle_map() {
        let entries = vec![
            HandleMapEntry {
                handle: Handle::new(0x10),
                offset: 0x100,
            },
            HandleMapEntry {
                handle: Handle::new(0x14),
                offset: 0x140,
            },
            HandleMapEntry {
                handle: Handle::new(0x21),
                offset: 0x180,
            },
            HandleMapEntry {
                handle: Handle::new(0x42),
                offset: 0x1FC,
            },
            HandleMapEntry {
                handle: Handle::new(0x100),
                offset: 0x4096,
            },
        ];
        let payload = write_ac1015_handle_map_payload(&entries).expect("writes successfully");
        let parsed = parse_handle_map(&payload).expect("reader recovers entries");
        assert_eq!(parsed, entries);
    }

    #[test]
    fn unsorted_entries_are_sorted_before_encoding() {
        // Reader contract requires monotonic handle order; the writer
        // sorts the caller's slice before emitting.
        let unsorted = vec![
            HandleMapEntry {
                handle: Handle::new(0x42),
                offset: 0x80,
            },
            HandleMapEntry {
                handle: Handle::new(0x10),
                offset: 0x40,
            },
            HandleMapEntry {
                handle: Handle::new(0x21),
                offset: 0x60,
            },
        ];
        let payload = write_ac1015_handle_map_payload(&unsorted).expect("writes successfully");
        let parsed = parse_handle_map(&payload).expect("reader recovers entries");
        let mut sorted = unsorted.clone();
        sorted.sort_by_key(|entry| entry.handle.value());
        assert_eq!(parsed, sorted);
    }

    #[test]
    fn negative_offset_delta_round_trips() {
        // A later object can land at a smaller file offset than the
        // previous one; the signed modular char must carry the
        // negative delta correctly.
        let entries = vec![
            HandleMapEntry {
                handle: Handle::new(1),
                offset: 0x200,
            },
            HandleMapEntry {
                handle: Handle::new(2),
                offset: 0x100,
            },
        ];
        let payload = write_ac1015_handle_map_payload(&entries).expect("writes successfully");
        let parsed = parse_handle_map(&payload).expect("reader recovers entries");
        assert_eq!(parsed, entries);
    }

    #[test]
    fn duplicate_handles_are_rejected() {
        let entries = vec![
            HandleMapEntry {
                handle: Handle::new(7),
                offset: 0x100,
            },
            HandleMapEntry {
                handle: Handle::new(7),
                offset: 0x200,
            },
        ];
        let err = write_ac1015_handle_map_payload(&entries)
            .expect_err("duplicate handles must be rejected");
        match err {
            DwgWriteError::InvalidDocument(msg) => {
                assert!(
                    msg.contains("duplicate handle"),
                    "InvalidDocument message should mention duplicate handle: `{msg}`"
                );
            }
            other => panic!("expected InvalidDocument, got {other:?}"),
        }
    }

    #[test]
    fn payload_overflow_returns_section_too_large() {
        // Worst-case packing: every (delta_handle, delta_loc) pair
        // takes at least 2 bytes (1-byte modular char + 1-byte signed
        // modular char). 1100 entries with delta 1 / 1 produce 2200
        // bytes of entry stream, which exceeds the 2032-byte budget.
        let entries: Vec<HandleMapEntry> = (1..=1100)
            .map(|i| HandleMapEntry {
                handle: Handle::new(i),
                offset: i as i64,
            })
            .collect();
        let err = write_ac1015_handle_map_payload(&entries)
            .expect_err("entry stream over 2032 bytes must be rejected");
        match err {
            DwgWriteError::SectionTooLarge {
                section,
                bytes,
                limit,
            } => {
                assert_eq!(section, "AcDb:Handles");
                assert!(
                    bytes > limit,
                    "SectionTooLarge bytes ({bytes}) should exceed limit ({limit})"
                );
                assert_eq!(limit, MAX_CHUNK_PAYLOAD);
            }
            other => panic!("expected SectionTooLarge, got {other:?}"),
        }
    }
}
