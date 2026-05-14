use glam::Vec3;
use h7cad_native_model::geom_ocs::{arbitrary_axis, ocs_to_wcs};
use truck_modeling::{builder, Point3};

use crate::command::EntityTransform;
use crate::entities::common::{
    diamond_grip, edit_prop as edit, parse_f64, pt_to_vec3, scale_pt, square_grip, transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::{SnapHint, TangentGeom};

const TAU: f64 = std::f64::consts::TAU;

// ── Free functions (angles in degrees) ──────────────────────────────────

/// Tessellate an ARC assuming OCS == WCS. Legacy 2D-plan path.
///
/// Callers with access to the entity's `extrusion` vector should
/// prefer [`to_truck_with_normal`] to render arcs on tilted planes
/// in the correct orientation.
pub fn to_truck(center: &[f64; 3], radius: f64, start_angle: f64, end_angle: f64) -> TruckEntity {
    to_truck_with_normal(center, radius, start_angle, end_angle, [0.0, 0.0, 1.0])
}

/// Tessellate an ARC with the DXF arbitrary-axis algorithm applied.
///
/// `start_angle` / `end_angle` are degrees in OCS (counterclockwise
/// around the OCS Z axis). `normal` is the entity's extrusion vector.
///
/// When `normal == (0, 0, 1)` this matches the legacy [`to_truck`]
/// behaviour up to floating-point round-off. When `normal == (0, 0, -1)`
/// the OCS basis flips Y, which automatically reverses the sweep
/// direction — replacing the previous ad-hoc `normal.z < 0` mitigation
/// with a principled OCS handling.
pub fn to_truck_with_normal(
    center: &[f64; 3],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    normal: [f64; 3],
) -> TruckEntity {
    let (ax, ay, _n) = arbitrary_axis(normal);
    let wcs_center = ocs_to_wcs(*center, [0.0, 0.0, 0.0], normal);

    let sa = start_angle.to_radians();
    let ea = end_angle.to_radians();
    let mut end = ea;
    if end < sa {
        end += TAU;
    }
    let mid_a = sa + (end - sa) * 0.5;

    let r = radius;
    let pt_at = |theta: f64| -> Point3 {
        let cx = r * theta.cos();
        let cy = r * theta.sin();
        Point3::new(
            wcs_center[0] + cx * ax[0] + cy * ay[0],
            wcs_center[1] + cx * ax[1] + cy * ay[1],
            wcs_center[2] + cx * ax[2] + cy * ay[2],
        )
    };
    let p_start = pt_at(sa);
    let p_end = pt_at(ea);
    let p_mid = pt_at(mid_a);

    let v_start = builder::vertex(p_start);
    let v_end = builder::vertex(p_end);
    let edge = builder::circle_arc(&v_start, &v_end, p_mid);
    TruckEntity {
        object: TruckObject::Curve(edge),
        snap_pts: vec![(pt_to_vec3(&wcs_center), SnapHint::Center)],
        tangent_geoms: vec![TangentGeom::Circle {
            center: [
                wcs_center[0] as f32,
                wcs_center[1] as f32,
                wcs_center[2] as f32,
            ],
            radius: r as f32,
        }],
        key_vertices: vec![],
        fill_tris: vec![],
    }
}

fn angle_span(start: f32, end: f32) -> f32 {
    let mut span = end - start;
    if span < 0.0 {
        span += std::f32::consts::TAU;
    }
    span
}

pub fn grips(center: &[f64; 3], radius: f64, start_angle: f64, end_angle: f64) -> Vec<GripDef> {
    let ctr = pt_to_vec3(center);
    let r = radius as f32;
    let sa = (start_angle as f32).to_radians();
    let ea = (end_angle as f32).to_radians();
    let ma = sa + angle_span(sa, ea) * 0.5;
    vec![
        diamond_grip(0, ctr),
        square_grip(1, ctr + Vec3::new(r * sa.cos(), r * sa.sin(), 0.0)),
        square_grip(2, ctr + Vec3::new(r * ea.cos(), r * ea.sin(), 0.0)),
        diamond_grip(3, ctr + Vec3::new(r * ma.cos(), r * ma.sin(), 0.0)),
    ]
}

pub fn properties(center: &[f64; 3], radius: f64, start_angle: f64, end_angle: f64) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Center X", "center_x", center[0]),
            edit("Center Y", "center_y", center[1]),
            edit("Center Z", "center_z", center[2]),
            edit("Radius", "radius", radius),
            edit("Start Angle", "start_angle", start_angle),
            edit("End Angle", "end_angle", end_angle),
        ],
    }
}

pub fn apply_geom_prop(
    center: &mut [f64; 3],
    radius: &mut f64,
    start_angle: &mut f64,
    end_angle: &mut f64,
    field: &str,
    value: &str,
) {
    let Some(v) = parse_f64(value) else { return };
    match field {
        "center_x" => center[0] = v,
        "center_y" => center[1] = v,
        "center_z" => center[2] = v,
        "radius" if v > 0.0 => *radius = v,
        "start_angle" => *start_angle = v,
        "end_angle" => *end_angle = v,
        _ => {}
    }
}

