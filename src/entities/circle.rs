use glam::Vec3;
use h7cad_native_model::geom_ocs::{arbitrary_axis, ocs_to_wcs};
use truck_modeling::{builder, Point3, Wire};

use crate::command::EntityTransform;
use crate::entities::common::{
    diamond_grip, edit_prop as edit, parse_f64, pt_to_vec3, ro_prop as ro, scale_pt, square_grip,
    transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::{SnapHint, TangentGeom};

// ── Free functions working on native fields ─────────────────────────────

/// Tessellate a CIRCLE assuming OCS == WCS (i.e. `normal = (0, 0, 1)`).
///
/// This is the legacy 2D-plan code path. Callers that have access to
/// the entity's `extrusion` vector should prefer
/// [`to_truck_with_normal`] so circles on tilted planes render in the
/// correct WCS plane. The two-argument form is kept as a thin wrapper
/// to avoid churn in test fixtures and call-sites that only ever
/// produce world-Z geometry.
pub fn to_truck(center: &[f64; 3], radius: f64) -> TruckEntity {
    to_truck_with_normal(center, radius, [0.0, 0.0, 1.0])
}

/// Tessellate a CIRCLE with the DXF arbitrary-axis algorithm applied.
///
/// `center` is the OCS center as stored on the entity (DXF code 10/20/30).
/// `normal` is the entity's extrusion vector (DXF code 210/220/230).
///
/// The four quadrant snap points are emitted along the OCS X / Y axes
/// (mapped to WCS through the OCS basis), so snap behavior on a tilted
/// circle still tracks the geometric quadrants the user sees.
///
/// When `normal == (0, 0, 1)` and the center already lies in the WCS,
/// this function reduces to the legacy [`to_truck`] behaviour up to
/// floating-point round-off.
pub fn to_truck_with_normal(center: &[f64; 3], radius: f64, normal: [f64; 3]) -> TruckEntity {
    let (ax, ay, _n) = arbitrary_axis(normal);
    let wcs_center = ocs_to_wcs(*center, [0.0, 0.0, 0.0], normal);
    let r = radius;

    let right_ocs = [
        wcs_center[0] + r * ax[0],
        wcs_center[1] + r * ax[1],
        wcs_center[2] + r * ax[2],
    ];
    let left_ocs = [
        wcs_center[0] - r * ax[0],
        wcs_center[1] - r * ax[1],
        wcs_center[2] - r * ax[2],
    ];
    let top_ocs = [
        wcs_center[0] + r * ay[0],
        wcs_center[1] + r * ay[1],
        wcs_center[2] + r * ay[2],
    ];
    let bot_ocs = [
        wcs_center[0] - r * ay[0],
        wcs_center[1] - r * ay[1],
        wcs_center[2] - r * ay[2],
    ];

    let right = builder::vertex(Point3::new(right_ocs[0], right_ocs[1], right_ocs[2]));
    let left = builder::vertex(Point3::new(left_ocs[0], left_ocs[1], left_ocs[2]));
    let top = Point3::new(top_ocs[0], top_ocs[1], top_ocs[2]);
    let bot = Point3::new(bot_ocs[0], bot_ocs[1], bot_ocs[2]);

    let upper = builder::circle_arc(&right, &left, top);
    let lower = builder::circle_arc(&left, &right, bot);
    let wire: Wire = [upper, lower].into_iter().collect();

    let cv = pt_to_vec3(&wcs_center);
    let ax_v = Vec3::new(ax[0] as f32, ax[1] as f32, ax[2] as f32);
    let ay_v = Vec3::new(ay[0] as f32, ay[1] as f32, ay[2] as f32);
    let rf = r as f32;

    TruckEntity {
        object: TruckObject::Contour(wire),
        snap_pts: vec![
            (cv, SnapHint::Center),
            (cv + ax_v * rf, SnapHint::Quadrant),
            (cv + ay_v * rf, SnapHint::Quadrant),
            (cv - ax_v * rf, SnapHint::Quadrant),
            (cv - ay_v * rf, SnapHint::Quadrant),
        ],
        tangent_geoms: vec![TangentGeom::Circle {
            center: [
                wcs_center[0] as f32,
                wcs_center[1] as f32,
                wcs_center[2] as f32,
            ],
            radius: rf,
        }],
        key_vertices: vec![],
        fill_tris: vec![],
    }
}

pub fn grips(center: &[f64; 3], radius: f64) -> Vec<GripDef> {
    let ctr = pt_to_vec3(center);
    let r = radius as f32;
    vec![
        diamond_grip(0, ctr),
        square_grip(1, ctr + Vec3::new(r, 0.0, 0.0)),
        square_grip(2, ctr + Vec3::new(0.0, r, 0.0)),
        square_grip(3, ctr - Vec3::new(r, 0.0, 0.0)),
        square_grip(4, ctr - Vec3::new(0.0, r, 0.0)),
    ]
}

pub fn properties(center: &[f64; 3], radius: f64) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Center X", "center_x", center[0]),
            edit("Center Y", "center_y", center[1]),
            edit("Center Z", "center_z", center[2]),
            edit("Radius", "radius", radius),
            ro("Diameter", "diameter", format!("{:.4}", radius * 2.0)),
            ro(
                "Circumference",
                "circumference",
                format!("{:.4}", radius * 2.0 * std::f64::consts::PI),
            ),
        ],
    }
}

