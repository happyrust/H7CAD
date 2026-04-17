//! M3-B brick 2b: resolve a decoded `AcDb:Handles` entry to the
//! byte-aligned slice of the owning object's on-disk record.
//!
//! AC1015 (R2000) stores each object as:
//!
//! ```text
//! [MS size]                       ← handle_offsets[i].offset points here
//! [size bytes of bit-packed body] ← BitReader territory, handled by brick 3
//! [2-byte CRC]                    ← validated by a later milestone
//! ```
//!
//! This module is **byte-aligned only**: it reads the `MS` size prefix
//! with `modular::read_modular_short` and returns a raw byte slice that
//! starts at the MS header and extends through the body. Decoding the
//! body is not this module's responsibility; that lives in brick 3's
//! class-routed object decoders.
//!
//! Offsets recovered from the Handle section are signed because the
//! on-disk delta encoding is signed. Real offsets are always positive
//! and in-range, but the tail of AutoCAD's handle map often contains
//! purged/garbage entries whose offsets fall outside the file or are
//! zero; those are filtered out by returning `None` from every lookup
//! helper so callers never see bogus slices.

use crate::handle_map::HandleMapEntry;
use crate::modular::read_modular_short;
use h7cad_native_model::Handle;

/// Maximum plausible object body size for AC1015 objects. Real
/// drawings stay far below this; anything larger almost certainly
/// means we followed a corrupt offset and should bail out.
const MAX_OBJECT_BODY_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// A read-only view over an AC1015 object stream, indexed by handle.
///
/// `file` is the entire DWG file (the same buffer fed into
/// `build_pending_document`) and `offsets` is the decoded Handle map
/// entries as surfaced by `PendingDocument.handle_offsets`. Both are
/// borrowed for the lifetime of the cursor to avoid allocating a copy
/// per object lookup.
#[derive(Debug, Clone, Copy)]
pub struct ObjectStreamCursor<'a> {
    file: &'a [u8],
    offsets: &'a [HandleMapEntry],
}

impl<'a> ObjectStreamCursor<'a> {
    /// Build a cursor around the raw file bytes and the decoded handle
    /// map. No validation happens up front; each lookup checks its
    /// offset against `file.len()` individually so a few stray
    /// garbage entries never poison the rest of the stream.
    pub fn new(file: &'a [u8], offsets: &'a [HandleMapEntry]) -> Self {
        Self { file, offsets }
    }

    /// Decode the `MS` size header at the given absolute file offset.
    /// Returns `(header_bytes, body_size)` on success.
    ///
    /// Returns `None` when:
    ///
    /// * `offset <= 0` or `offset >= file.len()` (purged/garbage entry),
    /// * the `MS` bytes are truncated,
    /// * or `body_size` exceeds `MAX_OBJECT_BODY_BYTES` (corruption).
    ///
    /// Does **not** require that `offset + header + body` actually fits
    /// inside the file; callers (e.g. `object_slice_by_handle`) make
    /// that final range check themselves so they can distinguish a
    /// plausibly-sized but out-of-range object from a grossly broken
    /// size prefix.
    pub fn object_size_at(&self, offset: i64) -> Option<(usize, usize)> {
        if offset <= 0 {
            return None;
        }
        let start = usize::try_from(offset).ok()?;
        if start >= self.file.len() {
            return None;
        }
        let mut cursor = start;
        let body_size = read_modular_short(self.file, &mut cursor)?;
        let body_size = usize::try_from(body_size).ok()?;
        if body_size > MAX_OBJECT_BODY_BYTES {
            return None;
        }
        let header_bytes = cursor.checked_sub(start)?;
        Some((header_bytes, body_size))
    }

