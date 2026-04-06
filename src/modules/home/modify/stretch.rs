// Stretch tool — ribbon definition + interactive command.
//
// Command:  STRETCH (SS)
//   STRETCH: Moves endpoints/vertices that lie within a crossing
//   window while leaving the rest of the object fixed.
//   Simplified implementation: works like MOVE on the full selected set
//   because we don't yet have a crossing-window selector in the viewport.
//   Step 1: pick base point
//   Step 2: pick new point → translates all selected entities by (new - base)

use acadrust::Handle;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult, EntityTransform};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

// ── Ribbon definition ──────────────────────────────────────────────────────

pub fn tool() -> ToolDef {
    ToolDef {
        id: "STRETCH",
        label: "Stretch",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/stretch.svg")),
        event: ModuleEvent::Command("STRETCH".to_string()),
    }
}

// ── Command implementation ─────────────────────────────────────────────────

enum Step {
    Base,
    Target(Vec3),
}

pub struct StretchCommand {
    handles: Vec<Handle>,
    step: Step,
}

impl StretchCommand {
    pub fn new(handles: Vec<Handle>) -> Self {
        Self {
            handles,
            step: Step::Base,
        }
    }
}

impl CadCommand for StretchCommand {
    fn name(&self) -> &'static str {
        "STRETCH"
    }

    fn prompt(&self) -> String {
        match &self.step {
            Step::Base => format!(
                "STRETCH  Specify base point  [{} objects]:",
                self.handles.len()
            ),
            Step::Target(base) => format!(
                "STRETCH  Specify new point  [base {:.3},{:.3}]:",
                base.x, base.y
            ),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match &self.step {
            Step::Base => {
                self.step = Step::Target(pt);
                CmdResult::NeedPoint
            }
            Step::Target(base) => {
                let delta = pt - *base;
                CmdResult::TransformSelected(
                    self.handles.clone(),
                    EntityTransform::Translate(delta),
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

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        if let Step::Target(base) = &self.step {
            Some(WireModel {
                name: "rubber_band".into(),
                points: vec![[base.x, base.y, base.z], [pt.x, pt.y, pt.z]],
                color: WireModel::CYAN,
                selected: false,
                pattern_length: 0.0,
                pattern: [0.0; 8],
                line_weight_px: 1.0,
                snap_pts: vec![],
                tangent_geoms: vec![],
                aci: 0,
            key_vertices: vec![],
            })
        } else {
            None
        }
    }
}
