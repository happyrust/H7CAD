//! AC1018 (AutoCAD R2004) section descriptors map decoder, R46-D scope.
//!
//! Layout reference: ACadSharp `src/ACadSharp/IO/DWG/DwgReader.cs`
//! `readFileHeaderAC18` (L571..L646). The on-disk AC1018 section
//! descriptor map sits at the seeker resolved by R46-C
//! (`page_map.lookup(section_map_id).seeker`); it carries a 20-byte
//! system page header followed by an LZ77-compressed payload that
//! itself contains a 20-byte header and `num_descriptions` variable-
//! sized descriptors. Each descriptor is 96 bytes plus 16 bytes per
//! local section page.
//!
//! R46-D wires R46-A/B/C together: it resolves the section map page
//! seeker via R46-C's [`PageMap`], reuses R46-C's
//! [`crate::page_map_ac1018::parse_system_page_header`] +
//! [`crate::lz77_ac18::decompress_ac18_lz77`] to obtain the
//! decompressed descriptor stream, and then walks fixed-layout
//! records. It does **not** modify
//! [`crate::file_header::section_count_offset`]: that path stays
//! `UnsupportedHeaderLayout` until R46-E lights up the end-to-end
//! `read_dwg` pipeline.

use std::collections::BTreeMap;
use std::fmt;

use crate::lz77_ac18::{decompress_ac18_lz77, Lz77DecodeError};
use crate::page_map_ac1018::{
    parse_system_page_header, PageMap, PageMapDecodeError, COMPRESSION_TYPE_LZ77,
    MAX_DECOMPRESSED_SIZE, SECTION_MAP_SECTION_TYPE, SYSTEM_PAGE_HEADER_LEN,
};

/// Length of the in-payload SectionDescriptorMap header (bytes), 20.
pub const SECTION_DESCRIPTOR_MAP_HEADER_LEN: usize = 20;

/// On-the-wire size of a single SectionDescriptor record (excluding
/// its trailing local-section pages), 96 bytes.
pub const SECTION_DESCRIPTOR_LEN: usize = 96;

/// On-the-wire size of a single LocalSectionMap record, 16 bytes.
pub const LOCAL_SECTION_MAP_LEN: usize = 16;

/// Length of the embedded section-name field, 64 bytes (null-terminated
/// Windows-1252 / ASCII string).
pub const SECTION_NAME_LEN: usize = 64;

/// Decoded AC1018 local section page entry. ACadSharp keeps these
/// inside `DwgSectionDescriptor.LocalSections`; we mirror that here
/// with `seeker` resolved via the [`PageMap`] handed in by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalSectionMap {
    /// Index into the AC1018 page map; resolves to the on-disk byte
    /// offset where the page's compressed payload lives.
    pub page_number: i32,
    /// Compressed size of this page on disk. ACadSharp reads this as
    /// `Int` (i32) and casts to `ulong`; we widen to `u64` to keep
    /// arithmetic with `offset` consistent.
    pub compressed_size: u64,
    /// Logical offset of the page within its parent section's
    /// reassembled payload.
    pub offset: u64,
    /// Decompressed size for this page. Initially inherited from the
    /// parent descriptor (`descriptor.decompressed_size`); the parser
    /// applies the ACadSharp tail-page correction
    /// (`size_left = compressed_size % decompressed_size`) so the
    /// last page of a section reports its true uncompressed length.
    pub decompressed_size: u64,
    /// File offset (bytes) at which the page's system page header
    /// begins, taken from `page_map.lookup(page_number).seeker`.
    pub seeker: i64,
}

/// Decoded AC1018 SectionDescriptor. Mirrors the read-path subset of
/// ACadSharp's `DwgSectionDescriptor`: we keep only the fields R46-E
/// needs to drive `build_pending_document` (sizes, compression mode,
/// section id, encryption flag, name, and the local-section list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionDescriptor {
    /// Total compressed size of the section (sum of all pages).
    pub compressed_size: u64,
    /// Number of pages stored on disk for this section.
    pub page_count: i32,
    /// Maximum decompressed size of a single page (normally 0x7400).
    pub decompressed_size: u64,
    /// Compression mode: `1` = no, `2` = yes (LZ77). ACadSharp
    /// throws on values outside `{1, 2}`; we keep the raw value so
    /// callers can decide on policy.
    pub compressed_code: i32,
    /// Section id (ACadSharp `int`; section 0 is the conventional
    /// "empty" section, the rest count down from `num_sections - 1`).
    pub section_id: i32,
    /// Encryption flag: `0` = no, `1` = yes, `2` = unknown.
    pub encrypted: i32,
    /// Section name (e.g. `"AcDb:Handles"`, `"AcDb:AcDbObjects"`).
    /// Decoded from the 64-byte trailing field with lossy UTF-8
    /// fallback; trimmed at the first NUL byte to match ACadSharp's
    /// `Split('\0')[0]` behaviour.
    pub name: String,
    /// Page-by-page layout records, in stream order.
    pub local_sections: Vec<LocalSectionMap>,
}

