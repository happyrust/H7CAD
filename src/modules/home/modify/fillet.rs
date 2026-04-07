// Fillet / Chamfer — ribbon definitions + full command implementations.
//
// FILLET (F):
//   Supports: Line-Line, Line-Arc, Arc-Line, Arc-Arc.
//   Finds intersection, computes tangent arc of radius R, trims both entities.
//   R=0 just extends/trims to the exact intersection (sharp corner).
//
// CHAMFER (CHA):
//   Pick two lines (line-only; arcs are not chamferable).
//   Finds intersection, backs off dist1 along line 1 and dist2 along line 2.

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

fn arc_pts(cx: f64, cy: f64, r: f64, a0_deg: f64, a1_deg: f64, z: f64) -> Vec<[f32; 3]> {
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
                (cy + r * ang.sin()) as f32,
                z as f32,
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
            a.center.z,
        ),
        _ => vec![],
    }
}

// ── Arc geometry helpers ───────────────────────────────────────────────────

/// Extract center, radius, start/end angle (degrees), elevation from an arc.
fn arc_geom(a: &ArcEnt) -> ([f64; 2], f64, f64, f64, f64) {
    ([a.center.x, a.center.y], a.radius, a.start_angle, a.end_angle, a.center.z)
}

/// Normalize angle to [0, 360).
fn norm360(a: f64) -> f64 { ((a % 360.0) + 360.0) % 360.0 }

/// Return the CCW angular span from `start` to `end`.
fn arc_span(start: f64, end: f64) -> f64 {
    let s = (end - start + 360.0) % 360.0;
    if s < 1e-6 { 360.0 } else { s }
}

/// Project a pick point onto an arc: return the parameter t ∈ [0, 360) = the angle.
fn arc_angle_at(center: [f64; 2], pt: [f64; 2]) -> f64 {
    norm360((pt[1] - center[1]).atan2(pt[0] - center[0]).to_degrees())
}

/// Clamp angle `a` into the arc range (CCW from `start` to `end`).
/// Returns the nearer endpoint if `a` is outside.
fn clamp_angle_to_arc(a: f64, start: f64, end: f64) -> f64 {
    let span = arc_span(start, end);
    let rel = (a - start + 360.0) % 360.0;
    if rel <= span { a }
    else if rel < span + (360.0 - span) / 2.0 { end }
    else { start }
}

/// Trim an arc so it goes from `new_start` to `new_end` (both in degrees).
fn trim_arc(orig: &ArcEnt, new_start: f64, new_end: f64) -> ArcEnt {
    let mut a = orig.clone();
    a.common.handle = Handle::NULL;
    a.start_angle = norm360(new_start);
    a.end_angle   = norm360(new_end);
    a
}

/// Intersect a line (point p + direction d) with a circle (center c, radius r).
/// Returns up to 2 parameter values t on the line.
fn line_circle_ts(px: f64, py: f64, dx: f64, dy: f64, cx: f64, cy: f64, r: f64) -> Vec<f64> {
    let fx = px - cx;
    let fy = py - cy;
    let a = dx * dx + dy * dy;
    let b = 2.0 * (fx * dx + fy * dy);
    let c = fx * fx + fy * fy - r * r;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 { return vec![]; }
    let sq = disc.sqrt();
    if disc < 1e-14 {
        vec![(-b) / (2.0 * a)]
    } else {
        vec![(-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a)]
    }
}

/// Intersect two circles. Returns intersection points.
fn circle_circle_pts(c1: [f64; 2], r1: f64, c2: [f64; 2], r2: f64) -> Vec<[f64; 2]> {
    let dx = c2[0] - c1[0];
    let dy = c2[1] - c1[1];
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1e-12 || d > r1 + r2 + 1e-9 || d < (r1 - r2).abs() - 1e-9 { return vec![]; }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h2 = r1 * r1 - a * a;
    if h2 < 0.0 { return vec![]; }
    let h = h2.sqrt();
    let mx = c1[0] + a * dx / d;
    let my = c1[1] + a * dy / d;
    if h < 1e-9 {
        vec![[mx, my]]
    } else {
        vec![
            [mx + h * dy / d, my - h * dx / d],
            [mx - h * dy / d, my + h * dx / d],
        ]
    }
}

