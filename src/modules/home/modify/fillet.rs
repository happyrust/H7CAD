// Fillet / Chamfer — ribbon definitions + full command implementations.
//
// FILLET (F):
//   Pick two lines. Command finds their intersection, computes a tangent arc
//   of radius R, trims both lines to the tangent points, and inserts the arc.
//   R=0 just extends/trims both lines to the exact intersection (sharp corner).
//
// CHAMFER (CHA):
//   Pick two lines. Command finds their intersection, backs off dist1 along
//   line 1 and dist2 along line 2, trims, and inserts a straight chamfer line.

use acadrust::entities::{Arc as ArcEnt, Line as LineEnt};
use acadrust::types::Vector3;
use acadrust::{EntityType, Handle};
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::home::defaults;
use crate::modules::IconKind;
use crate::scene::wire_model::WireModel;

// ── Dropdown constants ─────────────────────────────────────────────────────

pub const DROPDOWN_ID: &str = "fillet_chamfer";
pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../../assets/icons/fillet.svg"));

pub const DROPDOWN_ITEMS: &[(&str, &str, IconKind)] = &[
    (
        "FILLET",
        "Fillet",
        IconKind::Svg(include_bytes!("../../../../assets/icons/fillet.svg")),
    ),
    (
        "CHAMFER",
        "Chamfer",
        IconKind::Svg(include_bytes!("../../../../assets/icons/chamfer.svg")),
    ),
];

// ══════════════════════════════════════════════════════════════════════════
// Geometry
// ══════════════════════════════════════════════════════════════════════════

/// Intersect two infinite lines. Returns (t on L1, u on L2).
fn ll(
    ax: f64,
    ay: f64,
    dx: f64,
    dy: f64,
    cx: f64,
    cy: f64,
    ex: f64,
    ey: f64,
) -> Option<(f64, f64)> {
    let det = dx * ey - dy * ex;
    if det.abs() < 1e-10 {
        return None;
    }
    let t = ((cx - ax) * ey - (cy - ay) * ex) / det;
    let u = ((cx - ax) * dy - (cy - ay) * dx) / det;
    Some((t, u))
}

/// Extract coords and unit direction for a Line entity.
fn line_geom(l: &LineEnt) -> ([f64; 2], [f64; 2], [f64; 2], f64) {
    let p1 = [l.start.x, l.start.y];
    let p2 = [l.end.x, l.end.y];
    let dx = p2[0] - p1[0];
    let dy = p2[1] - p1[1];
    let len = (dx * dx + dy * dy).sqrt().max(1e-12);
    (p1, p2, [dx / len, dy / len], len)
}

/// Project click onto line, returning t ∈ ℝ.
fn project_click(click: [f64; 2], p1: [f64; 2], unit: [f64; 2]) -> f64 {
    (click[0] - p1[0]) * unit[0] + (click[1] - p1[1]) * unit[1]
}

// ── Fillet ─────────────────────────────────────────────────────────────────

