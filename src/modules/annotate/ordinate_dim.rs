// DIMORDINATE command — ordinate (datum) dimension.
//
// Measures the X or Y distance from the UCS origin (datum) to a feature point.
// The user picks:
//   1. The feature location.
//   2. The leader endpoint (where the annotation line ends).
//
// If the leader moves mainly in Y → X-type ordinate (shows X coordinate).
// If the leader moves mainly in X → Y-type ordinate (shows Y coordinate).

use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

pub fn tool() -> ToolDef {
    ToolDef {
        id: "DIMORDINATE",
        label: "Ordinate",
        icon: IconKind::Svg(include_bytes!("../../../assets/icons/dim_ordinate.svg")),
        event: ModuleEvent::Command("DIMORDINATE".to_string()),
    }
}

enum Step {
    FeaturePoint,
    LeaderEndpoint { feature: Vec3 },
}

pub struct OrdinateDimCommand {
    step: Step,
}

impl OrdinateDimCommand {
    pub fn new() -> Self {
        Self { step: Step::FeaturePoint }
    }
}

impl CadCommand for OrdinateDimCommand {
    fn name(&self) -> &'static str { "DIMORDINATE" }

    fn prompt(&self) -> String {
        match self.step {
            Step::FeaturePoint => "DIMORDINATE  Specify feature location:".into(),
            Step::LeaderEndpoint { .. } => "DIMORDINATE  Specify leader endpoint:".into(),
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        match self.step {
            Step::FeaturePoint => {
                self.step = Step::LeaderEndpoint { feature: pt };
                CmdResult::NeedPoint
            }
            Step::LeaderEndpoint { feature } => {
                let dx = (pt.x - feature.x).abs();
                let dy = (pt.z - feature.z).abs();
                let is_x = dy >= dx;
                let dim_type = 6 | if is_x { 0x40 } else { 0 };
                let entity = nm::Entity::new(nm::EntityData::Dimension {
                    dim_type,
                    block_name: String::new(),
                    style_name: String::new(),
                    definition_point: [0.0; 3],
                    text_midpoint: [pt.x as f64, 0.0, pt.z as f64],
                    text_override: String::new(),
                    attachment_point: 0,
                    measurement: 0.0,
                    text_rotation: 0.0,
                    horizontal_direction: 0.0,
                    flip_arrow1: false,
                    flip_arrow2: false,
                    first_point: [feature.x as f64, 0.0, feature.z as f64],
                    second_point: [pt.x as f64, 0.0, pt.z as f64],
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
    fn on_preview_wires(&mut self, _pt: Vec3) -> Vec<WireModel> { vec![] }
}
