// Scale tool — ribbon definition + interactive command.
//
// Command:  SCALE (SC)
//   Requires at least one entity selected.
//   Step 1: pick base (scale center)
//   Step 2: pick reference point  (defines reference distance)
//   Step 3: pick new point        (new distance = scale factor * objects)

use acadrust::Handle;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult, EntityTransform};
use crate::modules::home::defaults;
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

#[allow(dead_code)]
pub fn tool() -> ToolDef {
    ToolDef {
        id: "SCALE",
        label: "Scale",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/scale.svg")),
        event: ModuleEvent::Command("SCALE".to_string()),
    }
}

enum Step {
    Base,
    Ref { base: Vec3 },
    New { base: Vec3, ref_dist: f32 },
}

pub struct ScaleCommand {
    handles: Vec<Handle>,
    wire_models: Vec<WireModel>,
    step: Step,
    default_factor: f32,
}

impl ScaleCommand {
    pub fn new(handles: Vec<Handle>, wire_models: Vec<WireModel>) -> Self {
        Self {
            handles,
            wire_models,
            step: Step::Base,
            default_factor: defaults::get_scale_factor(),
        }
    }
}

impl CadCommand for ScaleCommand {
    fn name(&self) -> &'static str {
        "SCALE"
    }

    fn prompt(&self) -> String {
        match &self.step {
            Step::Base => format!(
                "SCALE  Specify base point  [{} objects]:",
                self.handles.len()
            ),
            Step::Ref { .. } => {
                "SCALE  Specify reference point  (or type scale factor directly):".into()
            }
            Step::New { ref_dist, .. } => format!(
                "SCALE  Specify new point or type scale factor  <{:.4}>  [ref={:.3}]:",
                self.default_factor, ref_dist
            ),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match &self.step {
            Step::Base => {
                self.step = Step::Ref { base: pt };
                CmdResult::NeedPoint
            }
            Step::Ref { base } => {
                let base = *base;
                let ref_dist = base.distance(pt).max(1e-6);
                self.step = Step::New { base, ref_dist };
                CmdResult::NeedPoint
            }
            Step::New { base, ref_dist } => {
                let base = *base;
                let new_dist = base.distance(pt).max(1e-6);
                let factor = new_dist / *ref_dist;
                defaults::set_scale_factor(factor);
                CmdResult::TransformSelected(
                    self.handles.clone(),
                    EntityTransform::Scale {
                        center: base,
                        factor,
                    },
                )
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        // At New-point step: Enter uses the stored default factor applied from base.
        if let Step::New { base, .. } = &self.step {
            let base = *base;
            let factor = self.default_factor;
            return CmdResult::TransformSelected(
                self.handles.clone(),
                EntityTransform::Scale {
                    center: base,
                    factor,
                },
            );
        }
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        if let Step::New { base, .. } | Step::Ref { base } = &self.step {
            let factor: f32 = text.trim().replace(',', ".").parse().ok()?;
            if factor > 0.0 {
                let base = *base;
                defaults::set_scale_factor(factor);
                return Some(CmdResult::TransformSelected(
                    self.handles.clone(),
                    EntityTransform::Scale {
                        center: base,
                        factor,
                    },
                ));
            }
        }
        None
    }

    fn on_preview_wires(&mut self, pt: Vec3) -> Vec<WireModel> {
        let (base, factor) = match &self.step {
            Step::Ref { base } => {
                // Reference pick: just show rubber-band line, no scale yet.
                return vec![WireModel::solid(
                    "rubber_band".into(),
                    vec![[base.x, base.y, base.z], [pt.x, pt.y, pt.z]],
                    WireModel::CYAN,
                    false,
                )];
            }
            Step::New { base, ref_dist } => {
                let f = base.distance(pt).max(1e-6) / ref_dist;
                (*base, f)
            }
            _ => return vec![],
        };
        let mut out: Vec<WireModel> = self
            .wire_models
            .iter()
            .map(|w| w.scaled(base, factor))
            .collect();
        out.push(WireModel::solid(
            "rubber_band".into(),
            vec![[base.x, base.y, base.z], [pt.x, pt.y, pt.z]],
            WireModel::CYAN,
            false,
        ));
        out
    }
}
