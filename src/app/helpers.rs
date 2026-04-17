use super::H7CAD;
use crate::scene::WireModel;
use crate::ui::overlay::GridPlane;
use acadrust::tables::Ucs;
use acadrust::Handle;
use h7cad_native_model as nm;
use std::path::Path;

impl H7CAD {
    pub(super) fn save_active_tab_to_path(&self, i: usize, path: &Path) -> Result<(), String> {
        use crate::store::CadStore;
        if let Some(store) = self.tabs[i].scene.native_store.as_ref() {
            store.save(path)
        } else {
            crate::io::save(&self.tabs[i].scene.document, path)
        }
    }

    /// Sync a compat entity from its native counterpart (reverse of
    /// `sync_native_entity_from_compat`).  Used after CadStore-based edits
    /// so the compat projection stays up-to-date for rendering/selection.
    pub(super) fn sync_compat_from_native(&mut self, i: usize, handle: Handle) {
        let nh = nm::Handle::new(handle.value());
        let Some(native_entity) = self.tabs[i].scene.native_doc()
            .and_then(|doc| doc.get_entity(nh))
        else {
            return;
        };
        let Some(compat_entity) = crate::io::native_bridge::native_entity_to_acadrust(native_entity) else {
            return;
        };
        if self.tabs[i].scene.document.get_entity(handle).is_some() {
            if let Some(existing) = self.tabs[i].scene.document.get_entity_mut(handle) {
                *existing = compat_entity;
            }
        }
    }

    /// Edit entities through CadStore with a single closure.  This is the
    /// native-first path: edits go directly to `NativeStore`, then the compat
    /// projection is updated.  Pushes an undo snapshot before the first change.
    pub(super) fn apply_store_edit(
        &mut self,
        i: usize,
        label: &'static str,
        mut edit: impl FnMut(&mut crate::store::NativeStore, nm::Handle),
    ) -> super::update::EditSummary {
        use super::update::EditSummary;

        let handles = self.property_target_handles(i);
        if handles.is_empty() {
            return EditSummary::default();
        }

        let mut summary = EditSummary::default();
        let mut snapshot_pushed = false;

        for handle in handles {
            let nh = nm::Handle::new(handle.value());
            let entity_exists = self.tabs[i]
                .scene
                .native_store
                .as_ref()
                .and_then(|s| s.inner().get_entity(nh))
                .is_some();

            if entity_exists {
                if !snapshot_pushed {
                    self.push_undo_snapshot(i, label);
                    snapshot_pushed = true;
                }
                if let Some(store) = self.tabs[i].scene.native_store.as_mut() {
                    edit(store, nh);
                }
                self.sync_compat_from_native(i, handle);
                summary.changed = true;
            } else {
                summary.unsupported = true;
            }
        }

        summary
    }
}

// ── Coordinate parsing ─────────────────────────────────────────────────────

/// Parse a typed coordinate string into a world-space Vec3.
/// Accepts "x,y"   → Vec3(x, y, 0)
///         "x,y,z" → Vec3(x, y, z)
/// Separators: comma or semicolon; decimal point or decimal comma.
pub(super) fn parse_coord(text: &str) -> Option<glam::Vec3> {
    let parts: Vec<f32> = text
        .split(|c| c == ',' || c == ';')
        .map(|s| s.trim().replace(',', "."))
        .filter_map(|s| s.parse().ok())
        .collect();
    match parts.as_slice() {
        [x, y] => Some(glam::Vec3::new(*x, *y, 0.0)),
        [x, y, z] => Some(glam::Vec3::new(*x, *y, *z)),
        _ => None,
    }
}

// ── UCS ↔ WCS transforms ───────────────────────────────────────────────────

/// Convert a point from UCS local coordinates to WCS.
///
/// WCS = origin + x_axis*u + y_axis*v + z_axis*w
pub(super) fn ucs_to_wcs(pt: glam::Vec3, ucs: &Ucs) -> glam::Vec3 {
    let o = glam::Vec3::new(ucs.origin.x as f32, ucs.origin.y as f32, ucs.origin.z as f32);
    let x = glam::Vec3::new(ucs.x_axis.x as f32, ucs.x_axis.y as f32, ucs.x_axis.z as f32);
    let y = glam::Vec3::new(ucs.y_axis.x as f32, ucs.y_axis.y as f32, ucs.y_axis.z as f32);
    let z_ax = ucs_z_axis(ucs);
    o + x * pt.x + y * pt.y + z_ax * pt.z
}

/// Convert a WCS point back to UCS local coordinates.
#[allow(dead_code)]
pub(super) fn wcs_to_ucs(pt: glam::Vec3, ucs: &Ucs) -> glam::Vec3 {
    let o = glam::Vec3::new(ucs.origin.x as f32, ucs.origin.y as f32, ucs.origin.z as f32);
    let x = glam::Vec3::new(ucs.x_axis.x as f32, ucs.x_axis.y as f32, ucs.x_axis.z as f32);
    let y = glam::Vec3::new(ucs.y_axis.x as f32, ucs.y_axis.y as f32, ucs.y_axis.z as f32);
    let z_ax = ucs_z_axis(ucs);
    let d = pt - o;
    glam::Vec3::new(d.dot(x), d.dot(y), d.dot(z_ax))
}

