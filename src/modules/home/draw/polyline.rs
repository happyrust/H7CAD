// Polyline tool — ribbon definition + interactive command.
//
// Command:  PLINE (PL)
//   Each click adds a vertex.  The rubber-band shows the segment being drawn.
//   Enter / C = close and commit.  Escape = commit as-is (if ≥2 vertices).

use acadrust::entities::LwVertex;
use acadrust::{EntityType, LwPolyline};

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use crate::scene::wire_model::WireModel;
use glam::Vec3;

// ── Ribbon definition ──────────────────────────────────────────────────────

pub fn tool() -> ToolDef {
    ToolDef {
        id: "PLINE",
        label: "Polyline",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/polyline.svg")),
        event: ModuleEvent::Command("PLINE".to_string()),
    }
}

// ── Command implementation ─────────────────────────────────────────────────

pub struct PlineCommand {
    vertices: Vec<Vec3>,
}

impl PlineCommand {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
        }
    }

    fn build_entity(&self, closed: bool) -> Option<EntityType> {
        if self.vertices.len() < 2 {
            return None;
        }
        let lw_verts: Vec<LwVertex> = self
            .vertices
            .iter()
            .map(|v| LwVertex::new(acadrust::types::Vector2::new(v.x as f64, v.y as f64)))
            .collect();
        let pline = LwPolyline {
            vertices: lw_verts,
            is_closed: closed,
            ..Default::default()
        };
        Some(EntityType::LwPolyline(pline))
    }
}

impl CadCommand for PlineCommand {
    fn name(&self) -> &'static str {
        "PLINE"
    }

    fn prompt(&self) -> String {
        if self.vertices.is_empty() {
            "PLINE  Specify start point:".into()
        } else {
            format!(
                "PLINE  Specify next point  [{}pts | Enter=done C=close Esc=cancel]:",
                self.vertices.len()
            )
        }
    }

    fn on_point(&mut self, pt: Vec3) -> CmdResult {
        self.vertices.push(pt);
        CmdResult::NeedPoint
    }

    fn on_enter(&mut self) -> CmdResult {
        match self.build_entity(false) {
            Some(e) => CmdResult::CommitAndExit(e),
            None => CmdResult::Cancel,
        }
    }

    fn on_escape(&mut self) -> CmdResult {
        match self.build_entity(false) {
            Some(e) => CmdResult::CommitAndExit(e),
            None => CmdResult::Cancel,
        }
    }

    fn on_mouse_move(&mut self, pt: Vec3) -> Option<WireModel> {
        if self.vertices.is_empty() {
            return None;
        }
        // Show all committed vertices + cursor as a continuous preview.
        let mut pts: Vec<[f32; 3]> = self.vertices.iter().map(|v| [v.x, v.y, v.z]).collect();
        pts.push([pt.x, pt.y, pt.z]);
        Some(WireModel::solid(
            "rubber_band".into(),
            pts,
            WireModel::CYAN,
            false,
        ))
    }
}
