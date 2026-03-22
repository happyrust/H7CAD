//! CPU-side hit-testing for wire geometry.
//!
//! All tests are performed in **screen space** — wire vertices are projected
//! to 2-D pixel coordinates, then compared against the cursor or selection box.
//! This matches the visual result the user sees.

use std::collections::HashMap;

use acadrust::Handle;
use glam::{Mat4, Vec3};
use iced::{Point, Rectangle};

use super::hatch_model::HatchModel;
use super::wire_model::WireModel;

/// Pixel radius used for single-click wire detection.
const CLICK_THRESHOLD_PX: f32 = 8.0;

// ── Single-click hit test ─────────────────────────────────────────────────

/// Return the `name` of the closest wire whose screen-space segments pass
/// within `CLICK_THRESHOLD_PX` pixels of `cursor`.
///
/// Returns `None` when no wire is close enough.
pub fn click_hit<'a>(
    cursor: Point,
    wires: &'a [WireModel],
    view_proj: Mat4,
    bounds: Rectangle,
) -> Option<&'a str> {
    let mut best_dist = CLICK_THRESHOLD_PX;
    let mut best: Option<&str> = None;

    for wire in wires {
        let screen: Vec<Point> = wire
            .points
            .iter()
            .map(|&p| world_to_screen(Vec3::from(p), view_proj, bounds))
            .collect();

        for seg in screen.windows(2) {
            let d = dist_point_to_segment(cursor, seg[0], seg[1]);
            if d < best_dist {
                best_dist = d;
                best = Some(&wire.name);
            }
        }
    }

    best
}

// ── Box / window selection ────────────────────────────────────────────────

/// Return the names of wires selected by a completed rectangular selection box.
///
/// - **Window mode** (`crossing = false`, left→right drag):
///   ALL projected points must lie inside the box.
/// - **Crossing mode** (`crossing = true`, right→left drag):
///   ANY projected point inside the box is sufficient.
pub fn box_hit<'a>(
    corner_a: Point,
    corner_b: Point,
    crossing: bool,
    wires: &'a [WireModel],
    view_proj: Mat4,
    bounds: Rectangle,
) -> Vec<&'a str> {
    let min_x = corner_a.x.min(corner_b.x);
    let max_x = corner_a.x.max(corner_b.x);
    let min_y = corner_a.y.min(corner_b.y);
    let max_y = corner_a.y.max(corner_b.y);

    // Ignore zero-area boxes.
    if (max_x - min_x) < 1.0 || (max_y - min_y) < 1.0 {
        return vec![];
    }

    let inside = |sp: Point| sp.x >= min_x && sp.x <= max_x && sp.y >= min_y && sp.y <= max_y;

    wires
        .iter()
        .filter_map(|wire| {
            if wire.points.is_empty() {
                return None;
            }

            let screen: Vec<Point> = wire
                .points
                .iter()
                .map(|&p| world_to_screen(Vec3::from(p), view_proj, bounds))
                .collect();

            let hit = if crossing {
                screen.iter().any(|&sp| inside(sp))
            } else {
                screen.iter().all(|&sp| inside(sp))
            };

            if hit {
                Some(wire.name.as_str())
            } else {
                None
            }
        })
        .collect()
}

// ── Polygon / lasso selection ─────────────────────────────────────────────

/// Return the names of wires selected by a freehand polygon lasso.
///
/// - **Window mode** (`crossing = false`): ALL projected points inside polygon.
/// - **Crossing mode** (`crossing = true`): ANY point inside OR any wire
///   segment crosses a polygon edge.
pub fn poly_hit<'a>(
    poly: &[Point],
    crossing: bool,
    wires: &'a [WireModel],
    view_proj: Mat4,
    bounds: Rectangle,
) -> Vec<&'a str> {
    if poly.len() < 3 {
        return vec![];
    }

    wires
        .iter()
        .filter_map(|wire| {
            if wire.points.is_empty() {
                return None;
            }

            let screen: Vec<Point> = wire
                .points
                .iter()
                .map(|&p| world_to_screen(Vec3::from(p), view_proj, bounds))
                .collect();

            let hit = if crossing {
                // Any vertex inside OR any wire segment crosses any polygon edge.
                screen.iter().any(|&sp| point_in_polygon(sp, poly))
                    || screen
                        .windows(2)
                        .any(|seg| segment_crosses_polygon(seg[0], seg[1], poly))
            } else {
                screen.iter().all(|&sp| point_in_polygon(sp, poly))
            };

            if hit {
                Some(wire.name.as_str())
            } else {
                None
            }
        })
        .collect()
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn world_to_screen(world: Vec3, view_proj: Mat4, bounds: Rectangle) -> Point {
    let ndc = view_proj.project_point3(world);
    Point::new(
        (ndc.x + 1.0) * 0.5 * bounds.width,
        (1.0 - ndc.y) * 0.5 * bounds.height,
    )
}