/// Return the normalised Z axis of a UCS (cross product of X and Y axes).
pub(super) fn ucs_z_axis(ucs: &Ucs) -> glam::Vec3 {
    let x = glam::Vec3::new(ucs.x_axis.x as f32, ucs.x_axis.y as f32, ucs.x_axis.z as f32);
    let y = glam::Vec3::new(ucs.y_axis.x as f32, ucs.y_axis.y as f32, ucs.y_axis.z as f32);
    x.cross(y).normalize_or_zero()
}

/// Build a UCS with `origin` and axes rotated by `angle_z_rad` around the Z axis.
pub(super) fn ucs_rotated_z(origin: glam::Vec3, angle_z: f32) -> Ucs {
    let cos = angle_z.cos() as f64;
    let sin = angle_z.sin() as f64;
    let mut ucs = Ucs::new("*ACTIVE*");
    ucs.origin = crate::types::Vector3::new(
        origin.x as f64, origin.y as f64, origin.z as f64,
    );
    ucs.x_axis = crate::types::Vector3::new(cos, sin, 0.0);
    ucs.y_axis = crate::types::Vector3::new(-sin, cos, 0.0);
    ucs
}

pub(super) fn angle_close(a: f32, b: f32, tol: f32) -> bool {
    let diff = (a - b).rem_euclid(std::f32::consts::TAU);
    let diff = if diff > std::f32::consts::PI {
        diff - std::f32::consts::TAU
    } else {
        diff
    };
    diff.abs() < tol
}

// ── Grid plane detection ───────────────────────────────────────────────────

/// Choose the grid plane whose normal is most aligned with the camera view direction.
pub(super) fn grid_plane_from_camera(pitch: f32, yaw: f32) -> GridPlane {
    let fz = pitch.sin().abs();
    let fy = (pitch.cos() * yaw.cos()).abs();
    let fx = (pitch.cos() * yaw.sin()).abs();
    if fz >= fy && fz >= fx {
        GridPlane::Xy
    } else if fy >= fx {
        GridPlane::Xz
    } else {
        GridPlane::Yz
    }
}

// ── Drawing constraint helpers ─────────────────────────────────────────────

/// Constrain `pt` to the nearest 90° direction from `base` (XY plane, Z-up).
pub(super) fn ortho_constrain(pt: glam::Vec3, base: glam::Vec3) -> glam::Vec3 {
    let dx = (pt.x - base.x).abs();
    let dy = (pt.y - base.y).abs();
    if dx >= dy {
        glam::Vec3::new(pt.x, base.y, pt.z)
    } else {
        glam::Vec3::new(base.x, pt.y, pt.z)
    }
}

/// Constrain `pt` to the nearest polar angle multiple from `base` (XY plane, Z-up).
pub(super) fn polar_constrain(pt: glam::Vec3, base: glam::Vec3, step_deg: f32) -> glam::Vec3 {
    let dx = pt.x - base.x;
    let dy = pt.y - base.y;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-6 {
        return pt;
    }
    let step = step_deg.to_radians();
    let angle = dy.atan2(dx);
    let snapped = (angle / step).round() * step;
    glam::Vec3::new(
        base.x + dist * snapped.cos(),
        base.y + dist * snapped.sin(),
        pt.z,
    )
}

// ── Clipboard / selection helpers ──────────────────────────────────────────

/// Compute the centroid of a set of wire models (average of all points).
pub(super) fn entities_centroid(wires: &[WireModel]) -> glam::Vec3 {
    let mut sum = glam::Vec3::ZERO;
    let mut count = 0usize;
    for w in wires {
        for p in &w.points {
            sum += glam::Vec3::from(*p);
            count += 1;
        }
    }
    if count > 0 { sum / count as f32 } else { glam::Vec3::ZERO }
}

/// Generate the next available auto group name ("*A1", "*A2", …).
pub(super) fn next_group_auto_name(scene: &crate::scene::Scene) -> String {
    let existing: std::collections::HashSet<String> =
        scene.groups().map(|g| g.name.clone()).collect();
    for n in 1..=9999 {
        let name = format!("*A{n}");
        if !existing.contains(&name) {
            return name;
        }
    }
    "*A".to_string()
}

// ── Entity type labels ─────────────────────────────────────────────────────

pub(super) fn entity_type_label(entity: &acadrust::EntityType) -> String {
    use acadrust::EntityType::*;
    match entity {
        Line(_) => "Line",
        Circle(_) => "Circle",
        Arc(_) => "Arc",
        Ellipse(_) => "Ellipse",
        Spline(_) => "Spline",
        LwPolyline(_) => "Polyline",
        Text(_) => "Text",
        MText(_) => "MText",
        Dimension(_) => "Dimension",
        Insert(_) => "Block Reference",
        Point(_) => "Point",
        Hatch(_) => "Hatch",
        _ => "Entity",
    }
    .to_string()
}