// ── Unified fillet entity type ─────────────────────────────────────────────

/// A pickable entity for FILLET: Line or Arc.
#[derive(Clone)]
enum FilletEntity {
    Line(LineEnt),
    Arc(ArcEnt),
}

impl FilletEntity {
    fn from_entity(e: &EntityType) -> Option<Self> {
        match e {
            EntityType::Line(l) => Some(Self::Line(l.clone())),
            EntityType::Arc(a)  => Some(Self::Arc(a.clone())),
            _ => None,
        }
    }

    fn to_entity_type(&self) -> EntityType {
        match self {
            Self::Line(l) => EntityType::Line(l.clone()),
            Self::Arc(a)  => EntityType::Arc(a.clone()),
        }
    }

    fn elevation(&self) -> f64 {
        match self {
            Self::Line(l) => l.start.z,
            Self::Arc(a)  => a.center.z,
        }
    }
}

/// Compute FILLET between two entities (Line or Arc).
/// Returns (trimmed_e1, trimmed_e2, optional_fillet_arc).
fn compute_fillet_entities(
    e1: &FilletEntity,
    click1: [f64; 2],
    e2: &FilletEntity,
    click2: [f64; 2],
    radius: f64,
) -> Option<(EntityType, EntityType, Option<EntityType>)> {
    let z = e1.elevation();

    match (e1, e2) {
        (FilletEntity::Line(l1), FilletEntity::Line(l2)) => {
            compute_fillet(l1, click1, l2, click2, radius)
        }
        (FilletEntity::Line(l), FilletEntity::Arc(a)) => {
            fillet_line_arc(l, click1, a, click2, radius, z)
        }
        (FilletEntity::Arc(a), FilletEntity::Line(l)) => {
            fillet_line_arc(l, click2, a, click1, radius, z)
                .map(|(new_l, new_a, arc)| (new_a, new_l, arc))
        }
        (FilletEntity::Arc(a1), FilletEntity::Arc(a2)) => {
            fillet_arc_arc(a1, click1, a2, click2, radius, z)
        }
    }
}

