use glam::Vec3;
use h7cad_native_model::geom_ocs::{arbitrary_axis, ocs_to_wcs};

use crate::command::EntityTransform;
use crate::entities::common::{
    edit_prop as edit, parse_f64, pt_to_vec3, ro_prop as ro, square_grip, transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::SnapHint;

// ── Marker geometry ─────────────────────────────────────────────────────

/// Build a diamond-shaped marker centred at `o` with half-extent
/// `size/2` along the local OCS axes `ax` and `ay`.
fn shape_marker_ocs(
    o: [f32; 3],
    ax: [f32; 3],
    ay: [f32; 3],
    size: f32,
) -> Vec<[f32; 3]> {
    let s = size * 0.5;
    let pt = |dx: f32, dy: f32| -> [f32; 3] {
        [
            o[0] + dx * ax[0] + dy * ay[0],
            o[1] + dx * ax[1] + dy * ay[1],
            o[2] + dx * ax[2] + dy * ay[2],
        ]
    };
    vec![
        pt(0.0, s),
        pt(s, 0.0),
        pt(0.0, -s),
        pt(-s, 0.0),
        pt(0.0, s),
        [f32::NAN; 3],
    ]
}

// ── Free functions ──────────────────────────────────────────────────────

/// Tessellate a SHAPE marker assuming OCS == WCS. Legacy path.
#[allow(dead_code)]
pub fn to_truck(insertion: &[f64; 3], size: f64) -> TruckEntity {
    to_truck_with_normal(insertion, size, [0.0, 0.0, 1.0])
}

/// Tessellate a SHAPE marker with the DXF arbitrary-axis algorithm.
///
/// `insertion` is the OCS insertion point; the diamond marker is
/// drawn in the OCS plane defined by `normal`.
pub fn to_truck_with_normal(
    insertion: &[f64; 3],
    size: f64,
    normal: [f64; 3],
) -> TruckEntity {
    let (ax, ay, _n) = arbitrary_axis(normal);
    let wcs_insertion = ocs_to_wcs(*insertion, [0.0, 0.0, 0.0], normal);
    let ox = wcs_insertion[0] as f32;
    let oy = wcs_insertion[1] as f32;
    let oz = wcs_insertion[2] as f32;
    let sz = (size as f32).abs().max(0.5);
    let snap_pt = Vec3::new(ox, oy, oz);
    let pts = shape_marker_ocs(
        [ox, oy, oz],
        [ax[0] as f32, ax[1] as f32, ax[2] as f32],
        [ay[0] as f32, ay[1] as f32, ay[2] as f32],
        sz,
    );
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

