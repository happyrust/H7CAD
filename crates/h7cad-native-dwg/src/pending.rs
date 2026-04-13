use crate::DwgVersion;
use h7cad_native_model::Handle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDocument {
    pub version: DwgVersion,
    pub section_count: u32,
    pub sections: Vec<PendingSection>,
    pub objects: Vec<PendingObject>,
    pub layers: Vec<PendingLayer>,
    pub entities: Vec<PendingEntity>,
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
