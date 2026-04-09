use glam::Vec3;
use truck_modeling::{builder, Point3, Wire};

use crate::command::EntityTransform;
use crate::entities::common::{
    diamond_grip, edit_prop as edit, parse_f64, pt_to_vec3, ro_prop as ro, scale_pt, square_grip,
    transform_pt,
};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::{SnapHint, TangentGeom};

// ── Free functions working on native fields ─────────────────────────────

pub fn to_truck(center: &[f64; 3], radius: f64) -> TruckEntity {
    let (cx, cy, cz, r) = (center[0], center[1], center[2], radius);
    let right = builder::vertex(Point3::new(cx + r, cy, cz));
    let left = builder::vertex(Point3::new(cx - r, cy, cz));
    let top = Point3::new(cx, cy + r, cz);
    let bot = Point3::new(cx, cy - r, cz);
    let upper = builder::circle_arc(&right, &left, top);
    let lower = builder::circle_arc(&left, &right, bot);
    let wire: Wire = [upper, lower].into_iter().collect();
    let cv = pt_to_vec3(center);
    let rf = r as f32;
    TruckEntity {
        object: TruckObject::Contour(wire),
        snap_pts: vec![
            (cv, SnapHint::Center),
            (cv + Vec3::new(rf, 0.0, 0.0), SnapHint::Quadrant),
            (cv + Vec3::new(0.0, rf, 0.0), SnapHint::Quadrant),
            (cv - Vec3::new(rf, 0.0, 0.0), SnapHint::Quadrant),
            (cv - Vec3::new(0.0, rf, 0.0), SnapHint::Quadrant),
        ],
        tangent_geoms: vec![TangentGeom::Circle {
            center: [cx as f32, cy as f32, cz as f32],
            radius: rf,
        }],
        key_vertices: vec![],
    }
}

pub fn grips(center: &[f64; 3], radius: f64) -> Vec<GripDef> {
    let ctr = pt_to_vec3(center);
    let r = radius as f32;
    vec![
        diamond_grip(0, ctr),
        square_grip(1, ctr + Vec3::new(r, 0.0, 0.0)),
        square_grip(2, ctr + Vec3::new(0.0, r, 0.0)),
        square_grip(3, ctr - Vec3::new(r, 0.0, 0.0)),
        square_grip(4, ctr - Vec3::new(0.0, r, 0.0)),
    ]
}

pub fn properties(center: &[f64; 3], radius: f64) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Center X", "center_x", center[0]),
            edit("Center Y", "center_y", center[1]),
            edit("Center Z", "center_z", center[2]),
            edit("Radius", "radius", radius),
            ro("Diameter", "diameter", format!("{:.4}", radius * 2.0)),
            ro(
                "Circumference",
                "circumference",
                format!("{:.4}", radius * 2.0 * std::f64::consts::PI),
            ),
        ],
    }
}

pub fn apply_geom_prop(center: &mut [f64; 3], radius: &mut f64, field: &str, value: &str) {
    let Some(v) = parse_f64(value) else { return };
    match field {
        "center_x" => center[0] = v,
        "center_y" => center[1] = v,
        "center_z" => center[2] = v,
        "radius" if v > 0.0 => *radius = v,
        _ => {}
    }
}

pub fn apply_grip(
    center: &mut [f64; 3],
    radius: &mut f64,
    grip_id: usize,
    apply: GripApply,
) {
    match (grip_id, apply) {
        (0, GripApply::Absolute(p)) => {
            center[0] = p.x as f64;
            center[1] = p.y as f64;
            center[2] = p.z as f64;
        }
        (0, GripApply::Translate(d)) => {
            center[0] += d.x as f64;
            center[1] += d.y as f64;
            center[2] += d.z as f64;
        }
        (1..=4, GripApply::Absolute(p)) => {
            let dx = p.x - center[0] as f32;
            let dy = p.y - center[1] as f32;
            *radius = ((dx * dx + dy * dy) as f64).sqrt();
        }
        _ => {}
    }
}

pub fn apply_transform(center: &mut [f64; 3], radius: &mut f64, t: &EntityTransform) {
    transform_pt(center, t);
    if let EntityTransform::Scale { center: c, factor } = t {
        let mut r_pt = [center[0] + *radius, center[1], center[2]];
        scale_pt(&mut r_pt, *c, *factor);
        *radius = ((r_pt[0] - center[0]).powi(2) + (r_pt[1] - center[1]).powi(2)).sqrt();
    }
}

// ── Trait impls (temporary adapters) ────────────────────────────────────

use crate::entities::common::v3_to_arr;
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};

impl TruckConvertible for acadrust::entities::Circle {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(self::to_truck(&v3_to_arr(&self.center), self.radius))
    }
}

impl Grippable for acadrust::entities::Circle {
    fn grips(&self) -> Vec<GripDef> {
        self::grips(&v3_to_arr(&self.center), self.radius)
    }
    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        let mut c = v3_to_arr(&self.center);
        let mut r = self.radius;
        self::apply_grip(&mut c, &mut r, grip_id, apply);
        self.center = crate::entities::common::arr_to_v3(&c);
        self.radius = r;
    }
}

impl PropertyEditable for acadrust::entities::Circle {
    fn geometry_properties(&self, _: &[String]) -> PropSection {
        properties(&v3_to_arr(&self.center), self.radius)
    }
    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        let mut c = v3_to_arr(&self.center);
        let mut r = self.radius;
        self::apply_geom_prop(&mut c, &mut r, field, value);
        self.center = crate::entities::common::arr_to_v3(&c);
        self.radius = r;
    }
}

impl Transformable for acadrust::entities::Circle {
    fn apply_transform(&mut self, t: &EntityTransform) {
        let mut c = v3_to_arr(&self.center);
        let mut r = self.radius;
        self::apply_transform(&mut c, &mut r, t);
        self.center = crate::entities::common::arr_to_v3(&c);
        self.radius = r;
    }
}
