use truck_modeling::{builder, Point3};

use crate::command::EntityTransform;
use crate::entities::common::{
    diamond_grip, distance_3d, edit_prop as edit, parse_f64, pt_to_vec3, ro_prop as ro,
    square_grip, transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::TangentGeom;

// ── Free functions working on native [f64;3] fields ─────────────────────

pub fn to_truck(start: &[f64; 3], end: &[f64; 3]) -> TruckEntity {
    let p0 = Point3::new(start[0], start[1], start[2]);
    let p1 = Point3::new(end[0], end[1], end[2]);
    let v0 = builder::vertex(p0);
    let v1 = builder::vertex(p1);
    let edge = builder::line(&v0, &v1);
    let kv = vec![
        [p0.x as f32, p0.y as f32, p0.z as f32],
        [p1.x as f32, p1.y as f32, p1.z as f32],
    ];
    TruckEntity {
        object: TruckObject::Curve(edge),
        snap_pts: vec![],
        tangent_geoms: vec![TangentGeom::Line {
            p1: kv[0],
            p2: kv[1],
        }],
        key_vertices: kv,
    }
}

pub fn grips(start: &[f64; 3], end: &[f64; 3]) -> Vec<GripDef> {
    let s = pt_to_vec3(start);
    let e = pt_to_vec3(end);
    let m = (s + e) * 0.5;
    vec![square_grip(0, s), square_grip(1, e), diamond_grip(2, m)]
}

pub fn properties(start: &[f64; 3], end: &[f64; 3]) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Start X", "start_x", start[0]),
            edit("Start Y", "start_y", start[1]),
            edit("Start Z", "start_z", start[2]),
            edit("End X", "end_x", end[0]),
            edit("End Y", "end_y", end[1]),
            edit("End Z", "end_z", end[2]),
            ro("Length", "length", format!("{:.4}", distance_3d(start, end))),
        ],
    }
}

pub fn apply_geom_prop(start: &mut [f64; 3], end: &mut [f64; 3], field: &str, value: &str) {
    let Some(v) = parse_f64(value) else { return };
    match field {
        "start_x" => start[0] = v,
        "start_y" => start[1] = v,
        "start_z" => start[2] = v,
        "end_x" => end[0] = v,
        "end_y" => end[1] = v,
        "end_z" => end[2] = v,
        _ => {}
    }
}

pub fn apply_grip(start: &mut [f64; 3], end: &mut [f64; 3], grip_id: usize, apply: GripApply) {
    match (grip_id, apply) {
        (0, GripApply::Absolute(p)) => {
            start[0] = p.x as f64;
            start[1] = p.y as f64;
            start[2] = p.z as f64;
        }
        (1, GripApply::Absolute(p)) => {
            end[0] = p.x as f64;
            end[1] = p.y as f64;
            end[2] = p.z as f64;
        }
        (2, GripApply::Translate(d)) => {
            start[0] += d.x as f64;
            start[1] += d.y as f64;
            start[2] += d.z as f64;
            end[0] += d.x as f64;
            end[1] += d.y as f64;
            end[2] += d.z as f64;
        }
        _ => {}
    }
}

pub fn apply_transform(start: &mut [f64; 3], end: &mut [f64; 3], t: &EntityTransform) {
    transform_pt(start, t);
    transform_pt(end, t);
}

// ── Trait impls (temporary adapters for acadrust EntityType dispatch) ────

use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};

#[cfg(feature = "acadrust-compat")]
impl TruckConvertible for acadrust::entities::Line {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        let s = [self.start.x, self.start.y, self.start.z];
        let e = [self.end.x, self.end.y, self.end.z];
        Some(self::to_truck(&s, &e))
    }
}

#[cfg(feature = "acadrust-compat")]
impl Grippable for acadrust::entities::Line {
    fn grips(&self) -> Vec<GripDef> {
        let s = [self.start.x, self.start.y, self.start.z];
        let e = [self.end.x, self.end.y, self.end.z];
        self::grips(&s, &e)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        let mut s = [self.start.x, self.start.y, self.start.z];
        let mut e = [self.end.x, self.end.y, self.end.z];
        self::apply_grip(&mut s, &mut e, grip_id, apply);
        self.start.x = s[0]; self.start.y = s[1]; self.start.z = s[2];
        self.end.x = e[0]; self.end.y = e[1]; self.end.z = e[2];
    }
}

#[cfg(feature = "acadrust-compat")]
impl PropertyEditable for acadrust::entities::Line {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        let s = [self.start.x, self.start.y, self.start.z];
        let e = [self.end.x, self.end.y, self.end.z];
        properties(&s, &e)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        let mut s = [self.start.x, self.start.y, self.start.z];
        let mut e = [self.end.x, self.end.y, self.end.z];
        self::apply_geom_prop(&mut s, &mut e, field, value);
        self.start.x = s[0]; self.start.y = s[1]; self.start.z = s[2];
        self.end.x = e[0]; self.end.y = e[1]; self.end.z = e[2];
    }
}

#[cfg(feature = "acadrust-compat")]
impl Transformable for acadrust::entities::Line {
    fn apply_transform(&mut self, t: &EntityTransform) {
        let mut s = [self.start.x, self.start.y, self.start.z];
        let mut e = [self.end.x, self.end.y, self.end.z];
        self::apply_transform(&mut s, &mut e, t);
        self.start.x = s[0]; self.start.y = s[1]; self.start.z = s[2];
        self.end.x = e[0]; self.end.y = e[1]; self.end.z = e[2];
    }
}
