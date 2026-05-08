// Grippable + PropertyEditable for Solid3D, Region, Body.
//
// Geometry lives in ACIS data — we cannot edit it via the properties panel.
// We expose the point_of_reference as a translate grip and show ACIS size
// as read-only info.  Grip translate also updates wire points so the wire
// fallback stays in sync; the caller (scene/mod.rs apply_grip) translates
// the MeshModel vertices to match.

use acadrust::entities::{Body, Region, Solid3D};
use glam::Vec3;

use crate::entities::common::{diamond_grip, ro_prop as ro};
use crate::entities::traits::{Grippable, PropertyEditable};
use crate::scene::object::{GripApply, GripDef, PropSection};

// ── shared helpers ────────────────────────────────────────────────────────────

fn por_to_vec3(v: &acadrust::types::Vector3) -> Vec3 {
    Vec3::new(v.x as f32, v.y as f32, v.z as f32)
}

fn translate_wires(wires: &mut Vec<acadrust::entities::Wire>, d: Vec3) {
    for wire in wires.iter_mut() {
        for pt in wire.points.iter_mut() {
            pt.x += d.x as f64;
            pt.y += d.y as f64;
            pt.z += d.z as f64;
        }
    }
}

fn acis_size_str(has_data: bool, sat_len: usize, sab_len: usize, is_binary: bool) -> String {
    if !has_data {
        return "none".to_string();
    }
    if is_binary {
        format!("{} bytes (SAB)", sab_len)
    } else {
        format!("{} bytes (SAT)", sat_len)
    }
}

// ── Solid3D ───────────────────────────────────────────────────────────────────

impl Grippable for Solid3D {
    fn grips(&self) -> Vec<GripDef> {
        vec![diamond_grip(0, por_to_vec3(&self.point_of_reference))]
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        if grip_id != 0 {
            return;
        }
        if let GripApply::Translate(d) = apply {
            self.point_of_reference.x += d.x as f64;
            self.point_of_reference.y += d.y as f64;
            self.point_of_reference.z += d.z as f64;
            translate_wires(&mut self.wires, d);
        }
    }
}

impl PropertyEditable for Solid3D {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        let size = acis_size_str(
            self.acis_data.has_data(),
            self.acis_data.sat_data.len(),
            self.acis_data.sab_data.len(),
            self.acis_data.is_binary,
        );
        PropSection {
            title: "Geometry".into(),
            props: vec![
                ro("Ref Pt X", "s3d_px", format!("{:.4}", self.point_of_reference.x)),
                ro("Ref Pt Y", "s3d_py", format!("{:.4}", self.point_of_reference.y)),
                ro("Ref Pt Z", "s3d_pz", format!("{:.4}", self.point_of_reference.z)),
                ro("ACIS Data", "s3d_acis", size),
            ],
        }
    }

    fn apply_geom_prop(&mut self, _field: &str, _value: &str) {}
}

// ── Region ────────────────────────────────────────────────────────────────────

impl Grippable for Region {
    fn grips(&self) -> Vec<GripDef> {
        vec![diamond_grip(0, por_to_vec3(&self.point_of_reference))]
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        if grip_id != 0 {
            return;
        }
        if let GripApply::Translate(d) = apply {
            self.point_of_reference.x += d.x as f64;
            self.point_of_reference.y += d.y as f64;
            self.point_of_reference.z += d.z as f64;
            translate_wires(&mut self.wires, d);
        }
    }
}

impl PropertyEditable for Region {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        let size = acis_size_str(
            self.acis_data.has_data(),
            self.acis_data.sat_data.len(),
            self.acis_data.sab_data.len(),
            self.acis_data.is_binary,
        );
        PropSection {
            title: "Geometry".into(),
            props: vec![
                ro("Ref Pt X", "rgn_px", format!("{:.4}", self.point_of_reference.x)),
                ro("Ref Pt Y", "rgn_py", format!("{:.4}", self.point_of_reference.y)),
                ro("Ref Pt Z", "rgn_pz", format!("{:.4}", self.point_of_reference.z)),
                ro("ACIS Data", "rgn_acis", size),
            ],
        }
    }

    fn apply_geom_prop(&mut self, _field: &str, _value: &str) {}
}

// ── Body ──────────────────────────────────────────────────────────────────────

impl Grippable for Body {
    fn grips(&self) -> Vec<GripDef> {
        vec![diamond_grip(0, por_to_vec3(&self.point_of_reference))]
    }

    fn apply_grip(&mut self, grip_id: usize, apply: GripApply) {
        if grip_id != 0 {
            return;
        }
        if let GripApply::Translate(d) = apply {
            self.point_of_reference.x += d.x as f64;
            self.point_of_reference.y += d.y as f64;
            self.point_of_reference.z += d.z as f64;
            translate_wires(&mut self.wires, d);
        }
    }
}

impl PropertyEditable for Body {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        let size = acis_size_str(
            self.acis_data.has_data(),
            self.acis_data.sat_data.len(),
            self.acis_data.sab_data.len(),
            self.acis_data.is_binary,
        );
        PropSection {
            title: "Geometry".into(),
            props: vec![
                ro("Ref Pt X", "bdy_px", format!("{:.4}", self.point_of_reference.x)),
                ro("Ref Pt Y", "bdy_py", format!("{:.4}", self.point_of_reference.y)),
                ro("Ref Pt Z", "bdy_pz", format!("{:.4}", self.point_of_reference.z)),
                ro("ACIS Data", "bdy_acis", size),
            ],
        }
    }

    fn apply_geom_prop(&mut self, _field: &str, _value: &str) {}
}
