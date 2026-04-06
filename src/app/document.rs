use crate::scene::Scene;
use crate::ui::{LayerPanel, PropertiesPanel};
use crate::command::CadCommand;
use crate::snap::SnapResult;
use crate::scene::grip::GripEdit;
use crate::scene::GripDef;
use acadrust::{CadDocument, Handle};
use acadrust::tables::Ucs;
use crate::linetypes;
use std::path::PathBuf;

// ── Per-document tab state ─────────────────────────────────────────────────

pub(super) struct DocumentTab {
    pub(super) scene: Scene,
    pub(super) current_path: Option<PathBuf>,
    pub(super) dirty: bool,
    pub(super) tab_title: String,
    pub(super) properties: PropertiesPanel,
    pub(super) layers: LayerPanel,
    pub(super) active_cmd: Option<Box<dyn CadCommand>>,
    pub(super) last_cmd: Option<String>,
    pub(super) snap_result: Option<SnapResult>,
    pub(super) active_grip: Option<GripEdit>,
    pub(super) selected_grips: Vec<GripDef>,
    pub(super) selected_handle: Option<Handle>,
    pub(super) wireframe: bool,
    pub(super) visual_style: String,
    pub(super) last_cursor_world: glam::Vec3,
    pub(super) history: HistoryState,
    pub(super) active_layer: String,
    /// Currently active UCS. `None` means WCS (identity transform).
    pub(super) active_ucs: Option<Ucs>,
}

impl DocumentTab {
    pub(super) fn new_drawing(n: usize) -> Self {
        let mut scene = Scene::new();
        linetypes::populate_document(&mut scene.document);
        Self {
            scene,
            current_path: None,
            dirty: false,
            tab_title: format!("Drawing{}", n),
            properties: PropertiesPanel::empty(),
            layers: LayerPanel::default(),
            active_cmd: None,
            last_cmd: None,
            snap_result: None,
            active_grip: None,
            selected_grips: vec![],
            selected_handle: None,
            wireframe: false,
            visual_style: "Shaded".into(),
            last_cursor_world: glam::Vec3::ZERO,
            history: HistoryState::default(),
            active_layer: "0".to_string(),
            active_ucs: None,
        }
    }

    pub(super) fn tab_display_name(&self) -> String {
        match &self.current_path {
            Some(p) => p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            None => self.tab_title.clone(),
        }
    }
}

#[derive(Clone)]
pub(super) struct HistorySnapshot {
    pub(super) document: CadDocument,
    pub(super) current_layout: String,
    pub(super) selected: Vec<Handle>,
    pub(super) dirty: bool,
    pub(super) label: String,
}

#[derive(Default)]
pub(super) struct HistoryState {
    pub(super) undo_stack: Vec<HistorySnapshot>,
    pub(super) redo_stack: Vec<HistorySnapshot>,
}