/// Parsed AC1018 SectionDescriptorMap: the catalogue of sections plus
/// each section's per-page LocalSectionMap list, indexed by name for
/// quick lookup.
///
/// The map preserves stream order via a `BTreeMap` (stable iteration
/// by name) plus a separate `order` vector that records insertion
/// order so callers can replay the original on-disk layout if needed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionDescriptorMap {
    /// Name → descriptor lookup table.
    pub descriptors: BTreeMap<String, SectionDescriptor>,
    /// Names in the order they appeared on disk.
    pub order: Vec<String>,
}

impl SectionDescriptorMap {
    /// Look up a single descriptor by name.
    pub fn lookup(&self, name: &str) -> Option<&SectionDescriptor> {
        self.descriptors.get(name)
    }

    /// Iterate descriptors in on-disk order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &SectionDescriptor)> {
        self.order
            .iter()
            .filter_map(|name| self.descriptors.get_key_value(name))
    }

    /// Number of descriptors.
    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    /// Convenience emptiness check.
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }
}

/// Errors that can surface while decoding an AC1018 section descriptor
/// map.
///
/// Kept independent of [`crate::DwgReadError`]: like R46-B/C, the
/// section-map decoder is a self-contained brick that R46-E will wrap
/// into the top-level error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionMapDecodeError {
    /// PageMap lookup or downstream PageMap decode error surfaced
    /// while reading the section map's system page.
    PageMap(PageMapDecodeError),
    /// `bytes` was too small to cover the system page header at the
    /// resolved seeker.
    TruncatedHeader {
        /// Offset where the truncated read started.
        offset: usize,
        /// Number of bytes the parser needed at `offset`.
        expected_at_least: usize,
    },
    /// The 20-byte system page header carried a `section_type` value
    /// other than [`SECTION_MAP_SECTION_TYPE`].
    InvalidPageType {
        /// The value that was found in the header.
        actual: u32,
        /// Offset of the offending header inside `bytes`.
        offset: usize,
    },
    /// The 20-byte system page header carried a `compression_type`
    /// other than [`COMPRESSION_TYPE_LZ77`].
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
    /// The decompressed buffer ended in the middle of the in-payload
    /// SectionDescriptorMap header.
    TruncatedHeaderStream {
        /// Total length of the decompressed buffer.
        decompressed_len: usize,
        /// Cursor position at which the parser hit EOF.
        cursor: usize,
    },
    /// The decompressed buffer ended in the middle of a descriptor
    /// (either the 96-byte fixed header or one of its 16-byte local
    /// section entries).
    TruncatedDescriptorStream {
        /// Total length of the decompressed buffer.
        decompressed_len: usize,
        /// Cursor position at which the parser hit EOF.
        cursor: usize,
        /// Index of the descriptor being decoded.
        descriptor_index: usize,
    },
    /// A LocalSectionMap referenced a `page_number` that the
    /// [`PageMap`] could not resolve.
    MissingPageInPageMap {
        /// The offending page number.
        page_number: i32,
        /// Index of the descriptor that referenced it.
        descriptor_index: usize,
    },
    /// An `unknown_04` / `unknown_08` / `unknown_0c` constant in the
    /// SectionDescriptorMap header drifted from the ODA spec value.
    /// Kept as a *warning surface* (R46-D does not return this today;
    /// reserved for future strict mode).
    #[doc(hidden)]
    UnexpectedHeaderConstant {
        /// Field offset (0x04 / 0x08 / 0x0C) that drifted.
        field_offset: usize,
        /// Expected value per the ODA spec.
        expected: u32,
        /// Actual value seen in the stream.
        actual: u32,
    },
}

impl fmt::Display for SectionMapDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PageMap(source) => write!(f, "AC1018 section map: page map decode failed: {source}"),
            Self::TruncatedHeader {
                offset,
                expected_at_least,
            } => write!(
                f,
                "AC1018 section map system page header truncated at offset {offset}: expected at least {expected_at_least} bytes"
            ),
            Self::InvalidPageType { actual, offset } => write!(
                f,
                "AC1018 section map: invalid section_type 0x{actual:08X} at offset {offset}"
            ),
            Self::UnsupportedCompressionType { actual, offset } => write!(
                f,
                "AC1018 section map: unsupported compression_type {actual} at offset {offset}"
            ),
            Self::OversizedDecompressedSize { value } => write!(
                f,
                "AC1018 section map: declared decompressed_size {value} exceeds the 16 MiB sanity cap"
            ),
            Self::Lz77 { source } => write!(f, "AC1018 section map LZ77 decode failed: {source}"),
            Self::TruncatedHeaderStream {
                decompressed_len,
                cursor,
            } => write!(
                f,
                "AC1018 section map: descriptor map header truncated at cursor {cursor} (total {decompressed_len} bytes)"
            ),
            Self::TruncatedDescriptorStream {
                decompressed_len,
                cursor,
                descriptor_index,
            } => write!(
                f,
                "AC1018 section map: descriptor #{descriptor_index} truncated at cursor {cursor} (total {decompressed_len} bytes)"
            ),
            Self::MissingPageInPageMap {
                page_number,
                descriptor_index,
            } => write!(
                f,
                "AC1018 section map: descriptor #{descriptor_index} references page_number {page_number} which is not present in the page map"
            ),
            Self::UnexpectedHeaderConstant {
                field_offset,
                expected,
                actual,
            } => write!(
                f,
                "AC1018 section map header constant at offset 0x{field_offset:02X}: expected 0x{expected:08X}, got 0x{actual:08X}"
            ),
        }
    }
}

