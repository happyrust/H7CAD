use crate::PendingObject;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRecordSummary {
    pub target: super::DispatchTarget,
    pub section_index: u32,
    pub record_index: u32,
    pub payload_size: usize,
}

pub fn record_payload_size(object: &PendingObject) -> usize {
    match object.kind {
        crate::PendingObjectKind::TableRecord { payload_size, .. }
        | crate::PendingObjectKind::EntityRecord { payload_size, .. }
        | crate::PendingObjectKind::ObjectRecord { payload_size, .. } => payload_size,
    }
}

pub fn record_index(object: &PendingObject) -> u32 {
    match object.kind {
        crate::PendingObjectKind::TableRecord { record_index, .. }
        | crate::PendingObjectKind::EntityRecord { record_index, .. }
        | crate::PendingObjectKind::ObjectRecord { record_index, .. } => record_index,
    }
}
