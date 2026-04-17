use h7cad_native_model as nm;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;
use glam::Vec3;

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/text.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "TEXT",
        label: "Text",
        icon: ICON,
        event: ModuleEvent::Command("TEXT".to_string()),
    }
}

enum Step {
    InsertPoint,
    WaitText(Vec3),
}

pub struct TextCommand {
    step: Step,
}

impl TextCommand {
    pub fn new() -> Self {
        Self {
            step: Step::InsertPoint,
        }
    }
}

impl CadCommand for TextCommand {
    fn name(&self) -> &'static str {
        "TEXT"
    }

    fn prompt(&self) -> String {
        match &self.step {
            Step::InsertPoint => "TEXT  Specify insertion point:".into(),
            Step::WaitText(pos) => format!(
                "TEXT  Type text, press Enter  [at {:.2},{:.2}]:",
                pos.x, pos.z
            ),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        self.step = Step::WaitText(pt);
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.step, Step::WaitText(_))
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        if let Step::WaitText(pos) = &self.step {
            if text.trim().is_empty() {
                return Some(CmdResult::Cancel);
            }
            let entity = nm::Entity::new(nm::EntityData::Text {
                insertion: [pos.x as f64, pos.y as f64, pos.z as f64],
                height: 0.25,
                value: text.to_string(),
                rotation: 0.0,
                style_name: String::new(),
                width_factor: 1.0,
                oblique_angle: 0.0,
                horizontal_alignment: 0,
                vertical_alignment: 0,
                alignment_point: None,
            });
            Some(CmdResult::CommitEntityNative(entity))
        } else {
            None
        }
    }

    fn on_mouse_move(&mut self, _pt: Vec3) -> Option<WireModel> {
        None
    }
}