pub fn apply_grip(
    center: &mut [f64; 3],
    radius: &mut f64,
    start_angle: &mut f64,
    end_angle: &mut f64,
    grip_id: usize,
    apply: GripApply,
) {
    match (grip_id, apply) {
        (0, GripApply::Translate(d)) => {
            center[0] += d.x as f64;
            center[1] += d.y as f64;
            center[2] += d.z as f64;
        }
        (0, GripApply::Absolute(p)) => {
            center[0] = p.x as f64;
            center[1] = p.y as f64;
            center[2] = p.z as f64;
        }
        (1, GripApply::Absolute(p)) => {
            let dx = p.x - center[0] as f32;
            let dy = p.y - center[1] as f32;
            *start_angle = (dy as f64).atan2(dx as f64).to_degrees();
        }
        (2, GripApply::Absolute(p)) => {
            let dx = p.x - center[0] as f32;
            let dy = p.y - center[1] as f32;
            *end_angle = (dy as f64).atan2(dx as f64).to_degrees();
        }
        (3, GripApply::Translate(d)) => {
            let sa = (*start_angle as f32).to_radians();
            let ea = (*end_angle as f32).to_radians();
            let span = angle_span(sa, ea);
            let mid_a = sa + span * 0.5;
            let r = *radius as f32;
            let mx = center[0] as f32 + r * mid_a.cos() + d.x;
            let my = center[1] as f32 + r * mid_a.sin() + d.y;
            let dx = mx - center[0] as f32;
            let dy = my - center[1] as f32;
            let new_r = (dx * dx + dy * dy).sqrt() as f64;
            if new_r > 1e-6 {
                *radius = new_r;
            }
        }
        _ => {}
    }
}

pub fn apply_transform(
    center: &mut [f64; 3],
    radius: &mut f64,
    start_angle: &mut f64,
    end_angle: &mut f64,
    t: &EntityTransform,
) {
    transform_pt(center, t);
    match t {
        EntityTransform::Scale { center: c, factor } => {
            let mut r_pt = [center[0] + *radius, center[1], center[2]];
            scale_pt(&mut r_pt, *c, *factor);
            *radius = ((r_pt[0] - center[0]).powi(2) + (r_pt[1] - center[1]).powi(2)).sqrt();
        }
        EntityTransform::Rotate { angle_rad, .. } => {
            *start_angle += (*angle_rad as f64).to_degrees();
            *end_angle += (*angle_rad as f64).to_degrees();
        }
        EntityTransform::Mirror { p1, p2 } => {
            let dx = (p2.x - p1.x) as f64;
            let dy = (p2.y - p1.y) as f64;
            let line_angle_deg = dy.atan2(dx).to_degrees();
            let tmp = *start_angle;
            *start_angle = 2.0 * line_angle_deg - *end_angle;
            *end_angle = 2.0 * line_angle_deg - tmp;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq_v3(a: Vec3, b: Vec3, eps: f32) -> bool {
        (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps
    }

    #[test]
    fn to_truck_default_normal_matches_legacy_2d() {
        let legacy = to_truck(&[1.0, 2.0, 0.0], 5.0, 0.0, 90.0);
        let with_n = to_truck_with_normal(&[1.0, 2.0, 0.0], 5.0, 0.0, 90.0, [0.0, 0.0, 1.0]);

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
    fn to_truck_center_translates_with_origin_under_default_normal() {
        let entity = to_truck_with_normal(&[10.0, 20.0, 30.0], 1.0, 0.0, 90.0, [0.0, 0.0, 1.0]);
        let center_snap = entity
            .snap_pts
            .iter()
            .find(|(_, h)| matches!(h, SnapHint::Center))
            .map(|(p, _)| *p)
            .expect("center snap pt");
        assert!(
            approx_eq_v3(center_snap, Vec3::new(10.0, 20.0, 30.0), 1.0e-4),
            "center should be in WCS at OCS center under default normal: {center_snap:?}"
        );
    }

    #[test]
    fn to_truck_with_x_normal_emits_center_in_wcs_yz_plane() {
        let entity = to_truck_with_normal(&[0.0, 0.0, 0.0], 1.0, 0.0, 90.0, [1.0, 0.0, 0.0]);
        let center_snap = entity
            .snap_pts
            .iter()
            .find(|(_, h)| matches!(h, SnapHint::Center))
            .map(|(p, _)| *p)
            .expect("center snap pt");
        assert!(
            approx_eq_v3(center_snap, Vec3::new(0.0, 0.0, 0.0), 1.0e-4),
            "center at origin should remain origin: {center_snap:?}"
        );
        if let TangentGeom::Circle {
            center: c,
            radius: r,
        } = entity.tangent_geoms[0]
        {
            assert!(c[0].abs() < 1e-5);
            assert!((r - 1.0).abs() < 1e-5);
        } else {
            panic!("expected Circle tangent geometry");
        }
    }

    #[test]
    fn to_truck_with_negative_z_normal_reverses_sweep_via_ocs_basis() {
        // For N=(0,0,-1): Ax=(1,0,0), Ay=(0,-1,0). The 0..90° arc on
        // OCS becomes WCS x→1, y→0; mid point at 45° has y < 0.
        let entity = to_truck_with_normal(&[0.0, 0.0, 0.0], 1.0, 0.0, 90.0, [0.0, 0.0, -1.0]);
        if let TangentGeom::Circle {
            center: c,
            radius: r,
        } = entity.tangent_geoms[0]
        {
            assert!(approx_eq_v3(
                Vec3::new(c[0], c[1], c[2]),
                Vec3::new(0.0, 0.0, 0.0),
                1e-5
            ));
            assert!((r - 1.0).abs() < 1e-5);
        } else {
            panic!("expected Circle tangent geometry");
        }
    }
}
