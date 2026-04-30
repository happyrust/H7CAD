//! AC1018 (AutoCAD R2004) page-map decoder, R46-C scope.
//!
//! Layout reference: ACadSharp `src/ACadSharp/IO/DWG/DwgReader.cs`
//! `readFileHeaderAC18` (L529..L569) and `getPageHeaderData`
//! (L649..L670). The on-disk AC1018 page map sits at the effective
//! `page_map_address = raw + 0x100` decoded by R46-A; it carries a
//! 20-byte system page header followed by an LZ77-compressed record
//! stream. Each record is `(number, size)` plus 16 bytes of gap
//! padding when `number < 0`.
//!
//! R46-C wires R46-A (encrypted metadata, for the page_map_address
//! pointer) and R46-B (DWG-LZ77 decompressor) together. It does
//! **not** modify [`crate::file_header::section_count_offset`]: that
//! path stays `UnsupportedHeaderLayout` until R46-E lights up the
//! end-to-end `read_dwg` pipeline.

use crate::lz77_ac18::{decompress_ac18_lz77, Lz77DecodeError};
use std::fmt;

/// Length of the system page header (bytes), 0x14 = 20.
pub const SYSTEM_PAGE_HEADER_LEN: usize = 0x14;

/// `section_type` value identifying a page-map system page.
pub const PAGE_MAP_SECTION_TYPE: u32 = 0x4163_0E3B;

/// `section_type` value identifying a section-descriptor-map system
/// page (used by R46-D).
pub const SECTION_MAP_SECTION_TYPE: u32 = 0x4163_003B;

/// Compression type indicating DWG-LZ77 encoding (the only mode
/// real-world AC1018 system pages use).
pub const COMPRESSION_TYPE_LZ77: u32 = 0x02;

/// Defensive cap on `decompressed_size`. A legit AC1018 page map /
/// section descriptor map sits well under 1 MiB; 16 MiB is a generous
/// upper bound that protects against malformed / adversarial input
/// trying to balloon the LZ77 output buffer.
pub const MAX_DECOMPRESSED_SIZE: u32 = 16 * 1024 * 1024;

/// Initial cumulative seeker. ACadSharp uses `0x100` as the base from
/// which valid records derive their file offsets (DwgReader.cs L538).
pub const INITIAL_SEEKER: i64 = 0x100;

/// Raw-byte view of a 20-byte system page header preceding any LZ77
/// system page (page map / section descriptor map) on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemPageHeader {
    /// `section_type` magic, see [`PAGE_MAP_SECTION_TYPE`] /
    /// [`SECTION_MAP_SECTION_TYPE`].
    pub section_type: u32,
    /// Decompressed size of the LZ77-encoded payload that follows.
    pub decompressed_size: u32,
    /// Compressed size of the LZ77-encoded payload that follows.
    pub compressed_size: u32,
    /// Compression type. The only real-world value is
    /// [`COMPRESSION_TYPE_LZ77`] = 2.
    pub compression_type: u32,
    /// Page checksum (not validated by R46-C; ACadSharp also does not
    /// validate on read).
    pub checksum: u32,
}

/// Decoded AC1018 page-map record. `number < 0` indicates a gap (an
/// unused range that nonetheless occupies `size` bytes in the file).
/// `seeker` is the cumulative byte offset at which a `number >= 0`
/// page begins; for `number < 0` it carries the cumulative offset at
/// the start of the gap range but should not be used for lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageMapRecord {
    /// Page number; `>= 0` is a valid page, `< 0` is a gap.
    pub number: i32,
    /// Page size in bytes (always non-negative on the wire even for
    /// gaps; ACadSharp keeps it as `i32` and we follow suit).
    pub size: i32,
    /// Cumulative on-disk offset, computed by the parser as
    /// `0x100 + Σ size` over the records that came before. Only
    /// meaningful when `number >= 0`.
    pub seeker: i64,
}

/// Parsed AC1018 page map. The records are stored in stream order,
/// preserving gap entries so callers can audit the on-disk layout.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageMap {
    /// Records in the order they appeared in the LZ77-decompressed
    /// stream.
    pub records: Vec<PageMapRecord>,
}

