use crate::{pending::PendingDocument, summarize_object, DispatchTarget, DwgReadError};
use h7cad_native_model::{
    BlockRecord, CadDocument, CadObject, Entity, EntityData, Handle, LayerProperties, Layout,
    ObjectData,
};

pub fn resolve_document(pending: &PendingDocument) -> Result<CadDocument, DwgReadError> {
    let mut doc = CadDocument::new();
    doc.header.handseed = doc.next_handle();

    for layer in &pending.layers {
        let mut props = LayerProperties::new(layer.name.clone());
        props.handle = layer.handle;
        doc.tables.layer.insert(props.name.clone(), props.handle);
        doc.layers.insert(props.name.clone(), props);
        doc.set_next_handle(layer.handle.value() + 1);
    }

    for object in &pending.objects {
        if let Some(block_record) = semantic_block_record(object) {
            match doc.block_record_by_handle(block_record.handle) {
                Some(existing) if existing.name == block_record.name => {}
                Some(_) => {
                    return Err(DwgReadError::SemanticDecode {
                        section_index: object.section_index,
                        record_index: record_index(object),
                        reason: format!(
                            "parsed block handle {:X} conflicts with existing block record",
                            block_record.handle.value()
                        ),
                    });
                }
                None => doc.insert_block_record(block_record),
            }
            doc.set_next_handle(object.handle.value() + 1);
        }
    }

    for object in &pending.objects {
        if let Some(layout) = semantic_layout(object)? {
            let layout_name = layout.name.clone();
            match doc.layouts.get(&layout.handle) {
                Some(existing)
                    if existing.name == layout_name
                        && existing.block_record_handle == layout.block_record_handle => {}
                Some(_) => {
                    return Err(DwgReadError::SemanticDecode {
                        section_index: object.section_index,
                        record_index: record_index(object),
                        reason: format!(
                            "parsed layout handle {:X} conflicts with existing layout",
                            layout.handle.value()
                        ),
                    });
                }
                None => doc.insert_layout(layout),
            }

            if let Some(block_record) = doc.block_records.get_mut(&semantic_handle(object)) {
                if block_record.layout_handle.is_none() {
                    block_record.layout_handle = doc
                        .layouts
                        .get(&semantic_handle(object))
                        .map(|layout| layout.handle);
                }
            }
            doc.set_next_handle(object.handle.value() + 1);
        }
    }

    for entity in &pending.entities {
        let resolved = semantic_entity(entity);
        doc.add_entity(resolved).map_err(|reason| DwgReadError::SemanticDecode {
            section_index: section_index_for_handle(pending, entity.handle),
            record_index: record_index_for_handle(pending, entity.handle),
            reason,
        })?;
        doc.set_next_handle(entity.handle.value() + 1);
        if entity.owner_handle != Handle::NULL {
            doc.set_next_handle(entity.owner_handle.value() + 1);
        }
    }

    for object in &pending.objects {
        if is_materialized_semantic_object(object) {
            continue;
        }
        let summary = summarize_object(object);
        let prefix = match summary.target {
            DispatchTarget::Table => "DWG_TABLE",
            DispatchTarget::Entity => "DWG_ENTITY",
            DispatchTarget::Object => "DWG_OBJECT",
        };
        let data = ObjectData::Unknown {
            object_type: format!(
                "{prefix}_SECTION_{}_RECORD_{}_SIZE_{}_{}_{}",
                summary.section_index,
                summary.record_index,
                summary.payload_size,
                summary.semantic_identity.to_ascii_uppercase().replace(':', "_"),
                summary.semantic_link.to_ascii_uppercase().replace(':', "_")
            ),
        };
        doc.objects.push(CadObject {
            handle: object.handle,
            owner_handle: object.owner_handle,
            data,
        });
        doc.set_next_handle(object.handle.value() + 1);
        if object.owner_handle != Handle::NULL {
            doc.set_next_handle(object.owner_handle.value() + 1);
        }
    }

    doc.repair_ownership();
    Ok(doc)
}

fn semantic_block_record(object: &crate::PendingObject) -> Option<BlockRecord> {
    let identity = object.semantic_identity.as_deref()?;
    let name = identity.strip_prefix("block:")?;
    let mut block = BlockRecord::new(object.handle, name);
    if let Some(layout_name) = object
        .semantic_link
        .as_deref()
        .and_then(|link| link.strip_prefix("layout:"))
    {
        if let Some(layout_handle) = reserved_layout_handle(layout_name) {
            block.layout_handle = Some(layout_handle);
        }
    }
    Some(block)
}

fn semantic_layout(object: &crate::PendingObject) -> Result<Option<Layout>, DwgReadError> {
    let Some(identity) = object.semantic_identity.as_deref() else {
        return Ok(None);
    };
    let Some(name) = identity.strip_prefix("layout:") else {
        return Ok(None);
    };
    let block_record_handle = object
        .semantic_link
        .as_deref()
        .and_then(|link| link.strip_prefix("block_handle:"))
        .and_then(|value| u64::from_str_radix(value, 16).ok())
        .map(Handle::new)
        .ok_or_else(|| DwgReadError::SemanticDecode {
            section_index: object.section_index,
            record_index: record_index(object),
            reason: "parsed layout is missing a valid block handle".to_string(),
        })?;
    Ok(Some(Layout::new(object.handle, name, block_record_handle)))
}

fn semantic_entity(entity: &crate::PendingEntity) -> Entity {
    let mut resolved = Entity::new(EntityData::Point {
        position: [0.0, 0.0, 0.0],
    });
    resolved.handle = entity.handle;
    resolved.owner_handle = entity.owner_handle;
    resolved.layer_name = if entity.layer_name.is_empty() {
        "0".to_string()
    } else {
        entity.layer_name.clone()
    };
    resolved
}

fn is_materialized_semantic_object(object: &crate::PendingObject) -> bool {
    matches!(
        object.semantic_identity.as_deref(),
        Some(identity) if identity.starts_with("block:") || identity.starts_with("layout:")
    ) || object
        .semantic_identity
        .as_deref()
        .is_some_and(|identity| identity.starts_with("entity:"))
}

fn section_index_for_handle(pending: &PendingDocument, handle: Handle) -> u32 {
    pending
        .objects
        .iter()
        .find(|object| object.handle == handle)
        .map(|object| object.section_index)
        .unwrap_or(0)
}

fn record_index_for_handle(pending: &PendingDocument, handle: Handle) -> u32 {
    pending
        .objects
        .iter()
        .find(|object| object.handle == handle)
        .map(record_index)
        .unwrap_or(0)
}

fn semantic_handle(object: &crate::PendingObject) -> Handle {
    object.handle
}

fn record_index(object: &crate::PendingObject) -> u32 {
    match object.kind {
        crate::PendingObjectKind::TableRecord { record_index, .. }
        | crate::PendingObjectKind::EntityRecord { record_index, .. }
        | crate::PendingObjectKind::ObjectRecord { record_index, .. } => record_index,
    }
}

fn reserved_layout_handle(name: &str) -> Option<Handle> {
    match name {
        "Model" => Some(Handle::new(3)),
        "Layout1" => Some(Handle::new(4)),
        _ => None,
    }
}
