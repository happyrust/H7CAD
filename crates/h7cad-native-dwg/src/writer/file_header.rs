//! AC1015 (R2000) file header writer — mirror of [`crate::DwgFileHeader::parse`]
//! and [`crate::SectionMap::parse`].
//!
//! Reader contract (see `crate::file_header` and `crate::section_map`):
//!
//! ```text
//! 0x00..0x06   "AC1015"
//! 0x06..0x0D   7 bytes (6 zeros + 1 release marker)
//! 0x0D..0x11   4 bytes preview-image seeker (little-endian u32)
//! 0x11..0x13   2 bytes undocumented
//! 0x13..0x15   2 bytes codepage (little-endian u16)
//! 0x15..0x19   4 bytes section_count (little-endian u32)
//! 0x19..       section locator directory, 9 bytes / record:
//!                 1 byte record_number
//!                 4 bytes seeker offset (LE u32)
//!                 4 bytes section size  (LE u32)
//! ```
//!
//! The reader only inspects the magic bytes and `section_count` from
//! the prefix; the bytes between `0x06` and `0x15` are tolerated as
//! opaque padding. The writer fills them with documented defaults so
//! downstream tooling that *does* inspect them sees plausible values:
//!
//! - Release marker (`0x06..0x0D`): six zeros + `0x00`. AutoCAD's own
//!   files put a small release-id byte here (e.g. `0x00` for plain
//!   R2000); a value of `0x00` is accepted everywhere we have data.
//! - Preview seeker (`0x0D..0x11`): `0xFFFFFFFF`, the documented
//!   sentinel for "no preview image". Writers that emit a preview can
//!   patch this in M2.T3 once the preview composer lands.
//! - Undocumented (`0x11..0x13`): zero. ACadSharp ignores these bytes
//!   on read; writers historically emit zeros.
//! - Codepage (`0x13..0x15`): `30` (ANSI_1252). Matches AutoCAD's
//!   default for R2000 drawings authored on Western locales.
//!
//! These defaults can be overridden in a follow-up brick once the
//! writer needs to honor a [`h7cad_native_model::CadDocument`] header
//! variable that maps to one of them; for the M2.T2 mile they exist
//! purely so the prefix is a fixed-size, deterministic byte slab.

use crate::section_map::SectionDescriptor;
use crate::DwgWriteError;

/// AC1015 file header prefix length: 6 magic + 7 release + 4 preview
/// + 2 undocumented + 2 codepage + 4 section_count = `0x19` bytes.
pub const AC1015_FILE_HEADER_PREFIX_LEN: usize = 0x19;

/// Each AC1015 section locator directory entry is 9 bytes: one
/// `record_number` byte followed by two little-endian u32s
/// (`seeker offset`, `section size`).
pub const AC1015_SECTION_LOCATOR_ENTRY_LEN: usize = 9;

const AC1015_MAGIC: [u8; 6] = *b"AC1015";
const AC1015_RELEASE_MARKER: [u8; 7] = [0; 7];
const AC1015_PREVIEW_SEEKER_DEFAULT: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
const AC1015_UNDOCUMENTED_DEFAULT: [u8; 2] = [0; 2];
/// ANSI_1252 codepage id, matching AutoCAD's default for R2000 drawings
/// authored under Western locales.
const AC1015_CODEPAGE_DEFAULT_LE: [u8; 2] = 30u16.to_le_bytes();

/// Maximum section_count the reader will accept. Mirrors the upper
/// bound enforced by [`crate::section_map::MAX_SECTION_RECORDS`].
const AC1015_MAX_SECTION_RECORDS: u32 = 128;