/// Fillet a Line and an Arc.
fn fillet_line_arc(
    line: &LineEnt,
    click_line: [f64; 2],
    arc: &ArcEnt,
    click_arc: [f64; 2],
    radius: f64,
    z: f64,
) -> Option<(EntityType, EntityType, Option<EntityType>)> {
    let (p1, _, u, _) = line_geom(line);
    let (ac, ar, a_start, a_end, _) = arc_geom(arc);

    // Intersection of infinite line with the arc's circle
    let ts = line_circle_ts(p1[0], p1[1], u[0], u[1], ac[0], ac[1], ar);

    if radius < 1e-9 {
        // r=0: trim to intersection (nearest to each click)
        let t_best = ts.iter().copied()
            .min_by(|a, b| {
                let da = (p1[0]+a*u[0]-click_line[0]).powi(2)+(p1[1]+a*u[1]-click_line[1]).powi(2);
                let db = (p1[0]+b*u[0]-click_line[0]).powi(2)+(p1[1]+b*u[1]-click_line[1]).powi(2);
                da.partial_cmp(&db).unwrap()
            })?;
        let ix = p1[0] + t_best * u[0];
        let iy = p1[1] + t_best * u[1];

        // Trim line to intersection
        let new_line = trim_line_to_point(line, [ix, iy], click_line)?;
        // Trim arc to intersection angle
        let i_angle = arc_angle_at(ac, [ix, iy]);
        let i_clamped = clamp_angle_to_arc(i_angle, a_start, a_end);
        let arc_click_angle = arc_angle_at(ac, click_arc);
        let arc_click_clamped = clamp_angle_to_arc(arc_click_angle, a_start, a_end);
        // Keep the arc side from i_clamped toward the click
        let new_arc = if {
            let sp_to_click = arc_span(i_clamped, arc_click_clamped);
            let sp_click_to_end = arc_span(arc_click_clamped, a_end);
            sp_to_click <= sp_click_to_end
        } {
            trim_arc(arc, i_clamped, a_end)
        } else {
            trim_arc(arc, a_start, i_clamped)
        };
        return Some((EntityType::Line(new_line), EntityType::Arc(new_arc), None));
    }

    // For r>0: find the fillet arc center — tangent to both the line and the circle.
    // The fillet circle center lies at distance (radius) from the line
    // and at distance |ar ± radius| from the arc center.
    // Sign: outside=ar+radius (external), inside=ar-radius (internal).
    let perp_x = -u[1];
    let perp_y =  u[0];

    let mut best: Option<(EntityType, EntityType, EntityType)> = None;
    let mut best_dist = f64::MAX;

    for sign_perp in &[-1.0_f64, 1.0_f64] {
        for sign_circle in &[-1.0_f64, 1.0_f64] {
            // Candidate fillet center offset from line by ±radius in perp direction.
            // Find point on offset line closest to arc center at distance |ar + sign*radius|.
            let off_dist = ar + sign_circle * radius;
            if off_dist < 1e-9 { continue; }

            // The fillet center is at distance `off_dist` from the arc center
            // and also at distance `radius` from the line (perpendicular).
            // Parametrize: fc = p1 + t*u + sign_perp*radius*perp
            // |fc - ac|^2 = off_dist^2
            // (p1[0] + t*u[0] + sign_perp*radius*perp_x - ac[0])^2 + ...= off_dist^2
            let qx = p1[0] + sign_perp * radius * perp_x - ac[0];
            let qy = p1[1] + sign_perp * radius * perp_y - ac[1];
            // (qx + t*u[0])^2 + (qy + t*u[1])^2 = off_dist^2
            let qa = u[0]*u[0] + u[1]*u[1]; // = 1.0
            let qb = 2.0*(qx*u[0] + qy*u[1]);
            let qc = qx*qx + qy*qy - off_dist*off_dist;
            let disc = qb*qb - 4.0*qa*qc;
            if disc < 0.0 { continue; }
            let sq = disc.sqrt();
            for &sign_t in &[-1.0_f64, 1.0_f64] {
                let t_fc = (-qb + sign_t * sq) / (2.0 * qa);
                let fc = [p1[0] + t_fc*u[0] + sign_perp*radius*perp_x,
                          p1[1] + t_fc*u[1] + sign_perp*radius*perp_y];

                // Tangent point on the line
                let tp_line = [p1[0] + t_fc*u[0], p1[1] + t_fc*u[1]];
                // Tangent point on the arc circle
                let fd = [(ac[0] - fc[0]), (ac[1] - fc[1])];
                let fdl = (fd[0]*fd[0]+fd[1]*fd[1]).sqrt().max(1e-12);
                let tp_arc = [ac[0] + fd[0]/fdl * ar, ac[1] + fd[1]/fdl * ar];

                // The tangent point on the arc must be within the arc's angular range
                let tp_arc_angle = arc_angle_at(ac, tp_arc);
                let tp_arc_clamped = clamp_angle_to_arc(tp_arc_angle, a_start, a_end);
                if (norm360(tp_arc_angle) - norm360(tp_arc_clamped)).abs() > 0.5 { continue; }

                // The tangent point on the line must be on the correct side of the click
                // (prefer the intersection closest to the click)
                let dist_to_click_line = (tp_line[0]-click_line[0]).hypot(tp_line[1]-click_line[1]);
                let dist_to_click_arc  = (tp_arc[0]-click_arc[0]).hypot(tp_arc[1]-click_arc[1]);
                let dist_total = dist_to_click_line + dist_to_click_arc;
                if dist_total >= best_dist { continue; }

                // Build trimmed line
                let Some(new_line) = trim_line_to_point(line, tp_line, click_line) else { continue };

                // Build trimmed arc
                let arc_click_angle = arc_angle_at(ac, click_arc);
                let arc_click_rel = (arc_click_angle - a_start + 360.0) % 360.0;
                let tp_arc_rel = (tp_arc_clamped - a_start + 360.0) % 360.0;
                let new_arc = if tp_arc_rel <= arc_click_rel {
                    trim_arc(arc, tp_arc_clamped, a_end)
                } else {
                    trim_arc(arc, a_start, tp_arc_clamped)
                };

                // Build fillet arc angles
                let fa_line = arc_angle_at(fc, tp_line);
                let fa_arc  = arc_angle_at(fc, tp_arc);
                let cross = (tp_line[0]-fc[0])*(tp_arc[1]-fc[1]) - (tp_line[1]-fc[1])*(tp_arc[0]-fc[0]);
                let (fstart, fend) = if cross >= 0.0 { (fa_line, fa_arc) } else { (fa_arc, fa_line) };
                let mut fillet_arc = ArcEnt::new();
                fillet_arc.common = line.common.clone();
                fillet_arc.common.handle = Handle::NULL;
                fillet_arc.center = Vector3::new(fc[0], fc[1], z);
                fillet_arc.radius = radius;
                fillet_arc.start_angle = norm360(fstart);
                fillet_arc.end_angle   = norm360(fend);

                best_dist = dist_total;
                best = Some((EntityType::Line(new_line), EntityType::Arc(new_arc), EntityType::Arc(fillet_arc)));
            }
        }
    }

    best.map(|(l, a, f)| (l, a, Some(f)))
}