/// Compute fillet: trim l1/l2 and insert a tangent arc of `radius`.
/// Returns (trimmed_l1, trimmed_l2, fillet_arc).
fn compute_fillet(
    l1: &LineEnt,
    click1: [f64; 2],
    l2: &LineEnt,
    click2: [f64; 2],
    radius: f64,
) -> Option<(EntityType, EntityType, Option<EntityType>)> {
    let (p1, _p2, u1, _len1) = line_geom(l1);
    let (p3, _p4, u2, _len2) = line_geom(l2);

    // Intersection of infinite lines
    let (t_p, u_p) = ll(p1[0], p1[1], u1[0], u1[1], p3[0], p3[1], u2[0], u2[1])?;

    // Intersection point
    let px = p1[0] + t_p * u1[0];
    let py = p1[1] + t_p * u1[1];

    // Direction from P toward each click (the "keep" side)
    let s1 = project_click(click1, [px, py], u1); // positive = along u1
    let s2 = project_click(click2, [px, py], u2);
    let dir1 = if s1 >= 0.0 {
        [u1[0], u1[1]]
    } else {
        [-u1[0], -u1[1]]
    };
    let dir2 = if s2 >= 0.0 {
        [u2[0], u2[1]]
    } else {
        [-u2[0], -u2[1]]
    };

    // Angle between the two keep-directions
    let cos_a = (dir1[0] * dir2[0] + dir1[1] * dir2[1]).clamp(-1.0, 1.0);
    let angle = cos_a.acos();

    // Lines are parallel / anti-parallel
    if angle < 1e-6 || (angle - std::f64::consts::PI).abs() < 1e-6 {
        return None;
    }

    let half = angle / 2.0;
    let z = l1.start.z;

    if radius < 1e-9 {
        // r = 0: just extend/trim both lines to the intersection
        let (new_l1, new_l2) = trim_to_point(l1, t_p, p1, u1, l2, u_p, p3, u2)?;
        return Some((EntityType::Line(new_l1), EntityType::Line(new_l2), None));
    }

    // Distance from P to tangent points
    let d = radius / half.tan();

    // Tangent points
    let t1 = [px + d * dir1[0], py + d * dir1[1]];
    let t2 = [px + d * dir2[0], py + d * dir2[1]];

    // Arc center: along bisector of dir1+dir2, distance = r / sin(half)
    let bx = dir1[0] + dir2[0];
    let by = dir1[1] + dir2[1];
    let blen = (bx * bx + by * by).sqrt();
    if blen < 1e-10 {
        return None;
    }
    let arc_dist = radius / half.sin();
    let arc_cx = px + arc_dist * bx / blen;
    let arc_cy = py + arc_dist * by / blen;

    // Arc angles in degrees
    let a_start_deg = (t1[1] - arc_cy).atan2(t1[0] - arc_cx).to_degrees();
    let a_end_deg = (t2[1] - arc_cy).atan2(t2[0] - arc_cx).to_degrees();

    // Pick CCW direction that fills the concave corner
    let cross = dir1[0] * dir2[1] - dir1[1] * dir2[0];
    let (start_deg, end_deg) = if cross <= 0.0 {
        (a_start_deg, a_end_deg)
    } else {
        (a_end_deg, a_start_deg)
    };

    // Trim l1 to T1 and l2 to T2
    let new_l1 = trim_to_xy(l1, t_p, t1, dir1, p1, u1)?;
    let new_l2 = trim_to_xy(l2, u_p, t2, dir2, p3, u2)?;

    // Build arc entity
    let mut arc = ArcEnt::new();
    arc.common = l1.common.clone();
    arc.common.handle = Handle::NULL;
    arc.center = Vector3::new(arc_cx, arc_cy, z);
    arc.radius = radius;
    arc.start_angle = start_deg;
    arc.end_angle = end_deg;

    Some((
        EntityType::Line(new_l1),
        EntityType::Line(new_l2),
        Some(EntityType::Arc(arc)),
    ))
}

/// Trim a line's parameter to an intersection t on the same side as dir (keep side).
fn trim_to_xy(
    orig: &LineEnt,
    t_isect: f64,
    tangent: [f64; 2],
    dir: [f64; 2],
    p1: [f64; 2],
    unit: [f64; 2],
) -> Option<LineEnt> {
    let z = orig.start.z;
    let mut l = orig.clone();
    l.common.handle = Handle::NULL;

    // t_tangent: parameter of the tangent point along the line from start
    let t_tan = (tangent[0] - p1[0]) * unit[0] + (tangent[1] - p1[1]) * unit[1];

    // dir is positive along unit → we keep the portion BEYOND t_tan in that direction
    // dir positive: keep from t_tan to +∞ (i.e. set start to tangent point)
    // dir negative: keep from -∞ to t_tan (i.e. set end to tangent point)
    let len = {
        let dx = orig.end.x - orig.start.x;
        let dy = orig.end.y - orig.start.y;
        (dx * dx + dy * dy).sqrt().max(1e-12)
    };
    let dot = dir[0] * unit[0] + dir[1] * unit[1]; // +1 or -1

    if dot > 0.0 {
        // keep from tangent to end → move start to tangent point
        l.start = Vector3::new(tangent[0], tangent[1], z);
    } else {
        // keep from start to tangent → move end to tangent point
        l.end = Vector3::new(tangent[0], tangent[1], z);
    }
    let _ = (t_isect, len, t_tan); // used implicitly via `dot`
    Some(l)
}

