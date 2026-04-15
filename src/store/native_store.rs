use std::collections::BTreeMap;
use std::path::Path;

use h7cad_native_model as nm;

use super::{CadStore, StoreSnapshot};

/// Primary `CadStore` implementation backed by `h7cad_native_model::CadDocument`.
pub struct NativeStore {
    doc: nm::CadDocument,
}

impl NativeStore {
    pub fn new(doc: nm::CadDocument) -> Self {
        Self { doc }
    }

    pub fn into_inner(self) -> nm::CadDocument {
        self.doc
    }

    pub fn inner(&self) -> &nm::CadDocument {
        &self.doc
    }

    pub fn inner_mut(&mut self) -> &mut nm::CadDocument {
        &mut self.doc
    }
}

impl CadStore for NativeStore {
    // ── Entity CRUD ──────────────────────────────────────────────────────

    fn get_entity(&self, handle: nm::Handle) -> Option<&nm::Entity> {
        self.doc.get_entity(handle)
    }

    fn get_entity_mut(&mut self, handle: nm::Handle) -> Option<&mut nm::Entity> {
        self.doc.get_entity_mut(handle)
    }

    fn add_entity(&mut self, entity: nm::Entity) -> Result<nm::Handle, String> {
        self.doc.add_entity(entity)
    }

    fn remove_entity(&mut self, handle: nm::Handle) -> Option<nm::Entity> {
        self.doc.remove_entity(handle)
    }

    fn entities(&self) -> &[nm::Entity] {
        &self.doc.entities
    }

    fn allocate_handle(&mut self) -> nm::Handle {
        self.doc.allocate_handle()
    }

    // ── Tables / metadata ────────────────────────────────────────────────

    fn layers(&self) -> &BTreeMap<String, nm::LayerProperties> {
        &self.doc.layers
    }

    fn layers_mut(&mut self) -> &mut BTreeMap<String, nm::LayerProperties> {
        &mut self.doc.layers
    }

    fn model_space_handle(&self) -> nm::Handle {
        self.doc.model_space_handle()
    }

    fn paper_space_handle(&self) -> nm::Handle {
        self.doc.paper_space_handle()
    }

    fn text_style_names(&self) -> Vec<String> {
        self.doc.text_styles.keys().cloned().collect()
    }

    // ── Common property editing ────────────────────────────────────────

    fn set_entity_layer(&mut self, handle: nm::Handle, layer: &str) -> bool {
        if let Some(entity) = self.doc.get_entity_mut(handle) {
            entity.layer_name = layer.to_string();
            true
        } else {
            false
        }
    }

    fn set_entity_color(&mut self, handle: nm::Handle, color_index: i16, true_color: i32) -> bool {
        if let Some(entity) = self.doc.get_entity_mut(handle) {
            entity.color_index = color_index;
            entity.true_color = true_color;
            true
        } else {
            false
        }
    }

    fn set_entity_linetype(&mut self, handle: nm::Handle, linetype: &str) -> bool {
        if let Some(entity) = self.doc.get_entity_mut(handle) {
            entity.linetype_name = linetype.to_string();
            true
        } else {
            false
        }
    }

    fn set_entity_lineweight(&mut self, handle: nm::Handle, lineweight: i16) -> bool {
        if let Some(entity) = self.doc.get_entity_mut(handle) {
            entity.lineweight = lineweight;
            true
        } else {
            false
        }
    }

    fn set_entity_invisible(&mut self, handle: nm::Handle, invisible: bool) -> bool {
        if let Some(entity) = self.doc.get_entity_mut(handle) {
            entity.invisible = invisible;
            true
        } else {
            false
        }
    }

    fn set_entity_transparency(&mut self, handle: nm::Handle, transparency: i32) -> bool {
        if let Some(entity) = self.doc.get_entity_mut(handle) {
            entity.transparency = transparency;
            true
        } else {
            false
        }
    }

    // ── Persistence ──────────────────────────────────────────────────────

    fn save(&self, path: &Path) -> Result<(), String> {
        crate::io::save_native(&self.doc, path)
    }

    // ── Snapshot / undo ──────────────────────────────────────────────────

    fn snapshot(&self) -> StoreSnapshot {
        StoreSnapshot {
            doc: self.doc.clone(),
        }
    }

    fn restore(&mut self, snapshot: StoreSnapshot) {
        self.doc = snapshot.doc;
    }
}
