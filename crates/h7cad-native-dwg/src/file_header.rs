use crate::{DwgReadError, DwgVersion};

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
        DwgVersion::Ac1015 => Ok(0x15),
        DwgVersion::Ac1018 => Ok(0x19),
        DwgVersion::Ac1012
        | DwgVersion::Ac1014
        | DwgVersion::Ac1021
        | DwgVersion::Ac1024
        | DwgVersion::Ac1027
        | DwgVersion::Ac1032 => Err(DwgReadError::UnsupportedVersion(version)),
    }
}