/// Trim both lines exactly to their intersection point (r=0 case).
fn trim_to_point(
    l1: &LineEnt,
    t_p: f64,
    p1: [f64; 2],
    u1: [f64; 2],
    l2: &LineEnt,
    u_p: f64,
    _p3: [f64; 2],
    _u2: [f64; 2],
) -> Option<(LineEnt, LineEnt)> {
    let px = p1[0] + t_p * u1[0];
    let py = p1[1] + t_p * u1[1];
    let z1 = l1.start.z;
    let z2 = l2.start.z;

    // For l1: if t_p is past the midpoint, keep start…P; else keep P…end
    // We use the same "which end is P closer to" logic
    let mut ll1 = l1.clone();
    ll1.common.handle = Handle::NULL;
    let mut ll2 = l2.clone();
    ll2.common.handle = Handle::NULL;

    if t_p >= 0.0 {
        ll1.end = Vector3::new(px, py, z1);
    } else {
        ll1.start = Vector3::new(px, py, z1);
    }

    if u_p >= 0.0 {
        ll2.end = Vector3::new(px, py, z2);
    } else {
        ll2.start = Vector3::new(px, py, z2);
    }

    Some((ll1, ll2))
}

// ── Point-generation helpers ──────────────────────────────────────────────

fn line_pts(l: &LineEnt) -> Vec<[f32; 3]> {
    vec![
        [l.start.x as f32, l.start.y as f32, l.start.z as f32],
        [l.end.x as f32, l.end.y as f32, l.end.z as f32],
    ]
}

fn arc_pts(cx: f64, cy: f64, r: f64, a0_deg: f64, a1_deg: f64, y: f64) -> Vec<[f32; 3]> {
    use std::f64::consts::TAU;
    let fn_norm = |a: f64| -> f64 { ((a % TAU) + TAU) % TAU };
    let a0 = a0_deg.to_radians();
    let a1 = a1_deg.to_radians();
    let span = {
        let s = fn_norm(a1) - fn_norm(a0);
        if s <= 0.0 {
            s + TAU
        } else {
            s
        }
    };
    let steps = (span.abs() * 20.0).ceil().max(4.0) as usize;
    (0..=steps)
        .map(|i| {
            let ang = fn_norm(a0) + span * (i as f64 / steps as f64);
            [
                (cx + r * ang.cos()) as f32,
                y as f32,
                (cy + r * ang.sin()) as f32,
            ]
        })
        .collect()
}

fn entity_pts(e: &EntityType) -> Vec<[f32; 3]> {
    match e {
        EntityType::Line(l) => line_pts(l),
        EntityType::Arc(a) => arc_pts(
            a.center.x,
            a.center.y,
            a.radius,
            a.start_angle,
            a.end_angle,
            a.center.y,
        ),
        _ => vec![],
    }
}

// ── Chamfer ────────────────────────────────────────────────────────────────

