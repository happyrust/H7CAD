//! Pure geometry helpers shared between the PDF (R33) and SVG (R43-A)
//! hatch pattern line emitters.  Neither helper writes any output bytes
//! — they only return geometric data so each exporter can format its own
//! `Op::DrawLine` (PDF) or `<polyline>` (SVG) tokens.

/// Axis-aligned bounding box of a polygon ring.  Returns `(x0, y0, x1, y1)`
/// in CAD world coordinates (no offset applied).
///
/// Empty input collapses to `(+inf, +inf, -inf, -inf)` — callers should
/// treat any AABB with `x1 < x0` or `y1 < y0` as degenerate and skip.
pub fn aabb_of(points: &[[f32; 2]]) -> (f32, f32, f32, f32) {
    let mut x0 = f32::INFINITY;
    let mut y0 = f32::INFINITY;
    let mut x1 = f32::NEG_INFINITY;
    let mut y1 = f32::NEG_INFINITY;
    for &[x, y] in points {
        if x < x0 {
            x0 = x;
        }
        if y < y0 {
            y0 = y;
        }
        if x > x1 {
            x1 = x;
        }
        if y > y1 {
            y1 = y;
        }
    }
    (x0, y0, x1, y1)
}

/// Liang-Barsky line-vs-AABB clip.  Returns `Some((t0, t1))` such that
/// the parametric ray `P(t) = (ox, oy) + t * (dx, dy)` intersects
/// `[bx0..bx1] × [by0..by1]` for `t ∈ [t0, t1]`, or `None` when the ray
/// misses the box entirely.
///
/// Used by both PDF (R33) and SVG (R43-A) hatch pattern emitters to clip
/// each generated parallel line to the boundary AABB before serialising.
#[allow(clippy::too_many_arguments)]
pub fn clip_line_aabb(
    ox: f32,
    oy: f32,
    dx: f32,
    dy: f32,
    bx0: f32,
    by0: f32,
    bx1: f32,
    by1: f32,
) -> Option<(f32, f32)> {
    let mut t0 = f32::NEG_INFINITY;
    let mut t1 = f32::INFINITY;

    for &(p, q) in &[
        (-dx, ox - bx0),
        (dx, bx1 - ox),
        (-dy, oy - by0),
        (dy, by1 - oy),
    ] {
        if p.abs() < 1e-8 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            if r > t1 {
                return None;
            }
            if r > t0 {
                t0 = r;
            }
        } else {
            if r < t0 {
                return None;
            }
            if r < t1 {
                t1 = r;
            }
        }
    }
    if t0 > t1 {
        None
    } else {
        Some((t0, t1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_of_includes_all_points() {
        let pts = [[1.0, 2.0], [3.0, -1.0], [-2.0, 4.0]];
        let (x0, y0, x1, y1) = aabb_of(&pts);
        assert_eq!(x0, -2.0);
        assert_eq!(y0, -1.0);
        assert_eq!(x1, 3.0);
        assert_eq!(y1, 4.0);
    }

    #[test]
    fn aabb_of_empty_collapses() {
        let (x0, y0, x1, y1) = aabb_of(&[]);
        assert!(x1 < x0 && y1 < y0);
    }

    #[test]
    fn clip_line_aabb_horizontal_through_box_returns_full_extent() {
        let got = clip_line_aabb(-1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 2.0, 2.0)
            .expect("ray must intersect box");
        // Horizontal ray at y=1, starts at x=-1, dir=(1,0). Intersects
        // box [0..2] x [0..2] from t=1 to t=3.
        assert!((got.0 - 1.0).abs() < 1e-6);
        assert!((got.1 - 3.0).abs() < 1e-6);
    }

    #[test]
    fn clip_line_aabb_misses_box_returns_none() {
        // Ray entirely above the box.
        let got = clip_line_aabb(-1.0, 5.0, 1.0, 0.0, 0.0, 0.0, 2.0, 2.0);
        assert!(got.is_none(), "ray above box must miss");
    }

    #[test]
    fn clip_line_aabb_diagonal_through_box() {
        // Ray from (-1,-1) along (1,1). Enters at (0,0) → t=1, exits at
        // (2,2) → t=3.
        let got = clip_line_aabb(-1.0, -1.0, 1.0, 1.0, 0.0, 0.0, 2.0, 2.0)
            .expect("diagonal ray must intersect");
        assert!((got.0 - 1.0).abs() < 1e-6);
        assert!((got.1 - 3.0).abs() < 1e-6);
    }
}
