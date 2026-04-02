// Explode tool — ribbon definition + command implementation.
//
// Command:  EXPLODE (X)
//   EXPLODE: Breaks compound objects into their constituent simple entities.
//
//   Supported:
//     LwPolyline → Lines (straight segments) + Arcs (bulge segments)
//
//   Unsupported entity types are skipped silently.

use std::f64::consts::TAU;

use acadrust::entities::EntityCommon;
use acadrust::entities::{Arc as ArcEnt, Circle as CircleEnt, Line as LineEnt, LwPolyline};
use acadrust::entities::{Polyline, Polyline2D};
use acadrust::types::Vector3;
use acadrust::{CadDocument, EntityType, Handle};

use crate::command::{CadCommand, CmdResult};
use crate::modules::{IconKind, ModuleEvent, ToolDef};
use glam::Vec3;

// ── Ribbon definition ──────────────────────────────────────────────────────

pub fn tool() -> ToolDef {
    ToolDef {
        id: "EXPLODE",
        label: "Explode",
        icon: IconKind::Svg(include_bytes!("../../../../assets/icons/explode.svg")),
        event: ModuleEvent::Command("EXPLODE".to_string()),
    }
}

// ── Geometry helpers ────────────────────────────────────────────────────────

/// Decompose an entity into its constituent simple entities.
/// Returns an empty vec if the entity cannot be exploded.
pub fn explode_entity(entity: &EntityType, document: &CadDocument) -> Vec<EntityType> {
    match entity {
        EntityType::LwPolyline(p) => explode_lwpolyline(p),
        EntityType::Polyline2D(p) => explode_polyline2d(p),
        EntityType::Polyline(p) => explode_polyline(p),
        EntityType::Polyline3D(p) => explode_polyline3d(p),
        EntityType::Insert(ins) => ins
            .explode_from_document(document)
            .into_iter()
            .map(normalize_insert_entity)
            .collect(),
        _ => vec![],
    }
}

fn explode_polyline(p: &Polyline) -> Vec<EntityType> {
    let n = p.vertices.len();
    if n < 2 {
        return vec![];
    }
    let closed = p.flags.is_closed();
    let n_segs = if closed { n } else { n - 1 };
    let mut result = Vec::new();
    for i in 0..n_segs {
        let v0 = &p.vertices[i];
        let v1 = &p.vertices[(i + 1) % n];
        let mut common = p.common.clone();
        common.handle = Handle::NULL;
        result.push(EntityType::Line(LineEnt {
            common,
            start: v0.location.clone(),
            end: v1.location.clone(),
            ..LineEnt::new()
        }));
    }
    result
}

fn explode_polyline3d(p: &acadrust::entities::Polyline3D) -> Vec<EntityType> {
    let n = p.vertices.len();
    if n < 2 {
        return vec![];
    }
    let closed = p.is_closed();
    let n_segs = if closed { n } else { n - 1 };
    let mut result = Vec::new();
    for i in 0..n_segs {
        let v0 = &p.vertices[i];
        let v1 = &p.vertices[(i + 1) % n];
        let mut common = p.common.clone();
        common.handle = Handle::NULL;
        result.push(EntityType::Line(LineEnt {
            common,
            start: v0.position.clone(),
            end: v1.position.clone(),
            ..LineEnt::new()
        }));
    }
    result
}

fn explode_polyline2d(p: &Polyline2D) -> Vec<EntityType> {
    let n = p.vertices.len();
    if n < 2 {
        return vec![];
    }
    let closed = p.is_closed();
    let n_segs = if closed { n } else { n - 1 };
    let elevation = p.elevation;

    let mut result = Vec::new();
    for i in 0..n_segs {
        let v0 = &p.vertices[i];
        let v1 = &p.vertices[(i + 1) % n];
        let p0 = [v0.location.x, v0.location.y];
        let p1 = [v1.location.x, v1.location.y];

        if v0.bulge.abs() < 1e-10 {
            let mut common = p.common.clone();
            common.handle = Handle::NULL;
            result.push(EntityType::Line(LineEnt {
                common,
                start: Vector3::new(p0[0], p0[1], elevation),
                end: Vector3::new(p1[0], p1[1], elevation),
                ..LineEnt::new()
            }));
        } else if let Some(arc) = bulge_to_arc(p0, p1, v0.bulge, elevation, &p.common) {
            result.push(arc);
        }
    }
    result
}

pub fn normalize_insert_entity(mut entity: EntityType) -> EntityType {
    match &mut entity {
        EntityType::Arc(arc) => {
            arc.start_angle = arc.start_angle.to_degrees();
            arc.end_angle = arc.end_angle.to_degrees();
        }
        EntityType::Ellipse(ell) => {
            let major_len = ell.major_axis_length();
            let full_span = {
                let mut span = ell.end_parameter - ell.start_parameter;
                if span < 0.0 {
                    span += std::f64::consts::TAU;
                }
                (span - std::f64::consts::TAU).abs() < 1e-6
            };
            if (ell.minor_axis_ratio - 1.0).abs() < 1e-6 && full_span {
                let mut circle = CircleEnt::new();
                circle.common = ell.common.clone();
                circle.center = ell.center;
                circle.radius = major_len;
                circle.normal = ell.normal;
                entity = EntityType::Circle(circle);
            }
        }
        _ => {}
    }

    entity.common_mut().handle = Handle::NULL;
    entity.common_mut().owner_handle = Handle::NULL;
    entity
}

