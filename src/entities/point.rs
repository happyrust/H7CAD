use truck_modeling::{builder, Point3};

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, parse_f64, pt_to_vec3, square_grip, transform_pt};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::SnapHint;

// ── Free functions ──────────────────────────────────────────────────────

pub fn to_truck(position: &[f64; 3]) -> TruckEntity {
    let p = Point3::new(position[0], position[1], position[2]);
    TruckEntity {
        object: TruckObject::Point(builder::vertex(p)),
        snap_pts: vec![(pt_to_vec3(position), SnapHint::Node)],
        tangent_geoms: vec![],
        key_vertices: vec![],
    }
}

pub fn grips(position: &[f64; 3]) -> Vec<GripDef> {
    vec![square_grip(0, pt_to_vec3(position))]
}

pub fn properties(position: &[f64; 3]) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("X", "loc_x", position[0]),
            edit("Y", "loc_y", position[1]),
            edit("Z", "loc_z", position[2]),
        ],
    }
}

pub fn apply_geom_prop(position: &mut [f64; 3], field: &str, value: &str) {
    let Some(v) = parse_f64(value) else { return };
    match field {
        "loc_x" => position[0] = v,
        "loc_y" => position[1] = v,
        "loc_z" => position[2] = v,
        _ => {}
    }
}

pub fn apply_grip(position: &mut [f64; 3], _grip_id: usize, apply: GripApply) {
    match apply {
        GripApply::Absolute(p) => {
            position[0] = p.x as f64;
            position[1] = p.y as f64;
            position[2] = p.z as f64;
        }
        GripApply::Translate(d) => {
            position[0] += d.x as f64;
            position[1] += d.y as f64;
            position[2] += d.z as f64;
        }
    }
}

pub fn apply_transform(position: &mut [f64; 3], t: &EntityTransform) {
    transform_pt(position, t);
}

