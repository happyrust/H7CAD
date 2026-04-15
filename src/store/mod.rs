mod native_store;

pub use native_store::NativeStore;

use h7cad_native_model as nm;
use std::collections::BTreeMap;
use std::path::Path;

/// Unified document store — single abstraction over the CAD document backend.
///
/// During the migration period both `NativeStore` (wrapping `nm::CadDocument`)
/// and legacy compat paths coexist.  All new consumer code should depend on
/// `CadStore` rather than reaching into concrete document types directly.
pub trait CadStore {
    // ── Entity CRUD ──────────────────────────────────────────────────────

    fn get_entity(&self, handle: nm::Handle) -> Option<&nm::Entity>;
    fn get_entity_mut(&mut self, handle: nm::Handle) -> Option<&mut nm::Entity>;
    fn add_entity(&mut self, entity: nm::Entity) -> Result<nm::Handle, String>;
    fn remove_entity(&mut self, handle: nm::Handle) -> Option<nm::Entity>;

    /// Top-level (model-space + paper-space root) entities.
    fn entities(&self) -> &[nm::Entity];

    fn allocate_handle(&mut self) -> nm::Handle;

    // ── Tables / metadata ────────────────────────────────────────────────

    fn layers(&self) -> &BTreeMap<String, nm::LayerProperties>;
    fn layers_mut(&mut self) -> &mut BTreeMap<String, nm::LayerProperties>;
    fn model_space_handle(&self) -> nm::Handle;
    fn paper_space_handle(&self) -> nm::Handle;
    fn text_style_names(&self) -> Vec<String>;

    // ── Common property editing ────────────────────────────────────────

    fn set_entity_layer(&mut self, handle: nm::Handle, layer: &str) -> bool;
    fn set_entity_color(&mut self, handle: nm::Handle, color_index: i16, true_color: i32) -> bool;
    fn set_entity_linetype(&mut self, handle: nm::Handle, linetype: &str) -> bool;
    fn set_entity_lineweight(&mut self, handle: nm::Handle, lineweight: i16) -> bool;
    fn set_entity_linetype_scale(&mut self, handle: nm::Handle, scale: f64) -> bool;
    fn set_entity_invisible(&mut self, handle: nm::Handle, invisible: bool) -> bool;
    fn set_entity_transparency(&mut self, handle: nm::Handle, transparency: i32) -> bool;

    // ── Persistence ──────────────────────────────────────────────────────

    fn save(&self, path: &Path) -> Result<(), String>;

    // ── Snapshot / undo ──────────────────────────────────────────────────

    fn snapshot(&self) -> StoreSnapshot;
    fn restore(&mut self, snapshot: StoreSnapshot);
}

/// Opaque undo snapshot.  Internally holds a cloned native document.
#[derive(Clone)]
pub struct StoreSnapshot {
    pub(crate) doc: nm::CadDocument,
}
