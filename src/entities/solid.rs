use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, parse_f64, pt_to_vec3, square_grip, transform_pt};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::SnapHint;

// ── Free functions (corners: [[f64;3]; 4]) ──────────────────────────────

pub fn to_truck(corners: &[[f64; 3]; 4]) -> TruckEntity {
    let p: Vec<[f32; 3]> = corners
        .iter()
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect();
    let pts = vec![
        p[0], p[1], [f32::NAN; 3],
        p[1], p[3], [f32::NAN; 3],
        p[3], p[2], [f32::NAN; 3],
        p[2], p[0],
    ];
    let snap = corners
        .iter()
        .map(|c| (pt_to_vec3(c), SnapHint::Node))
        .collect();
    Some(TruckEntity {
        object: TruckObject::Lines(pts),
        snap_pts: snap,
        tangent_geoms: vec![],
        key_vertices: p,
    })
    .unwrap()
}

pub fn grips(corners: &[[f64; 3]; 4]) -> Vec<GripDef> {
    corners
        .iter()
        .enumerate()
        .map(|(i, c)| square_grip(i, pt_to_vec3(c)))
        .collect()
}

pub fn properties(corners: &[[f64; 3]; 4], thickness: f64) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("P1 X", "sl_p1x", corners[0][0]),
            edit("P1 Y", "sl_p1y", corners[0][1]),
            edit("P1 Z", "sl_p1z", corners[0][2]),
            edit("P2 X", "sl_p2x", corners[1][0]),
            edit("P2 Y", "sl_p2y", corners[1][1]),
            edit("P2 Z", "sl_p2z", corners[1][2]),
            edit("P3 X", "sl_p3x", corners[2][0]),
            edit("P3 Y", "sl_p3y", corners[2][1]),
            edit("P3 Z", "sl_p3z", corners[2][2]),
            edit("P4 X", "sl_p4x", corners[3][0]),
            edit("P4 Y", "sl_p4y", corners[3][1]),
            edit("P4 Z", "sl_p4z", corners[3][2]),
            edit("Thickness", "sl_thick", thickness),
        ],
    }
}

pub fn apply_geom_prop(corners: &mut [[f64; 3]; 4], thickness: &mut f64, field: &str, value: &str) {
    let Some(v) = parse_f64(value) else { return };
    match field {
        "sl_p1x" => corners[0][0] = v,
        "sl_p1y" => corners[0][1] = v,
        "sl_p1z" => corners[0][2] = v,
        "sl_p2x" => corners[1][0] = v,
        "sl_p2y" => corners[1][1] = v,
        "sl_p2z" => corners[1][2] = v,
        "sl_p3x" => corners[2][0] = v,
        "sl_p3y" => corners[2][1] = v,
        "sl_p3z" => corners[2][2] = v,
        "sl_p4x" => corners[3][0] = v,
        "sl_p4y" => corners[3][1] = v,
        "sl_p4z" => corners[3][2] = v,
        "sl_thick" => *thickness = v,
        _ => {}
    }
}

pub fn apply_grip(corners: &mut [[f64; 3]; 4], grip_id: usize, apply: GripApply) {
    let Some(corner) = corners.get_mut(grip_id) else { return };
    match apply {
        GripApply::Translate(d) => {
            corner[0] += d.x as f64;
            corner[1] += d.y as f64;
            corner[2] += d.z as f64;
        }
        GripApply::Absolute(p) => {
            corner[0] = p.x as f64;
            corner[1] = p.y as f64;
            corner[2] = p.z as f64;
        }
    }
}

pub fn apply_transform(corners: &mut [[f64; 3]; 4], t: &EntityTransform) {
    for c in corners.iter_mut() {
        transform_pt(c, t);
    }
}

