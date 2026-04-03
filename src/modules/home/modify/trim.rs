// Trim / Extend — ribbon definitions + full command implementations.
//
// TRIM  (TR): Click the segment you want to remove. The command finds all
//             intersections of that entity with every other entity and trims
//             out the clicked interval. Stays active — click more segments,
//             press Enter to finish.
//
// EXTEND (EX): Click near one end of an entity.  The command extends that
//              endpoint to the nearest intersecting boundary. Stays active.

use std::f64::consts::TAU;

use acadrust::entities::{Arc as ArcEnt, Line as LineEnt, Ray as RayEnt, XLine as XLineEnt};
use acadrust::types::Vector3;
use acadrust::{EntityType, Handle};
use glam::Vec3;

use crate::command::{CadCommand, CmdResult};
use crate::modules::IconKind;
use crate::scene::wire_model::WireModel;

// ── Dropdown constants ─────────────────────────────────────────────────────

pub const DROPDOWN_ID: &str = "trim_extend";
pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../../assets/icons/trim.svg"));

pub const DROPDOWN_ITEMS: &[(&str, &str, IconKind)] = &[
    (
        "TRIM",
        "Trim",
        IconKind::Svg(include_bytes!("../../../../assets/icons/trim.svg")),
    ),
    (
        "EXTEND",
        "Extend",
        IconKind::Svg(include_bytes!("../../../../assets/icons/extend.svg")),
    ),
];

// ══════════════════════════════════════════════════════════════════════════
// Geometry helpers
// ══════════════════════════════════════════════════════════════════════════

/// Normalize angle to [0, 2π).
fn norm(a: f64) -> f64 {
    ((a % TAU) + TAU) % TAU
}

/// Is angle `a` within the arc from `s` to `e` (CCW, radians)?
fn in_arc(a: f64, s: f64, e: f64) -> bool {
    let (a, s, e) = (norm(a), norm(s), norm(e));
    if (e - s).abs() < 1e-9 || (e - s - TAU).abs() < 1e-9 {
        return true;
    }
    if s <= e {
        a >= s - 1e-9 && a <= e + 1e-9
    } else {
        a >= s - 1e-9 || a <= e + 1e-9
    }
}

/// Parametric t ∈ [0,1] on arc (a0→a1 CCW) for angle `a`.
fn arc_t(a: f64, a0: f64, a1: f64) -> f64 {
    let span = {
        let s = norm(a1) - norm(a0);
        if s <= 0.0 {
            s + TAU
        } else {
            s
        }
    };
    let da = {
        let d = norm(a) - norm(a0);
        if d < 0.0 {
            d + TAU
        } else {
            d
        }
    };
    (da / span).clamp(0.0, 1.0)
}

/// Intersect infinite lines (p+t·d) and (q+u·e). Returns (t, u).
fn ll(
    px: f64,
    py: f64,
    dx: f64,
    dy: f64,
    qx: f64,
    qy: f64,
    ex: f64,
    ey: f64,
) -> Option<(f64, f64)> {
    let det = dx * ey - dy * ex;
    if det.abs() < 1e-10 {
        return None;
    }
    let t = ((qx - px) * ey - (qy - py) * ex) / det;
    let u = ((qx - px) * dy - (qy - py) * dx) / det;
    Some((t, u))
}

/// Intersect infinite line (p+t·d) with circle (cx,cy,r). Returns t values.
fn lc(px: f64, py: f64, dx: f64, dy: f64, cx: f64, cy: f64, r: f64) -> Vec<f64> {
    let fx = px - cx;
    let fy = py - cy;
    let a = dx * dx + dy * dy;
    let b = 2.0 * (fx * dx + fy * dy);
    let c = fx * fx + fy * fy - r * r;
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return vec![];
    }
    let sq = disc.sqrt();
    if disc < 1e-14 {
        vec![(-b) / (2.0 * a)]
    } else {
        vec![(-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a)]
    }
}

/// Circle-circle intersection: angles on circle 1 where they meet.
fn cc_angles(cx1: f64, cy1: f64, r1: f64, cx2: f64, cy2: f64, r2: f64) -> Vec<f64> {
    let d = ((cx2 - cx1).powi(2) + (cy2 - cy1).powi(2)).sqrt();
    if d < 1e-9 || d > r1 + r2 + 1e-9 || d < (r1 - r2).abs() - 1e-9 {
        return vec![];
    }
    let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
    let h2 = r1 * r1 - a * a;
    if h2 < 0.0 {
        return vec![];
    }
    let h = h2.sqrt();
    let mx = cx1 + a * (cx2 - cx1) / d;
    let my = cy1 + a * (cy2 - cy1) / d;
    let px = h * (cy2 - cy1) / d;
    let py = -h * (cx2 - cx1) / d;
    let a1 = ((my + py) - cy1).atan2((mx + px) - cx1);
    let a2 = ((my - py) - cy1).atan2((mx - px) - cx1);
    if h < 1e-9 {
        vec![a1]
    } else {
        vec![a1, a2]
    }
}

// ── Boundary geometry ─────────────────────────────────────────────────────

