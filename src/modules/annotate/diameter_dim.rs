// DIMDIAMETER command — diameter dimension for circles and arcs.

use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/dim_diameter.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "DIMDIAMETER",
        label: "Diameter",
        icon: ICON,
        event: ModuleEvent::Command("DIMDIAMETER".to_string()),
    }
}

enum Step {
    CenterPoint,
    ArcPoint(Vec3),
    TextPoint { center: Vec3, arc_pt: Vec3 },
}

pub struct DiameterDimensionCommand {
    step: Step,
}

impl DiameterDimensionCommand {
    pub fn new() -> Self {
        Self { step: Step::CenterPoint }
    }
}

impl CadCommand for DiameterDimensionCommand {
    fn name(&self) -> &'static str { "DIMDIAMETER" }

    fn prompt(&self) -> String {
        match self.step {
            Step::CenterPoint    => "DIMDIAMETER  Specify center point:".into(),
            Step::ArcPoint(_)    => "DIMDIAMETER  Specify point on circle:".into(),
            Step::TextPoint {..} => "DIMDIAMETER  Specify dimension line location:".into(),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            Step::CenterPoint => {
                self.step = Step::ArcPoint(pt);
                CmdResult::NeedPoint
            }
            Step::ArcPoint(center) => {
                self.step = Step::TextPoint { center, arc_pt: pt };
                CmdResult::NeedPoint
            }
            Step::TextPoint { center, arc_pt } => {
                let measurement = (center.distance(arc_pt) * 2.0) as f64;
                let entity = nm::Entity::new(nm::EntityData::Dimension {
                    dim_type: 3,
                    block_name: String::new(),
                    style_name: String::new(),
                    definition_point: [arc_pt.x as f64, arc_pt.y as f64, arc_pt.z as f64],
                    text_midpoint: [pt.x as f64, pt.y as f64, pt.z as f64],
                    text_override: String::new(),
                    attachment_point: 0,
                    measurement,
                    text_rotation: 0.0,
                    horizontal_direction: 0.0,
                    flip_arrow1: false,
                    flip_arrow2: false,
                    first_point: [0.0; 3],
                    second_point: [0.0; 3],
                    angle_vertex: [center.x as f64, center.y as f64, center.z as f64],
                    dimension_arc: [0.0; 3],
                    leader_length: arc_pt.distance(pt) as f64,
                    rotation: 0.0,
                    ext_line_rotation: 0.0,
                });
                CmdResult::CommitAndExitNative(entity)
            }
        }
    }

    fn on_enter(&mut self) -> CmdResult { CmdResult::Cancel }

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        match self.step {
            Step::CenterPoint => None,
            Step::ArcPoint(center) => Some(preview_line(center, pt)),
            Step::TextPoint { center, arc_pt } => {
                let far = center + (center - arc_pt); // opposite point on circle
                Some(preview_line(far, pt))
            }
        }
    }
}

fn preview_line(a: Vec3, b: Vec3) -> WireModel {
    WireModel {
        name: "dimdia_preview".into(),
        points: vec![[a.x, a.y, a.z], [b.x, b.y, b.z]],
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