/// Emit the 25-byte (`0x19`) AC1015 file header prefix, with the
/// reader-relevant `section_count` filled in. The bytes between the
/// magic and the section_count carry documented defaults; see this
/// module's overview for the field map.
///
/// Returns [`DwgWriteError::InvalidValue`] if `section_count` exceeds
/// the reader's section-count cap; this matches the corresponding
/// reader sanity check and prevents writers from producing files that
/// would be immediately rejected on read-back.
pub fn write_ac1015_file_header_prefix(
    section_count: u32,
) -> Result<[u8; AC1015_FILE_HEADER_PREFIX_LEN], DwgWriteError> {
    if section_count > AC1015_MAX_SECTION_RECORDS {
        return Err(DwgWriteError::InvalidValue(format!(
            "AC1015 section_count {section_count} exceeds reader cap {AC1015_MAX_SECTION_RECORDS}"
        )));
    }

    let mut bytes = [0u8; AC1015_FILE_HEADER_PREFIX_LEN];
    bytes[0..6].copy_from_slice(&AC1015_MAGIC);
    bytes[6..0x0D].copy_from_slice(&AC1015_RELEASE_MARKER);
    bytes[0x0D..0x11].copy_from_slice(&AC1015_PREVIEW_SEEKER_DEFAULT);
    bytes[0x11..0x13].copy_from_slice(&AC1015_UNDOCUMENTED_DEFAULT);
    bytes[0x13..0x15].copy_from_slice(&AC1015_CODEPAGE_DEFAULT_LE);
    bytes[0x15..0x19].copy_from_slice(&section_count.to_le_bytes());
    Ok(bytes)
}