/// Virtual extent used to represent infinite ends of Ray / XLine.
const TRIM_EXTENT: f64 = 1_000_000.0;
/// If a trim interval endpoint is beyond this threshold it is treated as "infinite".
const INF_T: f64 = 0.9999;

enum Geo {
    Line {
        handle: Handle,
        p1: [f64; 2],
        p2: [f64; 2],
    },
    Arc {
        handle: Handle,
        cx: f64,
        cy: f64,
        r: f64,
        a0: f64,
        a1: f64,
    },
    Circle {
        handle: Handle,
        cx: f64,
        cy: f64,
        r: f64,
    },
    /// Semi-infinite line from base in +direction.
    Ray {
        handle: Handle,
        bx: f64,
        by: f64,
        dx: f64,
        dy: f64,
    },
    /// Fully-infinite line through base along direction.
    InfLine {
        handle: Handle,
        bx: f64,
        by: f64,
        dx: f64,
        dy: f64,
    },
}

fn build_geos(entities: &[EntityType]) -> Vec<Geo> {
    entities
        .iter()
        .filter_map(|e| {
            let h = e.common().handle;
            match e {
                EntityType::Line(l) => Some(Geo::Line {
                    handle: h,
                    p1: [l.start.x, l.start.y],
                    p2: [l.end.x, l.end.y],
                }),
                EntityType::Arc(a) => Some(Geo::Arc {
                    handle: h,
                    cx: a.center.x,
                    cy: a.center.y,
                    r: a.radius,
                    a0: a.start_angle.to_radians(),
                    a1: a.end_angle.to_radians(),
                }),
                EntityType::Circle(c) => Some(Geo::Circle {
                    handle: h,
                    cx: c.center.x,
                    cy: c.center.y,
                    r: c.radius,
                }),
                EntityType::Ray(r) => Some(Geo::Ray {
                    handle: h,
                    bx: r.base_point.x,
                    by: r.base_point.y,
                    dx: r.direction.x,
                    dy: r.direction.y,
                }),
                EntityType::XLine(x) => Some(Geo::InfLine {
                    handle: h,
                    bx: x.base_point.x,
                    by: x.base_point.y,
                    dx: x.direction.x,
                    dy: x.direction.y,
                }),
                _ => None,
            }
        })
        .collect()
}

// ── Intersection helpers ──────────────────────────────────────────────────

/// Sorted, deduped t-params ∈ [0,1] where LINE segment (ax,ay)→(bx,by) intersects boundaries.
fn line_seg_ts(ax: f64, ay: f64, bx: f64, by: f64, target: Handle, geos: &[Geo]) -> Vec<f64> {
    let (dx, dy) = (bx - ax, by - ay);
    let mut ts = vec![];
    for geo in geos {
        match geo {
            Geo::Line { handle, p1, p2 } => {
                if *handle == target {
                    continue;
                }
                let (ex, ey) = (p2[0] - p1[0], p2[1] - p1[1]);
                if let Some((t, u)) = ll(ax, ay, dx, dy, p1[0], p1[1], ex, ey) {
                    if (-1e-9..=1.0 + 1e-9).contains(&u) && (-1e-9..=1.0 + 1e-9).contains(&t) {
                        ts.push(t.clamp(0.0, 1.0));
                    }
                }
            }
            Geo::Arc {
                handle,
                cx,
                cy,
                r,
                a0,
                a1,
            } => {
                if *handle == target {
                    continue;
                }
                for t in lc(ax, ay, dx, dy, *cx, *cy, *r) {
                    if !(-1e-9..=1.0 + 1e-9).contains(&t) {
                        continue;
                    }
                    let ix = ax + t * dx;
                    let iy = ay + t * dy;
                    if in_arc((iy - cy).atan2(ix - cx), *a0, *a1) {
                        ts.push(t.clamp(0.0, 1.0));
                    }
                }
            }
            Geo::Circle { handle, cx, cy, r } => {
                if *handle == target {
                    continue;
                }
                for t in lc(ax, ay, dx, dy, *cx, *cy, *r) {
                    if (-1e-9..=1.0 + 1e-9).contains(&t) {
                        ts.push(t.clamp(0.0, 1.0));
                    }
                }
            }
            Geo::Ray { handle, bx: rbx, by: rby, dx: rdx, dy: rdy } => {
                if *handle == target { continue; }
                if let Some((t, u)) = ll(ax, ay, dx, dy, *rbx, *rby, *rdx, *rdy) {
                    // Ray: u >= 0 (semi-infinite)
                    if u >= -1e-9 && (-1e-9..=1.0 + 1e-9).contains(&t) {
                        ts.push(t.clamp(0.0, 1.0));
                    }
                }
            }
            Geo::InfLine { handle, bx: ibx, by: iby, dx: idx, dy: idy } => {
                if *handle == target { continue; }
                if let Some((t, _u)) = ll(ax, ay, dx, dy, *ibx, *iby, *idx, *idy) {
                    // XLine: any u accepted
                    if (-1e-9..=1.0 + 1e-9).contains(&t) {
                        ts.push(t.clamp(0.0, 1.0));
                    }
                }
            }
        }
    }
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    ts
}

