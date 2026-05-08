use glam::Vec3;
use h7cad_native_model::geom_ocs::ocs2d_to_wcs;
use truck_modeling::{builder, Edge, Point3, Wire};

use crate::command::EntityTransform;
use crate::entities::common::{
    diamond_grip, edit_prop as edit, parse_f64, ro_prop as ro, square_grip, transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::TangentGeom;

const TAU: f64 = std::f64::consts::TAU;

/// Lightweight vertex: (x, y, bulge)
pub type NmLwVertex = h7cad_native_model::LwVertex;

/// Midpoint position on an arc segment defined by its bulge.
pub fn arc_midpoint(p0: [f64; 2], p1: [f64; 2], bulge: f64) -> [f64; 2] {
    let angle = 4.0 * bulge.atan();
    let dx = p1[0] - p0[0];
    let dy = p1[1] - p0[1];
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1e-12 {
        return [(p0[0] + p1[0]) * 0.5, (p0[1] + p1[1]) * 0.5];
    }
    let r = (d / 2.0) / (angle / 2.0).sin().abs();
    let mx = (p0[0] + p1[0]) * 0.5;
    let my = (p0[1] + p1[1]) * 0.5;
    let px = -dy / d;
    let py = dx / d;
    let sign = if bulge > 0.0 { 1.0_f64 } else { -1.0_f64 };
    let h = r - (r * r - d * d / 4.0).max(0.0).sqrt();
    let cx = mx + sign * px * (r - h);
    let cy = my + sign * py * (r - h);
    let a0 = (p0[1] - cy).atan2(p0[0] - cx);
    let a1 = (p1[1] - cy).atan2(p1[0] - cx);
    let (sa, mut ea) = if bulge > 0.0 { (a0, a1) } else { (a1, a0) };
    if ea < sa { ea += TAU; }
    let mid_a = sa + (ea - sa) * 0.5;
    [cx + r * mid_a.cos(), cy + r * mid_a.sin()]
}

/// Compute the DXF bulge for an arc that passes through p0, mid_pt, and p1.
/// Returns None when the three points are collinear (straight segment).
pub fn bulge_from_midpoint(p0: [f64; 2], p1: [f64; 2], mid: [f64; 2]) -> Option<f64> {
    let ax = 2.0 * (mid[0] - p0[0]);
    let ay = 2.0 * (mid[1] - p0[1]);
    let bx = 2.0 * (p1[0] - p0[0]);
    let by = 2.0 * (p1[1] - p0[1]);
    let ca = mid[0] * mid[0] + mid[1] * mid[1] - p0[0] * p0[0] - p0[1] * p0[1];
    let cb = p1[0] * p1[0] + p1[1] * p1[1] - p0[0] * p0[0] - p0[1] * p0[1];
    let det = ax * by - ay * bx;
    if det.abs() < 1e-12 {
        return None;
    }
    let cx = (ca * by - cb * ay) / det;
    let cy = (ax * cb - bx * ca) / det;
    let a0 = (p0[1] - cy).atan2(p0[0] - cx);
    let a1 = (p1[1] - cy).atan2(p1[0] - cx);
    let cross = (p1[0] - p0[0]) * (mid[1] - p0[1]) - (p1[1] - p0[1]) * (mid[0] - p0[0]);
    let (sa, mut ea) = if cross > 0.0 { (a0, a1) } else { (a1, a0) };
    if ea < sa { ea += TAU; }
    let span = ea - sa;
    let bulge = (span / 4.0).tan();
    Some(if cross >= 0.0 { bulge } else { -bulge })
}

// ── Free functions ──────────────────────────────────────────────────────

/// Tessellate an LwPolyline assuming OCS == WCS. Legacy 2D-plan path.
///
/// Callers with access to the entity's `extrusion` vector should
/// prefer [`to_truck_with_normal`] so polylines on tilted planes
/// render in the correct WCS plane. Kept around for ergonomics and
/// for the `#[cfg(test)]` regression that locks the default-normal
/// path against the legacy two-argument output.
#[allow(dead_code)]
pub fn to_truck(vertices: &[NmLwVertex], closed: bool, elevation: f64) -> TruckEntity {
    to_truck_with_normal(vertices, closed, elevation, [0.0, 0.0, 1.0])
}

/// Tessellate an LwPolyline with the DXF arbitrary-axis algorithm.
///
/// Each `(x, y)` vertex is interpreted in OCS coordinates with
/// elevation `z = elevation`, then mapped to WCS via the entity's
/// extrusion vector. Bulge arcs are computed in OCS (preserving the
/// existing geometry math) and finally lifted to WCS — this keeps the
/// arc's center & midpoint on the correct tilted plane.
///
/// When `normal == (0, 0, 1)` and `elevation == 0.0` this matches
/// the legacy [`to_truck`] behaviour up to floating-point round-off.
pub fn to_truck_with_normal(
    vertices: &[NmLwVertex],
    closed: bool,
    elevation: f64,
    normal: [f64; 3],
) -> TruckEntity {
    if vertices.is_empty() {
        return TruckEntity {
            object: TruckObject::Point(builder::vertex(Point3::new(0.0, 0.0, 0.0))),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: vec![],
        };
    }

    let count = vertices.len();
    let seg_count = if closed { count } else { count - 1 };
    let mut edges: Vec<Edge> = Vec::new();
    let mut tangents: Vec<TangentGeom> = Vec::new();
    let mut key_verts: Vec<[f32; 3]> = Vec::new();

    let lift =
        |x: f64, y: f64| -> [f64; 3] { ocs2d_to_wcs(x, y, elevation, [0.0, 0.0, 0.0], normal) };
    let lift_pt = |x: f64, y: f64| -> Point3 {
        let p = lift(x, y);
        Point3::new(p[0], p[1], p[2])
    };

    for i in 0..seg_count {
        let v0 = &vertices[i];
        let v1 = &vertices[(i + 1) % count];
        // OCS planar coordinates (z = elevation, implicit).
        let p0_ocs = (v0.x, v0.y);
        let p1_ocs = (v1.x, v1.y);
        // WCS Point3 used by truck_modeling.
        let p0 = lift_pt(p0_ocs.0, p0_ocs.1);
        let p1 = lift_pt(p1_ocs.0, p1_ocs.1);
        let bulge = v0.bulge;

        if bulge.abs() < 1e-9 {
            let tv0 = builder::vertex(p0);
            let tv1 = builder::vertex(p1);
            edges.push(builder::line(&tv0, &tv1));
            tangents.push(TangentGeom::Line {
                p1: [p0.x as f32, p0.y as f32, p0.z as f32],
                p2: [p1.x as f32, p1.y as f32, p1.z as f32],
            });
        } else {
            let angle = 4.0 * bulge.atan();
            // Bulge arc geometry is computed in the OCS plane so the
            // center / midpoint stay coplanar with the polyline; only
            // the final vertices are lifted to WCS for truck.
            let dx = p1_ocs.0 - p0_ocs.0;
            let dy = p1_ocs.1 - p0_ocs.1;
            let d = (dx * dx + dy * dy).sqrt();
            let r = (d / 2.0) / (angle / 2.0).sin().abs();
            let mx = (p0_ocs.0 + p1_ocs.0) * 0.5;
            let my = (p0_ocs.1 + p1_ocs.1) * 0.5;
            let len = d.max(1e-12);
            let px = -dy / len;
            let py = dx / len;
            let sagitta_sign = if bulge > 0.0 { 1.0_f64 } else { -1.0_f64 };
            let h = r - (r * r - d * d / 4.0).max(0.0).sqrt();
            let cx_ocs = mx - sagitta_sign * px * (r - h);
            let cy_ocs = my - sagitta_sign * py * (r - h);
            let mid_a = {
                let a0 = (p0_ocs.1 - cy_ocs).atan2(p0_ocs.0 - cx_ocs);
                let a1 = (p1_ocs.1 - cy_ocs).atan2(p1_ocs.0 - cx_ocs);
                let (sa, mut ea) = if bulge > 0.0 { (a0, a1) } else { (a1, a0) };
                if ea < sa {
                    ea += TAU;
                }
                sa + (ea - sa) * 0.5
            };
            let mid_x_ocs = cx_ocs + r * mid_a.cos();
            let mid_y_ocs = cy_ocs + r * mid_a.sin();
            let p_mid = lift_pt(mid_x_ocs, mid_y_ocs);
            let center_wcs = lift(cx_ocs, cy_ocs);
            let tv0 = builder::vertex(p0);
            let tv1 = builder::vertex(p1);
            edges.push(builder::circle_arc(&tv0, &tv1, p_mid));
            tangents.push(TangentGeom::Circle {
                center: [
                    center_wcs[0] as f32,
                    center_wcs[1] as f32,
                    center_wcs[2] as f32,
                ],
                radius: r as f32,
            });
        }

        if i == 0 {
            key_verts.push([p0.x as f32, p0.y as f32, p0.z as f32]);
        }
        key_verts.push([p1.x as f32, p1.y as f32, p1.z as f32]);
    }

    TruckEntity {
        object: TruckObject::Contour(edges.into_iter().collect::<Wire>()),
        snap_pts: vec![],
        tangent_geoms: tangents,
        key_vertices: key_verts,
    }
}

pub fn grips(vertices: &[NmLwVertex], closed: bool, elevation: f64) -> Vec<GripDef> {
    let elev = elevation as f32;
    let n = vertices.len();
    let seg_count = if closed { n } else { n.saturating_sub(1) };

    let mut out: Vec<GripDef> = vertices
        .iter()
        .enumerate()
        .map(|(i, v)| square_grip(i, Vec3::new(v.x as f32, v.y as f32, elev)))
        .collect();

    for i in 0..seg_count {
        let v0 = &vertices[i];
        if v0.bulge.abs() < 1e-9 {
            continue;
        }
        let v1 = &vertices[(i + 1) % n];
        let mid = arc_midpoint([v0.x, v0.y], [v1.x, v1.y], v0.bulge);
        out.push(diamond_grip(n + i, Vec3::new(mid[0] as f32, mid[1] as f32, elev)));
    }
    out
}

pub fn properties(vertices: &[NmLwVertex], closed: bool, elevation: f64) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            ro("Vertices", "vertices", vertices.len().to_string()),
            ro("Closed", "closed", if closed { "Yes" } else { "No" }),
            edit("Elevation", "elevation", elevation),
        ],
    }
}

