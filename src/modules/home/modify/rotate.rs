// Rotate tool — ribbon definition + interactive command.
//
// Command:  ROTATE (RO)
//   Requires at least one entity selected before starting.
//   Step 1: pick rotation center
//   Step 2: pick reference point (defines the 0° direction)
//   Step 3: pick destination point → rotates by (dest_angle - ref_angle)

use acadrust::Handle;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult, EntityTransform};
use crate::modules::home::defaults;
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

// ── Ribbon definition ──────────────────────────────────────────────────────

pub fn tool() -> ToolDef {
    ToolDef {
        id: "ROTATE",
        label: "Rotate",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/rotate.svg")),
        event: ModuleEvent::Command("ROTATE".to_string()),
    }
}

// ── Command implementation ─────────────────────────────────────────────────

enum Step {
    Center,
    RefPoint { center: Vec3 },
    Angle { center: Vec3, ref_angle: f32 },
}

pub struct RotateCommand {
    handles: Vec<Handle>,
    wire_models: Vec<WireModel>,
    step: Step,
    default_angle: f32, // degrees
}

impl RotateCommand {
    pub fn new(handles: Vec<Handle>, wire_models: Vec<WireModel>) -> Self {
        Self {
            handles,
            wire_models,
            step: Step::Center,
            default_angle: defaults::get_rotate_angle(),
        }
    }
}

impl CadCommand for RotateCommand {
    fn name(&self) -> &'static str {
        "ROTATE"
    }

    fn prompt(&self) -> String {
        match &self.step {
            Step::Center => format!(
                "ROTATE  Specify rotation center  [{} objects]:",
                self.handles.len()
            ),
            Step::RefPoint { .. } => {
                "ROTATE  Specify reference point  (or skip: type angle directly):".into()
            }
            Step::Angle { ref_angle, .. } => format!(
                "ROTATE  Specify destination or type angle in degrees  <{:.4}>  [ref={:.1}°]:",
                self.default_angle,
                ref_angle.to_degrees()
            ),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match &self.step {
            Step::Center => {
                self.step = Step::RefPoint { center: pt };
                CmdResult::NeedPoint
            }
            Step::RefPoint { center } => {
                let center = *center;
                let ref_angle = (pt.y - center.y).atan2(pt.x - center.x);
                self.step = Step::Angle { center, ref_angle };
                CmdResult::NeedPoint
            }
            Step::Angle { center, ref_angle } => {
                let center = *center;
                let ref_angle = *ref_angle;
                let dest_angle = (pt.y - center.y).atan2(pt.x - center.x);
                let delta = dest_angle - ref_angle;
                defaults::set_rotate_angle(delta.to_degrees());
                self.default_angle = delta.to_degrees();
                CmdResult::TransformSelected(
                    self.handles.clone(),
                    EntityTransform::Rotate {
                        center,
                        angle_rad: delta,
                    },
                )
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult {
        // At Angle step: Enter uses the stored default angle.
        if let Step::Angle { center, .. } = &self.step {
            let center = *center;
            let angle_rad = self.default_angle.to_radians();
            return CmdResult::TransformSelected(
                self.handles.clone(),
                EntityTransform::Rotate { center, angle_rad },
            );
        }
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        if let Step::Angle { center, .. } = &self.step {
            let deg: f32 = text.trim().replace(',', ".").parse().ok()?;
            let center = *center;
            defaults::set_rotate_angle(deg);
            return Some(CmdResult::TransformSelected(
                self.handles.clone(),
                EntityTransform::Rotate {
                    center,
                    angle_rad: deg.to_radians(),
                },
            ));
        }
        None
    }

    fn on_preview_wires(&mut self, pt: Vec3) -> Vec<WireModel> {
        let (center, ref_angle) = match &self.step {
            Step::Angle { center, ref_angle } => (*center, *ref_angle),
            Step::RefPoint { center } => {
                // Show a reference line from center to cursor only.
                return vec![WireModel::solid(
                    "rubber_band".into(),
                    vec![[center.x, center.y, center.z], [pt.x, pt.y, pt.z]],
                    WireModel::CYAN,
                    false,
                )];
            }
            _ => return vec![],
        };
        let dest_angle = (pt.y - center.y).atan2(pt.x - center.x);
        let angle_rad = dest_angle - ref_angle;
        // Object ghosts rotated to new angle.
        let mut out: Vec<WireModel> = self
            .wire_models
            .iter()
            .map(|w| w.rotated(center, angle_rad))
            .collect();
        // Arc rubber-band showing the rotation sweep.
        let r = center.distance(pt).max(0.3);
        let mut end = dest_angle;
        if end < ref_angle {
            end += std::f32::consts::TAU;
        }
        let span = end - ref_angle;
        let segs = ((span.abs() / std::f32::consts::TAU) * 48.0)
            .ceil()
            .max(4.0) as u32;
        let mut arc_pts: Vec<[f32; 3]> = (0..=segs)
            .map(|i| {
                let a = ref_angle + span * (i as f32 / segs as f32);
                [center.x + r * a.cos(), center.y + r * a.sin(), center.z]
            })
            .collect();
        arc_pts.push([center.x, center.y, center.z]);
        arc_pts.push([
            center.x + r * ref_angle.cos(),
            center.y + r * ref_angle.sin(),
            center.z,
        ]);
        out.push(WireModel::solid(
            "rubber_band".into(),
            arc_pts,
            WireModel::CYAN,
            false,
        ));
        out
    }
}
