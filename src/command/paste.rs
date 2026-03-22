// PasteCommand — picks one insertion point then fires CmdResult::PasteClipboard.
//
// Holds pre-computed wire previews (from clipboard entities) and the centroid
// so it can show a translated rubber-band preview while the user moves the mouse.

use acadrust::Handle;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::scene::wire_model::WireModel;

pub struct PasteCommand {
    /// Wire models of the clipboard entities (used for preview).
    wires: Vec<WireModel>,
    /// Centroid of the clipboard entities (offset origin for translation).
    centroid: Vec3,
}

impl PasteCommand {
    pub fn new(wires: Vec<WireModel>, centroid: Vec3) -> Self {
        Self { wires, centroid }
    }
}

impl CadCommand for PasteCommand {
    fn name(&self) -> &'static str {
        "PASTECLIP"
    }

    fn prompt(&self) -> String {
        "PASTECLIP  Pick insertion point:".into()
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        CmdResult::PasteClipboard { base_pt: pt }
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_hover_entity(&mut self, _handle: Handle, _pt: Vec3) -> Vec<WireModel> {
        vec![]
    }

    fn on_preview_wires(&mut self, pt: Vec3) -> Vec<WireModel> {
        let delta = pt - self.centroid;
        self.wires.iter().map(|w| w.translated(delta)).collect()
    }
}
