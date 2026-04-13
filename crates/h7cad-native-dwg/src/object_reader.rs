mod common;
mod entities;
mod objects;
mod tables;

use crate::{PendingObject, PendingObjectKind};

pub use common::record_payload_size;
pub use common::{record_index, ParsedRecordSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchTarget {
    Table,
    Entity,
    Object,
}

pub fn dispatch_object(object: &PendingObject) -> DispatchTarget {
    match object.kind {
        PendingObjectKind::TableRecord { .. } => dispatch_table_record(object),
        PendingObjectKind::EntityRecord { .. } => dispatch_entity_record(object),
        PendingObjectKind::ObjectRecord { .. } => dispatch_object_record(object),
    }
}

pub fn dispatch_table_record(object: &PendingObject) -> DispatchTarget {
    tables::dispatch(object)
}

pub fn dispatch_entity_record(object: &PendingObject) -> DispatchTarget {
    entities::dispatch(object)
}

pub fn dispatch_object_record(object: &PendingObject) -> DispatchTarget {
    objects::dispatch(object)
}

pub fn summarize_object(object: &PendingObject) -> ParsedRecordSummary {
    match object.kind {
        PendingObjectKind::TableRecord { .. } => tables::summarize(object),
        PendingObjectKind::EntityRecord { .. } => entities::summarize(object),
        PendingObjectKind::ObjectRecord { .. } => objects::summarize(object),
    }
}