    /// Return the byte range covering `[MS header + body]` for the
    /// object with the given handle. The trailing 2-byte CRC is
    /// intentionally excluded because the higher layer that validates
    /// it needs the body boundary on its own terms.
    ///
    /// Returns `None` on:
    ///
    /// * unknown handle,
    /// * garbage / out-of-range offset,
    /// * truncated MS header,
    /// * or a body that would extend past the end of file.
    pub fn object_slice_by_handle(&self, handle: Handle) -> Option<&'a [u8]> {
        let entry = self
            .offsets
            .binary_search_by(|e| e.handle.value().cmp(&handle.value()))
            .ok()
            .map(|idx| self.offsets[idx])?;
        let (header_bytes, body_size) = self.object_size_at(entry.offset)?;
        let start = usize::try_from(entry.offset).ok()?;
        let end = start.checked_add(header_bytes)?.checked_add(body_size)?;
        if end > self.file.len() {
            return None;
        }
        Some(&self.file[start..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic DWG prefix: `[pad..]` then an MS(size) header
    /// followed by `size` body bytes. Returns `(buffer, offset)` where
    /// `offset` is the absolute position of the MS header.
    fn synth_object(pad: usize, body: &[u8]) -> (Vec<u8>, i64) {
        let mut buf = vec![0u8; pad];
        let offset = buf.len() as i64;
        // Single-chunk MS: word = body.len() (< 0x8000)
        let size = body.len();
        assert!(size < 0x8000, "test helper only supports single-chunk MS");
        buf.push((size & 0xFF) as u8);
        buf.push(((size >> 8) & 0xFF) as u8);
        buf.extend_from_slice(body);
        (buf, offset)
    }

    #[test]
    fn object_size_at_reads_single_chunk_ms() {
        let (buf, offset) = synth_object(64, &[0xAA; 12]);
        let cursor = ObjectStreamCursor::new(&buf, &[]);
        assert_eq!(cursor.object_size_at(offset), Some((2, 12)));
    }

    #[test]
    fn object_size_at_rejects_zero_and_negative_offsets() {
        let buf = vec![0u8; 32];
        let cursor = ObjectStreamCursor::new(&buf, &[]);
        assert!(cursor.object_size_at(0).is_none());
        assert!(cursor.object_size_at(-1).is_none());
    }

    #[test]
    fn object_size_at_rejects_offset_past_eof() {
        let buf = vec![0u8; 32];
        let cursor = ObjectStreamCursor::new(&buf, &[]);
        assert!(cursor.object_size_at(32).is_none());
        assert!(cursor.object_size_at(1_000_000).is_none());
    }

    #[test]
    fn object_size_at_rejects_truncated_ms_header() {
        let buf = vec![0x00];
        let cursor = ObjectStreamCursor::new(&buf, &[]);
        assert!(cursor.object_size_at(0).is_none());
    }

    #[test]
    fn object_slice_by_handle_round_trips() {
        let body = [0x12, 0x34, 0x56, 0x78];
        let (buf, offset) = synth_object(8, &body);
        let offsets = vec![HandleMapEntry {
            handle: Handle::new(0x2A),
            offset,
        }];
        let cursor = ObjectStreamCursor::new(&buf, &offsets);
        let slice = cursor
            .object_slice_by_handle(Handle::new(0x2A))
            .expect("handle present → slice returned");
        // Slice covers [MS(2 bytes) + body(4 bytes)] = 6 bytes.
        assert_eq!(slice.len(), 6);
        assert_eq!(&slice[0..2], &[0x04, 0x00]);
        assert_eq!(&slice[2..], &body);
    }

    #[test]
    fn object_slice_by_handle_returns_none_for_unknown_handle() {
        let (buf, offset) = synth_object(0, &[0xAA, 0xBB]);
        let offsets = vec![HandleMapEntry {
            handle: Handle::new(5),
            offset,
        }];
        let cursor = ObjectStreamCursor::new(&buf, &offsets);
        assert!(cursor.object_slice_by_handle(Handle::new(6)).is_none());
    }

    #[test]
    fn object_slice_by_handle_skips_body_past_eof() {
        // Claim body size 100 but only provide 4 body bytes.
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0x64, 0x00]); // MS = 100
        buf.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let offsets = vec![HandleMapEntry {
            handle: Handle::new(1),
            offset: 0,
        }];
        let cursor = ObjectStreamCursor::new(&buf, &offsets);
        assert!(cursor.object_slice_by_handle(Handle::new(1)).is_none());
    }

    #[test]
    fn object_slice_by_handle_skips_implausibly_large_body() {
        // Two-chunk MS that decodes to ≈ 64 MiB, well over the 16 MiB
        // cap imposed by MAX_OBJECT_BODY_BYTES:
        //   word1 = 0xFFFF → continuation + 0x7FFF low bits,
        //   word2 = 0x0800 → terminator + payload 0x0800, contributing
        //   0x0800 << 15 = 0x0400_0000.
        let buf = [0xFFu8, 0xFF, 0x00, 0x08];
        let offsets = vec![HandleMapEntry {
            handle: Handle::new(1),
            offset: 0,
        }];
        let cursor = ObjectStreamCursor::new(&buf, &offsets);
        assert!(cursor.object_size_at(0).is_none());
        assert!(cursor.object_slice_by_handle(Handle::new(1)).is_none());
    }
}