/// Sorted, deduped t-params ∈ [0,1] where ARC (cx,cy,r,a0→a1) intersects boundaries.
fn arc_seg_ts(
    cx: f64,
    cy: f64,
    r: f64,
    a0: f64,
    a1: f64,
    target: Handle,
    geos: &[Geo],
) -> Vec<f64> {
    let mut ts = vec![];
    for geo in geos {
        let angles: Vec<f64> = match geo {
            Geo::Line { handle, p1, p2 } => {
                if *handle == target {
                    continue;
                }
                let (ldx, ldy) = (p2[0] - p1[0], p2[1] - p1[1]);
                lc(p1[0], p1[1], ldx, ldy, cx, cy, r)
                    .into_iter()
                    .filter(|&u| (-1e-9..=1.0 + 1e-9).contains(&u))
                    .map(|u| (p1[1] + u * ldy - cy).atan2(p1[0] + u * ldx - cx))
                    .collect()
            }
            Geo::Arc {
                handle,
                cx: cx2,
                cy: cy2,
                r: r2,
                a0: a02,
                a1: a12,
            } => {
                if *handle == target {
                    continue;
                }
                cc_angles(cx, cy, r, *cx2, *cy2, *r2)
                    .into_iter()
                    .filter(|&a| in_arc(a, *a02, *a12))
                    .collect()
            }
            Geo::Circle {
                handle,
                cx: cx2,
                cy: cy2,
                r: r2,
            } => {
                if *handle == target {
                    continue;
                }
                cc_angles(cx, cy, r, *cx2, *cy2, *r2)
            }
            Geo::Ray { handle, bx: rbx, by: rby, dx: rdx, dy: rdy } => {
                if *handle == target { continue; }
                // Intersect arc circle with the Ray direction
                lc(*rbx, *rby, *rdx, *rdy, cx, cy, r)
                    .into_iter()
                    .filter(|&u| u >= -1e-9) // Ray: u >= 0
                    .map(|u| (rby + u * rdy - cy).atan2(rbx + u * rdx - cx))
                    .collect()
            }
            Geo::InfLine { handle, bx: ibx, by: iby, dx: idx, dy: idy } => {
                if *handle == target { continue; }
                // XLine: any u accepted
                lc(*ibx, *iby, *idx, *idy, cx, cy, r)
                    .into_iter()
                    .map(|u| (iby + u * idy - cy).atan2(ibx + u * idx - cx))
                    .collect()
            }
        };
        for a in angles {
            if in_arc(a, a0, a1) {
                ts.push(arc_t(a, a0, a1));
            }
        }
    }
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    ts
}

// ── Trim helpers ──────────────────────────────────────────────────────────

/// Remove the t-interval containing `t_click` from sorted ts.  Returns surviving pieces.
fn trim_intervals(ts: &[f64], t_click: f64) -> Vec<(f64, f64)> {
    let mut bounds = vec![0.0f64];
    bounds.extend_from_slice(ts);
    bounds.push(1.0);
    bounds.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

    let remove = bounds
        .windows(2)
        .position(|w| t_click >= w[0] - 1e-6 && t_click <= w[1] + 1e-6);

    bounds
        .windows(2)
        .enumerate()
        .filter(|(idx, _)| Some(*idx) != remove)
        .filter(|(_, w)| (w[1] - w[0]) > 1e-6)
        .map(|(_, w)| (w[0], w[1]))
        .collect()
}

fn lerp2(p1: [f64; 2], p2: [f64; 2], t: f64) -> [f64; 2] {
    [p1[0] + t * (p2[0] - p1[0]), p1[1] + t * (p2[1] - p1[1])]
}

/// Trim a Line entity. Returns the surviving line segments.
fn trim_line(orig: &LineEnt, ts: &[f64], t_click: f64) -> Vec<EntityType> {
    let p1 = [orig.start.x, orig.start.y];
    let p2 = [orig.end.x, orig.end.y];
    let z = orig.start.z;
    trim_intervals(ts, t_click)
        .into_iter()
        .filter_map(|(ta, tb)| {
            let a = lerp2(p1, p2, ta);
            let b = lerp2(p1, p2, tb);
            if (b[0] - a[0]).hypot(b[1] - a[1]) < 1e-6 {
                return None;
            }
            let mut l = orig.clone();
            l.common.handle = Handle::NULL;
            l.start = Vector3::new(a[0], a[1], z);
            l.end = Vector3::new(b[0], b[1], z);
            Some(EntityType::Line(l))
        })
        .collect()
}

