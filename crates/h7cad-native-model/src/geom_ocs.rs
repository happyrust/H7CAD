//! Object Coordinate System (OCS) ↔ World Coordinate System (WCS) helpers.
//!
//! 17 DXF entity types (Arc, Circle, Ellipse, Point, Line, Spline,
//! LwPolyline, Polyline, AttributeDefinition, AttributeEntity,
//! Dimension, Hatch, MLine, Leader, Insert, Shape, plus the implicit
//! 2D-on-tilted-plane case for Text/MText) carry a `normal` extrusion
//! vector that defines an OCS basis. Geometry coordinates inside such
//! an entity are expressed in OCS, **not WCS**. Renderers and writers
//! that ignore this assumption silently flatten the entity to the XY
//! plane whenever `normal ≠ (0, 0, 1)`.
//!
//! This module provides the canonical *DXF arbitrary axis algorithm*
//! described in the AutoCAD R12 spec (and unchanged since), so every
//! tessellator, snap producer, and writer can share the exact same
//! basis derivation.
//!
//! ## Algorithm
//!
//! Given a unit normal `N`:
//!
//! ```text
//! if |Nx| < 1/64 and |Ny| < 1/64:
//!     Ax = WorldY × N      (pick world Y as the reference up axis)
//! else:
//!     Ax = WorldZ × N      (pick world Z otherwise)
//! Ax = normalize(Ax)
//! Ay = N × Ax
//! ```
//!
//! `(Ax, Ay, N)` is then a right-handed orthonormal basis of the OCS.
//! Any OCS point `(x, y, z)` maps to WCS via:
//!
//! ```text
//! WCS = origin + x·Ax + y·Ay + z·N
//! ```
//!
//! When `N == (0, 0, 1)` the algorithm yields `Ax = (1, 0, 0)`,
//! `Ay = (0, 1, 0)` (i.e. `ocs_to_wcs` becomes the identity translation),
//! so callers can apply this transform unconditionally without
//! penalising the dominant 2D-plan case.
//!
//! Reference: DXF Reference (Autodesk), "Arbitrary Axis Algorithm".

/// Threshold below which the world-Y axis is preferred as the seed for
/// `Ax` to avoid the degenerate cross-product near the world Z axis.
/// The 1/64 magic number is mandated by the DXF spec (R12 onwards).
const ARBITRARY_AXIS_THRESHOLD: f64 = 1.0 / 64.0;

/// Length below which a `normal` vector is treated as degenerate and
/// the identity OCS (world basis) is returned instead. AutoCAD writes
/// `(0, 0, 1)` for default-orientation entities, but malformed DXF
/// can ship `(0, 0, 0)` — in that case we choose to keep tessellation
/// alive rather than return NaN.
const DEGENERATE_NORMAL_LEN_SQ: f64 = 1.0e-20;

#[inline]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn normalize_or(default: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    let len_sq = dot(v, v);
    if len_sq < DEGENERATE_NORMAL_LEN_SQ {
        default
    } else {
        let inv = 1.0 / len_sq.sqrt();
        [v[0] * inv, v[1] * inv, v[2] * inv]
    }
}

/// Compute the right-handed orthonormal OCS basis `(Ax, Ay, N)` for
/// the given extrusion normal using the DXF arbitrary axis algorithm.
///
/// Returns `(Ax, Ay, N_normalized)`. When `n` is degenerate (length²
/// below `1e-20`) the world basis `((1,0,0), (0,1,0), (0,0,1))` is
/// returned to keep downstream tessellation total.
///
/// `n` does not need to be pre-normalized; this function normalizes it
/// internally.
///
/// # Examples
///
/// ```
/// use h7cad_native_model::geom_ocs::arbitrary_axis;
///
/// let (ax, ay, n) = arbitrary_axis([0.0, 0.0, 1.0]);
/// assert_eq!(ax, [1.0, 0.0, 0.0]);
/// assert_eq!(ay, [0.0, 1.0, 0.0]);
/// assert_eq!(n,  [0.0, 0.0, 1.0]);
/// ```
pub fn arbitrary_axis(n: [f64; 3]) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let n = normalize_or([0.0, 0.0, 1.0], n);

    let ax_seed = if n[0].abs() < ARBITRARY_AXIS_THRESHOLD && n[1].abs() < ARBITRARY_AXIS_THRESHOLD
    {
        cross([0.0, 1.0, 0.0], n)
    } else {
        cross([0.0, 0.0, 1.0], n)
    };

    let ax = normalize_or([1.0, 0.0, 0.0], ax_seed);
    let ay = cross(n, ax);

    (ax, ay, n)
}

