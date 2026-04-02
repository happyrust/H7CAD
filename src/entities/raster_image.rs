use acadrust::entities::{RasterImage, Wipeout};
use glam::Vec3;

use crate::command::EntityTransform;
use crate::entities::common::{diamond_grip, edit_prop as edit, ro_prop as ro, square_grip};
use crate::entities::traits::{Grippable, PropertyEditable, Transformable, TruckConvertible};
use crate::scene::acad_to_truck::{TruckEntity, TruckObject};
use crate::scene::object::{GripApply, GripDef, PropSection, PropValue, Property};

// ── Shared geometry helpers ───────────────────────────────────────────────────

/// Compute the four world-space corners of an image/wipeout from its
/// insertion_point, u_vector, v_vector and pixel size.
///
/// Returns (p0, p1, p2, p3) in counter-clockwise order:
///   p0 = origin
///   p1 = origin + U*W
///   p2 = origin + U*W + V*H
///   p3 = origin + V*H
fn image_corners(
    origin: &acadrust::types::Vector3,
    u: &acadrust::types::Vector3,
    v: &acadrust::types::Vector3,
    w: f64,
    h: f64,
) -> [[f32; 3]; 4] {
    let ox = origin.x as f32;
    let oy = origin.y as f32;
    let oz = origin.z as f32;
    let ux = (u.x * w) as f32;
    let uy = (u.y * w) as f32;
    let uz = (u.z * w) as f32;
    let vx = (v.x * h) as f32;
    let vy = (v.y * h) as f32;
    let vz = (v.z * h) as f32;

    [
        [ox, oy, oz],
        [ox + ux, oy + uy, oz + uz],
        [ox + ux + vx, oy + uy + vy, oz + uz + vz],
        [ox + vx, oy + vy, oz + vz],
    ]
}

/// Rectangle border + X diagonals — used as a placeholder for images.
fn image_wire(corners: [[f32; 3]; 4], with_x: bool) -> Vec<[f32; 3]> {
    let [p0, p1, p2, p3] = corners;
    let mut pts = vec![p0, p1, p2, p3, p0];
    if with_x {
        pts.push([f32::NAN; 3]);
        pts.push(p0);
        pts.push(p2);
        pts.push([f32::NAN; 3]);
        pts.push(p1);
        pts.push(p3);
    }
    pts
}

fn reflect_vec3(
    vx: &mut f64,
    vy: &mut f64,
    ax: f64,
    ay: f64,
    len2: f64,
) {
    let dot = *vx * ax + *vy * ay;
    *vx = 2.0 * dot * ax / len2 - *vx;
    *vy = 2.0 * dot * ay / len2 - *vy;
}

// ── RasterImage ───────────────────────────────────────────────────────────────

impl TruckConvertible for RasterImage {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        let corners = image_corners(
            &self.insertion_point,
            &self.u_vector,
            &self.v_vector,
            self.size.x,
            self.size.y,
        );
        Some(TruckEntity {
            object: TruckObject::Lines(image_wire(corners, true)),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: corners.to_vec(),
        })
    }
}

impl Grippable for RasterImage {
    fn grips(&self) -> Vec<GripDef> {
        let corners = image_corners(
            &self.insertion_point,
            &self.u_vector,
            &self.v_vector,
            self.size.x,
            self.size.y,
        );
        vec![
            square_grip(0, Vec3::from(corners[0])),
            diamond_grip(1, Vec3::from(corners[1])),
            diamond_grip(2, Vec3::from(corners[2])),
            diamond_grip(3, Vec3::from(corners[3])),
        ]
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
        // Corner grips 1-3 are display-only (resizing changes u/v vectors,
        // which requires careful normalization — deferred).
    }
}

impl PropertyEditable for RasterImage {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        PropSection {
            title: "Geometry".into(),
            props: vec![
                ro("File", "ri_file", self.file_path.clone()),
                edit("Insert X", "ri_ox", self.insertion_point.x),
                edit("Insert Y", "ri_oy", self.insertion_point.y),
                edit("Insert Z", "ri_oz", self.insertion_point.z),
                edit("Brightness", "ri_bright", self.brightness as f64),
                edit("Contrast", "ri_contrast", self.contrast as f64),
                edit("Fade", "ri_fade", self.fade as f64),
                Property {
                    label: "Clipping".into(),
                    field: "ri_clip",
                    value: PropValue::BoolToggle {
                        field: "ri_clip",
                        value: self.clipping_enabled,
                    },
                },
            ],
        }
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        match field {
            "ri_clip" => {
                self.clipping_enabled =
                    if value == "toggle" { !self.clipping_enabled } else { value == "true" };
                return;
            }
            _ => {}
        }
        let Ok(v) = value.trim().parse::<f64>() else { return };
        match field {
            "ri_ox" => self.insertion_point.x = v,
            "ri_oy" => self.insertion_point.y = v,
            "ri_oz" => self.insertion_point.z = v,
            "ri_bright" => self.brightness = v.clamp(0.0, 100.0) as u8,
            "ri_contrast" => self.contrast = v.clamp(0.0, 100.0) as u8,
            "ri_fade" => self.fade = v.clamp(0.0, 100.0) as u8,
            _ => {}
        }
    }
}

