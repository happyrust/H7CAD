use acadrust::entities::Insert;
use crate::types::{Transform, Vector3};
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, parse_f64, ro_prop as ro, square_grip};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable};
use crate::scene::object::{GripApply, GripDef, PropSection};

fn grips(ins: &Insert) -> Vec<GripDef> {
    let p = Vec3::new(
        ins.insert_point.x as f32,
        ins.insert_point.y as f32,
        ins.insert_point.z as f32,
    );
    vec![square_grip(0, p)]
}

fn properties(ins: &Insert) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Insert X", "ins_x", ins.insert_point.x),
            edit("Insert Y", "ins_y", ins.insert_point.y),
            edit("Insert Z", "ins_z", ins.insert_point.z),
            edit("Scale X", "x_scale", ins.x_scale()),
            edit("Scale Y", "y_scale", ins.y_scale()),
            edit("Scale Z", "z_scale", ins.z_scale()),
            edit("Rotation", "rotation", ins.rotation.to_degrees()),
            ro("Block", "block", ins.block_name.clone()),
        ],
    }
}

fn apply_geom_prop(ins: &mut Insert, field: &str, value: &str) {
    let Some(v) = parse_f64(value) else {
        return;
    };
    match field {
        "ins_x" => ins.insert_point.x = v,
        "ins_y" => ins.insert_point.y = v,
        "ins_z" => ins.insert_point.z = v,
        "x_scale" => ins.set_x_scale(v),
        "y_scale" => ins.set_y_scale(v),
        "z_scale" => ins.set_z_scale(v),
        "rotation" => ins.rotation = v.to_radians(),
        _ => {}
    }
}

fn apply_grip(ins: &mut Insert, _grip_id: usize, apply: GripApply) {
    match apply {
        GripApply::Absolute(p) => {
            ins.insert_point.x = p.x as f64;
            ins.insert_point.y = p.y as f64;
            ins.insert_point.z = p.z as f64;
        }
        GripApply::Translate(d) => {
            ins.insert_point.x += d.x as f64;
            ins.insert_point.y += d.y as f64;
            ins.insert_point.z += d.z as f64;
        }
    }
}

fn apply_transform(ins: &mut Insert, t: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(ins, t, |entity, p1, p2| {
        let dx = (p2.x - p1.x) as f64;
        let dy = (p2.y - p1.y) as f64;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-12 {
            return;
        }

        let ux = dx / len;
        let uy = dy / len;
        let mirror = crate::types::Matrix4 {
            m: [
                [2.0 * ux * ux - 1.0, 2.0 * ux * uy, 0.0, 0.0],
                [2.0 * ux * uy, 2.0 * uy * uy - 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let t = Transform::from_translation(Vector3::new(-(p1.x as f64), -(p1.y as f64), 0.0))
            .then(&Transform::from_matrix(mirror))
            .then(&Transform::from_translation(Vector3::new(p1.x as f64, p1.y as f64, 0.0)));
        acadrust::Entity::apply_transform(entity, &t);
    });
}

impl Grippable for Insert {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for Insert {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for Insert {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
