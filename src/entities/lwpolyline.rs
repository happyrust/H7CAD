use glam::Vec3;
use truck_modeling::{builder, Edge, Point3, Wire};

use crate::command::EntityTransform;
use crate::entities::common::{
    edit_prop as edit, parse_f64, ro_prop as ro, square_grip, transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::TangentGeom;

const TAU: f64 = std::f64::consts::TAU;

/// Lightweight vertex: (x, y, bulge)
pub type NmLwVertex = h7cad_native_model::LwVertex;

// ── Free functions ──────────────────────────────────────────────────────

pub fn to_truck(vertices: &[NmLwVertex], closed: bool, elevation: f64) -> TruckEntity {
    if vertices.is_empty() {
        let v = builder::vertex(Point3::new(0.0, 0.0, 0.0));
        let edge = builder::line(&v, &v);
        return TruckEntity {
            object: TruckObject::Contour(std::iter::once(edge).collect()),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: vec![],
        };
    }

    let count = vertices.len();
    let seg_count = if closed { count } else { count - 1 };
    let mut edges: Vec<Edge> = Vec::new();
    let mut tangents: Vec<TangentGeom> = Vec::new();
    let mut key_verts: Vec<[f32; 3]> = Vec::new();

    let to_pt = |v: &NmLwVertex| -> Point3 { Point3::new(v.x, v.y, elevation) };

    for i in 0..seg_count {
        let v0 = &vertices[i];
        let v1 = &vertices[(i + 1) % count];
        let p0 = to_pt(v0);
        let p1 = to_pt(v1);
        let bulge = v0.bulge;

        if bulge.abs() < 1e-9 {
            let tv0 = builder::vertex(p0);
            let tv1 = builder::vertex(p1);
            edges.push(builder::line(&tv0, &tv1));
            tangents.push(TangentGeom::Line {
                p1: [p0.x as f32, p0.y as f32, p0.z as f32],
                p2: [p1.x as f32, p1.y as f32, p1.z as f32],
            });
        } else {
            let angle = 4.0 * bulge.atan();
            let dx = p1.x - p0.x;
            let dy = p1.y - p0.y;
            let d = (dx * dx + dy * dy).sqrt();
            let r = (d / 2.0) / (angle / 2.0).sin().abs();
            let mx = (p0.x + p1.x) * 0.5;
            let my = (p0.y + p1.y) * 0.5;
            let len = d.max(1e-12);
            let px = -dy / len;
            let py = dx / len;
            let sagitta_sign = if bulge > 0.0 { 1.0_f64 } else { -1.0_f64 };
            let h = r - (r * r - d * d / 4.0).max(0.0).sqrt();
            let cx = mx - sagitta_sign * px * (r - h);
            let cy = my - sagitta_sign * py * (r - h);
            let mid_a = {
                let a0 = (p0.y - cy).atan2(p0.x - cx);
                let a1 = (p1.y - cy).atan2(p1.x - cx);
                let (sa, mut ea) = if bulge > 0.0 { (a0, a1) } else { (a1, a0) };
                if ea < sa {
                    ea += TAU;
                }
                sa + (ea - sa) * 0.5
            };
            let p_mid = Point3::new(cx + r * mid_a.cos(), cy + r * mid_a.sin(), p0.z);
            let tv0 = builder::vertex(p0);
            let tv1 = builder::vertex(p1);
            edges.push(builder::circle_arc(&tv0, &tv1, p_mid));
            tangents.push(TangentGeom::Circle {
                center: [cx as f32, cy as f32, p0.z as f32],
                radius: r as f32,
            });
        }

        if i == 0 {
            key_verts.push([p0.x as f32, p0.y as f32, p0.z as f32]);
        }
        key_verts.push([p1.x as f32, p1.y as f32, p1.z as f32]);
    }

    TruckEntity {
        object: TruckObject::Contour(edges.into_iter().collect::<Wire>()),
        snap_pts: vec![],
        tangent_geoms: tangents,
        key_vertices: key_verts,
    }
}

pub fn grips(vertices: &[NmLwVertex], elevation: f64) -> Vec<GripDef> {
    let elev = elevation as f32;
    vertices
        .iter()
        .enumerate()
        .map(|(i, v)| square_grip(i, Vec3::new(v.x as f32, v.y as f32, elev)))
        .collect()
}

pub fn properties(vertices: &[NmLwVertex], closed: bool, elevation: f64) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            ro("Vertices", "vertices", vertices.len().to_string()),
            ro("Closed", "closed", if closed { "Yes" } else { "No" }),
            edit("Elevation", "elevation", elevation),
        ],
    }
}

pub fn apply_geom_prop(elevation: &mut f64, field: &str, value: &str) {
    if field == "elevation" {
        if let Some(v) = parse_f64(value) {
            *elevation = v;
        }
    }
}

pub fn apply_grip(vertices: &mut [NmLwVertex], grip_id: usize, apply: GripApply) {
    if let Some(v) = vertices.get_mut(grip_id) {
        match apply {
            GripApply::Absolute(p) => {
                v.x = p.x as f64;
                v.y = p.y as f64;
            }
            GripApply::Translate(d) => {
                v.x += d.x as f64;
                v.y += d.y as f64;
            }
        }
    }
}

pub fn apply_transform(vertices: &mut [NmLwVertex], t: &EntityTransform) {
    for v in vertices.iter_mut() {
        let mut pt = [v.x, v.y, 0.0];
        transform_pt(&mut pt, t);
        v.x = pt[0];
        v.y = pt[1];
    }
}