impl Transformable for RasterImage {
    fn apply_transform(&mut self, t: &EntityTransform) {
        crate::scene::transform::apply_standard_entity_transform(self, t, |entity, p1, p2| {
            crate::scene::transform::reflect_xy_point(
                &mut entity.insertion_point.x,
                &mut entity.insertion_point.y,
                p1,
                p2,
            );
            let ax = (p2.x - p1.x) as f64;
            let ay = (p2.y - p1.y) as f64;
            let len2 = ax * ax + ay * ay;
            if len2 > 1e-12 {
                reflect_vec3(&mut entity.u_vector.x, &mut entity.u_vector.y, ax, ay, len2);
                reflect_vec3(&mut entity.v_vector.x, &mut entity.v_vector.y, ax, ay, len2);
            }
        });
    }
}

// ── Wipeout ───────────────────────────────────────────────────────────────────

impl TruckConvertible for Wipeout {
    fn to_truck(&self, _document: &acadrust::CadDocument) -> Option<TruckEntity> {
        let corners = image_corners(
            &self.insertion_point,
            &self.u_vector,
            &self.v_vector,
            self.size.x,
            self.size.y,
        );

        // If clipping is enabled and there's a polygon boundary, show that.
        let pts = if self.clipping_enabled
            && self.clip_boundary_vertices.len() >= 3
            && matches!(
                self.clip_type,
                acadrust::entities::WipeoutClipType::Polygonal
            )
        {
            // Convert pixel-space boundary vertices to world space:
            // world = insertion_point + u_vector * v.x * size.x + v_vector * v.y * size.y
            let ox = self.insertion_point.x as f32;
            let oy = self.insertion_point.y as f32;
            let oz = self.insertion_point.z as f32;
            let mut poly: Vec<[f32; 3]> = self
                .clip_boundary_vertices
                .iter()
                .map(|v| {
                    let wx = (self.u_vector.x * v.x * self.size.x
                        + self.v_vector.x * v.y * self.size.y) as f32;
                    let wy = (self.u_vector.y * v.x * self.size.x
                        + self.v_vector.y * v.y * self.size.y) as f32;
                    let wz = (self.u_vector.z * v.x * self.size.x
                        + self.v_vector.z * v.y * self.size.y) as f32;
                    [ox + wx, oy + wy, oz + wz]
                })
                .collect();
            // Close the polygon.
            if let Some(&first) = poly.first() {
                poly.push(first);
            }
            poly
        } else {
            // Rectangular boundary — just the border, no diagonals (mask area).
            image_wire(corners, false)
        };

        Some(TruckEntity {
            object: TruckObject::Lines(pts),
            snap_pts: vec![],
            tangent_geoms: vec![],
            key_vertices: corners.to_vec(),
        })
    }
}

impl Grippable for Wipeout {
    fn grips(&self) -> Vec<GripDef> {
        let corners = image_corners(
            &self.insertion_point,
            &self.u_vector,
            &self.v_vector,
            self.size.x,
            self.size.y,
        );
        vec![
            square_grip(0, Vec3::from(corners[0])),
            diamond_grip(1, Vec3::from(corners[1])),
            diamond_grip(2, Vec3::from(corners[2])),
            diamond_grip(3, Vec3::from(corners[3])),
        ]
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

impl PropertyEditable for Wipeout {
    fn geometry_properties(&self, _text_style_names: &[String]) -> PropSection {
        PropSection {
            title: "Geometry".into(),
            props: vec![
                edit("Insert X", "wo_ox", self.insertion_point.x),
                edit("Insert Y", "wo_oy", self.insertion_point.y),
                edit("Insert Z", "wo_oz", self.insertion_point.z),
                edit("Brightness", "wo_bright", self.brightness as f64),
                edit("Contrast", "wo_contrast", self.contrast as f64),
                edit("Fade", "wo_fade", self.fade as f64),
                Property {
                    label: "Clipping".into(),
                    field: "wo_clip",
                    value: PropValue::BoolToggle {
                        field: "wo_clip",
                        value: self.clipping_enabled,
                    },
                },
            ],
        }
    }

    fn apply_geom_prop(&mut self, field: &str, value: &str) {
        match field {
            "wo_clip" => {
                self.clipping_enabled =
                    if value == "toggle" { !self.clipping_enabled } else { value == "true" };
                return;
            }
            _ => {}
        }
        let Ok(v) = value.trim().parse::<f64>() else { return };
        match field {
            "wo_ox" => self.insertion_point.x = v,
            "wo_oy" => self.insertion_point.y = v,
            "wo_oz" => self.insertion_point.z = v,
            "wo_bright" => self.brightness = v.clamp(0.0, 100.0) as u8,
            "wo_contrast" => self.contrast = v.clamp(0.0, 100.0) as u8,
            "wo_fade" => self.fade = v.clamp(0.0, 100.0) as u8,
            _ => {}
        }
    }
}

impl Transformable for Wipeout {
    fn apply_transform(&mut self, t: &EntityTransform) {
        crate::scene::transform::apply_standard_entity_transform(self, t, |entity, p1, p2| {
            crate::scene::transform::reflect_xy_point(
                &mut entity.insertion_point.x,
                &mut entity.insertion_point.y,
                p1,
                p2,
            );
            let ax = (p2.x - p1.x) as f64;
            let ay = (p2.y - p1.y) as f64;
            let len2 = ax * ax + ay * ay;
            if len2 > 1e-12 {
                reflect_vec3(&mut entity.u_vector.x, &mut entity.u_vector.y, ax, ay, len2);
                reflect_vec3(&mut entity.v_vector.x, &mut entity.v_vector.y, ax, ay, len2);
            }
        });
    }
}