impl PageMap {
    /// Look up a single valid record by page number. Returns `None`
    /// when the number is missing or refers to a gap entry.
    pub fn lookup(&self, number: i32) -> Option<&PageMapRecord> {
        if number < 0 {
            return None;
        }
        self.records
            .iter()
            .find(|record| record.number == number && record.number >= 0)
    }

    /// Iterate over all valid (non-gap) records in stream order.
    pub fn valid_records(&self) -> impl Iterator<Item = &PageMapRecord> {
        self.records.iter().filter(|record| record.number >= 0)
    }
}

/// Errors that can surface while decoding an AC1018 page map.
///
/// Kept independent of [`crate::DwgReadError`]: like R46-B, the
/// page-map decoder is a self-contained brick that R46-E will wrap
/// into the top-level error type when it wires the file-header layer
/// into `read_dwg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageMapDecodeError {
    /// `bytes` was too small to cover `[offset, offset + needed)`.
    TruncatedInput {
        /// Offset where the truncated read started.
        offset: usize,
        /// Number of bytes the parser needed at `offset`.
        expected_at_least: usize,
    },
    /// The 20-byte system page header carried a `section_type` value
    /// other than the one expected for the operation.
    InvalidPageType {
        /// The value that was found in the header.
        actual: u32,
        /// Offset of the offending header inside `bytes`.
        offset: usize,
    },
    /// The 20-byte system page header carried a `compression_type`
    /// that R46-C does not (yet) handle. Today only
    /// [`COMPRESSION_TYPE_LZ77`] = 2 is supported.
    UnsupportedCompressionType {
        /// The value that was found in the header.
        actual: u32,
        /// Offset of the offending header inside `bytes`.
        offset: usize,
    },
    /// `decompressed_size` declared by the system page header
    /// exceeded the defensive cap [`MAX_DECOMPRESSED_SIZE`].
    OversizedDecompressedSize {
        /// The header-declared decompressed size.
        value: u32,
    },
    /// LZ77 decompression failed; carries the underlying R46-B error.
    Lz77 {
        /// The underlying R46-B decoder error.
        source: Lz77DecodeError,
    },
    /// The decompressed record stream ended in the middle of a record
    /// (either the leading 8 bytes or the 16-byte gap padding).
    TruncatedRecordStream {
        /// Total length of the decompressed buffer.
        decompressed_len: usize,
        /// Cursor position at which the parser hit EOF.
        cursor: usize,
    },
}

impl fmt::Display for PageMapDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedInput {
                offset,
                expected_at_least,
            } => write!(
                f,
                "AC1018 page map truncated at offset {offset}: expected at least {expected_at_least} bytes"
            ),
            Self::InvalidPageType { actual, offset } => write!(
                f,
                "AC1018 page map: invalid section_type 0x{actual:08X} at offset {offset}"
            ),
            Self::UnsupportedCompressionType { actual, offset } => write!(
                f,
                "AC1018 page map: unsupported compression_type {actual} at offset {offset}"
            ),
            Self::OversizedDecompressedSize { value } => write!(
                f,
                "AC1018 page map: declared decompressed_size {value} exceeds the 16 MiB sanity cap"
            ),
            Self::Lz77 { source } => write!(f, "AC1018 page map LZ77 decode failed: {source}"),
            Self::TruncatedRecordStream {
                decompressed_len,
                cursor,
            } => write!(
                f,
                "AC1018 page map: decompressed record stream truncated at cursor {cursor} (total {decompressed_len} bytes)"
            ),
        }
    }
}

impl std::error::Error for PageMapDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lz77 { source } => Some(source),
            _ => None,
        }
    }
}

impl From<Lz77DecodeError> for PageMapDecodeError {
    fn from(value: Lz77DecodeError) -> Self {
        Self::Lz77 { source: value }
    }
}

