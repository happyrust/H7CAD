use crate::{DwgFileHeader, DwgReadError, DwgVersion};

/// Upper bound on the number of section locator records the parser will
/// tolerate before treating the count as corrupt. AutoCAD R15/R18 files
/// in the wild have fewer than a dozen records; clamping here prevents
/// garbage bytes from driving us into a gigabyte-scale allocation.
pub(crate) const MAX_SECTION_RECORDS: u32 = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionDescriptor {
    pub index: u32,
    /// AC1015 stores a 1-byte `record_number` before each seeker/size.
    /// Kept here so later milestones can route sections by identity.
    pub record_number: u8,
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionMap {
    pub version: DwgVersion,
    pub descriptors: Vec<SectionDescriptor>,
}

impl SectionMap {
    pub fn parse(bytes: &[u8], header: &DwgFileHeader) -> Result<Self, DwgReadError> {
        if header.section_count > MAX_SECTION_RECORDS {
            return Err(DwgReadError::TruncatedSectionDirectory {
                version: header.version,
                expected_at_least: 0,
                actual: header.section_count as usize,
            });
        }

        let entry_size = section_locator_entry_size(header.version)?;
        let needed = header.section_directory_offset + header.section_count as usize * entry_size;
        if bytes.len() < needed {
            return Err(DwgReadError::TruncatedSectionDirectory {
                version: header.version,
                expected_at_least: needed,
                actual: bytes.len(),
            });
        }

        let mut descriptors = Vec::with_capacity(header.section_count as usize);
        let mut cursor = header.section_directory_offset;
        for index in 0..header.section_count {
            let (record_number, offset, size) =
                decode_locator_entry(header.version, &bytes[cursor..cursor + entry_size]);
            descriptors.push(SectionDescriptor {
                index,
                record_number,
                offset,
                size,
            });
            cursor += entry_size;
        }

        Ok(Self {
            version: header.version,
            descriptors,
        })
    }

    pub fn read_section_payloads(&self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, DwgReadError> {
        self.descriptors
            .iter()
            .map(|descriptor| {
                let start = descriptor.offset as usize;
                let end = start.saturating_add(descriptor.size as usize);
                if end > bytes.len() {
                    return Err(DwgReadError::SectionOutOfBounds {
                        index: descriptor.index,
                        offset: start,
                        size: descriptor.size as usize,
                        actual: bytes.len(),
                    });
                }
                Ok(bytes[start..end].to_vec())
            })
            .collect()
    }
}

/// AC1015 section locator records are 9 bytes:
///   1 byte record number | 4 byte seeker | 4 byte size
/// Later versions (AC1018+) use a different on-disk layout and are
/// routed through a dedicated decoder path once implemented.
fn section_locator_entry_size(version: DwgVersion) -> Result<usize, DwgReadError> {
    match version {
        DwgVersion::Ac1015 => Ok(9),
        other => Err(DwgReadError::UnsupportedHeaderLayout { version: other }),
    }
}

fn decode_locator_entry(version: DwgVersion, entry: &[u8]) -> (u8, u32, u32) {
    match version {
        DwgVersion::Ac1015 => {
            let record_number = entry[0];
            let offset = u32::from_le_bytes([entry[1], entry[2], entry[3], entry[4]]);
            let size = u32::from_le_bytes([entry[5], entry[6], entry[7], entry[8]]);
            (record_number, offset, size)
        }
        _ => {
            // Only AC1015 currently reaches this path; other versions are
            // rejected upstream via `section_locator_entry_size`.
            (0, 0, 0)
        }
    }
}
