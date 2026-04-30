//! AC1018 (AutoCAD R2004) single section payload reassembly, R46-E1
//! scope.
//!
//! Layout reference: ACadSharp `src/ACadSharp/IO/DWG/DwgReader.cs`
//! `getSectionBuffer18` (L1032..L1076) + `decryptDataSection`
//! (L1078..L1100). Each AC1018 section listed in the descriptor map
//! decoded by R46-D is split into one or more on-disk pages whose
//! header is XOR-encrypted (32 bytes) and whose payload is either
//! LZ77-compressed (the common case, `compressed_code == 2`) or raw
//! (`compressed_code == 1`). R46-E1 walks those pages, decrypts the
//! per-page header, decompresses with R46-B (or copies verbatim),
//! and concatenates the per-page outputs into a single `Vec<u8>`
//! that R46-E2 will hand to the existing AC1015
//! `build_pending_document` pipeline.
//!
//! R46-E1 wires R46-B (LZ77 decompressor) and R46-D (
//! [`SectionDescriptor`] / [`LocalSectionMap`]) together. It does
//! **not** modify [`crate::file_header::section_count_offset`] or
//! [`crate::read_dwg`]: that path stays `UnsupportedHeaderLayout`
//! until R46-E2 lights up the end-to-end pipeline.

use std::fmt;

use crate::lz77_ac18::{decompress_ac18_lz77, Lz77DecodeError};
use crate::section_map_ac1018::SectionDescriptor;

/// Length of the per-page encrypted header (bytes), 32.
pub const PAGE_HEADER_LEN: usize = 0x20;

/// XOR base used by ACadSharp `decryptDataSection`:
/// `secMask = PAGE_HEADER_XOR_MAGIC ^ (page_offset as u32)`.
pub const PAGE_HEADER_XOR_MAGIC: u32 = 0x4164_536B;

/// `section_type` value identifying a data section page (i.e. a page
/// inside a SectionDescriptor's `local_sections` list, distinct from
/// the system pages handled by R46-C).
pub const DATA_SECTION_PAGE_TYPE: u32 = 0x4163_043B;

/// Defensive cap on a single page's LZ77-decompressed output. Real
/// AC1018 pages stay under 0x7400 (29 KiB) — the 16 MiB ceiling
/// matches R46-C's [`crate::page_map_ac1018::MAX_DECOMPRESSED_SIZE`]
/// and protects against malformed input ballooning the output.
pub const MAX_LZ77_OUTPUT_PER_PAGE: usize = 16 * 1024 * 1024;

/// Decrypted view of the 32-byte per-page header. All fields are
/// little-endian unsigned 32-bit integers after XOR-decryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncryptedPageHeader {
    /// `section_type`, must equal [`DATA_SECTION_PAGE_TYPE`] for a
    /// well-formed data page.
    pub section_type: u32,
    /// Section number (matches the parent
    /// [`SectionDescriptor::section_id`] on real samples).
    pub section_number: u32,
    /// Compressed size of this page on disk. Overrides the value in
    /// [`crate::section_map_ac1018::LocalSectionMap::compressed_size`]
    /// (ACadSharp does the same: the descriptor table holds an
    /// aggregate, the per-page header holds the per-page truth).
    pub compressed_size: u32,
    /// Decompressed size of this page (a.k.a. `page_size`).
    pub page_size: u32,
    /// Logical start offset of this page inside the section's
    /// reassembled buffer.
    pub start_offset: u32,
    /// CRC of the unencoded header bytes (kept but unverified).
    pub page_header_checksum: u32,
    /// CRC of the compressed payload bytes (kept but unverified).
    pub data_checksum: u32,
    /// Trailing reserved long; ACadSharp documents that ODA writes 0.
    pub oda: u32,
}