/// Fillet two arcs.
fn fillet_arc_arc(
    a1: &ArcEnt,
    click1: [f64; 2],
    a2: &ArcEnt,
    click2: [f64; 2],
    radius: f64,
    z: f64,
) -> Option<(EntityType, EntityType, Option<EntityType>)> {
    let (c1, r1, s1, e1, _) = arc_geom(a1);
    let (c2, r2, s2, e2, _) = arc_geom(a2);

    if radius < 1e-9 {
        // r=0: trim both arcs to their intersection point
        let pts = circle_circle_pts(c1, r1, c2, r2);
        if pts.is_empty() { return None; }
        // Pick the intersection point nearest to the average of the two clicks
        let cx = (click1[0] + click2[0]) / 2.0;
        let cy = (click1[1] + click2[1]) / 2.0;
        let ip = *pts.iter().min_by(|a, b| {
            (a[0]-cx).hypot(a[1]-cy).partial_cmp(&(b[0]-cx).hypot(b[1]-cy)).unwrap()
        })?;

        let ia1 = arc_angle_at(c1, ip);
        let ia2 = arc_angle_at(c2, ip);
        let ic1 = clamp_angle_to_arc(ia1, s1, e1);
        let ic2 = clamp_angle_to_arc(ia2, s2, e2);

        let ca1 = clamp_angle_to_arc(arc_angle_at(c1, click1), s1, e1);
        let ca2 = clamp_angle_to_arc(arc_angle_at(c2, click2), s2, e2);

        let new_a1 = if (ic1 - s1 + 360.0) % 360.0 <= (ca1 - s1 + 360.0) % 360.0 {
            trim_arc(a1, ic1, e1)
        } else {
            trim_arc(a1, s1, ic1)
        };
        let new_a2 = if (ic2 - s2 + 360.0) % 360.0 <= (ca2 - s2 + 360.0) % 360.0 {
            trim_arc(a2, ic2, e2)
        } else {
            trim_arc(a2, s2, ic2)
        };
        return Some((EntityType::Arc(new_a1), EntityType::Arc(new_a2), None));
    }

    // For r>0: find a circle of `radius` tangent to both arc circles.
    // Center lies at |r1 ± radius| from c1 and |r2 ± radius| from c2.
    let mut best: Option<(EntityType, EntityType, EntityType)> = None;
    let mut best_dist = f64::MAX;

    for sign1 in &[-1.0_f64, 1.0_f64] {
        let d1 = r1 + sign1 * radius;
        if d1 < 1e-9 { continue; }
        for sign2 in &[-1.0_f64, 1.0_f64] {
            let d2 = r2 + sign2 * radius;
            if d2 < 1e-9 { continue; }
            for fc in circle_circle_pts(c1, d1, c2, d2) {
                // Tangent points on each arc
                let fd1 = [(c1[0]-fc[0]), (c1[1]-fc[1])];
                let fdl1 = (fd1[0]*fd1[0]+fd1[1]*fd1[1]).sqrt().max(1e-12);
                let tp1 = [c1[0]+fd1[0]/fdl1*r1, c1[1]+fd1[1]/fdl1*r1];

                let fd2 = [(c2[0]-fc[0]), (c2[1]-fc[1])];
                let fdl2 = (fd2[0]*fd2[0]+fd2[1]*fd2[1]).sqrt().max(1e-12);
                let tp2 = [c2[0]+fd2[0]/fdl2*r2, c2[1]+fd2[1]/fdl2*r2];

                // Tangent points must lie within respective arc ranges
                let tp1a = arc_angle_at(c1, tp1);
                let tp2a = arc_angle_at(c2, tp2);
                let tc1 = clamp_angle_to_arc(tp1a, s1, e1);
                let tc2 = clamp_angle_to_arc(tp2a, s2, e2);
                if (norm360(tp1a) - norm360(tc1)).abs() > 0.5 { continue; }
                if (norm360(tp2a) - norm360(tc2)).abs() > 0.5 { continue; }

                let dist_total = (tp1[0]-click1[0]).hypot(tp1[1]-click1[1])
                               + (tp2[0]-click2[0]).hypot(tp2[1]-click2[1]);
                if dist_total >= best_dist { continue; }

                let ca1 = clamp_angle_to_arc(arc_angle_at(c1, click1), s1, e1);
                let ca2 = clamp_angle_to_arc(arc_angle_at(c2, click2), s2, e2);

                let new_a1 = if (tc1 - s1 + 360.0) % 360.0 <= (ca1 - s1 + 360.0) % 360.0 {
                    trim_arc(a1, tc1, e1)
                } else {
                    trim_arc(a1, s1, tc1)
                };
                let new_a2 = if (tc2 - s2 + 360.0) % 360.0 <= (ca2 - s2 + 360.0) % 360.0 {
                    trim_arc(a2, tc2, e2)
                } else {
                    trim_arc(a2, s2, tc2)
                };

                let fa1 = arc_angle_at(fc, tp1);
                let fa2 = arc_angle_at(fc, tp2);
                let cross = (tp1[0]-fc[0])*(tp2[1]-fc[1]) - (tp1[1]-fc[1])*(tp2[0]-fc[0]);
                let (fstart, fend) = if cross >= 0.0 { (fa1, fa2) } else { (fa2, fa1) };
                let mut fillet_arc = ArcEnt::new();
                fillet_arc.common = a1.common.clone();
                fillet_arc.common.handle = Handle::NULL;
                fillet_arc.center = Vector3::new(fc[0], fc[1], z);
                fillet_arc.radius = radius;
                fillet_arc.start_angle = norm360(fstart);
                fillet_arc.end_angle   = norm360(fend);

                best_dist = dist_total;
                best = Some((EntityType::Arc(new_a1), EntityType::Arc(new_a2), EntityType::Arc(fillet_arc)));
            }
        }
    }

    best.map(|(a1, a2, f)| (a1, a2, Some(f)))
}

