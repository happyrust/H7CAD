use acadrust::entities::Viewport;
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{diamond_grip, edit_prop as edit, parse_f64, square_grip};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable};
use crate::scene::object::{GripApply, GripDef, PropSection};

fn grips(vp: &Viewport) -> Vec<GripDef> {
    let cx = vp.center.x as f32;
    let cy = vp.center.y as f32;
    let cz = vp.center.z as f32;
    let hw = (vp.width / 2.0) as f32;
    let hh = (vp.height / 2.0) as f32;
    vec![
        diamond_grip(0, Vec3::new(cx, cy, cz)),
        square_grip(1, Vec3::new(cx + hw, cy + hh, cz)),
        square_grip(2, Vec3::new(cx - hw, cy + hh, cz)),
        square_grip(3, Vec3::new(cx - hw, cy - hh, cz)),
        square_grip(4, Vec3::new(cx + hw, cy - hh, cz)),
    ]
}

fn properties(vp: &Viewport) -> PropSection {
    PropSection {
        title: "Geometry".into(),
        props: vec![
            edit("Center X", "center_x", vp.center.x),
            edit("Center Y", "center_y", vp.center.y),
            edit("Center Z", "center_z", vp.center.z),
            edit("Width", "vp_w", vp.width),
            edit("Height", "vp_h", vp.height),
            edit("Scale", "vscale", vp.custom_scale),
            edit("Target X", "vtgt_x", vp.view_target.x),
            edit("Target Z", "vtgt_z", vp.view_target.z),
        ],
    }
}

fn apply_geom_prop(vp: &mut Viewport, field: &str, value: &str) {
    let Some(v) = parse_f64(value) else {
        return;
    };
    match field {
        "center_x" => vp.center.x = v,
        "center_y" => vp.center.y = v,
        "center_z" => vp.center.z = v,
        "vp_w" if v > 0.0 => vp.width = v,
        "vp_h" if v > 0.0 => vp.height = v,
        "vscale" if v > 0.0 => vp.custom_scale = v,
        "vtgt_x" => vp.view_target.x = v,
        "vtgt_z" => vp.view_target.z = v,
        _ => {}
    }
}

fn apply_grip(vp: &mut Viewport, grip_id: usize, apply: GripApply) {
    match (grip_id, apply) {
        (0, GripApply::Translate(d)) => {
            vp.center.x += d.x as f64;
            vp.center.y += d.y as f64;
            vp.center.z += d.z as f64;
        }
        (0, GripApply::Absolute(p)) => {
            vp.center.x = p.x as f64;
            vp.center.y = p.y as f64;
            vp.center.z = p.z as f64;
        }
        (1..=4, GripApply::Absolute(p)) => {
            let new_hw = (p.x as f64 - vp.center.x).abs();
            let new_hh = (p.y as f64 - vp.center.y).abs();
            if new_hw > 0.01 {
                vp.width = new_hw * 2.0;
            }
            if new_hh > 0.01 {
                vp.height = new_hh * 2.0;
            }
        }
        _ => {}
    }
}

fn apply_transform(vp: &mut Viewport, t: &EntityTransform) {
    crate::scene::transform::apply_standard_entity_transform(vp, t, |entity, p1, p2| {
        crate::scene::transform::reflect_xy_point(
            &mut entity.center.x,
            &mut entity.center.y,
            p1,
            p2,
        );
    });
}

impl Grippable for Viewport {
    fn grips(&self) -> Vec<GripDef> {
        grips(self)
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        apply_grip(self, grip_id, apply);
    }
}

impl PropertyEditable for Viewport {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        properties(self)
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        apply_geom_prop(self, field, value);
    }
}

impl Transformable for Viewport {
    fn apply_transform(&mut self, t: &EntityTransform) {
        apply_transform(self, t);
    }
}