impl std::error::Error for SectionMapDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PageMap(source) => Some(source),
            Self::Lz77 { source } => Some(source),
            _ => None,
        }
    }
}

impl From<PageMapDecodeError> for SectionMapDecodeError {
    fn from(value: PageMapDecodeError) -> Self {
        Self::PageMap(value)
    }
}

impl From<Lz77DecodeError> for SectionMapDecodeError {
    fn from(value: Lz77DecodeError) -> Self {
        Self::Lz77 { source: value }
    }
}

/// End-to-end AC1018 section descriptor map decoder.
///
/// Resolves the section map page seeker via `page_map.lookup(
/// section_map_id)`, reads the 20-byte system page header at that
/// offset, validates the section_type magic, LZ77-decompresses the
/// payload, and walks the descriptor stream until `num_descriptions`
/// records are consumed.
///
/// # Errors
///
/// See [`SectionMapDecodeError`] for the full list. Common failures:
///
/// - [`SectionMapDecodeError::PageMap`] when `section_map_id` is not
///   present in `page_map`.
/// - [`SectionMapDecodeError::InvalidPageType`] when the resolved
///   seeker does not point at a valid section map system page.
/// - [`SectionMapDecodeError::TruncatedDescriptorStream`] when the
///   declared decompressed_size cannot cover the descriptor records.
/// - [`SectionMapDecodeError::MissingPageInPageMap`] when a
///   LocalSectionMap references a page that the page map cannot
///   resolve.
pub fn parse_ac1018_section_map(
    bytes: &[u8],
    page_map: &PageMap,
    section_map_id: u32,
) -> Result<SectionDescriptorMap, SectionMapDecodeError> {
    // 1) Resolve the on-disk seeker for the section map page via R46-C.
    let section_map_record = page_map.lookup(section_map_id as i32).ok_or(
        SectionMapDecodeError::PageMap(PageMapDecodeError::TruncatedRecordStream {
            decompressed_len: 0,
            cursor: 0,
        }),
    )?;
    let section_map_offset: usize = section_map_record
        .seeker
        .try_into()
        .map_err(|_| SectionMapDecodeError::TruncatedHeader {
            offset: 0,
            expected_at_least: SYSTEM_PAGE_HEADER_LEN,
        })?;

    // 2) Read the 20-byte system page header at that seeker.
    let header = parse_system_page_header(bytes, section_map_offset).map_err(|err| match err {
        PageMapDecodeError::TruncatedInput {
            offset,
            expected_at_least,
        } => SectionMapDecodeError::TruncatedHeader {
            offset,
            expected_at_least,
        },
        other => SectionMapDecodeError::PageMap(other),
    })?;
    if header.section_type != SECTION_MAP_SECTION_TYPE {
        return Err(SectionMapDecodeError::InvalidPageType {
            actual: header.section_type,
            offset: section_map_offset,
        });
    }
    if header.compression_type != COMPRESSION_TYPE_LZ77 {
        return Err(SectionMapDecodeError::UnsupportedCompressionType {
            actual: header.compression_type,
            offset: section_map_offset,
        });
    }
    if header.decompressed_size > MAX_DECOMPRESSED_SIZE {
        return Err(SectionMapDecodeError::OversizedDecompressedSize {
            value: header.decompressed_size,
        });
    }

    // 3) Pull the LZ77-compressed payload bytes.
    let payload_start = section_map_offset
        .checked_add(SYSTEM_PAGE_HEADER_LEN)
        .ok_or(SectionMapDecodeError::TruncatedHeader {
            offset: section_map_offset,
            expected_at_least: SYSTEM_PAGE_HEADER_LEN,
        })?;
    let compressed_len = header.compressed_size as usize;
    let payload_end = payload_start
        .checked_add(compressed_len)
        .ok_or(SectionMapDecodeError::TruncatedHeader {
            offset: payload_start,
            expected_at_least: compressed_len,
        })?;
    let compressed = bytes
        .get(payload_start..payload_end)
        .ok_or(SectionMapDecodeError::TruncatedHeader {
            offset: payload_start,
            expected_at_least: compressed_len,
        })?;

    // 4) Decompress with R46-B.
    let decompressed = decompress_ac18_lz77(compressed, header.decompressed_size as usize)?;

    // 5) Parse the descriptor stream.
    parse_descriptors(&decompressed, page_map)
}