/// Trim a line endpoint nearest to the intersection point, keeping the click side.
fn trim_line_to_point(line: &LineEnt, isect: [f64; 2], click: [f64; 2]) -> Option<LineEnt> {
    let mut l = line.clone();
    l.common.handle = Handle::NULL;
    let (p1, _, u, len) = line_geom(line);
    if len < 1e-12 { return None; }
    // Parameter of intersection
    let t_i = (isect[0]-p1[0])*u[0] + (isect[1]-p1[1])*u[1];
    // Parameter of click
    let t_c = (click[0]-p1[0])*u[0] + (click[1]-p1[1])*u[1];
    // If click is past intersection on the + side, keep intersection..end
    if t_c >= t_i {
        l.start = Vector3::new(isect[0], isect[1], line.start.z);
    } else {
        l.end   = Vector3::new(isect[0], isect[1], line.end.z);
    }
    Some(l)
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
    WaitingForRadius,
    Second {
        h1: Handle,
        e1: FilletEntity,
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
                "FILLET  Select first object (Line/Arc)  [R={:.4} | type R to change]:",
                self.radius
            ),
            FilletStep::WaitingForRadius => format!(
                "FILLET  Enter fillet radius <{:.4}>:",
                self.radius
            ),
            FilletStep::Second { .. } => {
                format!("FILLET  Select second object (Line/Arc)  [R={:.4}]:", self.radius)
            }
        }
    }

    fn wants_text_input(&self) -> bool {
        matches!(self.step, FilletStep::WaitingForRadius)
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        match self.step {
            FilletStep::WaitingForRadius => {
                let t = text.trim();
                if t.is_empty() {
                    // Keep current radius, return to First
                    self.step = FilletStep::First;
                    return Some(CmdResult::NeedPoint);
                }
                if let Ok(v) = t.replace(',', ".").parse::<f64>() {
                    if v >= 0.0 {
                        self.radius = v;
                        defaults::set_fillet_radius(v as f32);
                    }
                    self.step = FilletStep::First;
                    return Some(CmdResult::NeedPoint);
                }
                // Invalid — stay and re-prompt
                Some(CmdResult::NeedPoint)
            }
            FilletStep::First | FilletStep::Second { .. } => {
                let t = text.trim();
                let upper = t.to_uppercase();
                // "R" alone → enter sub-step to collect radius
                if upper == "R" {
                    self.step = FilletStep::WaitingForRadius;
                    return Some(CmdResult::NeedPoint);
                }
                // "R 5.0" inline shorthand
                if upper.starts_with('R') {
                    let body = t[1..].trim();
                    if let Ok(v) = body.replace(',', ".").parse::<f64>() {
                        if v >= 0.0 {
                            self.radius = v;
                            defaults::set_fillet_radius(v as f32);
                        }
                        return Some(CmdResult::NeedPoint);
                    }
                    // "R" + invalid body → enter sub-step
                    self.step = FilletStep::WaitingForRadius;
                    return Some(CmdResult::NeedPoint);
                }
                None
            }
        }
    }

    fn needs_entity_pick(&self) -> bool {
        !matches!(self.step, FilletStep::WaitingForRadius)
    }

    fn on_entity_pick(&mut self, handle: Handle, pt: Vec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }
        let click = [pt.x as f64, pt.z as f64]; // Y-up: world Z = DXF Y

        match &self.step {
            FilletStep::WaitingForRadius => return CmdResult::NeedPoint,
            FilletStep::First => {
                let e1 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(FilletEntity::from_entity);
                if let Some(e) = e1 {
                    self.step = FilletStep::Second { h1: handle, e1: e, click1: click };
                    CmdResult::NeedPoint
                } else {
                    CmdResult::NeedPoint
                }
            }
            FilletStep::Second { h1, e1, click1 } => {
                let h1 = *h1;
                let e1 = e1.clone();
                let click1 = *click1;
                if handle == h1 {
                    return CmdResult::NeedPoint;
                }
                let e2 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(FilletEntity::from_entity);
                if let Some(e2) = e2 {
                    match compute_fillet_entities(&e1, click1, &e2, click, self.radius) {
                        Some((new_e1, new_e2, maybe_arc)) => {
                            let mut additions = vec![];
                            if let Some(arc) = maybe_arc { additions.push(arc); }
                            CmdResult::ReplaceMany(
                                vec![(h1, vec![new_e1]), (handle, vec![new_e2])],
                                additions,
                            )
                        }
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
        let click = [pt.x as f64, pt.z as f64];

        match &self.step {
            FilletStep::WaitingForRadius => vec![],
            FilletStep::First => {
                let pts = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(|e| {
                        FilletEntity::from_entity(e).map(|fe| entity_pts(&fe.to_entity_type()))
                    });
                if let Some(pts) = pts {
                    vec![WireModel::solid("fillet_hover".into(), pts, WireModel::CYAN, false)]
                } else {
                    vec![]
                }
            }
            FilletStep::Second { e1, click1, .. } => {
                let e1 = e1.clone();
                let click1 = *click1;
                let e2 = self
                    .all_entities
                    .iter()
                    .find(|e| e.common().handle == handle)
                    .and_then(FilletEntity::from_entity);
                if let Some(e2) = e2 {
                    if let Some((new_e1, new_e2, maybe_arc)) =
                        compute_fillet_entities(&e1, click1, &e2, click, self.radius)
                    {
                        let mut out = vec![
                            WireModel::solid("fillet_e1".into(), entity_pts(&new_e1), WireModel::CYAN, false),
                            WireModel::solid("fillet_e2".into(), entity_pts(&new_e2), WireModel::CYAN, false),
                        ];
                        if let Some(arc) = maybe_arc {
                            out.push(WireModel::solid("fillet_arc".into(), entity_pts(&arc), WireModel::CYAN, false));
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
    WaitingForDist1,
    WaitingForDist2,
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
                "CHAMFER  Select first line  [D1={:.4} D2={:.4} | type D to change]:",
                self.dist1, self.dist2
            ),
            ChamferStep::WaitingForDist1 => format!(
                "CHAMFER  Enter first chamfer distance <{:.4}>:",
                self.dist1
            ),
            ChamferStep::WaitingForDist2 => format!(
                "CHAMFER  Enter second chamfer distance <{:.4}>:",
                self.dist2
            ),
            ChamferStep::Second { .. } => format!(
                "CHAMFER  Select second line  [D1={:.4} D2={:.4}]:",
                self.dist1, self.dist2
            ),
        }
    }

    fn wants_text_input(&self) -> bool {
        matches!(
            self.step,
            ChamferStep::WaitingForDist1 | ChamferStep::WaitingForDist2
        )
    }

    fn on_text_input(&mut self, text: &str) -> Option<CmdResult> {
        match self.step {
            ChamferStep::WaitingForDist1 => {
                let t = text.trim();
                if t.is_empty() {
                    // Keep current dist1, move on to dist2
                    self.step = ChamferStep::WaitingForDist2;
                    return Some(CmdResult::NeedPoint);
                }
                if let Ok(v) = t.replace(',', ".").parse::<f64>() {
                    self.dist1 = v.max(0.0);
                    defaults::set_chamfer_dist1(self.dist1 as f32);
                    self.step = ChamferStep::WaitingForDist2;
                    return Some(CmdResult::NeedPoint);
                }
                // Invalid — stay and re-prompt
                Some(CmdResult::NeedPoint)
            }
            ChamferStep::WaitingForDist2 => {
                let t = text.trim();
                if t.is_empty() {
                    // Keep current dist2, return to First
                    self.step = ChamferStep::First;
                    return Some(CmdResult::NeedPoint);
                }
                if let Ok(v) = t.replace(',', ".").parse::<f64>() {
                    self.dist2 = v.max(0.0);
                    defaults::set_chamfer_dist2(self.dist2 as f32);
                    self.step = ChamferStep::First;
                    return Some(CmdResult::NeedPoint);
                }
                // Invalid — stay and re-prompt
                Some(CmdResult::NeedPoint)
            }
            ChamferStep::First | ChamferStep::Second { .. } => {
                let t = text.trim();
                let upper = t.to_uppercase();
                // "D" alone → enter sub-step to collect distances
                if upper == "D" {
                    self.step = ChamferStep::WaitingForDist1;
                    return Some(CmdResult::NeedPoint);
                }
                // "D 5.0" or "D 5.0 3.0" inline shorthand
                if upper.starts_with('D') {
                    let body = t[1..].trim();
                    let parts: Vec<f64> = body
                        .split_whitespace()
                        .filter_map(|s| s.replace(',', ".").parse::<f64>().ok())
                        .collect();
                    if !parts.is_empty() {
                        if let Some(&v) = parts.first() {
                            self.dist1 = v.max(0.0);
                            defaults::set_chamfer_dist1(self.dist1 as f32);
                        }
                        if let Some(&v) = parts.get(1) {
                            self.dist2 = v.max(0.0);
                            defaults::set_chamfer_dist2(self.dist2 as f32);
                        } else {
                            self.dist2 = self.dist1;
                            defaults::set_chamfer_dist2(self.dist2 as f32);
                        }
                        return Some(CmdResult::NeedPoint);
                    }
                    // "D" + invalid body → enter sub-step
                    self.step = ChamferStep::WaitingForDist1;
                    return Some(CmdResult::NeedPoint);
                }
                None
            }
        }
    }

    fn needs_entity_pick(&self) -> bool {
        !matches!(
            self.step,
            ChamferStep::WaitingForDist1 | ChamferStep::WaitingForDist2
        )
    }

    fn on_entity_pick(&mut self, handle: Handle, pt: Vec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }
        let click = [pt.x as f64, pt.y as f64];

        match &self.step {
            ChamferStep::WaitingForDist1 | ChamferStep::WaitingForDist2 => {
                return CmdResult::NeedPoint;
            }
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
            ChamferStep::WaitingForDist1 | ChamferStep::WaitingForDist2 => return vec![],
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