/// Even-odd ray-casting test: is `p` inside the polygon?
fn point_in_polygon(p: Point, poly: &[Point]) -> bool {
    let n = poly.len();
    let mut inside = false;
    let (mut xi, mut yi) = (poly[n - 1].x, poly[n - 1].y);
    for &pt in poly {
        let (xj, yj) = (pt.x, pt.y);
        if ((yi > p.y) != (yj > p.y)) && (p.x < (xj - xi) * (p.y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        (xi, yi) = (xj, yj);
    }
    inside
}

/// Does segment `[a, b]` cross any edge of the polygon?
fn segment_crosses_polygon(a: Point, b: Point, poly: &[Point]) -> bool {
    let n = poly.len();
    for i in 0..n {
        let c = poly[i];
        let d = poly[(i + 1) % n];
        if segments_intersect(a, b, c, d) {
            return true;
        }
    }
    false
}

/// Do segments `[a,b]` and `[c,d]` intersect?
fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let cross = |o: Point, p: Point, q: Point| -> f32 {
        (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x)
    };
    let d1 = cross(c, d, a);
    let d2 = cross(c, d, b);
    let d3 = cross(a, b, c);
    let d4 = cross(a, b, d);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    false
}

// ── Hatch hit-testing ─────────────────────────────────────────────────────

/// Return the Handle of the first hatch whose screen-space boundary polygon
/// contains `cursor`.
pub fn click_hit_hatch(
    cursor: Point,
    hatches: &HashMap<Handle, HatchModel>,
    view_proj: Mat4,
    bounds: Rectangle,
) -> Option<Handle> {
    for (&handle, hatch) in hatches {
        let screen: Vec<Point> = hatch
            .boundary
            .iter()
            .map(|&[x, y]| world_to_screen(Vec3::new(x, y, 0.0), view_proj, bounds))
            .collect();
        if screen.len() >= 3 && point_in_polygon(cursor, &screen) {
            return Some(handle);
        }
    }
    None
}

/// Return Handles of hatches selected by a completed rectangular selection box.
pub fn box_hit_hatch(
    corner_a: Point,
    corner_b: Point,
    crossing: bool,
    hatches: &HashMap<Handle, HatchModel>,
    view_proj: Mat4,
    bounds: Rectangle,
) -> Vec<Handle> {
    let min_x = corner_a.x.min(corner_b.x);
    let max_x = corner_a.x.max(corner_b.x);
    let min_y = corner_a.y.min(corner_b.y);
    let max_y = corner_a.y.max(corner_b.y);

    if (max_x - min_x) < 1.0 || (max_y - min_y) < 1.0 {
        return vec![];
    }

    let inside = |sp: Point| sp.x >= min_x && sp.x <= max_x && sp.y >= min_y && sp.y <= max_y;

    hatches
        .iter()
        .filter_map(|(&handle, hatch)| {
            if hatch.boundary.is_empty() {
                return None;
            }
            let screen: Vec<Point> = hatch
                .boundary
                .iter()
                .map(|&[x, y]| world_to_screen(Vec3::new(x, y, 0.0), view_proj, bounds))
                .collect();
            let hit = if crossing {
                screen.iter().any(|&sp| inside(sp))
            } else {
                screen.iter().all(|&sp| inside(sp))
            };
            if hit {
                Some(handle)
            } else {
                None
            }
        })
        .collect()
}

/// Return Handles of hatches selected by a freehand polygon lasso.
pub fn poly_hit_hatch(
    poly: &[Point],
    crossing: bool,
    hatches: &HashMap<Handle, HatchModel>,
    view_proj: Mat4,
    bounds: Rectangle,
) -> Vec<Handle> {
    if poly.len() < 3 {
        return vec![];
    }

    hatches
        .iter()
        .filter_map(|(&handle, hatch)| {
            if hatch.boundary.is_empty() {
                return None;
            }
            let screen: Vec<Point> = hatch
                .boundary
                .iter()
                .map(|&[x, y]| world_to_screen(Vec3::new(x, y, 0.0), view_proj, bounds))
                .collect();
            let hit = if crossing {
                screen.iter().any(|&sp| point_in_polygon(sp, poly))
                    || screen
                        .windows(2)
                        .any(|seg| segment_crosses_polygon(seg[0], seg[1], poly))
            } else {
                screen.iter().all(|&sp| point_in_polygon(sp, poly))
            };
            if hit {
                Some(handle)
            } else {
                None
            }
        })
        .collect()
}

/// Minimum distance from point `p` to line segment `[a, b]` in 2-D.
fn dist_point_to_segment(p: Point, a: Point, b: Point) -> f32 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let len2 = abx * abx + aby * aby;
    let t = if len2 < 1e-6 {
        0.0
    } else {
        let apx = p.x - a.x;
        let apy = p.y - a.y;
        ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0)
    };
    let cx = a.x + t * abx;
    let cy = a.y + t * aby;
    let dx = p.x - cx;
    let dy = p.y - cy;
    (dx * dx + dy * dy).sqrt()
}
