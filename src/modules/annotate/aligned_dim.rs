// DIMALIGNED command — aligned dimension (measures true distance between two points).

use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/dim_aligned.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "DIMALIGNED",
        label: "Aligned",
        icon: ICON,
        event: ModuleEvent::Command("DIMALIGNED".to_string()),
    }
}

enum Step {
    First,
    Second(Vec3),
    DimLine { p1: Vec3, p2: Vec3 },
}

pub struct AlignedDimensionCommand {
    step: Step,
}

impl AlignedDimensionCommand {
    pub fn new() -> Self {
        Self { step: Step::First }
    }
}

impl CadCommand for AlignedDimensionCommand {
    fn name(&self) -> &'static str { "DIMALIGNED" }

    fn prompt(&self) -> String {
        match self.step {
            Step::First           => "DIMALIGNED  Specify first extension line origin:".into(),
            Step::Second(_)       => "DIMALIGNED  Specify second extension line origin:".into(),
            Step::DimLine { .. }  => "DIMALIGNED  Specify dimension line location:".into(),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            Step::First => {
                self.step = Step::Second(pt);
                CmdResult::NeedPoint
            }
            Step::Second(p1) => {
                self.step = Step::DimLine { p1, p2: pt };
                CmdResult::NeedPoint
            }
            Step::DimLine { p1, p2 } => {
                let mid = Vec3::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5, (p1.z + p2.z) * 0.5);
                let measurement = (p2 - p1).length() as f64;
                let entity = nm::Entity::new(nm::EntityData::Dimension {
                    dim_type: 1,
                    block_name: String::new(),
                    style_name: String::new(),
                    definition_point: [pt.x as f64, pt.y as f64, pt.z as f64],
                    text_midpoint: [mid.x as f64, mid.y as f64, mid.z as f64],
                    text_override: String::new(),
                    attachment_point: 0,
                    measurement,
                    text_rotation: 0.0,
                    horizontal_direction: 0.0,
                    flip_arrow1: false,
                    flip_arrow2: false,
                    first_point: [p1.x as f64, p1.y as f64, p1.z as f64],
                    second_point: [p2.x as f64, p2.y as f64, p2.z as f64],
                    angle_vertex: [0.0; 3],
                    dimension_arc: [0.0; 3],
                    leader_length: 0.0,
                    rotation: 0.0,
                    ext_line_rotation: 0.0,
                });
                CmdResult::CommitAndExitNative(entity)
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult { CmdResult::Cancel }

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        let (p1, p2) = match self.step {
            Step::First => return None,
            Step::Second(p1) => (p1, pt),
            Step::DimLine { p1, p2 } => {
                return Some(preview_aligned(p1, p2, pt));
            }
        };
        Some(WireModel {
            name: "dimaligned_preview".into(),
            points: vec![[p1.x, p1.y, p1.z], [p2.x, p2.y, p2.z]],
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
    }
}

fn preview_aligned(p1: Vec3, p2: Vec3, dim_pt: Vec3) -> WireModel {
    // Show ext lines + dim line
    let dir = (p2 - p1).normalize_or_zero();
    let perp = Vec3::new(-dir.z, dir.y, dir.x).normalize_or_zero();
    let offset = (dim_pt - p2).dot(perp);
    let d1 = p1 + perp * offset;
    let d2 = p2 + perp * offset;
    WireModel {
        name: "dimaligned_preview".into(),
        points: vec![
            [p1.x, p1.y, p1.z], [d1.x, d1.y, d1.z],
            [f32::NAN, 0.0, 0.0],
            [p2.x, p2.y, p2.z], [d2.x, d2.y, d2.z],
            [f32::NAN, 0.0, 0.0],
            [d1.x, d1.y, d1.z], [d2.x, d2.y, d2.z],
        ],
        color: WireModel::CYAN,
        selected: false,
        pattern_length: 0.0,
        pattern: [0.0; 8],
        line_weight_px: 1.0,
        snap_pts: vec![],
        tangent_geoms: vec![],
        aci: 0,
            key_vertices: vec![],
    }
}