pub fn apply_geom_prop(elevation: &mut f64, field: &str, value: &str) {
    if field == "elevation" {
        if let Some(v) = parse_f64(value) {
            *elevation = v;
        }
    }
}

pub fn apply_grip(vertices: &mut [NmLwVertex], grip_id: usize, apply: GripApply) {
    let n = vertices.len();
    if grip_id < n {
        let v = &mut vertices[grip_id];
        match apply {
            GripApply::Absolute(p) => {
                v.x = p.x as f64;
                v.y = p.y as f64;
            }
            GripApply::Translate(d) => {
                v.x += d.x as f64;
                v.y += d.y as f64;
            }
        }
    } else {
        let seg = grip_id - n;
        if seg >= n.saturating_sub(1) {
            return;
        }
        let new_mid: [f64; 2] = match apply {
            GripApply::Absolute(p) => [p.x as f64, p.y as f64],
            GripApply::Translate(d) => {
                let v0 = &vertices[seg];
                let v1 = &vertices[(seg + 1) % n];
                let old = arc_midpoint([v0.x, v0.y], [v1.x, v1.y], v0.bulge);
                [old[0] + d.x as f64, old[1] + d.y as f64]
            }
        };
        let p0 = [vertices[seg].x, vertices[seg].y];
        let p1 = [vertices[(seg + 1) % n].x, vertices[(seg + 1) % n].y];
        if let Some(new_bulge) = bulge_from_midpoint(p0, p1, new_mid) {
            vertices[seg].bulge = new_bulge.clamp(-1e6, 1e6);
        }
    }
}

