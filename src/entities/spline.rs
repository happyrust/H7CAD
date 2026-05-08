use h7cad_native_model::geom_ocs::ocs_to_wcs;
use truck_modeling::{
    base::{BoundedCurve, ParametricCurve},
    builder, BSplineCurve, Curve, Edge, KnotVec, Point3, Wire,
};

use crate::command::EntityTransform;
use crate::entities::common::{pt_to_vec3, ro_prop as ro, square_grip, transform_pt};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};

// ── Free functions ──────────────────────────────────────────────────────

/// Tessellate a SPLINE assuming OCS == WCS. Legacy 2D-plan path.
///
/// Callers with access to the entity's `extrusion` vector should
/// prefer [`to_truck_with_normal`].
#[allow(dead_code)]
pub fn to_truck(degree: i32, knots: &[f64], control_points: &[[f64; 3]]) -> TruckEntity {
    to_truck_with_normal(degree, knots, control_points, [0.0, 0.0, 1.0])
}

/// Tessellate a SPLINE with the DXF arbitrary-axis algorithm.
///
/// `control_points` are interpreted in OCS coordinates (DXF stores
/// SPLINE control points relative to the entity's extrusion plane).
/// Each control point is lifted to WCS via the entity's normal vector
/// before the B-spline is constructed; downstream fitting / sampling
/// then operates in WCS. When `normal == (0, 0, 1)` this matches the
/// legacy [`to_truck`] behaviour up to floating-point round-off.
pub fn to_truck_with_normal(
    degree: i32,
    knots: &[f64],
    control_points: &[[f64; 3]],
    normal: [f64; 3],
) -> TruckEntity {
    let ctrl_pts: Vec<Point3> = control_points
        .iter()
        .map(|p| {
            let w = ocs_to_wcs(*p, [0.0, 0.0, 0.0], normal);
            Point3::new(w[0], w[1], w[2])
        })
        .collect();
    if ctrl_pts.len() < 2 {
        return TruckEntity {
            object: TruckObject::Point(builder::vertex(Point3::new(0.0, 0.0, 0.0))),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: vec![],
        };
    }
    let knot_vec = if !knots.is_empty() {
        KnotVec::from(knots.to_vec())
    } else {
        KnotVec::uniform_knot(degree as usize, ctrl_pts.len() - 1)
    };
    let bspline = BSplineCurve::new(knot_vec, ctrl_pts);
    let (t0, t1) = bspline.range_tuple();
    let p_start = bspline.subs(t0);
    let p_end = bspline.subs(t1);

    let key_vertices: Vec<[f32; 3]> = control_points
        .iter()
        .map(|p| {
            let w = ocs_to_wcs(*p, [0.0, 0.0, 0.0], normal);
            [w[0] as f32, w[1] as f32, w[2] as f32]
        })
        .collect();

    let is_closed = false;
    let gap = {
        let dx = (p_end.x - p_start.x) as f32;
        let dy = (p_end.y - p_start.y) as f32;
        let dz = (p_end.z - p_start.z) as f32;
        (dx * dx + dy * dy + dz * dz).sqrt()
    };

    let object = if is_closed && gap > 1e-6 {
        // The B-spline doesn't self-close — add an explicit closing segment.
        let v_start = builder::vertex(p_start);
        let v_end = builder::vertex(p_end);
        let v_close = builder::vertex(p_start);
        let main_edge = Edge::new(&v_start, &v_end, Curve::BSplineCurve(bspline));
        let close_edge = builder::line(&v_end, &v_close);
        let wire: Wire = [main_edge, close_edge].into_iter().collect();
        TruckObject::Contour(wire)
    } else {
        let v_start = builder::vertex(p_start);
        let v_end = builder::vertex(p_end);
        let edge = Edge::new(&v_start, &v_end, Curve::BSplineCurve(bspline));
        TruckObject::Curve(edge)
    };

    TruckEntity {
        object,
        snap_pts: vec![],
        tangent_geoms: vec![],
        key_vertices,
    }
}

pub fn grips(control_points: &[[f64; 3]]) -> Vec<GripDef> {
    control_points
        .iter()
        .enumerate()
        .map(|(i, p)| square_grip(i, pt_to_vec3(p)))
        .collect()
}

pub fn properties(
    degree: i32,
    control_points: &[[f64; 3]],
    fit_points: &[[f64; 3]],
) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            ro("Degree", "degree", degree.to_string()),
            ro("Control Pts", "ctrl_pts", control_points.len().to_string()),
            ro("Fit Pts", "fit_pts", fit_points.len().to_string()),
        ],
    }
}

pub fn apply_grip(control_points: &mut [[f64; 3]], grip_id: usize, apply: GripApply) {
    if let Some(cp) = control_points.get_mut(grip_id) {
        match apply {
            GripApply::Absolute(p) => {
                cp[0] = p.x as f64;
                cp[1] = p.y as f64;
                cp[2] = p.z as f64;
            }
            GripApply::Translate(d) => {
                cp[0] += d.x as f64;
                cp[1] += d.y as f64;
                cp[2] += d.z as f64;
            }
        }
    }
}

pub fn apply_transform(
    control_points: &mut [[f64; 3]],
    fit_points: &mut [[f64; 3]],
    t: &EntityTransform,
) {
    for cp in control_points.iter_mut() {
        transform_pt(cp, t);
    }
    for fp in fit_points.iter_mut() {
        transform_pt(fp, t);
    }
}
