use crate::{DwgReadError, DwgVersion};

/// DWG AC1015 (AutoCAD R2000) on-disk file header layout:
/// ```text
/// 0x00 6 bytes magic "AC1015"
/// 0x06 7 bytes (6 zeros + 1 release byte)
/// 0x0D 4 bytes preview address (image seeker)
/// 0x11 2 bytes undocumented
/// 0x13 2 bytes codepage
/// 0x15 4 bytes section_count (raw long)
/// 0x19 section locator directory, 9 bytes per record
/// ```
pub(crate) const AC1015_SECTION_COUNT_OFFSET: usize = 0x15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DwgFileHeader {
    pub version: DwgVersion,
    pub magic: String,
    pub section_directory_offset: usize,
    pub section_count: u32,
}

impl DwgFileHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self, DwgReadError> {
        let version = crate::sniff_version(bytes)?;
        let section_count_offset = section_count_offset(version)?;

        let needed = section_count_offset + 4;
        if bytes.len() < needed {
            return Err(DwgReadError::TruncatedHeader {
                expected_at_least: needed,
            });
        }

        let section_count = u32::from_le_bytes([
            bytes[section_count_offset],
            bytes[section_count_offset + 1],
            bytes[section_count_offset + 2],
            bytes[section_count_offset + 3],
        ]);

        // Guard against obvious junk. Real AC1015 drawings in the wild
        // report fewer than a dozen section records; any value above a
        // sanity cap almost certainly means we are reading from the
        // wrong offset for this (as-yet unsupported) layout.
        if section_count > crate::section_map::MAX_SECTION_RECORDS {
            return Err(DwgReadError::UnsupportedHeaderLayout { version });
        }

        Ok(Self {
            version,
            magic: String::from_utf8_lossy(&bytes[..6]).into_owned(),
            section_directory_offset: section_count_offset + 4,
            section_count,
        })
    }
}

pub(crate) fn section_count_offset(version: DwgVersion) -> Result<usize, DwgReadError> {
    match version {
        DwgVersion::Ac1015 => Ok(AC1015_SECTION_COUNT_OFFSET),
        // AC1018+ uses an encrypted 0x6C-byte metadata block starting at
        // offset 0x80. The section table does not live in plain sight,
        // so the previous hand-rolled 0x19 offset is deliberately
        // rejected until that decoder lands.
        DwgVersion::Ac1018 => Err(DwgReadError::UnsupportedHeaderLayout { version }),
        DwgVersion::Ac1012
        | DwgVersion::Ac1014
        | DwgVersion::Ac1021
        | DwgVersion::Ac1024
        | DwgVersion::Ac1027
        | DwgVersion::Ac1032 => Err(DwgReadError::UnsupportedVersion(version)),
    }
}