pub(super) fn entity_type_key(entity: &acadrust::EntityType) -> String {
    match entity {
        acadrust::EntityType::LwPolyline(_) => "pline",
        acadrust::EntityType::Circle(_) => "circle",
        acadrust::EntityType::Line(_) => "line",
        acadrust::EntityType::Arc(_) => "arc",
        acadrust::EntityType::Ellipse(_) => "ellipse",
        acadrust::EntityType::Spline(_) => "spline",
        acadrust::EntityType::Text(_) => "text",
        acadrust::EntityType::MText(_) => "mtext",
        acadrust::EntityType::Dimension(_) => "dimension",
        acadrust::EntityType::Insert(_) => "insert",
        acadrust::EntityType::Point(_) => "point",
        acadrust::EntityType::Hatch(_) => "hatch",
        _ => "entity",
    }
    .to_string()
}

pub(super) fn entity_type_label_native(entity: &nm::Entity) -> String {
    match &entity.data {
        nm::EntityData::Line { .. } => "Line",
        nm::EntityData::Circle { .. } => "Circle",
        nm::EntityData::Arc { .. } => "Arc",
        nm::EntityData::Ellipse { .. } => "Ellipse",
        nm::EntityData::Spline { .. } => "Spline",
        nm::EntityData::LwPolyline { .. } => "Polyline",
        nm::EntityData::Text { .. } => "Text",
        nm::EntityData::MText { .. } => "MText",
        nm::EntityData::Dimension { .. } => "Dimension",
        nm::EntityData::Insert { .. } => "Block Reference",
        nm::EntityData::Point { .. } => "Point",
        nm::EntityData::Hatch { .. } => "Hatch",
        nm::EntityData::Leader { .. } => "Leader",
        nm::EntityData::Viewport { .. } => "Viewport",
        _ => "Entity",
    }
    .to_string()
}

pub(super) fn entity_type_key_native(entity: &nm::Entity) -> String {
    match &entity.data {
        nm::EntityData::LwPolyline { .. } => "pline",
        nm::EntityData::Circle { .. } => "circle",
        nm::EntityData::Line { .. } => "line",
        nm::EntityData::Arc { .. } => "arc",
        nm::EntityData::Ellipse { .. } => "ellipse",
        nm::EntityData::Spline { .. } => "spline",
        nm::EntityData::Text { .. } => "text",
        nm::EntityData::MText { .. } => "mtext",
        nm::EntityData::Dimension { .. } => "dimension",
        nm::EntityData::Insert { .. } => "insert",
        nm::EntityData::Point { .. } => "point",
        nm::EntityData::Hatch { .. } => "hatch",
        nm::EntityData::Leader { .. } => "leader",
        nm::EntityData::Viewport { .. } => "viewport",
        _ => "entity",
    }
    .to_string()
}

pub(super) fn title_case_word(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => {
            let mut out = first.to_uppercase().collect::<String>();
            out.push_str(chars.as_str());
            out
        }
        None => String::new(),
    }
}

// ── Window icon ────────────────────────────────────────────────────────────

/// Builds a 32×32 RGBA icon: red background with H7 drawn in white pixels.
pub(super) fn build_window_icon() -> Vec<u8> {
    const W: usize = 32;
    const SZ: usize = W * W * 4;

    let bg = [176u8, 48, 32, 255];
    let fg = [255u8, 255, 255, 255];

    let mut px = vec![0u8; SZ];
    for i in 0..W * W {
        px[i * 4..i * 4 + 4].copy_from_slice(&bg);
    }

    fn stroke(px: &mut Vec<u8>, ax: i32, ay: i32, bx: i32, by: i32, fg: [u8; 4]) {
        let steps = ((bx - ax).abs().max((by - ay).abs()) * 3).max(1);
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let cx = ax as f32 + (bx - ax) as f32 * t;
            let cy = ay as f32 + (by - ay) as f32 * t;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let ix = cx.round() as i32 + dx;
                    let iy = cy.round() as i32 + dy;
                    if ix >= 0 && ix < W as i32 && iy >= 0 && iy < W as i32 {
                        let idx = (iy as usize * W + ix as usize) * 4;
                        px[idx..idx + 4].copy_from_slice(&fg);
                    }
                }
            }
        }
    }

    // H
    stroke(&mut px, 4, 5, 4, 26, fg);
    stroke(&mut px, 13, 5, 13, 26, fg);
    stroke(&mut px, 4, 15, 13, 15, fg);
    // 7
    stroke(&mut px, 17, 5, 27, 5, fg);
    stroke(&mut px, 27, 5, 20, 26, fg);
    stroke(&mut px, 20, 16, 26, 16, fg);

    px
}
