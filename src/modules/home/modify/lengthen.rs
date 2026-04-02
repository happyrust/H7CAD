// LENGTHEN command — extend or trim a Line or Arc by a specified delta or total.
//
// Options (entered as text after the entity pick):
//   DE <value>   — extend by delta (positive extends, negative trims)
//   TO <value>   — set total length (Line) or arc length (Arc)
//   P <pct>      — change by percentage (100 = no change, 150 = +50%)
//
// The entity is modified at whichever end is closest to the pick point.

use acadrust::entities::{Arc as ArcEnt, Line as LineEnt};
use acadrust::types::Vector3;
use acadrust::{EntityType, Handle};
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};

pub struct LengthenCommand {
    state: LenState,
}

enum LenState {
    PickEntity,
    PickOption { handle: Handle, pick_pt: Vec3 },
}

impl LengthenCommand {
    pub fn new() -> Self {
        Self { state: LenState::PickEntity }
    }
}

impl CadCommand for LengthenCommand {
    fn name(&self) -> &'static str { "LENGTHEN" }

    fn prompt(&self) -> String {
        match &self.state {
            LenState::PickEntity => "LENGTHEN  Select object:".into(),
            LenState::PickOption { .. } =>
                "LENGTHEN  Enter option [DE <delta> / TO <total> / P <pct>]:".into(),
        }
    }

    fn needs_entity_pick(&self) -> bool {
        matches!(self.state, LenState::PickEntity)
    }

    fn on_entity_pick(&mut self, handle: Handle, pt: Vec3) -> CmdResult {
        if handle.is_null() { return CmdResult::NeedPoint; }
        self.state = LenState::PickOption { handle, pick_pt: pt };
        CmdResult::NeedPoint
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.state, LenState::PickOption { .. })
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let (handle, pick_pt) = match &self.state {
            LenState::PickOption { handle, pick_pt } => (*handle, *pick_pt),
            _ => return None,
        };

        let text = text.trim().to_uppercase();
        if let Some(rest) = text.strip_prefix("DE ").or_else(|| text.strip_prefix("DE")) {
            let delta: f64 = rest.trim().replace(',', ".").parse().ok()?;
            Some(CmdResult::LengthenEntity { handle, pick_pt, mode: LenMode::Delta(delta) })
        } else if let Some(rest) = text.strip_prefix("TO ").or_else(|| text.strip_prefix("TO")) {
            let total: f64 = rest.trim().replace(',', ".").parse().ok().filter(|&v: &f64| v > 0.0)?;
            Some(CmdResult::LengthenEntity { handle, pick_pt, mode: LenMode::Total(total) })
        } else if let Some(rest) = text.strip_prefix("P ").or_else(|| text.strip_prefix("P")) {
            let pct: f64 = rest.trim().replace(',', ".").parse().ok().filter(|&v: &f64| v > 0.0)?;
            Some(CmdResult::LengthenEntity { handle, pick_pt, mode: LenMode::Percent(pct) })
        } else {
            // Try plain number as delta
            let delta: f64 = text.replace(',', ".").parse().ok()?;
            Some(CmdResult::LengthenEntity { handle, pick_pt, mode: LenMode::Delta(delta) })
        }
    }

    fn on_point(&mut self, _pt: Vec3) -> CmdResult { CmdResult::NeedPoint }
    fn on_enter(&mut self) -> CmdResult { CmdResult::Cancel }
}

// ── Mode enum (also used in CmdResult) ────────────────────────────────────

#[derive(Clone)]
pub enum LenMode {
    Delta(f64),
    Total(f64),
    Percent(f64),
}

// ── Geometry ───────────────────────────────────────────────────────────────

