// Mirror tool — ribbon definition + interactive command.
//
// Command:  MIRROR (MI)
//   Requires at least one entity selected.
//   Step 1: pick first mirror-line point
//   Step 2: pick second mirror-line point → mirrors

use acadrust::Handle;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult, EntityTransform};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

pub fn tool() -> ToolDef {
    ToolDef {
        id: "MIRROR",
        label: "Mirror",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/mirror.svg")),
        event: ModuleEvent::Command("MIRROR".to_string()),
    }
}

enum Step {
    P1,
    P2(Vec3),
}

pub struct MirrorCommand {
    handles: Vec<Handle>,
    wire_models: Vec<WireModel>,
    step: Step,
}

impl MirrorCommand {
    pub fn new(handles: Vec<Handle>, wire_models: Vec<WireModel>) -> Self {
        Self {
            handles,
            wire_models,
            step: Step::P1,
        }
    }
}

impl CadCommand for MirrorCommand {
    fn name(&self) -> &'static str {
        "MIRROR"
    }

    fn prompt(&self) -> String {
        match &self.step {
            Step::P1 => format!(
                "MIRROR  Specify first mirror-line point  [{} objects]:",
                self.handles.len()
            ),
            Step::P2(p1) => format!(
                "MIRROR  Specify second point  [p1={:.2},{:.2}]:",
                p1.x, p1.y
            ),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match &self.step {
            Step::P1 => {
                self.step = Step::P2(pt);
                CmdResult::NeedPoint
            }
            Step::P2(p1) => {
                let p1 = *p1;
                CmdResult::TransformSelected(
                    self.handles.clone(),
                    EntityTransform::Mirror { p1, p2: pt },
                )
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_preview_wires(&mut self, pt: Vec3) -> Vec<WireModel> {
        let Step::P2(p1) = &self.step else {
            return vec![];
        };
        let p1 = *p1;
        // Mirrored ghosts of all selected objects.
        let mut out: Vec<WireModel> = self
            .wire_models
            .iter()
            .map(|w| w.mirrored(p1, pt))
            .collect();
        // Mirror-axis line (rubber-band).
        out.push(WireModel::solid(
            "rubber_band".into(),
            vec![[p1.x, p1.y, p1.z], [pt.x, pt.y, pt.z]],
            WireModel::CYAN,
            false,
        ));
        out
    }
}