/// Compute chamfer: trim l1 by dist1 from intersection, l2 by dist2, add chamfer line.
fn compute_chamfer(
    l1: &LineEnt,
    click1: [f64; 2],
    dist1: f64,
    l2: &LineEnt,
    click2: [f64; 2],
    dist2: f64,
) -> Option<(EntityType, EntityType, EntityType)> {
    let (p1, _, u1, _) = line_geom(l1);
    let (p3, _, u2, _) = line_geom(l2);

    let (t_p, u_p) = ll(p1[0], p1[1], u1[0], u1[1], p3[0], p3[1], u2[0], u2[1])?;

    let px = p1[0] + t_p * u1[0];
    let py = p1[1] + t_p * u1[1];
    let z = l1.start.z;

    let s1 = project_click(click1, [px, py], u1);
    let s2 = project_click(click2, [px, py], u2);
    let dir1 = if s1 >= 0.0 {
        [u1[0], u1[1]]
    } else {
        [-u1[0], -u1[1]]
    };
    let dir2 = if s2 >= 0.0 {
        [u2[0], u2[1]]
    } else {
        [-u2[0], -u2[1]]
    };

    // Chamfer points: back off dist from P along keep-direction
    let c1 = [px + dist1 * dir1[0], py + dist1 * dir1[1]];
    let c2 = [px + dist2 * dir2[0], py + dist2 * dir2[1]];

    // Trim l1 to c1 and l2 to c2
    let new_l1 = trim_to_xy(l1, t_p, c1, dir1, p1, u1)?;
    let new_l2 = trim_to_xy(l2, u_p, c2, dir2, p3, u2)?;

    // Chamfer line
    let mut cline = l1.clone();
    cline.common.handle = Handle::NULL;
    cline.start = Vector3::new(c1[0], c1[1], z);
    cline.end = Vector3::new(c2[0], c2[1], z);

    Some((
        EntityType::Line(new_l1),
        EntityType::Line(new_l2),
        EntityType::Line(cline),
    ))
}

// ══════════════════════════════════════════════════════════════════════════
// FilletCommand
// ══════════════════════════════════════════════════════════════════════════

enum FilletStep {
    First,
    Second {
        h1: Handle,
        l1: LineEnt,
        click1: [f64; 2],
    },
}

pub struct FilletCommand {
    radius: f64,
    step: FilletStep,
    all_entities: Vec<EntityType>,
}

impl FilletCommand {
    pub fn new(radius: f32, all_entities: Vec<EntityType>) -> Self {
        Self {
            radius: radius as f64,
            step: FilletStep::First,
            all_entities,
        }
    }
}

