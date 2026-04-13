use crate::{DwgFileHeader, DwgReadError, DwgVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionDescriptor {
    pub index: u32,
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
        let entry_size = 8usize;
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
            let offset = u32::from_le_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]);
            let size = u32::from_le_bytes([
                bytes[cursor + 4],
                bytes[cursor + 5],
                bytes[cursor + 6],
                bytes[cursor + 7],
            ]);
            descriptors.push(SectionDescriptor {
                index,
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
