use super::{record_index, record_payload_size, DispatchTarget, ParsedRecordSummary};
use crate::PendingObject;

pub fn dispatch(_object: &PendingObject) -> DispatchTarget {
    DispatchTarget::Object
}

pub fn summarize(object: &PendingObject) -> ParsedRecordSummary {
    ParsedRecordSummary {
        target: DispatchTarget::Object,
        section_index: object.section_index,
        record_index: record_index(object),
        payload_size: record_payload_size(object),
        semantic_identity: object
            .semantic_identity
            .clone()
            .unwrap_or_else(|| "object".to_string()),
        semantic_link: object.semantic_link.clone().unwrap_or_default(),
    }
}
