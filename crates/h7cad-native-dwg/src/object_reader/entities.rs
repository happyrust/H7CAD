use super::{record_index, record_payload_size, DispatchTarget, ParsedRecordSummary};
use crate::PendingObject;

pub fn dispatch(_object: &PendingObject) -> DispatchTarget {
    DispatchTarget::Entity
}

pub fn summarize(object: &PendingObject) -> ParsedRecordSummary {
    ParsedRecordSummary {
        target: DispatchTarget::Entity,
        section_index: object.section_index,
        record_index: record_index(object),
        payload_size: record_payload_size(object),
    }
}