/// Trim an Arc entity. Returns the surviving arc segments.
fn trim_arc(orig: &ArcEnt, ts: &[f64], t_click: f64) -> Vec<EntityType> {
    let a0 = orig.start_angle.to_radians();
    let a1 = orig.end_angle.to_radians();
    let span = {
        let s = norm(a1) - norm(a0);
        if s <= 0.0 {
            s + TAU
        } else {
            s
        }
    };
    let angle_at = |t: f64| norm(a0) + span * t;

    trim_intervals(ts, t_click)
        .into_iter()
        .filter_map(|(ta, tb)| {
            if (tb - ta).abs() < 1e-6 {
                return None;
            }
            let mut a = orig.clone();
            a.common.handle = Handle::NULL;
            a.start_angle = angle_at(ta).to_degrees();
            a.end_angle = angle_at(tb).to_degrees();
            Some(EntityType::Arc(a))
        })
        .collect()
}

// ── Extend helper ─────────────────────────────────────────────────────────

/// Extend a Line to the nearest boundary on the extended side.
/// t_click < 0.5 → extend start (look for t < 0); t_click ≥ 0.5 → extend end (t > 1).
fn extend_line(orig: &LineEnt, t_click: f64, geos: &[Geo]) -> Option<EntityType> {
    let ax = orig.start.x;
    let ay = orig.start.y;
    let bx = orig.end.x;
    let by = orig.end.y;
    let (dx, dy) = (bx - ax, by - ay);
    let target = orig.common.handle;
    let extend_end = t_click >= 0.5;

    let mut best_t = if extend_end {
        f64::INFINITY
    } else {
        f64::NEG_INFINITY
    };

    for geo in geos {
        match geo {
            Geo::Line { handle, p1, p2 } => {
                if *handle == target {
                    continue;
                }
                let (ex, ey) = (p2[0] - p1[0], p2[1] - p1[1]);
                if let Some((t, u)) = ll(ax, ay, dx, dy, p1[0], p1[1], ex, ey) {
                    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
                        continue;
                    }
                    if extend_end && t > 1.0 + 1e-6 && t < best_t {
                        best_t = t;
                    }
                    if !extend_end && t < -1e-6 && t > best_t {
                        best_t = t;
                    }
                }
            }
            Geo::Arc {
                handle,
                cx,
                cy,
                r,
                a0,
                a1,
            } => {
                if *handle == target {
                    continue;
                }
                for t in lc(ax, ay, dx, dy, *cx, *cy, *r) {
                    let ix = ax + t * dx;
                    let iy = ay + t * dy;
                    if !in_arc((iy - cy).atan2(ix - cx), *a0, *a1) {
                        continue;
                    }
                    if extend_end && t > 1.0 + 1e-6 && t < best_t {
                        best_t = t;
                    }
                    if !extend_end && t < -1e-6 && t > best_t {
                        best_t = t;
                    }
                }
            }
            Geo::Circle { handle, cx, cy, r } => {
                if *handle == target {
                    continue;
                }
                for t in lc(ax, ay, dx, dy, *cx, *cy, *r) {
                    if extend_end && t > 1.0 + 1e-6 && t < best_t {
                        best_t = t;
                    }
                    if !extend_end && t < -1e-6 && t > best_t {
                        best_t = t;
                    }
                }
            }
            Geo::Ray { handle, bx: rbx, by: rby, dx: rdx, dy: rdy } => {
                if *handle == target { continue; }
                if let Some((t, u)) = ll(ax, ay, dx, dy, *rbx, *rby, *rdx, *rdy) {
                    if u >= -1e-9 { // only forward along the Ray
                        if extend_end && t > 1.0 + 1e-6 && t < best_t { best_t = t; }
                        if !extend_end && t < -1e-6 && t > best_t { best_t = t; }
                    }
                }
            }
            Geo::InfLine { handle, bx: ibx, by: iby, dx: idx, dy: idy } => {
                if *handle == target { continue; }
                if let Some((t, _u)) = ll(ax, ay, dx, dy, *ibx, *iby, *idx, *idy) {
                    if extend_end && t > 1.0 + 1e-6 && t < best_t { best_t = t; }
                    if !extend_end && t < -1e-6 && t > best_t { best_t = t; }
                }
            }
        }
    }

    if !best_t.is_finite() {
        return None;
    }
    let mut line = orig.clone();
    line.common.handle = Handle::NULL;
    let new_x = ax + best_t * dx;
    let new_y = ay + best_t * dy;
    if extend_end {
        line.end = Vector3::new(new_x, new_y, orig.end.z);
    } else {
        line.start = Vector3::new(new_x, new_y, orig.start.z);
    }
    Some(EntityType::Line(line))
}

