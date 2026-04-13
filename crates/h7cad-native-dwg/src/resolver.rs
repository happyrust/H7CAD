use crate::{pending::PendingDocument, summarize_object, DispatchTarget};
use h7cad_native_model::{CadDocument, CadObject, Handle, LayerProperties, ObjectData};

pub fn resolve_document(pending: &PendingDocument) -> CadDocument {
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
        let summary = summarize_object(object);
        let prefix = match summary.target {
            DispatchTarget::Table => "DWG_TABLE",
            DispatchTarget::Entity => "DWG_ENTITY",
            DispatchTarget::Object => "DWG_OBJECT",
        };
        let data = ObjectData::Unknown {
            object_type: format!(
                "{prefix}_SECTION_{}_RECORD_{}_SIZE_{}",
                summary.section_index, summary.record_index, summary.payload_size
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
    doc
}
