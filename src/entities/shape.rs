use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{
    edit_prop as edit, parse_f64, pt_to_vec3, ro_prop as ro, square_grip, transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::SnapHint;

// ── Marker geometry ─────────────────────────────────────────────────────

fn shape_marker(ox: f32, oy: f32, oz: f32, size: f32) -> Vec<[f32; 3]> {
    let s = size * 0.5;
    vec![
        [ox, oy + s, oz],
        [ox + s, oy, oz],
        [ox, oy - s, oz],
        [ox - s, oy, oz],
        [ox, oy + s, oz],
        [f32::NAN; 3],
    ]
}

// ── Free functions ──────────────────────────────────────────────────────

pub fn to_truck(insertion: &[f64; 3], size: f64) -> TruckEntity {
    let ox = insertion[0] as f32;
    let oy = insertion[1] as f32;
    let oz = insertion[2] as f32;
    let sz = (size as f32).abs().max(0.5);
    let snap_pt = Vec3::new(ox, oy, oz);
    let pts = shape_marker(ox, oy, oz, sz);
    TruckEntity {
        object: TruckObject::Lines(pts),
        snap_pts: vec![(snap_pt, SnapHint::Insertion)],
        tangent_geoms: vec![],
        key_vertices: vec![[ox, oy, oz]],
    }
}

pub fn grips(insertion: &[f64; 3]) -> Vec<GripDef> {
    vec![square_grip(0, pt_to_vec3(insertion))]
}

pub fn properties(
    insertion: &[f64; 3],
    size: f64,
    rotation_rad: f64,
    shape_name: &str,
    style_name: &str,
) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            ro("Name", "shp_name", shape_name.to_string()),
            ro("Style", "shp_style", style_name.to_string()),
            edit("Insert X", "shp_ix", insertion[0]),
            edit("Insert Y", "shp_iy", insertion[1]),
            edit("Insert Z", "shp_iz", insertion[2]),
            edit("Size", "shp_sz", size),
            edit("Rotation", "shp_rot", rotation_rad.to_degrees()),
        ],
    }
}

pub fn apply_geom_prop(
    insertion: &mut [f64; 3],
    size: &mut f64,
    rotation: &mut f64,
    field: &str,
    value: &str,
) {
    let Some(v) = parse_f64(value) else { return };
    match field {
        "shp_ix" => insertion[0] = v,
        "shp_iy" => insertion[1] = v,
        "shp_iz" => insertion[2] = v,
        "shp_sz" => *size = v.max(0.001),
        "shp_rot" => *rotation = v.to_radians(),
        _ => {}
    }
}

pub fn apply_grip(insertion: &mut [f64; 3], grip_id: usize, apply: GripApply) {
    if grip_id == 0 {
        match apply {
            GripApply::Translate(d) => {
                insertion[0] += d.x as f64;
                insertion[1] += d.y as f64;
                insertion[2] += d.z as f64;
            }
            GripApply::Absolute(p) => {
                insertion[0] = p.x as f64;
                insertion[1] = p.y as f64;
                insertion[2] = p.z as f64;
            }
        }
    }
}

pub fn apply_transform(insertion: &mut [f64; 3], t: &EntityTransform) {
    transform_pt(insertion, t);
}