/// Walk the decompressed descriptor stream produced by
/// [`parse_ac1018_section_map`].
///
/// Public for unit-test reuse: consumers that already have a
/// decompressed payload (e.g. fuzz fixtures) can drive the parser
/// directly without rebuilding a system page header.
pub fn parse_descriptors(
    decompressed: &[u8],
    page_map: &PageMap,
) -> Result<SectionDescriptorMap, SectionMapDecodeError> {
    let mut cursor = 0usize;

    // 5a) 20-byte in-payload header: num_descriptions + 4 unknown longs.
    if decompressed.len() < SECTION_DESCRIPTOR_MAP_HEADER_LEN {
        return Err(SectionMapDecodeError::TruncatedHeaderStream {
            decompressed_len: decompressed.len(),
            cursor,
        });
    }
    let num_descriptions = read_i32_le(decompressed, cursor);
    cursor += SECTION_DESCRIPTOR_MAP_HEADER_LEN;

    let mut map = SectionDescriptorMap::default();
    if num_descriptions <= 0 {
        return Ok(map);
    }
    let descriptor_count = num_descriptions as usize;

    for descriptor_index in 0..descriptor_count {
        // 5b) 96-byte SectionDescriptor fixed header.
        let descriptor_end = cursor
            .checked_add(SECTION_DESCRIPTOR_LEN)
            .ok_or(SectionMapDecodeError::TruncatedDescriptorStream {
                decompressed_len: decompressed.len(),
                cursor,
                descriptor_index,
            })?;
        if descriptor_end > decompressed.len() {
            return Err(SectionMapDecodeError::TruncatedDescriptorStream {
                decompressed_len: decompressed.len(),
                cursor,
                descriptor_index,
            });
        }

        let compressed_size = read_u64_le(decompressed, cursor);
        let page_count = read_i32_le(decompressed, cursor + 0x08);
        let decompressed_size = read_i32_le(decompressed, cursor + 0x0C) as u64;
        // 0x10 unknown
        let compressed_code = read_i32_le(decompressed, cursor + 0x14);
        let section_id = read_i32_le(decompressed, cursor + 0x18);
        let encrypted = read_i32_le(decompressed, cursor + 0x1C);
        let name_bytes = &decompressed[cursor + 0x20..cursor + 0x20 + SECTION_NAME_LEN];
        let name = decode_section_name(name_bytes);
        cursor = descriptor_end;

        let mut local_sections: Vec<LocalSectionMap> = Vec::new();
        if page_count > 0 {
            let pages = page_count as usize;
            for _ in 0..pages {
                let local_end = cursor.checked_add(LOCAL_SECTION_MAP_LEN).ok_or(
                    SectionMapDecodeError::TruncatedDescriptorStream {
                        decompressed_len: decompressed.len(),
                        cursor,
                        descriptor_index,
                    },
                )?;
                if local_end > decompressed.len() {
                    return Err(SectionMapDecodeError::TruncatedDescriptorStream {
                        decompressed_len: decompressed.len(),
                        cursor,
                        descriptor_index,
                    });
                }
                let page_number = read_i32_le(decompressed, cursor);
                let local_compressed_size = read_i32_le(decompressed, cursor + 4) as u32 as u64;
                let offset = read_u64_le(decompressed, cursor + 8);
                cursor = local_end;

                let page_record = page_map.lookup(page_number).ok_or(
                    SectionMapDecodeError::MissingPageInPageMap {
                        page_number,
                        descriptor_index,
                    },
                )?;

                local_sections.push(LocalSectionMap {
                    page_number,
                    compressed_size: local_compressed_size,
                    offset,
                    decompressed_size,
                    seeker: page_record.seeker,
                });
            }
        }

        // ACadSharp tail-page correction: the last local section's
        // decompressed_size is the remainder of compressed_size mod
        // decompressed_size when that remainder is non-zero. This
        // mirrors `descriptor.LocalSections[Last].DecompressedSize =
        // sizeLeft;` in DwgReader.cs L640..L642.
        if !local_sections.is_empty() && decompressed_size > 0 {
            let size_left = compressed_size % decompressed_size;
            if size_left > 0 {
                let last = local_sections.len() - 1;
                local_sections[last].decompressed_size = size_left;
            }
        }

        let descriptor = SectionDescriptor {
            compressed_size,
            page_count,
            decompressed_size,
            compressed_code,
            section_id,
            encrypted,
            name: name.clone(),
            local_sections,
        };

        // ACadSharp uses Dictionary<string, ...>; we use BTreeMap.
        // Duplicate names would silently overwrite in C#; we mirror
        // that by using `insert` (last write wins) and only push the
        // name to `order` on first sight.
        if !map.descriptors.contains_key(&name) {
            map.order.push(name.clone());
        }
        map.descriptors.insert(name, descriptor);
    }

    Ok(map)
}

fn decode_section_name(bytes: &[u8]) -> String {
    let nul = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..nul]).into_owned()
}

