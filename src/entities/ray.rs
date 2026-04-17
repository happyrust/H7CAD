use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{
    diamond_grip, edit_prop as edit, parse_f64, pt_to_vec3, square_grip, transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};

const DISPLAY_EXTENT: f64 = 1_000_000.0;

// ── Ray free functions (origin + direction as [f64;3]) ──────────────────

pub fn ray_to_truck(origin: &[f64; 3], direction: &[f64; 3]) -> TruckEntity {
    let far = [
        (origin[0] + direction[0] * DISPLAY_EXTENT) as f32,
        (origin[1] + direction[1] * DISPLAY_EXTENT) as f32,
        (origin[2] + direction[2] * DISPLAY_EXTENT) as f32,
    ];
    let start = [origin[0] as f32, origin[1] as f32, origin[2] as f32];
    TruckEntity {
        object: TruckObject::Lines(vec![start, far]),
        snap_pts: vec![],
        tangent_geoms: vec![],
        key_vertices: vec![start],
    }
}

pub fn ray_grips(origin: &[f64; 3], direction: &[f64; 3]) -> Vec<GripDef> {
    let guide_dist = 10.0_f64;
    vec![
        square_grip(0, pt_to_vec3(origin)),
        diamond_grip(
            1,
            Vec3::new(
                (origin[0] + direction[0] * guide_dist) as f32,
                (origin[1] + direction[1] * guide_dist) as f32,
                (origin[2] + direction[2] * guide_dist) as f32,
            ),
        ),
    ]
}

pub fn ray_properties(origin: &[f64; 3], direction: &[f64; 3], prefix: &str) -> PropSection {
    let (bp, dp) = (
        [
            &format!("{prefix}_bx"),
            &format!("{prefix}_by"),
            &format!("{prefix}_bz"),
        ],
        [
            &format!("{prefix}_dx"),
            &format!("{prefix}_dy"),
            &format!("{prefix}_dz"),
        ],
    );
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Base X", string_to_static(bp[0]), origin[0]),
            edit("Base Y", string_to_static(bp[1]), origin[1]),
            edit("Base Z", string_to_static(bp[2]), origin[2]),
            edit("Dir X", string_to_static(dp[0]), direction[0]),
            edit("Dir Y", string_to_static(dp[1]), direction[1]),
            edit("Dir Z", string_to_static(dp[2]), direction[2]),
        ],
    }
}

fn string_to_static(s: &str) -> &'static str {
    match s {
        "ray_bx" => "ray_bx",
        "ray_by" => "ray_by",
        "ray_bz" => "ray_bz",
        "ray_dx" => "ray_dx",
        "ray_dy" => "ray_dy",
        "ray_dz" => "ray_dz",
        "xl_bx" => "xl_bx",
        "xl_by" => "xl_by",
        "xl_bz" => "xl_bz",
        "xl_dx" => "xl_dx",
        "xl_dy" => "xl_dy",
        "xl_dz" => "xl_dz",
        _ => "",
    }
}

pub fn ray_apply_geom_prop(
    origin: &mut [f64; 3],
    direction: &mut [f64; 3],
    field: &str,
    value: &str,
) {
    let Some(v) = parse_f64(value) else { return };
    let idx = match field.chars().last() {
        Some('x') => 0,
        Some('y') => 1,
        Some('z') => 2,
        _ => return,
    };
    if field.contains("_b") {
        origin[idx] = v;
    } else if field.contains("_d") {
        direction[idx] = v;
    }
}

pub fn ray_apply_grip(
    origin: &mut [f64; 3],
    direction: &mut [f64; 3],
    grip_id: usize,
    apply: GripApply,
) {
    match (grip_id, apply) {
        (0, GripApply::Translate(d)) => {
            origin[0] += d.x as f64;
            origin[1] += d.y as f64;
            origin[2] += d.z as f64;
        }
        (0, GripApply::Absolute(p)) => {
            origin[0] = p.x as f64;
            origin[1] = p.y as f64;
            origin[2] = p.z as f64;
        }
        (1, GripApply::Absolute(p)) => {
            let dx = p.x as f64 - origin[0];
            let dy = p.y as f64 - origin[1];
            let dz = p.z as f64 - origin[2];
            let len = (dx * dx + dy * dy + dz * dz).sqrt();
            if len > 1e-9 {
                direction[0] = dx / len;
                direction[1] = dy / len;
                direction[2] = dz / len;
            }
        }
        _ => {}
    }
}

pub fn ray_apply_transform(
    origin: &mut [f64; 3],
    direction: &mut [f64; 3],
    t: &EntityTransform,
) {
    transform_pt(origin, t);
    crate::entities::common::transform_dir(direction, t);
}

// ── XLine free functions ────────────────────────────────────────────────

