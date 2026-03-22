//! H7CAD-style grip editing.

use acadrust::Handle;
use glam::{Mat4, Vec3};
use iced::{Point, Rectangle};

use super::object::{GripDef, GripShape};

/// Pixel radius for grip hit-detection.
pub const GRIP_THRESHOLD_PX: f32 = 8.0;
/// Half-size of the rendered grip square / diamond in pixels.
pub const GRIP_HALF_PX: f32 = 5.0;

// ── Active drag state ─────────────────────────────────────────────────────

/// Stored on `H7CAD` while a grip is being dragged.
#[derive(Clone, Debug)]
pub struct GripEdit {
    /// Handle of the entity being edited.
    pub handle: Handle,
    /// Index into the entity's grip list.
    pub grip_id: usize,
    /// `true` → midpoint / translate grip; `false` → endpoint / absolute grip.
    pub is_translate: bool,
    /// World-space position of the grip when the drag started (ortho/polar base).
    pub origin_world: Vec3,
    /// Last world-space cursor position (needed for incremental delta on translate drags).
    pub last_world: Vec3,
}

// ── Screen-space helpers ───────────────────────────────────────────────────

/// Project a slice of `GripDef`s to screen space.
/// Returns `(grip_id, screen_pos, is_midpoint, shape)` for each grip.
pub fn grips_to_screen(
    grips: &[GripDef],
    view_proj: Mat4,
    bounds: Rectangle,
) -> Vec<(usize, Point, bool, GripShape)> {
    grips
        .iter()
        .map(|g| {
            let ndc = view_proj.project_point3(g.world);
            let screen = Point::new(
                (ndc.x + 1.0) * 0.5 * bounds.width,
                (1.0 - ndc.y) * 0.5 * bounds.height,
            );
            (g.id, screen, g.is_midpoint, g.shape)
        })
        .collect()
}

/// Find the closest grip within `GRIP_THRESHOLD_PX` pixels of `cursor`.
/// Returns `(grip_id, is_translate, world_pos)` if found, else `None`.
pub fn find_hit_grip(
    cursor: Point,
    grips: &[GripDef],
    view_proj: Mat4,
    bounds: Rectangle,
) -> Option<(usize, bool, Vec3)> {
    let mut best_dist = GRIP_THRESHOLD_PX;
    let mut best: Option<(usize, bool, Vec3)> = None;

    for g in grips {
        let ndc = view_proj.project_point3(g.world);
        let screen = Point::new(
            (ndc.x + 1.0) * 0.5 * bounds.width,
            (1.0 - ndc.y) * 0.5 * bounds.height,
        );
        let dx = screen.x - cursor.x;
        let dy = screen.y - cursor.y;
        let d = (dx * dx + dy * dy).sqrt();
        if d < best_dist {
            best_dist = d;
            best = Some((g.id, g.is_midpoint, g.world));
        }
    }
    best
}
