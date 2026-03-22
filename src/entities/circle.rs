use acadrust::entities::Circle;
use glam::Vec3;
use truck_modeling::{builder, Point3, Wire};

use crate::command::EntityTransform;
use crate::entities::common::{
    diamond_grip, edit_prop as edit, parse_f64, ro_prop as ro, square_grip,
};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::{SnapHint, TangentGeom};

fn to_truck(circle: &Circle) -> TruckEntity {
    let cx = circle.center.x;
    let cy = circle.center.y;
    let cz = circle.center.z;
    let r = circle.radius;
    let right = builder::vertex(Point3::new(cx + r, cy, cz));
    let left = builder::vertex(Point3::new(cx - r, cy, cz));
    let top = Point3::new(cx, cy + r, cz);
    let bot = Point3::new(cx, cy - r, cz);
    let upper = builder::circle_arc(&right, &left, top);
    let lower = builder::circle_arc(&left, &right, bot);
    let wire: Wire = [upper, lower].into_iter().collect();
    let cv = Vec3::new(cx as f32, cy as f32, cz as f32);
    let rf = r as f32;
    TruckEntity {
        object: TruckObject::Contour(wire),
        snap_pts: vec![
            (cv, SnapHint::Center),
            (
                Vec3::new(cx as f32 + rf, cy as f32, cz as f32),
                SnapHint::Quadrant,
            ),
            (
                Vec3::new(cx as f32, cy as f32 + rf, cz as f32),
                SnapHint::Quadrant,
            ),
            (
                Vec3::new(cx as f32 - rf, cy as f32, cz as f32),
                SnapHint::Quadrant,
            ),
            (
                Vec3::new(cx as f32, cy as f32 - rf, cz as f32),
                SnapHint::Quadrant,
            ),
        ],
        tangent_geoms: vec![TangentGeom::Circle {
            center: [cx as f32, cy as f32, cz as f32],
            radius: rf,
        }],
        key_vertices: vec![],
    }
}

fn grips(circle: &Circle) -> Vec<GripDef> {
    let ctr = Vec3::new(
        circle.center.x as f32,
        circle.center.y as f32,
        circle.center.z as f32,
    );
    let r = circle.radius as f32;
    vec![
        diamond_grip(0, ctr),
        square_grip(1, ctr + Vec3::new(r, 0.0, 0.0)),
        square_grip(2, ctr + Vec3::new(0.0, r, 0.0)),
        square_grip(3, ctr - Vec3::new(r, 0.0, 0.0)),
        square_grip(4, ctr - Vec3::new(0.0, r, 0.0)),
    ]
}

fn properties(circle: &Circle) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Center X", "center_x", circle.center.x),
            edit("Center Y", "center_y", circle.center.y),
            edit("Center Z", "center_z", circle.center.z),
            edit("Radius", "radius", circle.radius),
            ro(
                "Diameter",
                "diameter",
                format!("{:.4}", circle.radius * 2.0),
            ),
            ro(
                "Circumference",
                "circumference",
                format!("{:.4}", circle.radius * 2.0 * std::f64::consts::PI),
            ),
        ],
    }
}

fn apply_geom_prop(circle: &mut Circle, field: &str, value: &str) {
    let Some(v) = parse_f64(value) else {
        return;
    };
    match field {
        "center_x" => circle.center.x = v,
        "center_y" => circle.center.y = v,
        "center_z" => circle.center.z = v,
        "radius" if v > 0.0 => circle.radius = v,
        _ => {}
    }
}

fn apply_grip(circle: &mut Circle, grip_id: usize, apply: GripApply) {
    match (grip_id, apply) {
        (0, GripApply::Absolute(p)) => {
            circle.center.x = p.x as f64;
            circle.center.y = p.y as f64;
            circle.center.z = p.z as f64;
        }
        (0, GripApply::Translate(d)) => {
            circle.center.x += d.x as f64;
            circle.center.y += d.y as f64;
            circle.center.z += d.z as f64;
        }
        (1..=4, GripApply::Absolute(p)) => {
            let cx = circle.center.x as f32;
            let cy = circle.center.y as f32;
            let dx = p.x - cx;
            let dy = p.y - cy;
            circle.radius = ((dx * dx + dy * dy) as f64).sqrt();
        }
        _ => {}
    }
}

fn apply_transform(circle: &mut Circle, t: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(circle, t, |entity, p1, p2| {
        crate::scene::transform::reflect_xy_point(
            &mut entity.center.x,
            &mut entity.center.y,
            p1,
            p2,
        );
    });
}

impl TruckConvertible for Circle {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        Some(to_truck(self))
    }
}

impl Grippable for Circle {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for Circle {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for Circle {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