pub fn normalize_entity_for_block(mut entity: EntityType) -> EntityType {
    if let EntityType::Arc(arc) = &mut entity {
        arc.start_angle = arc.start_angle.to_radians();
        arc.end_angle = arc.end_angle.to_radians();
    }
    entity
}

fn explode_lwpolyline(p: &LwPolyline) -> Vec<EntityType> {
    let n = p.vertices.len();
    if n < 2 {
        return vec![];
    }

    let elevation = p.elevation;
    let n_segs = if p.is_closed { n } else { n - 1 };

    let mut result = Vec::new();
    for i in 0..n_segs {
        let v0 = &p.vertices[i];
        let v1 = &p.vertices[(i + 1) % n];

        let p0 = [v0.location.x, v0.location.y];
        let p1 = [v1.location.x, v1.location.y];

        if v0.bulge.abs() < 1e-10 {
            // Straight segment → Line
            let mut common = p.common.clone();
            common.handle = Handle::NULL;
            let line = LineEnt {
                common,
                start: Vector3::new(p0[0], p0[1], elevation),
                end: Vector3::new(p1[0], p1[1], elevation),
                ..LineEnt::new()
            };
            result.push(EntityType::Line(line));
        } else {
            // Arc segment from bulge
            if let Some(arc) = bulge_to_arc(p0, p1, v0.bulge, elevation, &p.common) {
                result.push(arc);
            }
        }
    }
    result
}

/// Convert a polyline bulge segment to an Arc entity.
///   Arc angles are measured from the +X axis.
fn bulge_to_arc(
    p0: [f64; 2],
    p1: [f64; 2],
    bulge: f64,
    elevation: f64,
    common_src: &EntityCommon,
) -> Option<EntityType> {
    let chord_x = p1[0] - p0[0];
    let chord_y = p1[1] - p0[1];
    let chord_len = (chord_x * chord_x + chord_y * chord_y).sqrt();
    if chord_len < 1e-12 {
        return None;
    }

    // Included angle = 4 * atan(bulge)
    let b = bulge;
    let b2 = b * b;

    // Radius: r = chord * (1 + b²) / (4 * |b|)
    let r = chord_len * (1.0 + b2) / (4.0 * b.abs());

    // Perpendicular distance from midpoint to center:
    // d = r * cos(theta/2), where cos(theta/2) = (1 - b²) / (1 + b²)
    let d = r * (1.0 - b2) / (1.0 + b2);

    // Midpoint
    let mx = (p0[0] + p1[0]) * 0.5;
    let my = (p0[1] + p1[1]) * 0.5;

    // Left-perpendicular direction (rotate chord 90° CCW)
    let perp_x = -chord_y / chord_len;
    let perp_y = chord_x / chord_len;

    // Positive bulge → center is to the left of chord direction (CCW arc)
    let sign = b.signum();
    let cx = mx + sign * d * perp_x;
    let cy = my + sign * d * perp_y;

    // Angles from center to p0 and p1
    let a0_rad = (p0[1] - cy).atan2(p0[0] - cx);
    let a1_rad = (p1[1] - cy).atan2(p1[0] - cx);

    // acadrust arcs are always CCW: positive bulge → CCW from p0 to p1
    // Negative bulge → CW from p0 to p1 = CCW from p1 to p0
    let (start_deg, end_deg) = if b > 0.0 {
        (norm_deg(a0_rad.to_degrees()), norm_deg(a1_rad.to_degrees()))
    } else {
        (norm_deg(a1_rad.to_degrees()), norm_deg(a0_rad.to_degrees()))
    };

    let mut common = common_src.clone();
    common.handle = Handle::NULL;

    let arc = ArcEnt {
        common,
        center: Vector3::new(cx, cy, elevation),
        radius: r,
        start_angle: start_deg,
        end_angle: end_deg,
        ..ArcEnt::new()
    };
    Some(EntityType::Arc(arc))
}

fn norm_deg(a: f64) -> f64 {
    let t = TAU.to_degrees();
    ((a % t) + t) % t
}

// ── Command stub (kept for future interactive selection mode) ───────────────

pub struct ExplodeCommand;

impl ExplodeCommand {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl CadCommand for ExplodeCommand {
    fn name(&self) -> &'static str {
        "EXPLODE"
    }
    fn prompt(&self) -> String {
        "EXPLODE  Select objects to explode:".into()
    }

    fn on_point(&mut self, _pt: Vec3) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}
