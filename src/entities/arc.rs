use glam::Vec3;
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

pub fn to_truck(center: &[f64; 3], radius: f64, start_angle: f64, end_angle: f64) -> TruckEntity {
    let (cx, cy, cz, r) = (center[0], center[1], center[2], radius);
    let sa = start_angle.to_radians();
    let ea = end_angle.to_radians();
    let mut end = ea;
    if end < sa {
        end += TAU;
    }
    let mid_a = sa + (end - sa) * 0.5;
    let p_start = Point3::new(cx + r * sa.cos(), cy + r * sa.sin(), cz);
    let p_end = Point3::new(cx + r * ea.cos(), cy + r * ea.sin(), cz);
    let p_mid = Point3::new(cx + r * mid_a.cos(), cy + r * mid_a.sin(), cz);
    let v_start = builder::vertex(p_start);
    let v_end = builder::vertex(p_end);
    let edge = builder::circle_arc(&v_start, &v_end, p_mid);
    TruckEntity {
        object: TruckObject::Curve(edge),
        snap_pts: vec![(pt_to_vec3(center), SnapHint::Center)],
        tangent_geoms: vec![TangentGeom::Circle {
            center: [cx as f32, cy as f32, cz as f32],
            radius: r as f32,
        }],
        key_vertices: vec![],
    }
}

fn angle_span(start: f32, end: f32) -> f32 {
    let mut span = end - start;
    if span < 0.0 {
        span += std::f32::consts::TAU;
    }
    span
}

pub fn grips(
    center: &[f64; 3],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
) -> Vec<GripDef> {
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

pub fn properties(
    center: &[f64; 3],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
) -> PropSection {
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