impl CadCommand for FilletCommand {
    fn name(&self) -> &'static str {
        "FILLET"
    }

    fn prompt(&self) -> String {
        match &self.step {
            FilletStep::First => format!(
                "FILLET  Select first line  [R={:.4} | type R <val> to change]:",
                self.radius
            ),
            FilletStep::Second { .. } => {
                format!("FILLET  Select second line  [R={:.4}]:", self.radius)
            }
        }
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.step, FilletStep::First)
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        let body = if t.to_uppercase().starts_with('R') {
            t[1..].trim()
        } else {
            t
        };
        if let Ok(v) = body.replace(',', ".").parse::<f64>() {
            if v >= 0.0 {
                self.radius = v;
                defaults::set_fillet_radius(v as f32);
            }
        }
        None
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn on_entity_pick(&mut self, handle: Handle, pt: Vec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }
        let click = [pt.x as f64, pt.y as f64];

        match &self.step {
            FilletStep::First => {
                // Must be a line
                let l1 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        if let EntityType::Line(l) = e {
                            Some(l.clone())
                        } else {
                            None
                        }
                    });
                if let Some(l) = l1 {
                    self.step = FilletStep::Second {
                        h1: handle,
                        l1: l,
                        click1: click,
                    };
                    CmdResult::NeedPoint
                } else {
                    CmdResult::NeedPoint // not a line — ignore
                }
            }
            FilletStep::Second { h1, l1, click1 } => {
                let h1 = *h1;
                let l1 = l1.clone();
                let click1 = *click1;
                if handle == h1 {
                    return CmdResult::NeedPoint;
                }

                let l2 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        if let EntityType::Line(l) = e {
                            Some(l.clone())
                        } else {
                            None
                        }
                    });

                if let Some(l2) = l2 {
                    match compute_fillet(&l1, click1, &l2, click, self.radius) {
                        Some((new_l1, new_l2, maybe_arc)) => {
                            let mut additions = vec![];
                            if let Some(arc) = maybe_arc {
                                additions.push(arc);
                            }
                            CmdResult::ReplaceMany(
                                vec![(h1, vec![new_l1]), (handle, vec![new_l2])],
                                additions,
                            )
                        }
                        None => {
                            // Could not compute fillet (parallel lines, etc.)
                            CmdResult::NeedPoint
                        }
                    }
                } else {
                    CmdResult::NeedPoint
                }
            }
        }
    }

    fn on_hover_entity(&mut self, handle: Handle, pt: Vec3) -> Vec<WireModel> {
        if handle.is_null() {
            return vec![];
        }
        let click = [pt.x as f64, pt.y as f64];

        match &self.step {
            FilletStep::First => {
                // Highlight hovered line in cyan
                let pts = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        if let EntityType::Line(l) = e {
                            Some(line_pts(l))
                        } else {
                            None
                        }
                    });
                if let Some(pts) = pts {
                    vec![WireModel::solid(
                        "fillet_hover".into(),
                        pts,
                        WireModel::CYAN,
                        false,
                    )]
                } else {
                    vec![]
                }
            }
            FilletStep::Second { l1, click1, .. } => {
                let l1 = l1.clone();
                let click1 = *click1;
                if handle.is_null() {
                    return vec![];
                }
                let l2 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        if let EntityType::Line(l) = e {
                            Some(l.clone())
                        } else {
                            None
                        }
                    });
                if let Some(l2) = l2 {
                    if let Some((new_l1, new_l2, maybe_arc)) =
                        compute_fillet(&l1, click1, &l2, click, self.radius)
                    {
                        let mut out = vec![
                            WireModel::solid(
                                "fillet_l1".into(),
                                entity_pts(&new_l1),
                                WireModel::CYAN,
                                false,
                            ),
                            WireModel::solid(
                                "fillet_l2".into(),
                                entity_pts(&new_l2),
                                WireModel::CYAN,
                                false,
                            ),
                        ];
                        if let Some(arc) = maybe_arc {
                            out.push(WireModel::solid(
                                "fillet_arc".into(),
                                entity_pts(&arc),
                                WireModel::CYAN,
                                false,
                            ));
                        }
                        return out;
                    }
                }
                vec![]
            }
        }
    }

    fn on_point(&mut self, _pt: Vec3) -> CmdResult {
        CmdResult::NeedPoint
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}

// ══════════════════════════════════════════════════════════════════════════
// ChamferCommand
// ══════════════════════════════════════════════════════════════════════════

enum ChamferStep {
    First,
    Second {
        h1: Handle,
        l1: LineEnt,
        click1: [f64; 2],
    },
}

pub struct ChamferCommand {
    dist1: f64,
    dist2: f64,
    step: ChamferStep,
    all_entities: Vec<EntityType>,
}

impl ChamferCommand {
    pub fn new(dist: f32, all_entities: Vec<EntityType>) -> Self {
        Self {
            dist1: dist as f64,
            dist2: defaults::get_chamfer_dist2() as f64,
            step: ChamferStep::First,
            all_entities,
        }
    }
}

