use crate::{DwgReadError, DwgVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DwgReaderCursor<'a> {
    version: DwgVersion,
    bytes: &'a [u8],
    byte_offset: usize,
}

impl<'a> DwgReaderCursor<'a> {
    pub fn new(version: DwgVersion, bytes: &'a [u8]) -> Self {
        Self {
            version,
            bytes,
            byte_offset: 0,
        }
    }

    pub fn version(&self) -> DwgVersion {
        self.version
    }

    pub fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.byte_offset)
    }

    pub fn read_u8(&mut self) -> Result<u8, DwgReadError> {
        let value = *self
            .bytes
            .get(self.byte_offset)
            .ok_or(DwgReadError::UnexpectedEof {
                context: "reader byte",
            })?;
        self.byte_offset += 1;
        Ok(value)
    }

    pub fn read_u32_le(&mut self) -> Result<u32, DwgReadError> {
        let bytes = self.read_exact(4, "reader u32")?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_exact(&mut self, len: usize, context: &'static str) -> Result<&'a [u8], DwgReadError> {
        if self.remaining() < len {
            return Err(DwgReadError::UnexpectedEof { context });
        }
        let start = self.byte_offset;
        let end = start + len;
        self.byte_offset = end;
        Ok(&self.bytes[start..end])
    }
}
