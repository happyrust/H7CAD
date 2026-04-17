use crate::{DwgVersion, HandleMapEntry};
use h7cad_native_model::Handle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDocument {
    pub version: DwgVersion,
    pub section_count: u32,
    pub sections: Vec<PendingSection>,
    pub objects: Vec<PendingObject>,
    pub layers: Vec<PendingLayer>,
    pub entities: Vec<PendingEntity>,
    /// Decoded `AcDb:Handles` section, one entry per `(handle, offset)`
    /// pair. Entries appear in strictly increasing handle order because
    /// the underlying stream uses a delta encoding. Empty on versions /
    /// layouts where no Handle section was present in the descriptor
    /// table.
    pub handle_offsets: Vec<HandleMapEntry>,
}

impl PendingDocument {
    pub fn new(version: DwgVersion, section_count: u32) -> Self {
        Self {
            version,
            section_count,
            sections: Vec::new(),
            objects: Vec::new(),
            layers: Vec::new(),
            entities: Vec::new(),
            handle_offsets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSection {
    pub index: u32,
    pub offset: u32,
    pub size: u32,
    pub record_count: u32,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingLayer {
    pub handle: Handle,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEntity {
    pub handle: Handle,
    pub owner_handle: Handle,
    pub layer_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingObject {
    pub handle: Handle,
    pub owner_handle: Handle,
    pub section_index: u32,
    pub kind: PendingObjectKind,
    pub semantic_identity: Option<String>,
    pub semantic_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingObjectKind {
    TableRecord {
        record_index: u32,
        payload_size: usize,
    },
    EntityRecord {
        record_index: u32,
        payload_size: usize,
    },
    ObjectRecord {
        record_index: u32,
        payload_size: usize,
    },
}