/// Emit the AC1015 section locator directory: one
/// [`AC1015_SECTION_LOCATOR_ENTRY_LEN`]-byte record per descriptor,
/// in the order supplied by the caller. Each record encodes
/// `record_number` then little-endian `offset` then little-endian
/// `size`, the inverse of [`crate::section_map::SectionMap::parse`].
///
/// Returns [`DwgWriteError::InvalidValue`] if more descriptors are
/// supplied than the reader's section-count cap permits.
pub fn write_ac1015_section_locator_directory(
    descriptors: &[SectionDescriptor],
) -> Result<Vec<u8>, DwgWriteError> {
    if descriptors.len() as u32 > AC1015_MAX_SECTION_RECORDS {
        return Err(DwgWriteError::InvalidValue(format!(
            "AC1015 section directory length {} exceeds reader cap {AC1015_MAX_SECTION_RECORDS}",
            descriptors.len()
        )));
    }

    let mut bytes = Vec::with_capacity(descriptors.len() * AC1015_SECTION_LOCATOR_ENTRY_LEN);
    for descriptor in descriptors {
        bytes.push(descriptor.record_number);
        bytes.extend_from_slice(&descriptor.offset.to_le_bytes());
        bytes.extend_from_slice(&descriptor.size.to_le_bytes());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DwgFileHeader, DwgVersion, SectionMap};

    #[test]
    fn prefix_has_fixed_length_and_starts_with_ac1015_magic() {
        let bytes = write_ac1015_file_header_prefix(0).expect("section_count 0 is in range");
        assert_eq!(bytes.len(), AC1015_FILE_HEADER_PREFIX_LEN);
        assert_eq!(bytes.len(), 0x19);
        assert_eq!(&bytes[0..6], b"AC1015");
    }

    #[test]
    fn prefix_encodes_section_count_at_documented_offset() {
        let bytes = write_ac1015_file_header_prefix(7).expect("section_count 7 is in range");
        // section_count lives at `0x15..0x19`, little-endian.
        assert_eq!(&bytes[0x15..0x19], &7u32.to_le_bytes());
    }

    #[test]
    fn prefix_uses_documented_defaults_for_padding_bytes() {
        let bytes = write_ac1015_file_header_prefix(0).expect("section_count 0 is in range");
        // Release marker (6 zeros + release byte) — all zero by default.
        assert_eq!(&bytes[0x06..0x0D], &[0u8; 7]);
        // Preview seeker — sentinel "no preview".
        assert_eq!(&bytes[0x0D..0x11], &[0xFF, 0xFF, 0xFF, 0xFF]);
        // Undocumented — zeros.
        assert_eq!(&bytes[0x11..0x13], &[0u8; 2]);
        // Codepage — ANSI_1252 (`30`).
        assert_eq!(&bytes[0x13..0x15], &30u16.to_le_bytes());
    }

    #[test]
    fn prefix_rejects_section_count_above_reader_cap() {
        let err = write_ac1015_file_header_prefix(AC1015_MAX_SECTION_RECORDS + 1)
            .expect_err("section_count above cap must be rejected");
        match err {
            DwgWriteError::InvalidValue(msg) => assert!(
                msg.contains("section_count"),
                "error should mention section_count: `{msg}`"
            ),
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }

    #[test]
    fn prefix_round_trips_through_dwg_file_header_parser() {
        // Build a fixture: prefix + zero-section directory. The parser
        // only needs the prefix to populate `DwgFileHeader`, but we
        // append a (length-zero) directory to match the on-disk layout.
        let prefix = write_ac1015_file_header_prefix(0).expect("section_count 0 is in range");
        let directory =
            write_ac1015_section_locator_directory(&[]).expect("empty directory writes");
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(&directory);

        let parsed = DwgFileHeader::parse(&bytes).expect("writer prefix must parse back");
        assert_eq!(parsed.version, DwgVersion::Ac1015);
        assert_eq!(parsed.magic, "AC1015");
        assert_eq!(parsed.section_count, 0);
        assert_eq!(parsed.section_directory_offset, 0x19);
    }

    #[test]
    fn directory_length_matches_descriptor_count() {
        let descriptors = sample_six_known_sections();
        let directory =
            write_ac1015_section_locator_directory(&descriptors).expect("six entries fit");
        assert_eq!(
            directory.len(),
            6 * AC1015_SECTION_LOCATOR_ENTRY_LEN,
            "expected {} bytes for 6 entries",
            6 * AC1015_SECTION_LOCATOR_ENTRY_LEN
        );
    }

    #[test]
    fn directory_round_trips_through_section_map_parser() {
        let descriptors = sample_six_known_sections();
        let prefix = write_ac1015_file_header_prefix(descriptors.len() as u32)
            .expect("six descriptors fit the cap");
        let directory =
            write_ac1015_section_locator_directory(&descriptors).expect("six entries fit");

        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(&directory);

        let header = DwgFileHeader::parse(&bytes).expect("file header parses");
        let parsed_map = SectionMap::parse(&bytes, &header).expect("section map parses");

        assert_eq!(parsed_map.version, DwgVersion::Ac1015);
        assert_eq!(parsed_map.descriptors.len(), descriptors.len());
        for (parsed, original) in parsed_map.descriptors.iter().zip(descriptors.iter()) {
            assert_eq!(parsed.record_number, original.record_number);
            assert_eq!(parsed.offset, original.offset);
            assert_eq!(parsed.size, original.size);
        }
    }

    #[test]
    fn directory_rejects_more_entries_than_reader_cap() {
        // Build a synthetic descriptor slice longer than the reader's
        // cap. Using an iterator avoids materialising the data twice.
        let descriptors: Vec<SectionDescriptor> = (0..AC1015_MAX_SECTION_RECORDS + 1)
            .map(|i| SectionDescriptor {
                index: i,
                record_number: 0,
                offset: 0,
                size: 0,
            })
            .collect();
        let err = write_ac1015_section_locator_directory(&descriptors)
            .expect_err("over-cap directory must be rejected");
        match err {
            DwgWriteError::InvalidValue(msg) => assert!(
                msg.contains("section directory length"),
                "error should mention section directory length: `{msg}`"
            ),
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }

    fn sample_six_known_sections() -> Vec<SectionDescriptor> {
        // Six AC1015 known sections in canonical order. Offsets and
        // sizes are illustrative; the reader does not validate that
        // they refer to real payloads, only that the directory bytes
        // round-trip exactly. The corresponding payloads are emitted
        // by F5.M2.T3 onwards.
        (0..6)
            .map(|n| SectionDescriptor {
                index: n as u32,
                record_number: n,
                offset: 0x100 + (n as u32) * 0x40,
                size: 0x20 + (n as u32),
            })
            .collect()
    }
}