/// Map an OCS point to WCS using the arbitrary axis algorithm.
///
/// `origin` is the WCS anchor of the entity (e.g. circle center,
/// arc center, polyline elevation reference). `p_ocs` is the point
/// expressed in OCS coordinates. `normal` is the entity's extrusion
/// vector (typically code 210/220/230 in DXF).
///
/// `WCS = origin + x·Ax + y·Ay + z·N`
///
/// When `normal == (0, 0, 1)` and `origin == (0, 0, 0)` this is a
/// no-op: `p_ocs` is returned unchanged. Callers can therefore apply
/// this transform unconditionally without performance concern in the
/// dominant 2D-plan case (the basis derivation is ~6 mul + 6 add).
///
/// # Examples
///
/// ```
/// use h7cad_native_model::geom_ocs::ocs_to_wcs;
///
/// let p = ocs_to_wcs([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
/// assert_eq!(p, [1.0, 0.0, 0.0]);
///
/// // For N=+x, the arbitrary-axis algorithm yields Ax=(0,1,0), Ay=(0,0,1),
/// // so OCS x maps to WCS +y (not −z).
/// let p = ocs_to_wcs([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
/// assert!((p[1] - 1.0).abs() < 1e-12);
/// ```
pub fn ocs_to_wcs(p_ocs: [f64; 3], origin: [f64; 3], normal: [f64; 3]) -> [f64; 3] {
    let (ax, ay, n) = arbitrary_axis(normal);
    [
        origin[0] + p_ocs[0] * ax[0] + p_ocs[1] * ay[0] + p_ocs[2] * n[0],
        origin[1] + p_ocs[0] * ax[1] + p_ocs[1] * ay[1] + p_ocs[2] * n[1],
        origin[2] + p_ocs[0] * ax[2] + p_ocs[1] * ay[2] + p_ocs[2] * n[2],
    ]
}

/// 2D variant for entities whose OCS coordinates are stored as
/// `(x, y)` pairs with an implicit `z = elevation` (LwPolyline,
/// Polyline2D, Hatch boundary edges, etc.).
///
/// Equivalent to `ocs_to_wcs([x, y, elevation], origin, normal)` but
/// avoids the temporary array.
pub fn ocs2d_to_wcs(
    x: f64,
    y: f64,
    elevation: f64,
    origin: [f64; 3],
    normal: [f64; 3],
) -> [f64; 3] {
    let (ax, ay, n) = arbitrary_axis(normal);
    [
        origin[0] + x * ax[0] + y * ay[0] + elevation * n[0],
        origin[1] + x * ax[1] + y * ay[1] + elevation * n[1],
        origin[2] + x * ax[2] + y * ay[2] + elevation * n[2],
    ]
}

