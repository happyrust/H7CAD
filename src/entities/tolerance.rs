use acadrust::entities::Tolerance;
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{edit_prop as edit, ro_prop as ro, square_grip};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection};
use crate::scene::wire_model::SnapHint;
use crate::scene::{cxf, transform};

impl TruckConvertible for Tolerance {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        if self.text.is_empty() {
            return None;
        }
        // GDT tolerance text — render with the standard font.
        // Special GDT symbols (Ⓗ, ⊕ etc.) will display as-is if the font
        // supports them, otherwise as placeholder glyphs.
        let snap_pt = Vec3::new(
            self.insertion_point.x as f32,
            self.insertion_point.y as f32,
            self.insertion_point.z as f32,
        );

        // Determine rotation from the direction vector.
        let angle = (self.direction.y as f32).atan2(self.direction.x as f32);
        // Use a default height of 2.5 (standard annotation size).
        let height = 2.5_f32;

        let strokes = cxf::tessellate_text_ex(
            [self.insertion_point.x as f32, self.insertion_point.y as f32],
            height,
            angle,
            1.0,
            0.0,
            "txt", // standard CAD font
            &self.text,
        );

        Some(TruckEntity {
            object: TruckObject::Text(strokes),
            snap_pts: vec![(snap_pt, SnapHint::Insertion)],
            tangent_geoms: vec![],
            key_vertices: vec![],
        })
    }
}

impl Grippable for Tolerance {
    fn grips(&self) -> Vec<GripDef> {
        vec![square_grip(
            0,
            Vec3::new(
                self.insertion_point.x as f32,
                self.insertion_point.y as f32,
                self.insertion_point.z as f32,
            ),
        )]
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        if grip_id == 0 {
            match apply {
                GripApply::Translate(d) => {
                    self.insertion_point.x += d.x as f64;
                    self.insertion_point.y += d.y as f64;
                    self.insertion_point.z += d.z as f64;
                }
                GripApply::Absolute(p) => {
                    self.insertion_point.x = p.x as f64;
                    self.insertion_point.y = p.y as f64;
                    self.insertion_point.z = p.z as f64;
                }
            }
        }
    }
}

impl PropertyEditable for Tolerance {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        PropSection {
            title: "Geometry".into(),
            props: vec![
                ro("Text", "tol_text", self.text.clone()),
                edit("Insert X", "tol_ix", self.insertion_point.x),
                edit("Insert Y", "tol_iy", self.insertion_point.y),
                edit("Insert Z", "tol_iz", self.insertion_point.z),
            ],
        }
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        let Ok(v) = value.trim().parse::<f64>() else { return };
        match field {
            "tol_ix" => self.insertion_point.x = v,
            "tol_iy" => self.insertion_point.y = v,
            "tol_iz" => self.insertion_point.z = v,
            _ => {}
        }
    }
}

impl Transformable for Tolerance {
    fn apply_transform(&mut self, t: &EntityTransform) {
        transform::apply_standard_entity_transform(self, t, |entity, p1, p2| {
            transform::reflect_xy_point(
                &mut entity.insertion_point.x,
                &mut entity.insertion_point.y,
                p1,
                p2,
            );
        });
    }
}