/// Trim a Ray entity.
/// Virtual t ∈ [0,1]: t=0 → base_point, t=1 → base + TRIM_EXTENT * dir.
/// Surviving pieces become Lines (finite) or Rays (still semi-infinite).
fn trim_ray(orig: &RayEnt, ts: &[f64], t_click: f64) -> Vec<EntityType> {
    let bx = orig.base_point.x;
    let by = orig.base_point.y;
    let bz = orig.base_point.z;
    let dx = orig.direction.x;
    let dy = orig.direction.y;
    let dz = orig.direction.z;
    let pt = |t: f64| [bx + t * dx * TRIM_EXTENT, by + t * dy * TRIM_EXTENT, bz + t * dz * TRIM_EXTENT];

    trim_intervals(ts, t_click)
        .into_iter()
        .filter_map(|(ta, tb)| {
            let pa = pt(ta);
            let pb = pt(tb);
            if (pb[0] - pa[0]).hypot(pb[1] - pa[1]) < 1e-6 { return None; }

            if tb > INF_T {
                // Still extends to infinity → remains a Ray with new base
                let r = RayEnt::new(
                    Vector3::new(pa[0], pa[1], pa[2]),
                    Vector3::new(dx, dy, dz),
                );
                let mut r = r;
                r.common = orig.common.clone();
                r.common.handle = Handle::NULL;
                Some(EntityType::Ray(r))
            } else {
                // Finite segment → Line
                let mut l = LineEnt { common: orig.common.clone(), ..LineEnt::new() };
                l.common.handle = Handle::NULL;
                l.start = Vector3::new(pa[0], pa[1], pa[2]);
                l.end = Vector3::new(pb[0], pb[1], pb[2]);
                Some(EntityType::Line(l))
            }
        })
        .collect()
}

/// Trim an XLine entity.
/// Virtual t ∈ [0,1]: t=0 → base - dir*TRIM_EXTENT, t=0.5 → base, t=1 → base + dir*TRIM_EXTENT.
/// Surviving pieces become Lines (finite), Rays (one infinite end), or the original XLine (both ends).
fn trim_xline(orig: &XLineEnt, ts: &[f64], t_click: f64) -> Vec<EntityType> {
    let bx = orig.base_point.x;
    let by = orig.base_point.y;
    let bz = orig.base_point.z;
    let dx = orig.direction.x;
    let dy = orig.direction.y;
    let dz = orig.direction.z;
    // Point at virtual t: scale factor s = 2t - 1 ∈ [-1, +1]
    let pt = |t: f64| {
        let s = 2.0 * t - 1.0;
        [bx + s * dx * TRIM_EXTENT, by + s * dy * TRIM_EXTENT, bz + s * dz * TRIM_EXTENT]
    };

    trim_intervals(ts, t_click)
        .into_iter()
        .filter_map(|(ta, tb)| {
            let pa = pt(ta);
            let pb = pt(tb);
            let ext_neg = ta < 1.0 - INF_T; // extends toward -infinity
            let ext_pos = tb > INF_T;        // extends toward +infinity

            match (ext_neg, ext_pos) {
                (true, true) => {
                    // Whole XLine survived (shouldn't happen after a real trim)
                    let mut x = orig.clone();
                    x.common.handle = Handle::NULL;
                    Some(EntityType::XLine(x))
                }
                (true, false) => {
                    // Extends toward -infinity: Ray at pb pointing in -dir
                    let r = RayEnt::new(
                        Vector3::new(pb[0], pb[1], pb[2]),
                        Vector3::new(-dx, -dy, -dz),
                    );
                    let mut r = r;
                    r.common = orig.common.clone();
                    r.common.handle = Handle::NULL;
                    Some(EntityType::Ray(r))
                }
                (false, true) => {
                    // Extends toward +infinity: Ray at pa pointing in +dir
                    let r = RayEnt::new(
                        Vector3::new(pa[0], pa[1], pa[2]),
                        Vector3::new(dx, dy, dz),
                    );
                    let mut r = r;
                    r.common = orig.common.clone();
                    r.common.handle = Handle::NULL;
                    Some(EntityType::Ray(r))
                }
                (false, false) => {
                    // Finite segment
                    let mut l = LineEnt { common: orig.common.clone(), ..LineEnt::new() };
                    l.common.handle = Handle::NULL;
                    l.start = Vector3::new(pa[0], pa[1], pa[2]);
                    l.end = Vector3::new(pb[0], pb[1], pb[2]);
                    Some(EntityType::Line(l))
                }
            }
        })
        .collect()
}

// ── Point-generation helpers ──────────────────────────────────────────────

const DIM_RED: [f32; 4] = [1.0, 0.3, 0.3, 0.6];

fn line_pts(l: &LineEnt) -> Vec<[f32; 3]> {
    vec![
        [l.start.x as f32, l.start.y as f32, l.start.z as f32],
        [l.end.x as f32, l.end.y as f32, l.end.z as f32],
    ]
}

fn arc_pts(cx: f64, cy: f64, r: f64, a0: f64, a1: f64, y: f64) -> Vec<[f32; 3]> {
    let span = {
        let s = norm(a1) - norm(a0);
        if s <= 0.0 {
            s + TAU
        } else {
            s
        }
    };
    let steps = (span.abs() * 20.0).ceil().max(4.0) as usize;
    (0..=steps)
        .map(|i| {
            let ang = norm(a0) + span * (i as f64 / steps as f64);
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
            a.start_angle.to_radians(),
            a.end_angle.to_radians(),
            a.center.y,
        ),
        // For preview, show a 20-unit section of semi-infinite results
        EntityType::Ray(r) => {
            let bx = r.base_point.x;
            let by = r.base_point.y;
            let bz = r.base_point.z;
            let far_x = bx + r.direction.x * 20.0;
            let far_y = by + r.direction.y * 20.0;
            let far_z = bz + r.direction.z * 20.0;
            vec![[bx as f32, bz as f32, by as f32], [far_x as f32, far_z as f32, far_y as f32]]
        }
        _ => vec![],
    }
}