pub fn apply_geom_prop(center: &mut [f64; 3], radius: &mut f64, field: &str, value: &str) {
    let Some(v) = parse_f64(value) else { return };
    match field {
        "center_x" => center[0] = v,
        "center_y" => center[1] = v,
        "center_z" => center[2] = v,
        "radius" if v > 0.0 => *radius = v,
        _ => {}
    }
}

pub fn apply_grip(center: &mut [f64; 3], radius: &mut f64, grip_id: usize, apply: GripApply) {
    match (grip_id, apply) {
        (0, GripApply::Absolute(p)) => {
            center[0] = p.x as f64;
            center[1] = p.y as f64;
            center[2] = p.z as f64;
        }
        (0, GripApply::Translate(d)) => {
            center[0] += d.x as f64;
            center[1] += d.y as f64;
            center[2] += d.z as f64;
        }
        (1..=4, GripApply::Absolute(p)) => {
            let dx = p.x - center[0] as f32;
            let dy = p.y - center[1] as f32;
            *radius = ((dx * dx + dy * dy) as f64).sqrt();
        }
        _ => {}
    }
}

pub fn apply_transform(center: &mut [f64; 3], radius: &mut f64, t: &EntityTransform) {
    transform_pt(center, t);
    if let EntityTransform::Scale { center: c, factor } = t {
        let mut r_pt = [center[0] + *radius, center[1], center[2]];
        scale_pt(&mut r_pt, *c, *factor);
        *radius = ((r_pt[0] - center[0]).powi(2) + (r_pt[1] - center[1]).powi(2)).sqrt();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::wire_model::SnapHint;

    fn approx_eq_v3(a: Vec3, b: Vec3, eps: f32) -> bool {
        (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps
    }

    #[test]
    fn to_truck_default_normal_matches_legacy_2d() {
        let legacy = to_truck(&[1.0, 2.0, 0.0], 5.0);
        let with_n = to_truck_with_normal(&[1.0, 2.0, 0.0], 5.0, [0.0, 0.0, 1.0]);

        assert_eq!(legacy.snap_pts.len(), with_n.snap_pts.len());
        for (a, b) in legacy.snap_pts.iter().zip(with_n.snap_pts.iter()) {
            assert!(
                approx_eq_v3(a.0, b.0, 1.0e-5),
                "snap pt mismatch: legacy={:?} new={:?}",
                a.0,
                b.0
            );
        }
    }

    #[test]
    fn to_truck_with_x_normal_emits_quadrants_in_yz_plane() {
        let entity = to_truck_with_normal(&[0.0, 0.0, 0.0], 1.0, [1.0, 0.0, 0.0]);
        let quadrants: Vec<Vec3> = entity
            .snap_pts
            .iter()
            .filter(|(_, h)| matches!(h, SnapHint::Quadrant))
            .map(|(p, _)| *p)
            .collect();
        assert_eq!(quadrants.len(), 4);
        for q in &quadrants {
            assert!(
                q.x.abs() < 1.0e-5,
                "quadrant should lie in YZ plane (x≈0): {q:?}"
            );
            let r2 = q.y * q.y + q.z * q.z;
            assert!(
                (r2 - 1.0).abs() < 1.0e-5,
                "quadrant should lie on unit circle in YZ plane: {q:?} (r²={r2})"
            );
        }
    }

    #[test]
    fn to_truck_with_tilted_normal_center_in_wcs() {
        let entity = to_truck_with_normal(&[10.0, 20.0, 30.0], 2.0, [0.0, 1.0, 0.0]);
        let center = entity
            .snap_pts
            .iter()
            .find(|(_, h)| matches!(h, SnapHint::Center))
            .map(|(p, _)| *p)
            .expect("center snap pt");
        let TangentGeom::Circle {
            center: tg_c,
            radius: tg_r,
        } = entity.tangent_geoms[0]
        else {
            panic!("expected Circle tangent geometry");
        };
        assert!(approx_eq_v3(
            center,
            Vec3::new(tg_c[0], tg_c[1], tg_c[2]),
            1.0e-5
        ));
        assert!((tg_r - 2.0).abs() < 1.0e-5);
    }

    #[test]
    fn to_truck_quadrants_remain_orthogonal_under_arbitrary_normal() {
        let entity = to_truck_with_normal(&[0.0, 0.0, 0.0], 3.0, [0.5, 0.5, 0.7071]);
        let quadrants: Vec<Vec3> = entity
            .snap_pts
            .iter()
            .filter(|(_, h)| matches!(h, SnapHint::Quadrant))
            .map(|(p, _)| *p)
            .collect();
        assert_eq!(quadrants.len(), 4);
        for q in &quadrants {
            let r = (q.x * q.x + q.y * q.y + q.z * q.z).sqrt();
            assert!(
                (r - 3.0).abs() < 1.0e-4,
                "quadrant must be on circle of radius 3: got |q|={r}"
            );
        }
        let opposite_dot = quadrants[0].dot(quadrants[2]);
        let perpendicular_dot = quadrants[0].dot(quadrants[1]);
        assert!(
            (opposite_dot + 9.0).abs() < 1.0e-3,
            "opposite quadrants should anti-align: dot={opposite_dot}"
        );
        assert!(
            perpendicular_dot.abs() < 1.0e-3,
            "perpendicular quadrants should be orthogonal: dot={perpendicular_dot}"
        );
    }
}