/// `true` if `normal` is close enough to `(0, 0, 1)` that the OCS
/// basis collapses to the world basis. Useful as a fast-path predicate
/// to skip per-point transform when the dominant 2D-plan case applies.
///
/// Tolerance is `1e-12` per component, well below the `1/64` threshold
/// used inside `arbitrary_axis` so that any normal which would have
/// triggered the world-Y branch is *not* short-circuited.
#[inline]
pub fn is_world_normal(normal: [f64; 3]) -> bool {
    normal[0].abs() < 1.0e-12 && normal[1].abs() < 1.0e-12 && (normal[2] - 1.0).abs() < 1.0e-12
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: [f64; 3], b: [f64; 3], eps: f64) -> bool {
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps && (a[2] - b[2]).abs() < eps
    }

    #[test]
    fn arbitrary_axis_z_axis_returns_identity() {
        let (ax, ay, n) = arbitrary_axis([0.0, 0.0, 1.0]);
        assert_eq!(ax, [1.0, 0.0, 0.0]);
        assert_eq!(ay, [0.0, 1.0, 0.0]);
        assert_eq!(n, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn arbitrary_axis_below_threshold_uses_y_world_branch() {
        let n = [1.0 / 128.0, 0.0, 1.0];
        let (ax, ay, n_out) = arbitrary_axis(n);
        let dot_axay = dot(ax, ay);
        let dot_axn = dot(ax, n_out);
        let dot_ayn = dot(ay, n_out);
        assert!(dot_axay.abs() < 1.0e-10, "Ax·Ay = {dot_axay}");
        assert!(dot_axn.abs() < 1.0e-10, "Ax·N = {dot_axn}");
        assert!(dot_ayn.abs() < 1.0e-10, "Ay·N = {dot_ayn}");
        assert!((dot(ax, ax) - 1.0).abs() < 1.0e-10);
        assert!((dot(ay, ay) - 1.0).abs() < 1.0e-10);
        assert!((dot(n_out, n_out) - 1.0).abs() < 1.0e-10);
    }

    #[test]
    fn arbitrary_axis_above_threshold_uses_z_world_branch() {
        let (ax, ay, n) = arbitrary_axis([1.0, 0.0, 0.0]);
        assert!(approx_eq(ax, [0.0, 1.0, 0.0], 1.0e-12));
        assert!(approx_eq(ay, [0.0, 0.0, 1.0], 1.0e-12));
        assert_eq!(n, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn arbitrary_axis_negative_z_normal_handled() {
        let (ax, ay, n) = arbitrary_axis([0.0, 0.0, -1.0]);
        let dot_axay = dot(ax, ay);
        let dot_axn = dot(ax, n);
        let dot_ayn = dot(ay, n);
        assert!(dot_axay.abs() < 1.0e-10);
        assert!(dot_axn.abs() < 1.0e-10);
        assert!(dot_ayn.abs() < 1.0e-10);
        assert_eq!(n, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn arbitrary_axis_unnormalized_normal_normalizes_first() {
        let (ax, ay, n) = arbitrary_axis([0.0, 0.0, 5.0]);
        assert!(approx_eq(ax, [1.0, 0.0, 0.0], 1.0e-12));
        assert!(approx_eq(ay, [0.0, 1.0, 0.0], 1.0e-12));
        assert!(approx_eq(n, [0.0, 0.0, 1.0], 1.0e-12));
    }

    #[test]
    fn arbitrary_axis_degenerate_normal_returns_world_basis() {
        let (ax, ay, n) = arbitrary_axis([0.0, 0.0, 0.0]);
        assert_eq!(ax, [1.0, 0.0, 0.0]);
        assert_eq!(ay, [0.0, 1.0, 0.0]);
        assert_eq!(n, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn arbitrary_axis_basis_orthonormal_random_grid() {
        let samples = [
            [0.1, 0.2, 0.97],
            [0.5, 0.5, 0.707],
            [-0.3, 0.6, 0.74],
            [1.0 / 64.0, 1.0 / 64.0, 1.0 - 1.0 / 64.0],
            [1.0 / 65.0, 1.0 / 65.0, 1.0 - 1.0 / 65.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.6, 0.8, 0.0],
            [1.0 / 64.0 - 1e-9, 0.0, 0.99],
        ];
        for &raw_n in &samples {
            let (ax, ay, n) = arbitrary_axis(raw_n);
            let dot_axay = dot(ax, ay);
            let dot_axn = dot(ax, n);
            let dot_ayn = dot(ay, n);
            assert!(
                dot_axay.abs() < 1.0e-9,
                "Ax·Ay = {dot_axay} for n={raw_n:?}"
            );
            assert!(dot_axn.abs() < 1.0e-9, "Ax·N = {dot_axn} for n={raw_n:?}");
            assert!(dot_ayn.abs() < 1.0e-9, "Ay·N = {dot_ayn} for n={raw_n:?}");
            assert!((dot(ax, ax) - 1.0).abs() < 1.0e-9);
            assert!((dot(ay, ay) - 1.0).abs() < 1.0e-9);
            assert!((dot(n, n) - 1.0).abs() < 1.0e-9);

            let cross_axay = cross(ax, ay);
            assert!(
                approx_eq(cross_axay, n, 1.0e-9),
                "Ax × Ay should equal N (right-handed) for n={raw_n:?}"
            );
        }
    }

    #[test]
    fn ocs_to_wcs_identity_when_normal_is_world_z() {
        for p in [[0.0; 3], [1.0, 2.0, 3.0], [-5.0, 0.0, 7.0]] {
            let q = ocs_to_wcs(p, [0.0; 3], [0.0, 0.0, 1.0]);
            assert!(approx_eq(p, q, 1.0e-12));
        }
    }

    #[test]
    fn ocs_to_wcs_translates_by_origin() {
        let q = ocs_to_wcs([1.0, 2.0, 3.0], [10.0, 20.0, 30.0], [0.0, 0.0, 1.0]);
        assert!(approx_eq(q, [11.0, 22.0, 33.0], 1.0e-12));
    }

    #[test]
    fn ocs_to_wcs_with_x_normal_maps_ocs_basis_to_world_y_z() {
        let q_x = ocs_to_wcs([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let q_y = ocs_to_wcs([0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let q_z = ocs_to_wcs([0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        assert!(
            approx_eq(q_x, [0.0, 1.0, 0.0], 1.0e-12),
            "OCS x → WCS +y for N=+x, got {q_x:?}"
        );
        assert!(
            approx_eq(q_y, [0.0, 0.0, 1.0], 1.0e-12),
            "OCS y → WCS +z for N=+x, got {q_y:?}"
        );
        assert!(
            approx_eq(q_z, [1.0, 0.0, 0.0], 1.0e-12),
            "OCS z → WCS +x for N=+x, got {q_z:?}"
        );
    }

    #[test]
    fn ocs_to_wcs_preserves_distances_under_rotation() {
        let n = [0.5, 0.5, 0.707_106_781_186_547_5];
        let p1 = [1.0, 0.0, 0.0];
        let p2 = [0.0, 1.0, 0.0];
        let q1 = ocs_to_wcs(p1, [0.0; 3], n);
        let q2 = ocs_to_wcs(p2, [0.0; 3], n);

        let ocs_dist =
            ((p1[0] - p2[0]).powi(2) + (p1[1] - p2[1]).powi(2) + (p1[2] - p2[2]).powi(2)).sqrt();
        let wcs_dist =
            ((q1[0] - q2[0]).powi(2) + (q1[1] - q2[1]).powi(2) + (q1[2] - q2[2]).powi(2)).sqrt();
        assert!(
            (ocs_dist - wcs_dist).abs() < 1.0e-9,
            "OCS→WCS must be isometric: {ocs_dist} vs {wcs_dist}"
        );
    }

    #[test]
    fn ocs2d_to_wcs_matches_ocs_to_wcs_with_zero_z() {
        let n = [0.3, -0.4, 0.866_025_403_784_438_6];
        let origin = [1.0, 2.0, 3.0];
        let q3d = ocs_to_wcs([4.0, 5.0, 6.0], origin, n);
        let q2d = ocs2d_to_wcs(4.0, 5.0, 6.0, origin, n);
        assert!(approx_eq(q3d, q2d, 1.0e-12));
    }

    #[test]
    fn is_world_normal_detects_canonical_z() {
        assert!(is_world_normal([0.0, 0.0, 1.0]));
        assert!(is_world_normal([1e-15, -1e-15, 1.0 - 1e-15]));
        assert!(!is_world_normal([0.0, 0.0, -1.0]));
        assert!(!is_world_normal([1.0 / 128.0, 0.0, 1.0]));
        assert!(!is_world_normal([0.0, 0.0, 0.5]));
    }
}
