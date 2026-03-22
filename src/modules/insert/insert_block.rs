use acadrust::entities::Insert;
use acadrust::types::Vector3;
use acadrust::EntityType;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

pub fn tool() -> ToolDef {
    ToolDef {
        id: "INSERT",
        label: "Insert Block",
        icon: IconKind::Svg(include_bytes!("../../../assets/icons/blocks/insert.svg")),
        event: ModuleEvent::Command("INSERT".to_string()),
    }
}

enum Step {
    Name,
    Point { name: String },
}

pub struct InsertBlockCommand {
    available: Vec<String>,
    step: Step,
}

impl InsertBlockCommand {
    pub fn new(available: Vec<String>) -> Self {
        Self {
            available,
            step: Step::Name,
        }
    }
}

impl CadCommand for InsertBlockCommand {
    fn name(&self) -> &'static str {
        "INSERT"
    }

    fn prompt(&self) -> String {
        match &self.step {
            Step::Name => {
                let hint = if self.available.is_empty() {
                    String::new()
                } else {
                    format!("  [{}]", self.available.join(", "))
                };
                format!("INSERT  Enter block name:{hint}")
            }
            Step::Point { name } => format!("INSERT  Specify insertion point for \"{}\":", name),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match &self.step {
            Step::Name => CmdResult::NeedPoint,
            Step::Point { name } => {
                CmdResult::CommitAndExit(EntityType::Insert(Insert::new(
                    name.clone(),
                    Vector3::new(pt.x as f64, pt.y as f64, pt.z as f64),
                )))
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.step, Step::Name)
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        if !matches!(self.step, Step::Name) {
            return None;
        }
        let name = text.trim();
        if !self.available.iter().any(|candidate| candidate.eq_ignore_ascii_case(name)) {
            return None;
        }
        self.step = Step::Point {
            name: name.to_string(),
        };
        Some(CmdResult::NeedPoint)
    }

    fn on_preview_wires(&mut self, _pt: Vec3) -> Vec<WireModel> {
        vec![]
    }
}