/// Apply LENGTHEN to a Line or Arc.
/// `pick_pt` determines which end to extend/trim (closest end is modified).
pub fn lengthen_entity(entity: &EntityType, pick_pt: Vec3, mode: &LenMode) -> Option<EntityType> {
    match entity {
        EntityType::Line(l) => lengthen_line(l, pick_pt, mode),
        EntityType::Arc(a)  => lengthen_arc(a, pick_pt, mode),
        _ => None,
    }
}

fn lengthen_line(line: &LineEnt, pick_pt: Vec3, mode: &LenMode) -> Option<EntityType> {
    let s = Vec3::new(line.start.x as f32, line.start.z as f32, 0.0);
    let e = Vec3::new(line.end.x as f32,   line.end.z as f32,   0.0);
    let p = Vec3::new(pick_pt.x, pick_pt.z, 0.0);

    let current_len = (e - s).length() as f64;
    if current_len < 1e-10 { return None; }

    let new_len = apply_mode(current_len, mode)?;
    if new_len < 1e-10 { return None; }

    let dir = (e - s) / current_len as f32;

    // Which end is closer to pick?
    let dist_to_start = (p - s).length();
    let dist_to_end   = (p - e).length();

    let mut result = line.clone();
    result.common.handle = Handle::NULL;

    if dist_to_end <= dist_to_start {
        // Extend/trim the end
        let new_end = s + dir * new_len as f32;
        result.end = xz_to_v3(new_end, line.end.z);
    } else {
        // Extend/trim the start (move start backward along dir)
        let new_start = e - dir * new_len as f32;
        result.start = xz_to_v3(new_start, line.start.z);
    }
    Some(EntityType::Line(result))
}

fn lengthen_arc(arc: &ArcEnt, pick_pt: Vec3, mode: &LenMode) -> Option<EntityType> {
    let cx = arc.center.x as f32;
    let cy = arc.center.z as f32; // Y-up: DXF Y → world Z

    // Current arc span
    let span = arc_span_deg(arc.start_angle, arc.end_angle);
    let current_arc_len = arc.radius * span.to_radians();

    let new_arc_len = apply_mode(current_arc_len, mode)?;
    if new_arc_len < 1e-10 { return None; }
    let new_span_deg = (new_arc_len / arc.radius).to_degrees();

    // Which end (start or end angle) is closer to pick?
    let start_rad = arc.start_angle.to_radians();
    let end_rad   = arc.end_angle.to_radians();

    let start_pt = Vec3::new(
        cx + arc.radius as f32 * start_rad.cos() as f32,
        pick_pt.y,
        cy + arc.radius as f32 * start_rad.sin() as f32,
    );
    let end_pt = Vec3::new(
        cx + arc.radius as f32 * end_rad.cos() as f32,
        pick_pt.y,
        cy + arc.radius as f32 * end_rad.sin() as f32,
    );
    let dist_start = (pick_pt - start_pt).length();
    let dist_end   = (pick_pt - end_pt).length();

    let delta_span = new_span_deg - span;

    let mut result = arc.clone();
    result.common.handle = Handle::NULL;

    if dist_end <= dist_start {
        // Extend end angle
        result.end_angle = arc.start_angle + new_span_deg;
    } else {
        // Extend start angle (move start backwards)
        result.start_angle = arc.end_angle - new_span_deg;
    }
    let _ = delta_span;
    Some(EntityType::Arc(result))
}

fn apply_mode(current: f64, mode: &LenMode) -> Option<f64> {
    match mode {
        LenMode::Delta(d)   => Some(current + d),
        LenMode::Total(t)   => Some(*t),
        LenMode::Percent(p) => Some(current * p / 100.0),
    }
}

fn arc_span_deg(start: f64, end: f64) -> f64 {
    let span = ((end - start) + 360.0) % 360.0;
    if span < 1e-6 { 360.0 } else { span }
}

fn xz_to_v3(v: Vec3, z: f64) -> Vector3 {
    // v is (world_x, world_z, 0) → DXF (x, world_z, z)
    Vector3::new(v.x as f64, v.y as f64, z)
}
