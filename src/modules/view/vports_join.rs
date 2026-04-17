// VPJOIN tool — ribbon definition + viewport-merge geometry helpers.
//
// `VPJOIN` merges two edge-adjacent paper-space viewports into a single one
// that covers their union rectangle.  H7CAD stores viewports as
// `Viewport { center: Vector3 (x, _, z), width, height }` (world-XZ plane,
// Y-up) — `JoinRect` below uses the same XZ axis pair.

use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub const ICON: IconKind = IconKind::Svg(include_bytes!("../../../assets/icons/vports_join.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "VPJOIN",
        label: "Join",
        icon: ICON,
        event: ModuleEvent::Command("VPJOIN".to_string()),
    }
}

/// Axis-aligned paper-space rectangle for two viewports about to be joined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JoinRect {
    /// Paper-space center X (= world X).
    pub cx: f64,
    /// Paper-space center Y (= world Z for H7CAD's Y-up convention).
    pub cy: f64,
    /// Width (paper-space X extent).
    pub w: f64,
    /// Height (paper-space Y extent).
    pub h: f64,
}

impl JoinRect {
    pub fn new(cx: f64, cy: f64, w: f64, h: f64) -> Self {
        Self { cx, cy, w, h }
    }
    pub fn x_min(&self) -> f64 { self.cx - self.w * 0.5 }
    pub fn x_max(&self) -> f64 { self.cx + self.w * 0.5 }
    pub fn y_min(&self) -> f64 { self.cy - self.h * 0.5 }
    pub fn y_max(&self) -> f64 { self.cy + self.h * 0.5 }
}

/// Tolerance for edge-coincidence checks.  Paper-space viewport rectangles
/// in H7CAD use `f64` and typically have integer-ish dimensions, so 1e-6 is
/// safely tighter than any realistic user-driven layout.
pub const JOIN_EPS: f64 = 1.0e-6;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= JOIN_EPS
}

/// Attempt to join two rectangles.  Returns `Some(merged)` iff they share an
/// entire edge (common `x` with identical `[y_min, y_max]`, **or** common
/// `y` with identical `[x_min, x_max]`).  The merged rectangle is the
/// bounding box of the union.
///
/// Two non-adjacent, partially-overlapping, or offset-edge cases all return
/// `None` — the caller should surface a descriptive error message.
pub fn join_rects(a: JoinRect, b: JoinRect) -> Option<JoinRect> {
    // Horizontal adjacency: share a vertical edge.
    let horizontal = (approx_eq(a.x_max(), b.x_min()) || approx_eq(b.x_max(), a.x_min()))
        && approx_eq(a.y_min(), b.y_min())
        && approx_eq(a.y_max(), b.y_max());

    // Vertical adjacency: share a horizontal edge.
    let vertical = (approx_eq(a.y_max(), b.y_min()) || approx_eq(b.y_max(), a.y_min()))
        && approx_eq(a.x_min(), b.x_min())
        && approx_eq(a.x_max(), b.x_max());

    if !horizontal && !vertical {
        return None;
    }

    let x_min = a.x_min().min(b.x_min());
    let x_max = a.x_max().max(b.x_max());
    let y_min = a.y_min().min(b.y_min());
    let y_max = a.y_max().max(b.y_max());
    let w = x_max - x_min;
    let h = y_max - y_min;
    let cx = x_min + w * 0.5;
    let cy = y_min + h * 0.5;
    Some(JoinRect::new(cx, cy, w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_horizontal_edge() {
        // Two 10×10 rects sitting side-by-side at x=10 with matching y span.
        let a = JoinRect::new(5.0, 5.0, 10.0, 10.0);
        let b = JoinRect::new(15.0, 5.0, 10.0, 10.0);
        let merged = join_rects(a, b).expect("adjacent rects should merge");
        assert_eq!(merged, JoinRect::new(10.0, 5.0, 20.0, 10.0));
    }

    #[test]
    fn joins_vertical_edge() {
        // Stacked vertically: b sits on top of a at y=10.
        let a = JoinRect::new(5.0, 5.0, 10.0, 10.0);
        let b = JoinRect::new(5.0, 15.0, 10.0, 10.0);
        let merged = join_rects(a, b).expect("stacked rects should merge");
        assert_eq!(merged, JoinRect::new(5.0, 10.0, 10.0, 20.0));
    }

    #[test]
    fn join_is_commutative() {
        let a = JoinRect::new(5.0, 5.0, 10.0, 10.0);
        let b = JoinRect::new(15.0, 5.0, 10.0, 10.0);
        assert_eq!(join_rects(a, b), join_rects(b, a));
    }

    #[test]
    fn rejects_gap_between_rects() {
        // 1-unit gap — not edge-adjacent.
        let a = JoinRect::new(5.0, 5.0, 10.0, 10.0);
        let b = JoinRect::new(16.0, 5.0, 10.0, 10.0);
        assert_eq!(join_rects(a, b), None);
    }

    #[test]
    fn rejects_overlap() {
        // Overlapping rects are NOT joinable — VPJOIN requires sharing an
        // entire edge, not partial overlap.
        let a = JoinRect::new(5.0, 5.0, 10.0, 10.0);
        let b = JoinRect::new(12.0, 5.0, 10.0, 10.0);
        assert_eq!(join_rects(a, b), None);
    }

    #[test]
    fn rejects_offset_edges() {
        // Same x-edge but different y span — not a full edge match.
        let a = JoinRect::new(5.0, 5.0, 10.0, 10.0);
        let b = JoinRect::new(15.0, 10.0, 10.0, 10.0);
        assert_eq!(join_rects(a, b), None);
    }

    #[test]
    fn handles_within_epsilon() {
        // Edges coincide up to JOIN_EPS/2 — should still merge.
        let a = JoinRect::new(5.0, 5.0, 10.0, 10.0);
        let b = JoinRect::new(15.0 + JOIN_EPS * 0.5, 5.0, 10.0, 10.0);
        assert!(join_rects(a, b).is_some());
    }
}
