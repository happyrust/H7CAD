// UngroupCommand — selection gathering then dissolve all groups that
// contain any of the selected handles.
// Result: CmdResult::DeleteGroups { handles }

use acadrust::Handle;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::scene::wire_model::WireModel;

pub struct UngroupCommand;

impl UngroupCommand {
    pub fn new() -> Self {
        Self
    }
}

impl CadCommand for UngroupCommand {
    fn name(&self) -> &'static str {
        "UNGROUP"
    }

    fn prompt(&self) -> String {
        "UNGROUP  Select grouped objects:".into()
    }

    fn is_selection_gathering(&self) -> bool {
        true
    }

    fn on_selection_complete(&mut self, handles: Vec<Handle>) -> CmdResult {
        if handles.is_empty() {
            return CmdResult::NeedPoint;
        }
        CmdResult::DeleteGroups { handles }
    }

    fn on_point(&mut self, _pt: Vec3) -> CmdResult {
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_hover_entity(&mut self, _handle: Handle, _pt: Vec3) -> Vec<WireModel> {
        vec![]
    }
}
