use crate::types::{Transform, Vector3};
use glam::Vec3;

use crate::command::EntityTransform;

#[inline]
fn to_v3(v: Vec3) -> Vector3 {
    Vector3::new(v.x as f64, v.y as f64, v.z as f64)
}

pub fn apply_standard_transform<T>(entity: &mut T, center: Vec3, angle_rad: f32)
where
    T: acadrust::Entity,
{
    let z_axis = Vector3::new(0.0, 0.0, 1.0);
    let t = Transform::from_translation(to_v3(-center))
        .then(&Transform::from_rotation(z_axis, angle_rad as f64))
        .then(&Transform::from_translation(to_v3(center)));
    entity.apply_transform(&t);
}

pub fn apply_standard_scale<T>(entity: &mut T, center: Vec3, factor: f32)
where
    T: acadrust::Entity,
{
    let s = factor as f64;
    let t = Transform::from_scaling_with_origin(Vector3::new(s, s, s), to_v3(center));
    entity.apply_transform(&t);
}

pub fn apply_standard_entity_transform<T, F>(entity: &mut T, t: &EntityTransform, mirror: F)
where
    T: acadrust::Entity,
    F: FnOnce(&mut T, Vec3, Vec3),
{
    match t {
        EntityTransform::Translate(d) => entity.translate(to_v3(*d)),
        EntityTransform::Rotate { center, angle_rad } => {
            apply_standard_transform(entity, *center, *angle_rad)
        }
        EntityTransform::Scale { center, factor } => apply_standard_scale(entity, *center, *factor),
        EntityTransform::Mirror { p1, p2 } => mirror(entity, *p1, *p2),
    }
}

pub fn reflect_xy_point(x: &mut f64, y: &mut f64, p1: Vec3, p2: Vec3) {
    let ax = (p2.x - p1.x) as f64;
    let ay = (p2.y - p1.y) as f64;
    let len2 = ax * ax + ay * ay;
    if len2 < 1e-12 {
        return;
    }
    let rx = *x - p1.x as f64;
    let ry = *y - p1.y as f64;
    let dot = rx * ax + ry * ay;
    let mx = 2.0 * dot * ax / len2 - rx;
    let my = 2.0 * dot * ay / len2 - ry;
    *x = p1.x as f64 + mx;
    *y = p1.y as f64 + my;
}

