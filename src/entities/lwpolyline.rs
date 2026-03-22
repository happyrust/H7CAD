use acadrust::entities::{LwPolyline, LwVertex};
use glam::Vec3;
use truck_modeling::{builder, Edge, Point3, Wire};

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, parse_f64, ro_prop as ro, square_grip};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::TangentGeom;

const TAU: f64 = std::f64::consts::TAU;

fn to_truck(pline: &LwPolyline) -> TruckEntity {
    let verts = &pline.vertices;
    if verts.is_empty() {
        let v = builder::vertex(Point3::new(0.0, 0.0, 0.0));
        let edge = builder::line(&v, &v);
        return TruckEntity {
            object: TruckObject::Contour(std::iter::once(edge).collect()),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: vec![],
        };
    }

    let elev = pline.elevation;
    let count = verts.len();
    let seg_count = if pline.is_closed { count } else { count - 1 };
    let mut edges: Vec<Edge> = Vec::new();
    let mut tangents: Vec<TangentGeom> = Vec::new();
    let mut key_verts: Vec<[f32; 3]> = Vec::new();

    let to_pt = |v: &LwVertex| -> Point3 { Point3::new(v.location.x, v.location.y, elev) };

    for i in 0..seg_count {
        let v0 = &verts[i];
        let v1 = &verts[(i + 1) % count];
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
            let angle = 4.0 * (bulge as f64).atan();
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

fn grips(pline: &LwPolyline) -> Vec<GripDef> {
    let elev = pline.elevation as f32;
    pline
        .vertices
        .iter()
        .enumerate()
        .map(|(i, v)| square_grip(i, Vec3::new(v.location.x as f32, v.location.y as f32, elev)))
        .collect()
}

fn properties(pline: &LwPolyline) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            ro("Vertices", "vertices", pline.vertices.len().to_string()),
            ro(
                "Closed",
                "closed",
                if pline.is_closed { "Yes" } else { "No" },
            ),
            edit("Elevation", "elevation", pline.elevation),
        ],
    }
}

fn apply_geom_prop(pline: &mut LwPolyline, field: &str, value: &str) {
    let Some(v) = parse_f64(value) else {
        return;
    };
    if field == "elevation" {
        pline.elevation = v;
    }
}

fn apply_grip(pline: &mut LwPolyline, grip_id: usize, apply: GripApply) {
    if let Some(v) = pline.vertices.get_mut(grip_id) {
        match apply {
            GripApply::Absolute(p) => {
                v.location.x = p.x as f64;
                v.location.y = p.y as f64;
            }
            GripApply::Translate(d) => {
                v.location.x += d.x as f64;
                v.location.y += d.y as f64;
            }
        }
    }
}

fn apply_transform(pline: &mut LwPolyline, t: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(pline, t, |entity, p1, p2| {
        for v in &mut entity.vertices {
            crate::scene::transform::reflect_xy_point(&mut v.location.x, &mut v.location.y, p1, p2);
        }
    });
}

impl TruckConvertible for LwPolyline {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(to_truck(self))
    }
}

impl Grippable for LwPolyline {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for LwPolyline {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for LwPolyline {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