pub fn apply_transform(vertices: &mut [NmLwVertex], t: &EntityTransform) {
    for v in vertices.iter_mut() {
        let mut pt = [v.x, v.y, 0.0];
        transform_pt(&mut pt, t);
        v.x = pt[0];
        v.y = pt[1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vert(x: f64, y: f64, bulge: f64) -> NmLwVertex {
        NmLwVertex {
            x,
            y,
            bulge,
            start_width: 0.0,
            end_width: 0.0,
        }
    }

    #[test]
    fn to_truck_default_normal_matches_legacy_2d() {
        let verts = vec![
            vert(0.0, 0.0, 0.0),
            vert(1.0, 0.0, 0.0),
            vert(1.0, 1.0, 0.0),
        ];
        let legacy = to_truck(&verts, false, 0.0);
        let with_n = to_truck_with_normal(&verts, false, 0.0, [0.0, 0.0, 1.0]);
        assert_eq!(legacy.key_vertices.len(), with_n.key_vertices.len());
        for (a, b) in legacy.key_vertices.iter().zip(with_n.key_vertices.iter()) {
            for k in 0..3 {
                assert!(
                    (a[k] - b[k]).abs() < 1e-5,
                    "key vertex mismatch [{k}]: legacy={a:?} new={b:?}"
                );
            }
        }
    }

    #[test]
    fn to_truck_with_x_normal_lifts_vertices_into_yz_plane() {
        let verts = vec![
            vert(0.0, 0.0, 0.0),
            vert(1.0, 0.0, 0.0),
            vert(0.0, 1.0, 0.0),
        ];
        let entity = to_truck_with_normal(&verts, false, 0.0, [1.0, 0.0, 0.0]);
        assert_eq!(entity.key_vertices.len(), 3);
        for kv in &entity.key_vertices {
            assert!(
                kv[0].abs() < 1e-4,
                "all vertices should lie in WCS YZ plane (x≈0): {kv:?}"
            );
        }
    }

    #[test]
    fn to_truck_bulge_arc_center_lifted_to_wcs_under_tilted_normal() {
        let verts = vec![vert(0.0, 0.0, 1.0), vert(2.0, 0.0, 0.0)];
        let entity = to_truck_with_normal(&verts, false, 0.0, [0.0, 1.0, 0.0]);
        assert_eq!(entity.tangent_geoms.len(), 1);
        let TangentGeom::Circle { center, .. } = entity.tangent_geoms[0] else {
            panic!("expected Circle tangent geom for bulge arc");
        };
        assert!(
            center[1].abs() < 1e-4,
            "arc center should lie in WCS XZ plane under N=+y: {center:?}"
        );
    }

    #[test]
    fn to_truck_elevation_under_default_normal_offsets_z() {
        let verts = vec![vert(0.0, 0.0, 0.0), vert(1.0, 0.0, 0.0)];
        let entity = to_truck_with_normal(&verts, false, 5.0, [0.0, 0.0, 1.0]);
        for kv in &entity.key_vertices {
            assert!(
                (kv[2] - 5.0).abs() < 1e-4,
                "vertex z should equal elevation 5: {kv:?}"
            );
        }
    }
}