impl CadCommand for ChamferCommand {
    fn name(&self) -> &'static str {
        "CHAMFER"
    }

    fn prompt(&self) -> String {
        match &self.step {
            ChamferStep::First => format!(
                "CHAMFER  Select first line  [D1={:.4} D2={:.4} | type D <d1> <d2>]:",
                self.dist1, self.dist2
            ),
            ChamferStep::Second { .. } => format!(
                "CHAMFER  Select second line  [D1={:.4} D2={:.4}]:",
                self.dist1, self.dist2
            ),
        }
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.step, ChamferStep::First)
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        let t = text.trim();
        let body = if t.to_uppercase().starts_with('D') {
            t[1..].trim()
        } else {
            t
        };
        let parts: Vec<f64> = body
            .split_whitespace()
            .filter_map(|s| s.replace(',', ".").parse::<f64>().ok())
            .collect();
        if let Some(&v) = parts.first() {
            self.dist1 = v.max(0.0);
            defaults::set_chamfer_dist1(self.dist1 as f32);
        }
        if let Some(&v) = parts.get(1) {
            self.dist2 = v.max(0.0);
            defaults::set_chamfer_dist2(self.dist2 as f32);
        } else if parts.len() == 1 {
            self.dist2 = self.dist1;
            defaults::set_chamfer_dist2(self.dist2 as f32);
        }
        None
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn on_entity_pick(&mut self, handle: Handle, pt: Vec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }
        let click = [pt.x as f64, pt.y as f64];

        match &self.step {
            ChamferStep::First => {
                let l1 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        if let EntityType::Line(l) = e {
                            Some(l.clone())
                        } else {
                            None
                        }
                    });
                if let Some(l) = l1 {
                    self.step = ChamferStep::Second {
                        h1: handle,
                        l1: l,
                        click1: click,
                    };
                    CmdResult::NeedPoint
                } else {
                    CmdResult::NeedPoint
                }
            }
            ChamferStep::Second { h1, l1, click1 } => {
                let h1 = *h1;
                let l1 = l1.clone();
                let click1 = *click1;
                if handle == h1 {
                    return CmdResult::NeedPoint;
                }

                let l2 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        if let EntityType::Line(l) = e {
                            Some(l.clone())
                        } else {
                            None
                        }
                    });

                if let Some(l2) = l2 {
                    match compute_chamfer(&l1, click1, self.dist1, &l2, click, self.dist2) {
                        Some((new_l1, new_l2, chamfer_line)) => CmdResult::ReplaceMany(
                            vec![(h1, vec![new_l1]), (handle, vec![new_l2])],
                            vec![chamfer_line],
                        ),
                        None => CmdResult::NeedPoint,
                    }
                } else {
                    CmdResult::NeedPoint
                }
            }
        }
    }

    fn on_hover_entity(&mut self, handle: Handle, pt: Vec3) -> Vec<WireModel> {
        if handle.is_null() {
            return vec![];
        }
        let click = [pt.x as f64, pt.y as f64];

        match &self.step {
            ChamferStep::First => {
                let pts = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        if let EntityType::Line(l) = e {
                            Some(line_pts(l))
                        } else {
                            None
                        }
                    });
                if let Some(pts) = pts {
                    vec![WireModel::solid(
                        "chamfer_hover".into(),
                        pts,
                        WireModel::CYAN,
                        false,
                    )]
                } else {
                    vec![]
                }
            }
            ChamferStep::Second { l1, click1, .. } => {
                let l1 = l1.clone();
                let click1 = *click1;
                let l2 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        if let EntityType::Line(l) = e {
                            Some(l.clone())
                        } else {
                            None
                        }
                    });
                if let Some(l2) = l2 {
                    if let Some((new_l1, new_l2, cline)) =
                        compute_chamfer(&l1, click1, self.dist1, &l2, click, self.dist2)
                    {
                        return vec![
                            WireModel::solid(
                                "chamfer_l1".into(),
                                entity_pts(&new_l1),
                                WireModel::CYAN,
                                false,
                            ),
                            WireModel::solid(
                                "chamfer_l2".into(),
                                entity_pts(&new_l2),
                                WireModel::CYAN,
                                false,
                            ),
                            WireModel::solid(
                                "chamfer_line".into(),
                                entity_pts(&cline),
                                WireModel::CYAN,
                                false,
                            ),
                        ];
                    }
                }
                vec![]
            }
        }
    }

    fn on_point(&mut self, _pt: Vec3) -> CmdResult {
        CmdResult::NeedPoint
    }
    fn on_enter(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
    fn on_escape(&mut self) -> CmdResult {
        CmdResult::Cancel
    }
}