/// Errors that can surface while reassembling an AC1018 section.
///
/// Kept independent of [`crate::DwgReadError`]: like R46-B/C/D, the
/// section-data brick is self-contained; R46-E2 will wrap these into
/// the top-level error type when it lights up `read_dwg` for AC1018.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionDataDecodeError {
    /// `bytes` was too small to cover the 32-byte page header at the
    /// requested seeker.
    TruncatedPageHeader {
        /// Offset where the truncated read started.
        offset: usize,
        /// Number of bytes the parser needed at `offset`.
        expected_at_least: usize,
    },
    /// The decrypted page header carried a `section_type` value other
    /// than [`DATA_SECTION_PAGE_TYPE`]. Almost always means the seeker
    /// pointed at a system page (PageMap / SectionDescriptorMap) by
    /// mistake.
    InvalidPageType {
        /// The decrypted value found in the header.
        actual: u32,
        /// Offset of the offending header inside `bytes`.
        offset: usize,
    },
    /// Either `seeker` is outside `bytes`, or the declared
    /// `compressed_size` would read past the end of `bytes`.
    PageOutOfBounds {
        /// Original (signed) seeker from the LocalSectionMap.
        seeker: i64,
        /// Total file length, for diagnostic context.
        file_len: usize,
    },
    /// LZ77 decompression of a page payload failed; carries the
    /// underlying R46-B error.
    Lz77 {
        /// Underlying LZ77 decoder error.
        source: Lz77DecodeError,
    },
    /// Caller asked to read a descriptor that has no local sections.
    /// Real-world AC1018 always has at least one page per section;
    /// this guards against synthetic descriptors / mistaken lookups.
    EmptyDescriptor {
        /// Section name for diagnostic context.
        name: String,
    },
    /// `descriptor.compressed_code` was neither `1` (uncompressed)
    /// nor `2` (LZ77). ACadSharp throws on construction; we throw
    /// here on read.
    UnsupportedCompressedCode {
        /// The offending value.
        actual: i32,
    },
}

impl fmt::Display for SectionDataDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedPageHeader {
                offset,
                expected_at_least,
            } => write!(
                f,
                "AC1018 section data: truncated page header at offset {offset} (expected at least {expected_at_least} bytes)"
            ),
            Self::InvalidPageType { actual, offset } => write!(
                f,
                "AC1018 section data: invalid page section_type 0x{actual:08X} at offset {offset} (expected 0x{DATA_SECTION_PAGE_TYPE:08X})"
            ),
            Self::PageOutOfBounds { seeker, file_len } => write!(
                f,
                "AC1018 section data: page seeker {seeker} out of bounds for file of {file_len} bytes"
            ),
            Self::Lz77 { source } => write!(
                f,
                "AC1018 section data: LZ77 decode failed: {source}"
            ),
            Self::EmptyDescriptor { name } => write!(
                f,
                "AC1018 section data: descriptor {name:?} has no local sections to read"
            ),
            Self::UnsupportedCompressedCode { actual } => write!(
                f,
                "AC1018 section data: unsupported compressed_code {actual} (expected 1 or 2)"
            ),
        }
    }
}

impl std::error::Error for SectionDataDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lz77 { source } => Some(source),
            _ => None,
        }
    }
}

impl From<Lz77DecodeError> for SectionDataDecodeError {
    fn from(value: Lz77DecodeError) -> Self {
        Self::Lz77 { source: value }
    }
}

/// Decrypt the 32-byte page header at `bytes[page_offset..]`.
///
/// `secMask = PAGE_HEADER_XOR_MAGIC ^ (page_offset as u32)`; each
/// 4-byte field is XOR'd with that mask to recover the plaintext.
///
/// # Errors
///
/// [`SectionDataDecodeError::TruncatedPageHeader`] when `bytes` does
/// not cover `[page_offset, page_offset + 0x20)`.
pub fn decrypt_page_header(
    bytes: &[u8],
    page_offset: usize,
) -> Result<EncryptedPageHeader, SectionDataDecodeError> {
    let end = page_offset
        .checked_add(PAGE_HEADER_LEN)
        .ok_or(SectionDataDecodeError::TruncatedPageHeader {
            offset: page_offset,
            expected_at_least: PAGE_HEADER_LEN,
        })?;
    let slice = bytes
        .get(page_offset..end)
        .ok_or(SectionDataDecodeError::TruncatedPageHeader {
            offset: page_offset,
            expected_at_least: PAGE_HEADER_LEN,
        })?;
    let sec_mask = PAGE_HEADER_XOR_MAGIC ^ (page_offset as u32);
    Ok(EncryptedPageHeader {
        section_type: read_u32_le(slice, 0x00) ^ sec_mask,
        section_number: read_u32_le(slice, 0x04) ^ sec_mask,
        compressed_size: read_u32_le(slice, 0x08) ^ sec_mask,
        page_size: read_u32_le(slice, 0x0C) ^ sec_mask,
        start_offset: read_u32_le(slice, 0x10) ^ sec_mask,
        page_header_checksum: read_u32_le(slice, 0x14) ^ sec_mask,
        data_checksum: read_u32_le(slice, 0x18) ^ sec_mask,
        oda: read_u32_le(slice, 0x1C) ^ sec_mask,
    })
}