/// Read the 20-byte system page header at `bytes[offset..]`.
///
/// # Errors
///
/// [`PageMapDecodeError::TruncatedInput`] when `bytes` does not cover
/// `[offset, offset + 0x14)`.
pub fn parse_system_page_header(
    bytes: &[u8],
    offset: usize,
) -> Result<SystemPageHeader, PageMapDecodeError> {
    let end = offset
        .checked_add(SYSTEM_PAGE_HEADER_LEN)
        .ok_or(PageMapDecodeError::TruncatedInput {
            offset,
            expected_at_least: SYSTEM_PAGE_HEADER_LEN,
        })?;
    let slice = bytes
        .get(offset..end)
        .ok_or(PageMapDecodeError::TruncatedInput {
            offset,
            expected_at_least: SYSTEM_PAGE_HEADER_LEN,
        })?;
    Ok(SystemPageHeader {
        section_type: read_u32_le(slice, 0x00),
        decompressed_size: read_u32_le(slice, 0x04),
        compressed_size: read_u32_le(slice, 0x08),
        compression_type: read_u32_le(slice, 0x0C),
        checksum: read_u32_le(slice, 0x10),
    })
}

/// Decode the AC1018 page map at `bytes[page_map_offset..]`.
///
/// The function reads the 20-byte system page header, validates the
/// `section_type` against [`PAGE_MAP_SECTION_TYPE`], LZ77-decompresses
/// the payload via R46-B, and walks the record stream until the
/// decompressed buffer is exhausted.
///
/// # Errors
///
/// See [`PageMapDecodeError`] for the full list. The most common
/// failure on truncated / corrupt input is
/// [`PageMapDecodeError::TruncatedInput`] or
/// [`PageMapDecodeError::TruncatedRecordStream`]; the most common
/// failure on a misaligned `page_map_offset` is
/// [`PageMapDecodeError::InvalidPageType`].
pub fn parse_ac1018_page_map(
    bytes: &[u8],
    page_map_offset: usize,
) -> Result<PageMap, PageMapDecodeError> {
    let header = parse_system_page_header(bytes, page_map_offset)?;
    if header.section_type != PAGE_MAP_SECTION_TYPE {
        return Err(PageMapDecodeError::InvalidPageType {
            actual: header.section_type,
            offset: page_map_offset,
        });
    }
    if header.compression_type != COMPRESSION_TYPE_LZ77 {
        return Err(PageMapDecodeError::UnsupportedCompressionType {
            actual: header.compression_type,
            offset: page_map_offset,
        });
    }
    if header.decompressed_size > MAX_DECOMPRESSED_SIZE {
        return Err(PageMapDecodeError::OversizedDecompressedSize {
            value: header.decompressed_size,
        });
    }

    let payload_start = page_map_offset
        .checked_add(SYSTEM_PAGE_HEADER_LEN)
        .ok_or(PageMapDecodeError::TruncatedInput {
            offset: page_map_offset,
            expected_at_least: SYSTEM_PAGE_HEADER_LEN,
        })?;
    let compressed_len = header.compressed_size as usize;
    let payload_end =
        payload_start
            .checked_add(compressed_len)
            .ok_or(PageMapDecodeError::TruncatedInput {
                offset: payload_start,
                expected_at_least: compressed_len,
            })?;
    let compressed = bytes
        .get(payload_start..payload_end)
        .ok_or(PageMapDecodeError::TruncatedInput {
            offset: payload_start,
            expected_at_least: compressed_len,
        })?;

    let decompressed = decompress_ac18_lz77(compressed, header.decompressed_size as usize)?;
    parse_records(&decompressed)
}