pub fn xline_to_truck(origin: &[f64; 3], direction: &[f64; 3]) -> TruckEntity {
    let far_pos = [
        (origin[0] + direction[0] * DISPLAY_EXTENT) as f32,
        (origin[1] + direction[1] * DISPLAY_EXTENT) as f32,
        (origin[2] + direction[2] * DISPLAY_EXTENT) as f32,
    ];
    let far_neg = [
        (origin[0] - direction[0] * DISPLAY_EXTENT) as f32,
        (origin[1] - direction[1] * DISPLAY_EXTENT) as f32,
        (origin[2] - direction[2] * DISPLAY_EXTENT) as f32,
    ];
    TruckEntity {
        object: TruckObject::Lines(vec![far_neg, far_pos]),
        snap_pts: vec![],
        tangent_geoms: vec![],
        key_vertices: vec![[origin[0] as f32, origin[1] as f32, origin[2] as f32]],
    }
}

// ── Trait impls (temporary adapters) ────────────────────────────────────

use crate::entities::common::{arr_to_v3, v3_to_arr};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};

#[cfg(feature = "acadrust-compat")]
impl TruckConvertible for acadrust::entities::Ray {
    fn to_truck(&self, _doc: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(ray_to_truck(&v3_to_arr(&self.base_point), &v3_to_arr(&self.direction)))
    }
}

#[cfg(feature = "acadrust-compat")]
impl Grippable for acadrust::entities::Ray {
    fn grips(&self) -> Vec<GripDef> {
        ray_grips(&v3_to_arr(&self.base_point), &v3_to_arr(&self.direction))
    }
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        let mut o = v3_to_arr(&self.base_point);
        let mut d = v3_to_arr(&self.direction);
        ray_apply_grip(&mut o, &mut d, grip_id, apply);
        self.base_point = arr_to_v3(&o);
        self.direction = arr_to_v3(&d);
    }
}

#[cfg(feature = "acadrust-compat")]
impl PropertyEditable for acadrust::entities::Ray {
    fn geometry_properties(&self, _: &[String]) -> PropSection {
        ray_properties(&v3_to_arr(&self.base_point), &v3_to_arr(&self.direction), "ray")
    }
    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        let mut o = v3_to_arr(&self.base_point);
        let mut d = v3_to_arr(&self.direction);
        ray_apply_geom_prop(&mut o, &mut d, field, value);
        self.base_point = arr_to_v3(&o);
        self.direction = arr_to_v3(&d);
    }
}

#[cfg(feature = "acadrust-compat")]
impl Transformable for acadrust::entities::Ray {
    fn apply_transform(&mut self, t: &EntityTransform) {
        let mut o = v3_to_arr(&self.base_point);
        let mut d = v3_to_arr(&self.direction);
        ray_apply_transform(&mut o, &mut d, t);
        self.base_point = arr_to_v3(&o);
        self.direction = arr_to_v3(&d);
    }
}

// ── XLine trait impls ───────────────────────────────────────────────────

#[cfg(feature = "acadrust-compat")]
impl TruckConvertible for acadrust::entities::XLine {
    fn to_truck(&self, _doc: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(xline_to_truck(&v3_to_arr(&self.base_point), &v3_to_arr(&self.direction)))
    }
}

#[cfg(feature = "acadrust-compat")]
impl Grippable for acadrust::entities::XLine {
    fn grips(&self) -> Vec<GripDef> {
        ray_grips(&v3_to_arr(&self.base_point), &v3_to_arr(&self.direction))
    }
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        let mut o = v3_to_arr(&self.base_point);
        let mut d = v3_to_arr(&self.direction);
        ray_apply_grip(&mut o, &mut d, grip_id, apply);
        self.base_point = arr_to_v3(&o);
        self.direction = arr_to_v3(&d);
    }
}

#[cfg(feature = "acadrust-compat")]
impl PropertyEditable for acadrust::entities::XLine {
    fn geometry_properties(&self, _: &[String]) -> PropSection {
        ray_properties(&v3_to_arr(&self.base_point), &v3_to_arr(&self.direction), "xl")
    }
    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        let mut o = v3_to_arr(&self.base_point);
        let mut d = v3_to_arr(&self.direction);
        ray_apply_geom_prop(&mut o, &mut d, field, value);
        self.base_point = arr_to_v3(&o);
        self.direction = arr_to_v3(&d);
    }
}

#[cfg(feature = "acadrust-compat")]
impl Transformable for acadrust::entities::XLine {
    fn apply_transform(&mut self, t: &EntityTransform) {
        let mut o = v3_to_arr(&self.base_point);
        let mut d = v3_to_arr(&self.direction);
        ray_apply_transform(&mut o, &mut d, t);
        self.base_point = arr_to_v3(&o);
        self.direction = arr_to_v3(&d);
    }
}