fn read_i32_le(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_map_ac1018::{
        PageMap, PageMapRecord, COMPRESSION_TYPE_LZ77, INITIAL_SEEKER, SYSTEM_PAGE_HEADER_LEN,
    };

    /// Build a synthetic `PageMap` containing a single record whose
    /// `seeker` points right after a synthetic system page header.
    fn page_map_with_section_map_at(section_map_id: u32, seeker: i64) -> PageMap {
        PageMap {
            records: vec![PageMapRecord {
                number: section_map_id as i32,
                size: 0x100,
                seeker,
            }],
        }
    }

    /// Build an in-payload SectionDescriptorMap header (20 bytes) +
    /// `num_descriptions` descriptors back-to-back. Used by the
    /// `parse_descriptors` unit tests that bypass the LZ77 layer.
    fn build_in_payload_header(num_descriptions: i32) -> Vec<u8> {
        let mut buf = vec![0u8; SECTION_DESCRIPTOR_MAP_HEADER_LEN];
        buf[0x00..0x04].copy_from_slice(&num_descriptions.to_le_bytes());
        buf[0x04..0x08].copy_from_slice(&0x02i32.to_le_bytes());
        buf[0x08..0x0C].copy_from_slice(&0x7400i32.to_le_bytes());
        buf[0x0C..0x10].copy_from_slice(&0i32.to_le_bytes());
        buf[0x10..0x14].copy_from_slice(&num_descriptions.to_le_bytes());
        buf
    }

    /// Append a single descriptor (96 bytes + 16 * page_count bytes)
    /// to `out`, with the given fields and `local_sections` array.
    #[allow(clippy::too_many_arguments)]
    fn push_descriptor(
        out: &mut Vec<u8>,
        compressed_size: u64,
        page_count: i32,
        decompressed_size: u32,
        compressed_code: i32,
        section_id: i32,
        encrypted: i32,
        name: &str,
        local_sections: &[(i32, i32, u64)],
    ) {
        let mut header = vec![0u8; SECTION_DESCRIPTOR_LEN];
        header[0x00..0x08].copy_from_slice(&compressed_size.to_le_bytes());
        header[0x08..0x0C].copy_from_slice(&page_count.to_le_bytes());
        header[0x0C..0x10].copy_from_slice(&(decompressed_size as i32).to_le_bytes());
        // 0x10 unknown stays 0
        header[0x14..0x18].copy_from_slice(&compressed_code.to_le_bytes());
        header[0x18..0x1C].copy_from_slice(&section_id.to_le_bytes());
        header[0x1C..0x20].copy_from_slice(&encrypted.to_le_bytes());
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(SECTION_NAME_LEN);
        header[0x20..0x20 + name_len].copy_from_slice(&name_bytes[..name_len]);
        out.extend_from_slice(&header);
        for &(page_number, comp_size, offset) in local_sections {
            let mut entry = vec![0u8; LOCAL_SECTION_MAP_LEN];
            entry[0x00..0x04].copy_from_slice(&page_number.to_le_bytes());
            entry[0x04..0x08].copy_from_slice(&comp_size.to_le_bytes());
            entry[0x08..0x10].copy_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&entry);
        }
    }

    #[test]
    fn parse_descriptors_decodes_synthetic_two_descriptor_payload() {
        // Page map with two valid pages used by the second descriptor.
        let page_map = PageMap {
            records: vec![
                PageMapRecord {
                    number: 1,
                    size: 0x7400,
                    seeker: 0x1000,
                },
                PageMapRecord {
                    number: 2,
                    size: 0x7400,
                    seeker: 0x8400,
                },
            ],
        };

        // Descriptor 0: AcDb:Empty, page_count=0, no local sections.
        // Descriptor 1: AcDb:Handles, page_count=2, two local sections.
        let mut payload = build_in_payload_header(2);
        push_descriptor(
            &mut payload,
            0,
            0,
            0x7400,
            2,
            0,
            0,
            "AcDb:Empty",
            &[],
        );
        push_descriptor(
            &mut payload,
            0xE800, // 2 * 0x7400 — clean multiple, no tail correction
            2,
            0x7400,
            2,
            5,
            0,
            "AcDb:Handles",
            &[(1, 0x7400, 0), (2, 0x7400, 0x7400)],
        );

        let map = parse_descriptors(&payload, &page_map).expect("synthetic descriptors decode");
        assert_eq!(map.len(), 2);
        assert_eq!(map.order, vec!["AcDb:Empty", "AcDb:Handles"]);

        let empty = map.lookup("AcDb:Empty").unwrap();
        assert_eq!(empty.compressed_size, 0);
        assert_eq!(empty.page_count, 0);
        assert!(empty.local_sections.is_empty());

        let handles = map.lookup("AcDb:Handles").unwrap();
        assert_eq!(handles.compressed_size, 0xE800);
        assert_eq!(handles.page_count, 2);
        assert_eq!(handles.section_id, 5);
        assert_eq!(handles.encrypted, 0);
        assert_eq!(handles.compressed_code, 2);
        assert_eq!(handles.local_sections.len(), 2);
        assert_eq!(handles.local_sections[0].page_number, 1);
        assert_eq!(handles.local_sections[0].seeker, 0x1000);
        assert_eq!(handles.local_sections[0].compressed_size, 0x7400);
        assert_eq!(handles.local_sections[0].decompressed_size, 0x7400);
        assert_eq!(handles.local_sections[1].page_number, 2);
        assert_eq!(handles.local_sections[1].seeker, 0x8400);
        // No tail correction: 0xE800 % 0x7400 == 0
        assert_eq!(handles.local_sections[1].decompressed_size, 0x7400);
    }

    #[test]
    fn parse_descriptors_handles_zero_page_count_descriptor() {
        let page_map = PageMap { records: vec![] };
        let mut payload = build_in_payload_header(1);
        push_descriptor(
            &mut payload,
            0,
            0,
            0x7400,
            2,
            0,
            0,
            "AcDb:Empty",
            &[],
        );
        let map = parse_descriptors(&payload, &page_map).expect("zero-page descriptor decodes");
        assert_eq!(map.len(), 1);
        let empty = map.lookup("AcDb:Empty").unwrap();
        assert!(empty.local_sections.is_empty());
    }

    #[test]
    fn parse_descriptors_resolves_local_section_seeker_via_page_map() {
        let page_map = PageMap {
            records: vec![PageMapRecord {
                number: 7,
                size: 0x7400,
                seeker: 0xABCDE,
            }],
        };
        let mut payload = build_in_payload_header(1);
        push_descriptor(
            &mut payload,
            0x7400,
            1,
            0x7400,
            2,
            1,
            0,
            "AcDb:Header",
            &[(7, 0x7400, 0)],
        );
        let map = parse_descriptors(&payload, &page_map).expect("seeker resolution succeeds");
        let header = map.lookup("AcDb:Header").unwrap();
        assert_eq!(header.local_sections[0].page_number, 7);
        assert_eq!(header.local_sections[0].seeker, 0xABCDE);
    }

    #[test]
    fn parse_descriptors_corrects_last_page_decompressed_size() {
        // compressed_size = 0x8000, decompressed_size = 0x7400 →
        // size_left = 0x8000 - 0x7400 = 0xC00. Last page must report
        // decompressed_size = 0xC00, not 0x7400.
        let page_map = PageMap {
            records: vec![
                PageMapRecord {
                    number: 1,
                    size: 0x7400,
                    seeker: 0x1000,
                },
                PageMapRecord {
                    number: 2,
                    size: 0x7400,
                    seeker: 0x8400,
                },
            ],
        };
        let mut payload = build_in_payload_header(1);
        push_descriptor(
            &mut payload,
            0x8000,
            2,
            0x7400,
            2,
            3,
            0,
            "AcDb:AcDbObjects",
            &[(1, 0x7400, 0), (2, 0xC00, 0x7400)],
        );
        let map = parse_descriptors(&payload, &page_map).expect("tail correction descriptor decodes");
        let objects = map.lookup("AcDb:AcDbObjects").unwrap();
        assert_eq!(objects.local_sections[0].decompressed_size, 0x7400);
        assert_eq!(
            objects.local_sections[1].decompressed_size, 0xC00,
            "tail page must inherit size_left, not full descriptor.decompressed_size"
        );
    }

    #[test]
    fn parse_descriptors_rejects_truncated_header_stream() {
        let page_map = PageMap { records: vec![] };
        let payload = vec![0u8; SECTION_DESCRIPTOR_MAP_HEADER_LEN - 1];
        let err = parse_descriptors(&payload, &page_map).unwrap_err();
        assert_eq!(
            err,
            SectionMapDecodeError::TruncatedHeaderStream {
                decompressed_len: SECTION_DESCRIPTOR_MAP_HEADER_LEN - 1,
                cursor: 0,
            }
        );
    }

    #[test]
    fn parse_descriptors_rejects_truncated_descriptor_stream() {
        let page_map = PageMap { records: vec![] };
        let mut payload = build_in_payload_header(1);
        // Put only half a descriptor on the wire (48 bytes instead of
        // 96).
        payload.extend(std::iter::repeat(0u8).take(SECTION_DESCRIPTOR_LEN / 2));
        let err = parse_descriptors(&payload, &page_map).unwrap_err();
        assert!(
            matches!(
                err,
                SectionMapDecodeError::TruncatedDescriptorStream {
                    descriptor_index: 0,
                    ..
                }
            ),
            "expected truncated descriptor error, got {err:?}"
        );
    }

    #[test]
    fn parse_descriptors_rejects_missing_page_in_page_map() {
        let page_map = PageMap { records: vec![] };
        let mut payload = build_in_payload_header(1);
        push_descriptor(
            &mut payload,
            0x7400,
            1,
            0x7400,
            2,
            1,
            0,
            "AcDb:Header",
            &[(99, 0x7400, 0)], // page 99 does not exist in page_map
        );
        let err = parse_descriptors(&payload, &page_map).unwrap_err();
        assert_eq!(
            err,
            SectionMapDecodeError::MissingPageInPageMap {
                page_number: 99,
                descriptor_index: 0,
            }
        );
    }

    #[test]
    fn parse_descriptors_handles_invalid_utf8_name_gracefully() {
        let page_map = PageMap { records: vec![] };
        let mut payload = build_in_payload_header(1);
        // Manually craft a descriptor whose name field contains a
        // non-UTF-8 byte (0xFF) that String::from_utf8_lossy must
        // replace with U+FFFD instead of panicking.
        let mut header = vec![0u8; SECTION_DESCRIPTOR_LEN];
        header[0x00..0x08].copy_from_slice(&0u64.to_le_bytes());
        header[0x14..0x18].copy_from_slice(&2i32.to_le_bytes());
        header[0x20] = b'A';
        header[0x21] = 0xFF;
        header[0x22] = b'B';
        // 0x23+ stay 0 (NUL terminator)
        payload.extend_from_slice(&header);

        let map = parse_descriptors(&payload, &page_map).expect("lossy decode does not panic");
        assert_eq!(map.len(), 1);
        let key = map.order.first().expect("at least one descriptor");
        assert!(
            key.contains('A') && key.contains('B'),
            "lossy name should still contain the ASCII parts: {key:?}"
        );
    }

    /// End-to-end via `parse_ac1018_section_map`: build a synthetic
    /// system page header + LZ77-encoded descriptor payload, then
    /// decode through the public entry. Verifies the section_type
    /// validation, LZ77 dispatch, and descriptor walk all hang
    /// together.
    #[test]
    fn parse_ac1018_section_map_decodes_end_to_end_synthetic() {
        // 1) Build the in-payload descriptor stream we want to land
        //    after LZ77 decompression.
        let page_map_inner = PageMap {
            records: vec![PageMapRecord {
                number: 3,
                size: 0x7400,
                seeker: 0x4000,
            }],
        };
        let mut decompressed = build_in_payload_header(1);
        push_descriptor(
            &mut decompressed,
            0x7400,
            1,
            0x7400,
            2,
            7,
            0,
            "AcDb:Header",
            &[(3, 0x7400, 0)],
        );

        // 2) Wrap with the LZ77 leading-literal preamble used by R46-B.
        //    We use the chain-byte path because the payload is far
        //    longer than 16 bytes (1 header + 96 + 16 = 132 bytes).
        let raw_len = decompressed.len();
        assert!(
            raw_len > 18,
            "test fixture must be long enough to require the chain byte"
        );
        let mut compressed = Vec::with_capacity(raw_len + 8);
        // opcode1 = 0x00 → enter literal_count chain
        compressed.push(0x00);
        // chain bytes: each non-zero byte adds (lowbits + 0x0F + last_byte),
        // each 0x00 adds 0xFF and continues. We need total = raw_len - 3.
        let mut remaining = (raw_len as i64) - 3;
        // Walk the C# semantics:
        //   lowbits = 0; while (b == 0) lowbits += 0xFF; lowbits += 0x0F + b;
        // For raw_len = 132 → remaining = 129.
        // Try one non-zero byte: pick `b` such that `0x0F + b = 129` → b = 0x72 = 114.
        // 114 < 256 so a single chain byte suffices.
        let chain_byte: u8 = (remaining - 0x0F) as u8;
        compressed.push(chain_byte);
        remaining -= i64::from(0x0F + chain_byte as i32);
        assert_eq!(remaining, 0, "chain byte arithmetic did not zero out");
        compressed.extend_from_slice(&decompressed);
        compressed.push(0x11); // terminator

        // 3) Build the on-disk system page header for the section map
        //    (section_type = SECTION_MAP_SECTION_TYPE).
        let section_map_seeker = SYSTEM_PAGE_HEADER_LEN as i64; // place the page right after a 0-padded prefix
        let prefix_len = section_map_seeker as usize;
        let mut bytes = vec![0u8; prefix_len];
        bytes.extend_from_slice(&build_system_page(
            SECTION_MAP_SECTION_TYPE,
            decompressed.len() as u32,
            compressed.len() as u32,
            COMPRESSION_TYPE_LZ77,
            0,
        ));
        bytes.extend_from_slice(&compressed);

        // 4) Build a page map that resolves section_map_id to that
        //    seeker. Note this is a different page map from the
        //    `page_map_inner` we used to resolve the *descriptor's*
        //    local sections; in real life R46-C produces a single
        //    page map that does both, but here we keep them separate
        //    to focus the test on the section map dispatch path.
        let page_map_outer = page_map_with_section_map_at(42, section_map_seeker);

        let map = parse_ac1018_section_map(&bytes, &page_map_outer, 42);
        let err = map.expect_err("page_map_outer cannot resolve descriptor's page #3 — error path");
        assert!(
            matches!(
                err,
                SectionMapDecodeError::MissingPageInPageMap {
                    page_number: 3,
                    descriptor_index: 0,
                }
            ),
            "expected MissingPageInPageMap, got {err:?}"
        );

        // Now drive the happy path with a page map that resolves both
        // section_map_id (42) and descriptor.local_section.page_number (3).
        let mut combined_records = page_map_inner.records.clone();
        combined_records.push(PageMapRecord {
            number: 42,
            size: 0x7400,
            seeker: section_map_seeker,
        });
        let combined_page_map = PageMap {
            records: combined_records,
        };
        let map = parse_ac1018_section_map(&bytes, &combined_page_map, 42)
            .expect("end-to-end happy path decodes");
        assert_eq!(map.len(), 1);
        let header = map.lookup("AcDb:Header").expect("AcDb:Header found");
        assert_eq!(header.local_sections.len(), 1);
        assert_eq!(header.local_sections[0].seeker, 0x4000);
        assert_eq!(header.section_id, 7);
    }

    #[test]
    fn parse_ac1018_section_map_rejects_invalid_section_type() {
        let page_map = page_map_with_section_map_at(1, INITIAL_SEEKER);
        let prefix_len = INITIAL_SEEKER as usize;
        let mut bytes = vec![0u8; prefix_len];
        bytes.extend_from_slice(&build_system_page(
            0xDEAD_BEEF, // wrong section_type
            0,
            0,
            COMPRESSION_TYPE_LZ77,
            0,
        ));
        let err = parse_ac1018_section_map(&bytes, &page_map, 1).unwrap_err();
        assert_eq!(
            err,
            SectionMapDecodeError::InvalidPageType {
                actual: 0xDEAD_BEEF,
                offset: prefix_len,
            }
        );
    }

    #[test]
    fn parse_ac1018_section_map_rejects_unsupported_compression() {
        let page_map = page_map_with_section_map_at(1, INITIAL_SEEKER);
        let prefix_len = INITIAL_SEEKER as usize;
        let mut bytes = vec![0u8; prefix_len];
        bytes.extend_from_slice(&build_system_page(
            SECTION_MAP_SECTION_TYPE,
            0,
            0,
            0x01, // not LZ77
            0,
        ));
        let err = parse_ac1018_section_map(&bytes, &page_map, 1).unwrap_err();
        assert_eq!(
            err,
            SectionMapDecodeError::UnsupportedCompressionType {
                actual: 0x01,
                offset: prefix_len,
            }
        );
    }

    #[test]
    fn parse_ac1018_section_map_rejects_oversized_decompressed_size() {
        let page_map = page_map_with_section_map_at(1, INITIAL_SEEKER);
        let prefix_len = INITIAL_SEEKER as usize;
        let mut bytes = vec![0u8; prefix_len];
        bytes.extend_from_slice(&build_system_page(
            SECTION_MAP_SECTION_TYPE,
            MAX_DECOMPRESSED_SIZE + 1,
            0,
            COMPRESSION_TYPE_LZ77,
            0,
        ));
        let err = parse_ac1018_section_map(&bytes, &page_map, 1).unwrap_err();
        assert_eq!(
            err,
            SectionMapDecodeError::OversizedDecompressedSize {
                value: MAX_DECOMPRESSED_SIZE + 1
            }
        );
    }

    #[test]
    fn parse_ac1018_section_map_returns_page_map_error_when_section_id_missing() {
        let page_map = PageMap { records: vec![] };
        let bytes = vec![0u8; 0x1000];
        let err = parse_ac1018_section_map(&bytes, &page_map, 999).unwrap_err();
        assert!(
            matches!(err, SectionMapDecodeError::PageMap(_)),
            "expected PageMap error, got {err:?}"
        );
    }

    #[test]
    fn section_map_decode_error_display_strings_include_diagnostics() {
        let trunc_header = format!(
            "{}",
            SectionMapDecodeError::TruncatedHeader {
                offset: 0x100,
                expected_at_least: 20
            }
        );
        assert!(trunc_header.contains("offset 256"));
        assert!(trunc_header.contains("at least 20"));

        let invalid = format!(
            "{}",
            SectionMapDecodeError::InvalidPageType {
                actual: 0xCAFEBABE,
                offset: 0
            }
        );
        assert!(invalid.contains("0xCAFEBABE"));

        let unsupported = format!(
            "{}",
            SectionMapDecodeError::UnsupportedCompressionType {
                actual: 1,
                offset: 0
            }
        );
        assert!(unsupported.contains("compression_type 1"));

        let oversized = format!(
            "{}",
            SectionMapDecodeError::OversizedDecompressedSize { value: 99 }
        );
        assert!(oversized.contains("99"));

        let lz77 = format!(
            "{}",
            SectionMapDecodeError::Lz77 {
                source: Lz77DecodeError::TruncatedInput
            }
        );
        assert!(lz77.contains("LZ77"));

        let trunc_header_stream = format!(
            "{}",
            SectionMapDecodeError::TruncatedHeaderStream {
                decompressed_len: 30,
                cursor: 28
            }
        );
        assert!(trunc_header_stream.contains("cursor 28"));
        assert!(trunc_header_stream.contains("total 30"));

        let trunc_desc = format!(
            "{}",
            SectionMapDecodeError::TruncatedDescriptorStream {
                decompressed_len: 200,
                cursor: 150,
                descriptor_index: 3
            }
        );
        assert!(trunc_desc.contains("descriptor #3"));
        assert!(trunc_desc.contains("cursor 150"));

        let missing = format!(
            "{}",
            SectionMapDecodeError::MissingPageInPageMap {
                page_number: 7,
                descriptor_index: 2
            }
        );
        assert!(missing.contains("page_number 7"));
        assert!(missing.contains("descriptor #2"));
    }

    /// Build a 20-byte system page header followed by no payload.
    /// Reused by the rejection-path tests that don't need a real
    /// LZ77 stream behind the header.
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