/// Reassemble a single AC1018 section's payload by walking
/// `descriptor.local_sections` in order, decrypting each page header,
/// LZ77-decompressing the compressed pages (or copying raw pages
/// verbatim), and concatenating the per-page outputs.
///
/// # Errors
///
/// See [`SectionDataDecodeError`] for the full list. The most common
/// failure on a misaligned `seeker` is
/// [`SectionDataDecodeError::InvalidPageType`]; the most common
/// failure on a truncated file is
/// [`SectionDataDecodeError::PageOutOfBounds`].
pub fn read_section_payload(
    bytes: &[u8],
    descriptor: &SectionDescriptor,
) -> Result<Vec<u8>, SectionDataDecodeError> {
    if descriptor.local_sections.is_empty() {
        return Err(SectionDataDecodeError::EmptyDescriptor {
            name: descriptor.name.clone(),
        });
    }
    if descriptor.compressed_code != 1 && descriptor.compressed_code != 2 {
        return Err(SectionDataDecodeError::UnsupportedCompressedCode {
            actual: descriptor.compressed_code,
        });
    }

    let mut out = Vec::with_capacity(
        (descriptor.decompressed_size as usize)
            .saturating_mul(descriptor.local_sections.len()),
    );

    for local in &descriptor.local_sections {
        let page_offset: usize =
            local
                .seeker
                .try_into()
                .map_err(|_| SectionDataDecodeError::PageOutOfBounds {
                    seeker: local.seeker,
                    file_len: bytes.len(),
                })?;
        // `decrypt_page_header` would also catch this, but it would
        // surface as `TruncatedPageHeader` which obscures the real
        // cause (the seeker, not the header layout). Pre-check so
        // callers get the more actionable error variant.
        if page_offset.saturating_add(PAGE_HEADER_LEN) > bytes.len() {
            return Err(SectionDataDecodeError::PageOutOfBounds {
                seeker: local.seeker,
                file_len: bytes.len(),
            });
        }
        let header = decrypt_page_header(bytes, page_offset)?;
        if header.section_type != DATA_SECTION_PAGE_TYPE {
            return Err(SectionDataDecodeError::InvalidPageType {
                actual: header.section_type,
                offset: page_offset,
            });
        }

        let payload_start = page_offset + PAGE_HEADER_LEN;
        let compressed_size = header.compressed_size as usize;
        let payload_end = payload_start.checked_add(compressed_size).ok_or(
            SectionDataDecodeError::PageOutOfBounds {
                seeker: local.seeker,
                file_len: bytes.len(),
            },
        )?;
        if payload_end > bytes.len() {
            return Err(SectionDataDecodeError::PageOutOfBounds {
                seeker: local.seeker,
                file_len: bytes.len(),
            });
        }
        let compressed = &bytes[payload_start..payload_end];

        if descriptor.compressed_code == 2 {
            // ACadSharp's `DwgLZ77AC18Decompressor.DecompressToDest`
            // does **not** cap the output by `page_size` — it writes
            // until it hits the terminator opcode and trusts the
            // caller-side `MemoryStream` to fault out on OOM. Mirror
            // that behaviour by using a generous cap (the same 16 MiB
            // sanity ceiling R46-C uses for system-page payloads).
            // R46-E2 will tighten this when the section reassembly
            // talks to a callable allocator that knows the real
            // decompressed length up front.
            let decompressed = decompress_ac18_lz77(
                compressed,
                MAX_LZ77_OUTPUT_PER_PAGE,
            )?;
            out.extend_from_slice(&decompressed);
        } else {
            // compressed_code == 1: verbatim copy.
            out.extend_from_slice(compressed);
        }
    }

    Ok(out)
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section_map_ac1018::{LocalSectionMap, SectionDescriptor};

    /// Build the 32-byte XOR-encrypted page header for a given
    /// `page_offset`. Used by every unit test that needs to drive
    /// `decrypt_page_header` / `read_section_payload`.
    fn build_encrypted_page_header(
        page_offset: usize,
        plain: EncryptedPageHeader,
    ) -> [u8; PAGE_HEADER_LEN] {
        let sec_mask = PAGE_HEADER_XOR_MAGIC ^ (page_offset as u32);
        let mut out = [0u8; PAGE_HEADER_LEN];
        out[0x00..0x04].copy_from_slice(&(plain.section_type ^ sec_mask).to_le_bytes());
        out[0x04..0x08].copy_from_slice(&(plain.section_number ^ sec_mask).to_le_bytes());
        out[0x08..0x0C].copy_from_slice(&(plain.compressed_size ^ sec_mask).to_le_bytes());
        out[0x0C..0x10].copy_from_slice(&(plain.page_size ^ sec_mask).to_le_bytes());
        out[0x10..0x14].copy_from_slice(&(plain.start_offset ^ sec_mask).to_le_bytes());
        out[0x14..0x18].copy_from_slice(&(plain.page_header_checksum ^ sec_mask).to_le_bytes());
        out[0x18..0x1C].copy_from_slice(&(plain.data_checksum ^ sec_mask).to_le_bytes());
        out[0x1C..0x20].copy_from_slice(&(plain.oda ^ sec_mask).to_le_bytes());
        out
    }

    fn sample_plain_header(compressed_size: u32, page_size: u32) -> EncryptedPageHeader {
        EncryptedPageHeader {
            section_type: DATA_SECTION_PAGE_TYPE,
            section_number: 5,
            compressed_size,
            page_size,
            start_offset: 0,
            page_header_checksum: 0xCAFEBABE,
            data_checksum: 0x12345678,
            oda: 0,
        }
    }

    /// Wrap raw bytes in an LZ77 leading-literal preamble + terminator.
    /// Mirrors `parse_ac1018_page_map_decodes_two_valid_records` in
    /// `page_map_ac1018.rs`. Caller picks `chain_byte` so that the
    /// total literal count matches `raw.len()`.
    fn lz77_wrap_leading_literal(raw: &[u8]) -> Vec<u8> {
        // For raw.len() ≤ 18, opcode1 = 0x0F & (raw.len() - 3) suffices.
        // For longer, use 0x00 + chain byte path.
        let mut out = Vec::with_capacity(raw.len() + 8);
        if raw.len() < 3 {
            // Still legal: opcode1 = 0x00 + chain byte (lowbits +
            // 0x0F + chain) sets total literals.
            // Easiest: pad raw to 3+ for this synthetic helper; tests
            // that need < 3 bytes should build by hand.
            panic!("lz77_wrap_leading_literal helper requires raw.len() >= 3");
        }
        if raw.len() <= 18 {
            // opcode1 lowbits = raw.len() - 3; literals follow.
            let opcode1 = (raw.len() as u8) - 3;
            out.push(opcode1);
            out.extend_from_slice(raw);
        } else {
            // chain byte path: opcode1 = 0x00 → enter chain;
            // chain byte such that 0x0F + chain = raw.len() - 3.
            let need = (raw.len() as i64) - 3;
            assert!(
                (0x0F..=0x0F + 0xFFi64).contains(&need),
                "lz77_wrap_leading_literal helper supports raw.len() up to 0x102, got {}",
                raw.len()
            );
            let chain_byte = (need - 0x0F) as u8;
            out.push(0x00);
            out.push(chain_byte);
            out.extend_from_slice(raw);
        }
        out.push(0x11); // terminator
        out
    }

    #[test]
    fn decrypt_page_header_decodes_synthetic_at_zero_offset() {
        let plain = sample_plain_header(0x100, 0x7400);
        let encrypted = build_encrypted_page_header(0, plain);
        let decoded = decrypt_page_header(&encrypted, 0).expect("synthetic header decrypts");
        assert_eq!(decoded, plain);
    }

    #[test]
    fn decrypt_page_header_position_dependent_xor() {
        // Same plaintext, two different page offsets → encrypted bytes
        // differ but decoded plaintext matches.
        let plain = sample_plain_header(0x40, 0x80);
        let off_a = 0x100;
        let off_b = 0x10BC20;
        let enc_a = build_encrypted_page_header(off_a, plain);
        let enc_b = build_encrypted_page_header(off_b, plain);
        assert_ne!(enc_a, enc_b, "different page_offset must produce different ciphertext");

        // Place each encrypted block inside a buffer big enough to host
        // its declared offset.
        let mut bytes_a = vec![0u8; off_a];
        bytes_a.extend_from_slice(&enc_a);
        let mut bytes_b = vec![0u8; off_b];
        bytes_b.extend_from_slice(&enc_b);

        assert_eq!(decrypt_page_header(&bytes_a, off_a).unwrap(), plain);
        assert_eq!(decrypt_page_header(&bytes_b, off_b).unwrap(), plain);
    }

    #[test]
    fn decrypt_page_header_rejects_truncated_input() {
        let bytes = vec![0u8; PAGE_HEADER_LEN - 1];
        let err = decrypt_page_header(&bytes, 0).unwrap_err();
        assert_eq!(
            err,
            SectionDataDecodeError::TruncatedPageHeader {
                offset: 0,
                expected_at_least: PAGE_HEADER_LEN,
            }
        );
    }

    #[test]
    fn read_section_payload_decompresses_single_page() {
        // 1) Raw payload to embed inside the LZ77 stream.
        let raw_payload: Vec<u8> = (0u8..16).collect();
        let compressed = lz77_wrap_leading_literal(&raw_payload);

        // 2) Build the 32-byte encrypted page header at offset 0.
        let plain = sample_plain_header(compressed.len() as u32, raw_payload.len() as u32);
        let encrypted_header = build_encrypted_page_header(0, plain);

        // 3) bytes = encrypted header + compressed payload
        let mut bytes = Vec::with_capacity(PAGE_HEADER_LEN + compressed.len());
        bytes.extend_from_slice(&encrypted_header);
        bytes.extend_from_slice(&compressed);

        // 4) Descriptor with one local section pointing at offset 0.
        let descriptor = SectionDescriptor {
            compressed_size: compressed.len() as u64,
            page_count: 1,
            decompressed_size: raw_payload.len() as u64,
            compressed_code: 2,
            section_id: 5,
            encrypted: 0,
            name: "Synthetic:Single".into(),
            local_sections: vec![LocalSectionMap {
                page_number: 1,
                compressed_size: compressed.len() as u64,
                offset: 0,
                decompressed_size: raw_payload.len() as u64,
                seeker: 0,
            }],
        };

        let out = read_section_payload(&bytes, &descriptor).expect("single-page decode");
        assert_eq!(out, raw_payload);
    }

    #[test]
    fn read_section_payload_concatenates_multiple_pages() {
        // Page 0: raw [0x10..0x20)
        let raw_a: Vec<u8> = (0x10u8..0x20).collect();
        let compressed_a = lz77_wrap_leading_literal(&raw_a);
        let header_a_offset = 0usize;
        let plain_a = sample_plain_header(compressed_a.len() as u32, raw_a.len() as u32);
        let enc_header_a = build_encrypted_page_header(header_a_offset, plain_a);

        // Page 1: raw [0x80..0x90), placed right after page 0.
        let raw_b: Vec<u8> = (0x80u8..0x90).collect();
        let compressed_b = lz77_wrap_leading_literal(&raw_b);
        let header_b_offset = PAGE_HEADER_LEN + compressed_a.len();
        let plain_b = sample_plain_header(compressed_b.len() as u32, raw_b.len() as u32);
        let enc_header_b = build_encrypted_page_header(header_b_offset, plain_b);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&enc_header_a);
        bytes.extend_from_slice(&compressed_a);
        bytes.extend_from_slice(&enc_header_b);
        bytes.extend_from_slice(&compressed_b);

        let descriptor = SectionDescriptor {
            compressed_size: (compressed_a.len() + compressed_b.len()) as u64,
            page_count: 2,
            decompressed_size: raw_a.len() as u64,
            compressed_code: 2,
            section_id: 7,
            encrypted: 0,
            name: "Synthetic:TwoPage".into(),
            local_sections: vec![
                LocalSectionMap {
                    page_number: 1,
                    compressed_size: compressed_a.len() as u64,
                    offset: 0,
                    decompressed_size: raw_a.len() as u64,
                    seeker: header_a_offset as i64,
                },
                LocalSectionMap {
                    page_number: 2,
                    compressed_size: compressed_b.len() as u64,
                    offset: raw_a.len() as u64,
                    decompressed_size: raw_b.len() as u64,
                    seeker: header_b_offset as i64,
                },
            ],
        };

        let out = read_section_payload(&bytes, &descriptor).expect("two-page concat");
        let mut expected = Vec::with_capacity(raw_a.len() + raw_b.len());
        expected.extend_from_slice(&raw_a);
        expected.extend_from_slice(&raw_b);
        assert_eq!(out, expected);
    }

    #[test]
    fn read_section_payload_handles_uncompressed_section() {
        let raw: Vec<u8> = (0u8..32).collect();
        let plain = sample_plain_header(raw.len() as u32, raw.len() as u32);
        let enc_header = build_encrypted_page_header(0, plain);

        let mut bytes = Vec::with_capacity(PAGE_HEADER_LEN + raw.len());
        bytes.extend_from_slice(&enc_header);
        bytes.extend_from_slice(&raw);

        let descriptor = SectionDescriptor {
            compressed_size: raw.len() as u64,
            page_count: 1,
            decompressed_size: raw.len() as u64,
            compressed_code: 1, // uncompressed
            section_id: 0,
            encrypted: 0,
            name: "Synthetic:Raw".into(),
            local_sections: vec![LocalSectionMap {
                page_number: 1,
                compressed_size: raw.len() as u64,
                offset: 0,
                decompressed_size: raw.len() as u64,
                seeker: 0,
            }],
        };

        let out = read_section_payload(&bytes, &descriptor).expect("uncompressed copy");
        assert_eq!(out, raw);
    }

    #[test]
    fn read_section_payload_rejects_invalid_page_type() {
        let mut plain = sample_plain_header(4, 4);
        plain.section_type = 0xDEADBEEF;
        let enc_header = build_encrypted_page_header(0, plain);

        let mut bytes = Vec::with_capacity(PAGE_HEADER_LEN + 4);
        bytes.extend_from_slice(&enc_header);
        bytes.extend_from_slice(&[0u8; 4]);

        let descriptor = SectionDescriptor {
            compressed_size: 4,
            page_count: 1,
            decompressed_size: 4,
            compressed_code: 2,
            section_id: 0,
            encrypted: 0,
            name: "Synthetic:WrongType".into(),
            local_sections: vec![LocalSectionMap {
                page_number: 1,
                compressed_size: 4,
                offset: 0,
                decompressed_size: 4,
                seeker: 0,
            }],
        };

        let err = read_section_payload(&bytes, &descriptor).unwrap_err();
        assert_eq!(
            err,
            SectionDataDecodeError::InvalidPageType {
                actual: 0xDEADBEEF,
                offset: 0,
            }
        );
    }

    #[test]
    fn read_section_payload_rejects_unsupported_compression_code() {
        let descriptor = SectionDescriptor {
            compressed_size: 0,
            page_count: 1,
            decompressed_size: 0,
            compressed_code: 7, // not in {1, 2}
            section_id: 0,
            encrypted: 0,
            name: "Synthetic:BadCode".into(),
            local_sections: vec![LocalSectionMap {
                page_number: 1,
                compressed_size: 0,
                offset: 0,
                decompressed_size: 0,
                seeker: 0,
            }],
        };
        let err = read_section_payload(&[], &descriptor).unwrap_err();
        assert_eq!(
            err,
            SectionDataDecodeError::UnsupportedCompressedCode { actual: 7 }
        );
    }

    #[test]
    fn read_section_payload_rejects_page_out_of_bounds() {
        let descriptor = SectionDescriptor {
            compressed_size: 4,
            page_count: 1,
            decompressed_size: 4,
            compressed_code: 2,
            section_id: 0,
            encrypted: 0,
            name: "Synthetic:OOB".into(),
            local_sections: vec![LocalSectionMap {
                page_number: 1,
                compressed_size: 4,
                offset: 0,
                decompressed_size: 4,
                seeker: 0x1000, // way past empty buffer
            }],
        };
        let err = read_section_payload(&[], &descriptor).unwrap_err();
        assert!(
            matches!(err, SectionDataDecodeError::PageOutOfBounds { .. }),
            "expected PageOutOfBounds, got {err:?}"
        );
    }

    #[test]
    fn read_section_payload_rejects_negative_seeker() {
        let descriptor = SectionDescriptor {
            compressed_size: 4,
            page_count: 1,
            decompressed_size: 4,
            compressed_code: 2,
            section_id: 0,
            encrypted: 0,
            name: "Synthetic:NegSeeker".into(),
            local_sections: vec![LocalSectionMap {
                page_number: 1,
                compressed_size: 4,
                offset: 0,
                decompressed_size: 4,
                seeker: -1,
            }],
        };
        let err = read_section_payload(&[0u8; 0x100], &descriptor).unwrap_err();
        assert!(
            matches!(err, SectionDataDecodeError::PageOutOfBounds { seeker: -1, .. }),
            "expected PageOutOfBounds with seeker == -1, got {err:?}"
        );
    }

    #[test]
    fn read_section_payload_rejects_empty_descriptor() {
        let descriptor = SectionDescriptor {
            compressed_size: 0,
            page_count: 0,
            decompressed_size: 0,
            compressed_code: 2,
            section_id: 0,
            encrypted: 0,
            name: "Synthetic:Empty".into(),
            local_sections: Vec::new(),
        };
        let err = read_section_payload(&[], &descriptor).unwrap_err();
        assert_eq!(
            err,
            SectionDataDecodeError::EmptyDescriptor {
                name: "Synthetic:Empty".into(),
            }
        );
    }

    #[test]
    fn section_data_decode_error_display_strings_include_diagnostics() {
        let trunc = format!(
            "{}",
            SectionDataDecodeError::TruncatedPageHeader {
                offset: 0x10,
                expected_at_least: 32
            }
        );
        assert!(trunc.contains("offset 16"));
        assert!(trunc.contains("at least 32"));

        let invalid = format!(
            "{}",
            SectionDataDecodeError::InvalidPageType {
                actual: 0xCAFE_BABE,
                offset: 0x100
            }
        );
        assert!(invalid.contains("0xCAFEBABE"));
        assert!(invalid.contains("offset 256"));

        let oob = format!(
            "{}",
            SectionDataDecodeError::PageOutOfBounds {
                seeker: 0x10000,
                file_len: 0x100
            }
        );
        assert!(oob.contains("65536"));
        assert!(oob.contains("256"));

        let lz77 = format!(
            "{}",
            SectionDataDecodeError::Lz77 {
                source: Lz77DecodeError::TruncatedInput
            }
        );
        assert!(lz77.contains("LZ77"));

        let empty = format!(
            "{}",
            SectionDataDecodeError::EmptyDescriptor {
                name: "AcDb:Foo".into()
            }
        );
        assert!(empty.contains("\"AcDb:Foo\""));

        let bad = format!(
            "{}",
            SectionDataDecodeError::UnsupportedCompressedCode { actual: 7 }
        );
        assert!(bad.contains("compressed_code 7"));
    }
}