/// Decode the record stream of a decompressed page-map payload.
fn parse_records(decompressed: &[u8]) -> Result<PageMap, PageMapDecodeError> {
    let mut records = Vec::new();
    let mut cursor = 0usize;
    let mut total: i64 = INITIAL_SEEKER;

    while cursor < decompressed.len() {
        // Each record needs at least 8 bytes (number + size).
        let head = decompressed.get(cursor..cursor + 8).ok_or(
            PageMapDecodeError::TruncatedRecordStream {
                decompressed_len: decompressed.len(),
                cursor,
            },
        )?;
        let number = read_i32_le(head, 0);
        let size = read_i32_le(head, 4);
        cursor += 8;

        let seeker = total;
        if number < 0 {
            // Gap: 16 more bytes (Parent / Left / Right / 0x00) follow
            // and are discarded. ACadSharp DwgReader.cs L557..L564.
            let gap_end =
                cursor
                    .checked_add(16)
                    .ok_or(PageMapDecodeError::TruncatedRecordStream {
                        decompressed_len: decompressed.len(),
                        cursor,
                    })?;
            if gap_end > decompressed.len() {
                return Err(PageMapDecodeError::TruncatedRecordStream {
                    decompressed_len: decompressed.len(),
                    cursor,
                });
            }
            cursor = gap_end;
        }

        records.push(PageMapRecord {
            number,
            size,
            seeker,
        });

        // ACadSharp adds `size` to `total` for both valid and gap
        // records, treating gaps as on-disk space too.
        total = total.saturating_add(i64::from(size));
    }

    Ok(PageMap { records })
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_i32_le(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 20-byte system page header round-trips: build it field-by-field
    /// little-endian and parse it back.
    #[test]
    fn parse_system_page_header_decodes_synthetic_header() {
        let mut bytes = vec![0u8; SYSTEM_PAGE_HEADER_LEN];
        bytes[0x00..0x04].copy_from_slice(&PAGE_MAP_SECTION_TYPE.to_le_bytes());
        bytes[0x04..0x08].copy_from_slice(&0x0000_1000u32.to_le_bytes());
        bytes[0x08..0x0C].copy_from_slice(&0x0000_0200u32.to_le_bytes());
        bytes[0x0C..0x10].copy_from_slice(&COMPRESSION_TYPE_LZ77.to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());

        let header = parse_system_page_header(&bytes, 0).expect("synthetic header parses");
        assert_eq!(header.section_type, PAGE_MAP_SECTION_TYPE);
        assert_eq!(header.decompressed_size, 0x1000);
        assert_eq!(header.compressed_size, 0x200);
        assert_eq!(header.compression_type, COMPRESSION_TYPE_LZ77);
        assert_eq!(header.checksum, 0xDEAD_BEEF);
    }

    #[test]
    fn parse_system_page_header_rejects_truncated_input() {
        let bytes = vec![0u8; SYSTEM_PAGE_HEADER_LEN - 1];
        let err = parse_system_page_header(&bytes, 0).unwrap_err();
        assert_eq!(
            err,
            PageMapDecodeError::TruncatedInput {
                offset: 0,
                expected_at_least: SYSTEM_PAGE_HEADER_LEN,
            }
        );
    }

    /// End-to-end synthetic page map with two valid records,
    /// LZ77-encoded via the leading-literal preamble (R46-B's
    /// `(opcode1 & 0xF0) == 0` path).
    ///
    /// Decompressed payload = 16 bytes:
    ///   Number=1, Size=0x100, Number=2, Size=0x200
    /// LZ77 wrapper:
    ///   opcode1 = 0x0D (literalCount = 13, +3 = 16 literals)
    ///   ...16 raw bytes...
    ///   0x11 terminator
    /// → compressed_size = 18 bytes total.
    #[test]
    fn parse_ac1018_page_map_decodes_two_valid_records() {
        let raw_records: [u8; 16] = [
            0x01, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, // number=1 size=0x100
            0x02, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, // number=2 size=0x200
        ];
        let mut compressed = Vec::with_capacity(18);
        compressed.push(0x0D); // leading-literal preamble: 13 + 3 = 16 literals
        compressed.extend_from_slice(&raw_records);
        compressed.push(0x11); // terminator

        let mut bytes = build_system_page(
            PAGE_MAP_SECTION_TYPE,
            raw_records.len() as u32,
            compressed.len() as u32,
            COMPRESSION_TYPE_LZ77,
            0x1234_5678,
        );
        bytes.extend_from_slice(&compressed);

        let map = parse_ac1018_page_map(&bytes, 0).expect("synthetic page map decodes");
        assert_eq!(map.records.len(), 2);
        assert_eq!(
            map.records[0],
            PageMapRecord {
                number: 1,
                size: 0x100,
                seeker: 0x100,
            }
        );
        assert_eq!(
            map.records[1],
            PageMapRecord {
                number: 2,
                size: 0x200,
                seeker: 0x200, // 0x100 + 0x100 (record 0 size)
            }
        );
        assert_eq!(map.lookup(1).map(|r| r.seeker), Some(0x100));
        assert_eq!(map.lookup(2).map(|r| r.seeker), Some(0x200));
        assert!(map.lookup(99).is_none());
        assert_eq!(map.valid_records().count(), 2);
    }

    /// Negative-record path: a `number == -1` record carries 16 extra
    /// bytes (Parent/Left/Right/0x00) and its `size` still contributes
    /// to the cumulative seeker. Layout:
    ///   record 0: number=1 size=0x100   (8 bytes)
    ///   record 1: number=-1 size=0x200  (8 + 16 = 24 bytes)
    ///   record 2: number=3 size=0x300   (8 bytes)
    /// Total decompressed = 40 bytes.
    /// LZ77 wrapper:
    ///   opcode1 = 0x00 → literal_count chain
    ///   chain byte 0x16 → lowbits = 0x0F + 0x16 = 0x25 = 37
    ///   +3 = 40 literals
    ///   ...40 raw bytes...
    ///   0x11 terminator
    #[test]
    fn parse_ac1018_page_map_handles_negative_record_with_gap_padding() {
        let mut raw = Vec::with_capacity(40);
        raw.extend_from_slice(&1i32.to_le_bytes());
        raw.extend_from_slice(&0x100i32.to_le_bytes());

        raw.extend_from_slice(&(-1i32).to_le_bytes());
        raw.extend_from_slice(&0x200i32.to_le_bytes());
        raw.extend_from_slice(&0i32.to_le_bytes()); // gap.parent
        raw.extend_from_slice(&1i32.to_le_bytes()); // gap.left
        raw.extend_from_slice(&2i32.to_le_bytes()); // gap.right
        raw.extend_from_slice(&0i32.to_le_bytes()); // gap.0x00

        raw.extend_from_slice(&3i32.to_le_bytes());
        raw.extend_from_slice(&0x300i32.to_le_bytes());

        assert_eq!(raw.len(), 40);

        let mut compressed = Vec::with_capacity(43);
        compressed.push(0x00); // leading-literal preamble entering literal_count chain
        compressed.push(0x16); // chain byte → lowbits = 0x0F + 0x16 = 37, +3 = 40
        compressed.extend_from_slice(&raw);
        compressed.push(0x11);

        let mut bytes = build_system_page(
            PAGE_MAP_SECTION_TYPE,
            raw.len() as u32,
            compressed.len() as u32,
            COMPRESSION_TYPE_LZ77,
            0,
        );
        bytes.extend_from_slice(&compressed);

        let map = parse_ac1018_page_map(&bytes, 0).expect("synthetic gap stream decodes");
        assert_eq!(map.records.len(), 3);
        assert_eq!(
            map.records[0],
            PageMapRecord {
                number: 1,
                size: 0x100,
                seeker: 0x100,
            }
        );
        assert_eq!(
            map.records[1],
            PageMapRecord {
                number: -1,
                size: 0x200,
                seeker: 0x200, // 0x100 + 0x100
            }
        );
        assert_eq!(
            map.records[2],
            PageMapRecord {
                number: 3,
                size: 0x300,
                seeker: 0x400, // 0x100 + 0x100 + 0x200
            }
        );
        assert!(
            map.lookup(-1).is_none(),
            "lookup must reject negative numbers (they refer to gaps)"
        );
        assert_eq!(map.lookup(3).map(|r| r.seeker), Some(0x400));
        assert_eq!(map.valid_records().count(), 2);
    }

    #[test]
    fn parse_ac1018_page_map_rejects_invalid_section_type() {
        let bytes = build_system_page(
            0xDEAD_BEEF, // Wrong section_type
            0,
            0,
            COMPRESSION_TYPE_LZ77,
            0,
        );
        let err = parse_ac1018_page_map(&bytes, 0).unwrap_err();
        assert_eq!(
            err,
            PageMapDecodeError::InvalidPageType {
                actual: 0xDEAD_BEEF,
                offset: 0,
            }
        );
    }

    #[test]
    fn parse_ac1018_page_map_rejects_unsupported_compression() {
        let bytes = build_system_page(
            PAGE_MAP_SECTION_TYPE,
            0,
            0,
            0x01, // Not LZ77
            0,
        );
        let err = parse_ac1018_page_map(&bytes, 0).unwrap_err();
        assert_eq!(
            err,
            PageMapDecodeError::UnsupportedCompressionType {
                actual: 0x01,
                offset: 0,
            }
        );
    }

    #[test]
    fn parse_ac1018_page_map_rejects_oversized_decompressed_size() {
        let oversized = MAX_DECOMPRESSED_SIZE + 1;
        let bytes = build_system_page(
            PAGE_MAP_SECTION_TYPE,
            oversized,
            0,
            COMPRESSION_TYPE_LZ77,
            0,
        );
        let err = parse_ac1018_page_map(&bytes, 0).unwrap_err();
        assert_eq!(
            err,
            PageMapDecodeError::OversizedDecompressedSize { value: oversized }
        );
    }

    /// Truncated record stream: declared decompressed_size says 12
    /// bytes (= 1.5 records, illegal) but the LZ77 stream produces a
    /// stream that ends mid-record. The parser must surface
    /// [`PageMapDecodeError::TruncatedRecordStream`] rather than
    /// returning a half-decoded record.
    #[test]
    fn parse_ac1018_page_map_rejects_truncated_record_stream() {
        let raw: Vec<u8> = (0u8..12).collect(); // 12 bytes: 1.5 records
        let mut compressed = Vec::with_capacity(15);
        compressed.push(0x09); // 9 + 3 = 12 literals
        compressed.extend_from_slice(&raw);
        compressed.push(0x11);

        let mut bytes = build_system_page(
            PAGE_MAP_SECTION_TYPE,
            raw.len() as u32,
            compressed.len() as u32,
            COMPRESSION_TYPE_LZ77,
            0,
        );
        bytes.extend_from_slice(&compressed);

        let err = parse_ac1018_page_map(&bytes, 0).unwrap_err();
        assert_eq!(
            err,
            PageMapDecodeError::TruncatedRecordStream {
                decompressed_len: 12,
                cursor: 8,
            }
        );
    }

    #[test]
    fn page_map_decode_error_display_strings_include_diagnostics() {
        let truncated = format!(
            "{}",
            PageMapDecodeError::TruncatedInput {
                offset: 0x10,
                expected_at_least: 20
            }
        );
        assert!(truncated.contains("offset 16"));
        assert!(truncated.contains("at least 20"));

        let invalid = format!(
            "{}",
            PageMapDecodeError::InvalidPageType {
                actual: 0xDEAD_BEEF,
                offset: 0
            }
        );
        assert!(invalid.contains("0xDEADBEEF"));

        let unsupported = format!(
            "{}",
            PageMapDecodeError::UnsupportedCompressionType { actual: 1, offset: 0 }
        );
        assert!(unsupported.contains("compression_type 1"));

        let oversized = format!(
            "{}",
            PageMapDecodeError::OversizedDecompressedSize { value: 99 }
        );
        assert!(oversized.contains("99"));

        let lz77 = format!(
            "{}",
            PageMapDecodeError::Lz77 {
                source: Lz77DecodeError::TruncatedInput
            }
        );
        assert!(lz77.contains("LZ77"));

        let stream = format!(
            "{}",
            PageMapDecodeError::TruncatedRecordStream {
                decompressed_len: 30,
                cursor: 28
            }
        );
        assert!(stream.contains("cursor 28"));
        assert!(stream.contains("total 30"));
    }

    /// Build a 20-byte system page header followed by no payload.
    fn build_system_page(
        section_type: u32,
        decompressed_size: u32,
        compressed_size: u32,
        compression_type: u32,
        checksum: u32,
    ) -> Vec<u8> {
        let mut bytes = vec![0u8; SYSTEM_PAGE_HEADER_LEN];
        bytes[0x00..0x04].copy_from_slice(&section_type.to_le_bytes());
        bytes[0x04..0x08].copy_from_slice(&decompressed_size.to_le_bytes());
        bytes[0x08..0x0C].copy_from_slice(&compressed_size.to_le_bytes());
        bytes[0x0C..0x10].copy_from_slice(&compression_type.to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&checksum.to_le_bytes());
        bytes
    }
}
