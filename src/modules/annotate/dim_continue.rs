// DIMCONTINUE command — chain linear/aligned dimensions end-to-end.
//
// Each new point becomes the second extension line origin of a new dimension,
// whose first extension line origin is the second extension line of the previous dim.
// The dimension line stays at the same perpendicular offset as the base dimension.
//
// Constructed from commands.rs after finding the last placed linear/aligned dimension.

use h7cad_native_model as nm;
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/dim_continue.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "DIMCONTINUE",
        label: "Continue",
        icon: ICON,
        event: ModuleEvent::Command("DIMCONTINUE".to_string()),
    }
}

pub struct DimContinueCommand {
    /// Fixed first-extension-line origin for the current step (moves each iteration).
    chain_p1: Vec3,
    /// Direction along the dimension axis (0.0 = horizontal, PI/2 = vertical).
    rotation: f64,
    /// Perpendicular distance from the extension-line axis to the dimension line.
    /// Preserved from the base dimension.
    dim_offset: f32,
    /// Direction of "up" perpendicular to the dim axis (points toward the dim line).
    perp: Vec3,
    /// True once we have a base dimension loaded.
    ready: bool,
}

impl DimContinueCommand {
    /// No base dim found — will show an error prompt and cancel immediately.
    pub fn new() -> Self {
        Self {
            chain_p1: Vec3::ZERO,
            rotation: 0.0,
            dim_offset: 0.0,
            perp: Vec3::Y,
            ready: false,
        }
    }

    /// Build from the last placed dimension.
    ///
    /// `p1` / `p2` — extension line origins of the base dim.
    /// `definition_point` — where the dim line was placed (defines perpendicular offset).
    /// `rotation` — 0.0 = horizontal dim, PI/2 = vertical dim.
    pub fn from_base(p1: Vec3, p2: Vec3, definition_point: Vec3, rotation: f64) -> Self {
        // Axis unit vector along the measurement direction.
        let axis = if rotation.abs() < 0.1 { Vec3::X } else { Vec3::Y };
        // Perpendicular unit vector toward the dim line.
        let perp = Vec3::new(-axis.y, axis.x, 0.0);
        let dim_offset = (definition_point - p1).dot(perp);
        Self {
            chain_p1: p2,
            rotation,
            dim_offset,
            perp,
            ready: true,
        }
    }
}

impl CadCommand for DimContinueCommand {
    fn name(&self) -> &'static str { "DIMCONTINUE" }

    fn prompt(&self) -> String {
        if !self.ready {
            "DIMCONTINUE  No base dimension found. Place a dimension first.".into()
        } else {
            "DIMCONTINUE  Specify a second extension line origin (Enter to exit):".into()
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        if !self.ready {
            return CmdResult::Cancel;
        }
        let p1 = self.chain_p1;
        let p2 = pt;

        let dim_line_pt = p1 + self.perp * self.dim_offset;
        let text_mid = (dim_line_pt + (p2 + self.perp * self.dim_offset)) * 0.5;
        let axis = Vec3::new(self.rotation.cos() as f32, self.rotation.sin() as f32, 0.0);
        let measurement = (p2 - p1).dot(axis).abs() as f64;
        let rotation_deg = self.rotation.to_degrees();
        let entity = nm::Entity::new(nm::EntityData::Dimension {
            dim_type: 0,
            block_name: String::new(),
            style_name: String::new(),
            definition_point: [dim_line_pt.x as f64, dim_line_pt.y as f64, dim_line_pt.z as f64],
            text_midpoint: [text_mid.x as f64, text_mid.y as f64, text_mid.z as f64],
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
            rotation: rotation_deg,
            ext_line_rotation: 0.0,
        });

        self.chain_p1 = p2;

        CmdResult::CommitEntityNative(entity)
    }

    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        if !self.ready {
            return None;
        }
        let p1 = self.chain_p1;
        let dim_line_pt = p1 + self.perp * self.dim_offset;
        let dim_line_pt2 = pt + self.perp * self.dim_offset;
        Some(WireModel {
            name: "dimcont_preview".into(),
            points: vec![
                [p1.x, p1.y, p1.z], [dim_line_pt.x, dim_line_pt.y, dim_line_pt.z],
                [f32::NAN, 0.0, 0.0],
                [pt.x, pt.y, pt.z], [dim_line_pt2.x, dim_line_pt2.y, dim_line_pt2.z],
                [f32::NAN, 0.0, 0.0],
                [dim_line_pt.x, dim_line_pt.y, dim_line_pt.z],
                [dim_line_pt2.x, dim_line_pt2.y, dim_line_pt2.z],
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
        })
    }
}
