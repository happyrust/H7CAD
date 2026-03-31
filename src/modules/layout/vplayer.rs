// VPLAYER — per-viewport layer freeze/thaw command.
//
// Usage (command line):
//   VPLAYER
//   > F <layer_name>   → freeze layer in active viewport
//   > T <layer_name>   → thaw layer in active viewport
//   > Enter            → exit
//
// Layer names are case-insensitive. Multiple space-separated names are accepted.

use acadrust::Handle;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::scene::wire_model::WireModel;

pub struct VplayerCommand {
    vp_handle: Handle,
}

impl VplayerCommand {
    pub fn new(vp_handle: Handle) -> Self {
        Self { vp_handle }
    }
}

impl CadCommand for VplayerCommand {
    fn name(&self) -> &'static str {
        "VPLAYER"
    }

    fn prompt(&self) -> String {
        "VPLAYER  F <layer> = Freeze  |  T <layer> = Thaw  |  Enter = Exit".to_string()
    }

    fn wants_text_input(&self) -> bool {
        true
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let text = text.trim();
        if text.is_empty() {
            return Some(CmdResult::Cancel);
        }

        let mut parts = text.splitn(2, char::is_whitespace);
        let op = parts.next().unwrap_or("").to_uppercase();
        let rest = parts.next().unwrap_or("").trim();

        let layer_names: Vec<String> = rest
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if layer_names.is_empty() {
            return None; // no layer name given — ignore and re-prompt
        }

        match op.as_str() {
            "F" | "FREEZE" => Some(CmdResult::VpLayerUpdate {
                vp_handle: self.vp_handle,
                freeze: layer_names,
                thaw: vec![],
            }),
            "T" | "THAW" => Some(CmdResult::VpLayerUpdate {
                vp_handle: self.vp_handle,
                freeze: vec![],
                thaw: layer_names,
            }),
            _ => None, // unknown op — ignore
        }
    }

    fn on_point(&mut self, _pt: Vec3) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_mouse_move(&mut self, _pt: Vec3) -> Option<WireModel> {
        None
    }
}