// ══════════════════════════════════════════════════════════════════════════
// TrimCommand
// ══════════════════════════════════════════════════════════════════════════

pub struct TrimCommand {
    all_entities: Vec<EntityType>,
    geos: Vec<Geo>,
}

impl TrimCommand {
    pub fn new(all_entities: Vec<EntityType>) -> Self {
        let geos = build_geos(&all_entities);
        Self { all_entities, geos }
    }
}

impl CadCommand for TrimCommand {
    fn name(&self) -> &'static str {
        "TRIM"
    }

    fn prompt(&self) -> String {
        "TRIM  Click segment to remove  [Enter=done]:".into()
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn on_entity_pick(&mut self, handle: Handle, pt: Vec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }

        let entity = self
            .all_entities
            .iter()
            .find(|e| e.common().handle == handle);

        let result: Option<Vec<EntityType>> = match entity {
            Some(EntityType::Line(l)) => {
                let ax = l.start.x;
                let ay = l.start.y;
                let bx = l.end.x;
                let by = l.end.y;
                let ts = line_seg_ts(ax, ay, bx, by, handle, &self.geos);
                if ts.is_empty() {
                    return CmdResult::NeedPoint;
                }
                let dx = bx - ax;
                let dy = by - ay;
                let len2 = dx * dx + dy * dy;
                let t_click = if len2 > 1e-12 {
                    ((pt.x as f64 - ax) * dx + (pt.y as f64 - ay) * dy) / len2
                } else {
                    0.5
                };
                Some(trim_line(l, &ts, t_click))
            }
            Some(EntityType::Arc(a)) => {
                let cx = a.center.x;
                let cy = a.center.y;
                let a0 = a.start_angle.to_radians();
                let a1 = a.end_angle.to_radians();
                let ts = arc_seg_ts(cx, cy, a.radius, a0, a1, handle, &self.geos);
                if ts.is_empty() {
                    return CmdResult::NeedPoint;
                }
                let click_angle = (pt.y as f64 - cy).atan2(pt.x as f64 - cx);
                let t_click = arc_t(click_angle, a0, a1);
                Some(trim_arc(a, &ts, t_click))
            }
            Some(EntityType::Ray(r)) => {
                // Virtual segment: base → base + dir * TRIM_EXTENT (t ∈ [0,1])
                let bx = r.base_point.x;
                let by = r.base_point.y;
                let ex = bx + r.direction.x * TRIM_EXTENT;
                let ey = by + r.direction.y * TRIM_EXTENT;
                let ts = line_seg_ts(bx, by, ex, ey, handle, &self.geos);
                if ts.is_empty() { return CmdResult::NeedPoint; }
                let dx = r.direction.x * TRIM_EXTENT;
                let dy = r.direction.y * TRIM_EXTENT;
                let len2 = dx * dx + dy * dy;
                let t_click = if len2 > 1e-12 {
                    ((pt.x as f64 - bx) * dx + (pt.y as f64 - by) * dy) / len2
                } else { 0.5 };
                Some(trim_ray(r, &ts, t_click))
            }
            Some(EntityType::XLine(x)) => {
                // Virtual segment: base - dir*TRIM_EXTENT → base + dir*TRIM_EXTENT
                let bx = x.base_point.x - x.direction.x * TRIM_EXTENT;
                let by = x.base_point.y - x.direction.y * TRIM_EXTENT;
                let ex = x.base_point.x + x.direction.x * TRIM_EXTENT;
                let ey = x.base_point.y + x.direction.y * TRIM_EXTENT;
                let ts = line_seg_ts(bx, by, ex, ey, handle, &self.geos);
                if ts.is_empty() { return CmdResult::NeedPoint; }
                let dx = ex - bx;
                let dy = ey - by;
                let len2 = dx * dx + dy * dy;
                let t_click = if len2 > 1e-12 {
                    ((pt.x as f64 - bx) * dx + (pt.y as f64 - by) * dy) / len2
                } else { 0.5 };
                Some(trim_xline(x, &ts, t_click))
            }
            _ => None,
        };

        if let Some(new_entities) = result {
            // Snapshot is updated in on_entity_replaced once we know the real handles.
            // Pre-stage: remove old entry now so geos exclude it immediately.
            if let Some(pos) = self
                .all_entities
                .iter()
                .position(|e| e.common().handle == handle)
            {
                self.all_entities.remove(pos);
                // Add pieces with NULL handles as geometry-only placeholders.
                self.all_entities.extend(new_entities.clone());
                self.geos = build_geos(&self.all_entities);
            }
            CmdResult::ReplaceEntity(handle, new_entities)
        } else {
            self.command_line_hint();
            CmdResult::NeedPoint
        }
    }

    fn on_entity_replaced(&mut self, _old: Handle, new_handles: &[acadrust::Handle]) {
        // The last new_handles.len() entries in all_entities are the trimmed pieces
        // that were appended with NULL handles. Assign their real document handles.
        let start = self.all_entities.len().saturating_sub(new_handles.len());
        for (e, &h) in self.all_entities[start..]
            .iter_mut()
            .zip(new_handles.iter())
        {
            match e {
                EntityType::Line(l) => l.common.handle = h,
                EntityType::Arc(a) => a.common.handle = h,
                EntityType::Ray(r) => r.common.handle = h,
                EntityType::XLine(x) => x.common.handle = h,
                _ => {}
            }
        }
        self.geos = build_geos(&self.all_entities);
    }

    fn on_hover_entity(&mut self, handle: Handle, pt: Vec3) -> Vec<WireModel> {
        if handle.is_null() {
            return vec![];
        }

        let entity = self
            .all_entities
            .iter()
            .find(|e| e.common().handle == handle);

        match entity {
            Some(EntityType::Line(l)) => {
                let ax = l.start.x;
                let ay = l.start.y;
                let bx = l.end.x;
                let by = l.end.y;
                let ts = line_seg_ts(ax, ay, bx, by, handle, &self.geos);
                if ts.is_empty() {
                    return vec![];
                }
                let dx = bx - ax;
                let dy = by - ay;
                let len2 = dx * dx + dy * dy;
                let t_click = if len2 > 1e-12 {
                    ((pt.x as f64 - ax) * dx + (pt.y as f64 - ay) * dy) / len2
                } else {
                    0.5
                };
                let survivors = trim_line(l, &ts, t_click);
                let p1 = [l.start.x as f32, l.start.y as f32, l.start.y as f32];
                let p2 = [l.end.x as f32, l.end.y as f32, l.end.y as f32];
                let removed = WireModel::solid("trim_rm".into(), vec![p1, p2], DIM_RED, false);
                let mut out = vec![removed];
                for (i, e) in survivors.iter().enumerate() {
                    let pts = entity_pts(e);
                    out.push(WireModel::solid(
                        format!("trim_keep_{i}"),
                        pts,
                        WireModel::CYAN,
                        false,
                    ));
                }
                out
            }
            Some(EntityType::Arc(a)) => {
                let cx = a.center.x;
                let cy = a.center.y;
                let a0 = a.start_angle.to_radians();
                let a1 = a.end_angle.to_radians();
                let ts = arc_seg_ts(cx, cy, a.radius, a0, a1, handle, &self.geos);
                if ts.is_empty() {
                    return vec![];
                }
                let click_angle = (pt.y as f64 - cy).atan2(pt.x as f64 - cx);
                let t_click = arc_t(click_angle, a0, a1);
                let survivors = trim_arc(a, &ts, t_click);
                let orig_pts = arc_pts(cx, cy, a.radius, a0, a1, a.center.y);
                let removed = WireModel::solid("trim_rm".into(), orig_pts, DIM_RED, false);
                let mut out = vec![removed];
                for (i, e) in survivors.iter().enumerate() {
                    let pts = entity_pts(e);
                    out.push(WireModel::solid(
                        format!("trim_keep_{i}"),
                        pts,
                        WireModel::CYAN,
                        false,
                    ));
                }
                out
            }
            Some(EntityType::Ray(r)) => {
                let bx = r.base_point.x;
                let by = r.base_point.y;
                let ex = bx + r.direction.x * TRIM_EXTENT;
                let ey = by + r.direction.y * TRIM_EXTENT;
                let ts = line_seg_ts(bx, by, ex, ey, handle, &self.geos);
                if ts.is_empty() { return vec![]; }
                let dx = r.direction.x * TRIM_EXTENT;
                let dy = r.direction.y * TRIM_EXTENT;
                let len2 = dx * dx + dy * dy;
                let t_click = if len2 > 1e-12 {
                    ((pt.x as f64 - bx) * dx + (pt.y as f64 - by) * dy) / len2
                } else { 0.5 };
                let survivors = trim_ray(r, &ts, t_click);
                // Show a finite preview section (20 units) for the original ray
                let far = [(bx + r.direction.x * 20.0) as f32, (by + r.direction.y * 20.0) as f32, r.base_point.z as f32];
                let base = [bx as f32, by as f32, r.base_point.z as f32];
                let removed = WireModel::solid("trim_rm".into(), vec![base, far], DIM_RED, false);
                let mut out = vec![removed];
                for (i, e) in survivors.iter().enumerate() {
                    let pts = entity_pts(e);
                    out.push(WireModel::solid(format!("trim_keep_{i}"), pts, WireModel::CYAN, false));
                }
                out
            }
            Some(EntityType::XLine(x)) => {
                let bx = x.base_point.x;
                let by = x.base_point.y;
                let ex_start = bx - x.direction.x * TRIM_EXTENT;
                let ey_start = by - x.direction.y * TRIM_EXTENT;
                let ex_end = bx + x.direction.x * TRIM_EXTENT;
                let ey_end = by + x.direction.y * TRIM_EXTENT;
                let ts = line_seg_ts(ex_start, ey_start, ex_end, ey_end, handle, &self.geos);
                if ts.is_empty() { return vec![]; }
                let dx = ex_end - ex_start;
                let dy = ey_end - ey_start;
                let len2 = dx * dx + dy * dy;
                let t_click = if len2 > 1e-12 {
                    ((pt.x as f64 - ex_start) * dx + (pt.y as f64 - ey_start) * dy) / len2
                } else { 0.5 };
                let survivors = trim_xline(x, &ts, t_click);
                // Show a finite 40-unit preview section around base
                let neg = [(bx - x.direction.x * 20.0) as f32, (by - x.direction.y * 20.0) as f32, x.base_point.z as f32];
                let pos = [(bx + x.direction.x * 20.0) as f32, (by + x.direction.y * 20.0) as f32, x.base_point.z as f32];
                let removed = WireModel::solid("trim_rm".into(), vec![neg, pos], DIM_RED, false);
                let mut out = vec![removed];
                for (i, e) in survivors.iter().enumerate() {
                    let pts = entity_pts(e);
                    out.push(WireModel::solid(format!("trim_keep_{i}"), pts, WireModel::CYAN, false));
                }
                out
            }
            _ => vec![],
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

impl TrimCommand {
    fn command_line_hint(&self) {}
}

// ══════════════════════════════════════════════════════════════════════════
// ExtendCommand
// ══════════════════════════════════════════════════════════════════════════

pub struct ExtendCommand {
    all_entities: Vec<EntityType>,
    geos: Vec<Geo>,
    /// (old_handle, new_entity_with_updated_geometry) — set in on_entity_pick,
    /// consumed in on_entity_replaced to patch the snapshot with both new handle + geometry.
    pending_replace: Option<(Handle, EntityType)>,
}

impl ExtendCommand {
    pub fn new(all_entities: Vec<EntityType>) -> Self {
        let geos = build_geos(&all_entities);
        Self {
            all_entities,
            geos,
            pending_replace: None,
        }
    }
}

impl CadCommand for ExtendCommand {
    fn name(&self) -> &'static str {
        "EXTEND"
    }

    fn prompt(&self) -> String {
        "EXTEND  Click near end of object to extend  [Enter=done]:".into()
    }

    fn needs_entity_pick(&self) -> bool {
        true
    }

    fn on_entity_pick(&mut self, handle: Handle, pt: Vec3) -> CmdResult {
        if handle.is_null() {
            return CmdResult::NeedPoint;
        }

        let entity = self
            .all_entities
            .iter()
            .find(|e| e.common().handle == handle);

        let result: Option<EntityType> = match entity {
            Some(EntityType::Line(l)) => {
                let ax = l.start.x;
                let ay = l.start.y;
                let bx = l.end.x;
                let by = l.end.y;
                let dx = bx - ax;
                let dy = by - ay;
                let len2 = dx * dx + dy * dy;
                let t_click = if len2 > 1e-12 {
                    ((pt.x as f64 - ax) * dx + (pt.y as f64 - ay) * dy) / len2
                } else {
                    0.5
                };
                extend_line(l, t_click, &self.geos)
            }
            _ => None,
        };

        if let Some(new_entity) = result {
            // Save the extended entity so on_entity_replaced can patch the snapshot
            // with both the new geometry and the real document handle.
            self.pending_replace = Some((handle, new_entity.clone()));
            CmdResult::ReplaceEntity(handle, vec![new_entity])
        } else {
            CmdResult::NeedPoint
        }
    }

    fn on_entity_replaced(&mut self, old: Handle, new_handles: &[acadrust::Handle]) {
        if let (Some(&new_handle), Some((pending_old, mut new_entity))) =
            (new_handles.first(), self.pending_replace.take())
        {
            if pending_old == old {
                // Update the snapshot entry: replace geometry + assign real handle.
                if let EntityType::Line(l) = &mut new_entity {
                    l.common.handle = new_handle;
                }
                if let Some(pos) = self
                    .all_entities
                    .iter()
                    .position(|e| e.common().handle == old)
                {
                    self.all_entities[pos] = new_entity;
                }
                self.geos = build_geos(&self.all_entities);
            }
        }
    }

    fn on_hover_entity(&mut self, handle: Handle, pt: Vec3) -> Vec<WireModel> {
        if handle.is_null() {
            return vec![];
        }

        let entity = self
            .all_entities
            .iter()
            .find(|e| e.common().handle == handle);
        if let Some(EntityType::Line(l)) = entity {
            let ax = l.start.x;
            let ay = l.start.y;
            let bx = l.end.x;
            let by = l.end.y;
            let dx = bx - ax;
            let dy = by - ay;
            let len2 = dx * dx + dy * dy;
            let t_click = if len2 > 1e-12 {
                ((pt.x as f64 - ax) * dx + (pt.y as f64 - ay) * dy) / len2
            } else {
                0.5
            };
            if let Some(extended) = extend_line(l, t_click, &self.geos) {
                let pts = entity_pts(&extended);
                return vec![WireModel::solid(
                    "extend_prev".into(),
                    pts,
                    WireModel::CYAN,
                    false,
                )];
            }
        }
        vec![]
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
