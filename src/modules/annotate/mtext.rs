use acadrust::types::Vector3;
use acadrust::{EntityType, MText};

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;
use glam::Vec3;

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/mtext.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "MTEXT",
        label: "MText",
        icon: ICON,
        event: ModuleEvent::Command("MTEXT".to_string()),
    }
}

enum Step {
    InsertPoint,
    WaitText(Vec3),
}

pub struct MTextCommand {
    step: Step,
}

impl MTextCommand {
    pub fn new() -> Self {
        Self {
            step: Step::InsertPoint,
        }
    }
}

impl CadCommand for MTextCommand {
    fn name(&self) -> &'static str {
        "MTEXT"
    }

    fn prompt(&self) -> String {
        match &self.step {
            Step::InsertPoint => "MTEXT  Specify insertion point:".into(),
            Step::WaitText(pos) => format!(
                "MTEXT  Type text, press Enter  [at {:.2},{:.2}]:",
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
            let mt = MText {
                insertion_point: Vector3::new(pos.x as f64, pos.y as f64, pos.z as f64),
                value: text.to_string(),
                height: 0.25,
                rectangle_width: text.len() as f64 * 0.15,
                ..Default::default()
            };
            Some(CmdResult::CommitEntity(EntityType::MText(mt)))
        } else {
            None
        }
    }

    fn on_mouse_move(&mut self, _pt: Vec3) -> Option<WireModel> {
        None
    }
}
