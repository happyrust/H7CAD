use glam::Vec3;

use crate::command::EntityTransform;
use crate::scene::object::{GripDef, GripShape, PropValue, Property};
use crate::scene::transform::reflect_xy_point;

pub fn square_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: false,
        shape: GripShape::Square,
    }
}

pub fn diamond_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: true,
        shape: GripShape::Diamond,
    }
}

pub fn triangle_grip(id: usize, world: Vec3) -> GripDef {
    GripDef {
        id,
        world,
        is_midpoint: false,
        shape: GripShape::Triangle,
    }
}

pub fn edit_prop(label: &'static str, field: &'static str, value: f64) -> Property {
    Property {
        label: label.into(),
        field,
        value: PropValue::EditText(format!("{value:.4}")),
    }
}

pub fn ro_prop(label: &'static str, field: &'static str, value: impl Into<String>) -> Property {
    Property {
        label: label.into(),
        field,
        value: PropValue::ReadOnly(value.into()),
    }
}

pub fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

// ── Point-level geometry transforms on [f64; 3] ─────────────────────────

#[inline]
pub fn translate_pt(pt: &mut [f64; 3], d: Vec3) {
    pt[0] += d.x as f64;
    pt[1] += d.y as f64;
    pt[2] += d.z as f64;
}

#[inline]
pub fn rotate_pt_z(pt: &mut [f64; 3], center: Vec3, angle_rad: f32) {
    let (sin, cos) = (angle_rad as f64).sin_cos();
    let dx = pt[0] - center.x as f64;
    let dy = pt[1] - center.y as f64;
    pt[0] = center.x as f64 + dx * cos - dy * sin;
    pt[1] = center.y as f64 + dx * sin + dy * cos;
}

#[inline]
pub fn scale_pt(pt: &mut [f64; 3], center: Vec3, factor: f32) {
    let f = factor as f64;
    for i in 0..3 {
        let c = [center.x as f64, center.y as f64, center.z as f64][i];
        pt[i] = c + (pt[i] - c) * f;
    }
}

#[inline]
pub fn mirror_pt(pt: &mut [f64; 3], p1: Vec3, p2: Vec3) {
    let (mut x, mut y) = (pt[0], pt[1]);
    reflect_xy_point(&mut x, &mut y, p1, p2);
    pt[0] = x;
    pt[1] = y;
}

pub fn transform_pt(pt: &mut [f64; 3], t: &EntityTransform) {
    match t {
        EntityTransform::Translate(d) => translate_pt(pt, *d),
        EntityTransform::Rotate { center, angle_rad } => rotate_pt_z(pt, *center, *angle_rad),
        EntityTransform::Scale { center, factor } => scale_pt(pt, *center, *factor),
        EntityTransform::Mirror { p1, p2 } => mirror_pt(pt, *p1, *p2),
    }
}

pub fn transform_angle(angle: &mut f64, t: &EntityTransform) {
    match t {
        EntityTransform::Rotate { angle_rad, .. } => *angle += *angle_rad as f64,
        EntityTransform::Mirror { p1, p2 } => {
            let mirror_angle = ((p2.y - p1.y) as f64).atan2((p2.x - p1.x) as f64);
            *angle = 2.0 * mirror_angle - *angle;
        }
        _ => {}
    }
}

#[inline]
pub fn distance_3d(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[inline]
pub fn pt_to_vec3(p: &[f64; 3]) -> Vec3 {
    Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)
}

#[inline]
pub fn v3_to_arr(v: &acadrust::types::Vector3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

#[inline]
pub fn arr_to_v3(a: &[f64; 3]) -> acadrust::types::Vector3 {
    acadrust::types::Vector3::new(a[0], a[1], a[2])
}

pub fn mirror_dir(dir: &mut [f64; 3], p1: Vec3, p2: Vec3) {
    let ax = (p2.x - p1.x) as f64;
    let ay = (p2.y - p1.y) as f64;
    let len2 = ax * ax + ay * ay;
    if len2 > 1e-12 {
        let dot = dir[0] * ax + dir[1] * ay;
        dir[0] = 2.0 * dot * ax / len2 - dir[0];
        dir[1] = 2.0 * dot * ay / len2 - dir[1];
    }
}

pub fn transform_dir(dir: &mut [f64; 3], t: &EntityTransform) {
    match t {
        EntityTransform::Rotate { angle_rad, .. } => {
            let (sin, cos) = (*angle_rad as f64).sin_cos();
            let (dx, dy) = (dir[0], dir[1]);
            dir[0] = dx * cos - dy * sin;
            dir[1] = dx * sin + dy * cos;
        }
        EntityTransform::Mirror { p1, p2 } => mirror_dir(dir, *p1, *p2),
        _ => {}
    }
}
