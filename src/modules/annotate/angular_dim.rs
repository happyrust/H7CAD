use h7cad_native_model as nm;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;
use glam::Vec3;

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/dim_angular.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "DIMANGULAR",
        label: "Angular",
        icon: ICON,
        event: ModuleEvent::Command("DIMANGULAR".to_string()),
    }
}

enum Step {
    Vertex,
    FirstRay(Vec3),
    SecondRay { vertex: Vec3, first: Vec3 },
    ArcPoint { vertex: Vec3, first: Vec3, second: Vec3 },
}

pub struct AngularDimensionCommand {
    step: Step,
}

impl AngularDimensionCommand {
    pub fn new() -> Self {
        Self { step: Step::Vertex }
    }
}

impl CadCommand for AngularDimensionCommand {
    fn name(&self) -> &'static str {
        "DIMANGULAR"
    }

    fn prompt(&self) -> String {
        match self.step {
            Step::Vertex => "DIMANGULAR  Specify angle vertex:".into(),
            Step::FirstRay(_) => "DIMANGULAR  Specify first extension line point:".into(),
            Step::SecondRay { .. } => "DIMANGULAR  Specify second extension line point:".into(),
            Step::ArcPoint { .. } => "DIMANGULAR  Specify dimension arc location:".into(),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            Step::Vertex => {
                self.step = Step::FirstRay(pt);
                CmdResult::NeedPoint
            }
            Step::FirstRay(vertex) => {
                self.step = Step::SecondRay { vertex, first: pt };
                CmdResult::NeedPoint
            }
            Step::SecondRay { vertex, first } => {
                self.step = Step::ArcPoint {
                    vertex,
                    first,
                    second: pt,
                };
                CmdResult::NeedPoint
            }
            Step::ArcPoint {
                vertex,
                first,
                second,
            } => {
                let a0 = ((first.y - vertex.y) as f64).atan2((first.x - vertex.x) as f64);
                let a1 = ((second.y - vertex.y) as f64).atan2((second.x - vertex.x) as f64);
                let delta = {
                    let d = a1 - a0;
                    let mut d = d.rem_euclid(std::f64::consts::TAU);
                    if d > std::f64::consts::PI {
                        d -= std::f64::consts::TAU;
                    }
                    d.abs()
                };
                let measurement = delta.to_degrees();
                let entity = nm::Entity::new(nm::EntityData::Dimension {
                    dim_type: 5,
                    block_name: String::new(),
                    style_name: String::new(),
                    definition_point: [pt.x as f64, pt.y as f64, pt.z as f64],
                    text_midpoint: [pt.x as f64, pt.y as f64, pt.z as f64],
                    text_override: String::new(),
                    attachment_point: 0,
                    measurement,
                    text_rotation: 0.0,
                    horizontal_direction: 0.0,
                    flip_arrow1: false,
                    flip_arrow2: false,
                    first_point: [first.x as f64, first.y as f64, first.z as f64],
                    second_point: [second.x as f64, second.y as f64, second.z as f64],
                    angle_vertex: [vertex.x as f64, vertex.y as f64, vertex.z as f64],
                    dimension_arc: [0.0; 3],
                    leader_length: 0.0,
                    rotation: 0.0,
                    ext_line_rotation: 0.0,
                });
                CmdResult::CommitAndExitNative(entity)
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
        match self.step {
            Step::Vertex => None,
            Step::FirstRay(vertex) => Some(preview_wire(vec![vertex, pt])),
            Step::SecondRay { vertex, first } => {
                Some(preview_wire(vec![vertex, first, Vec3::new(f32::NAN, f32::NAN, f32::NAN), vertex, pt]))
            }
            Step::ArcPoint {
                vertex,
                first,
                second,
            } => Some(preview_wire(angular_preview(vertex, first, second, pt))),
        }
    }
}

fn preview_wire(points: Vec<Vec3>) -> WireModel {
    WireModel {
        name: "dimangular_preview".to_string(),
        points: points.into_iter().map(|p| [p.x, p.y, p.z]).collect(),
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

fn angular_preview(vertex: Vec3, first: Vec3, second: Vec3, arc_pt: Vec3) -> Vec<Vec3> {
    let mut points = vec![
        vertex,
        first,
        Vec3::new(f32::NAN, f32::NAN, f32::NAN),
        vertex,
        second,
        Vec3::new(f32::NAN, f32::NAN, f32::NAN),
    ];
    let r = vertex.distance(arc_pt);
    if r <= 1e-6 {
        return points;
    }
    let a0 = (first.y - vertex.y).atan2(first.x - vertex.x);
    let mut a1 = (second.y - vertex.y).atan2(second.x - vertex.x);
    let mut delta = a1 - a0;
    while delta <= 0.0 {
        delta += std::f32::consts::TAU;
    }
    if delta > std::f32::consts::PI {
        a1 -= std::f32::consts::TAU;
        delta = a1 - a0;
    }
    let steps = 24;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let a = a0 + delta * t;
        points.push(vertex + Vec3::new(a.cos() * r, a.sin() * r, 0.0));
    }
    points
}
