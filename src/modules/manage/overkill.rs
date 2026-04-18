//! OVERKILL — remove duplicate / overlapping geometry.
//!
//! Supports the four simple geometric primitives that cover ~80% of real
//! CAD duplication: `Line`, `Circle`, `Arc`, `Point`.  More complex
//! entities (Polyline, Hatch, Text, Dimension, Spline) are conservatively
//! skipped — they return `None` from `geom_key` so they can never be
//! flagged as duplicates.
//!
//! The algorithm is a single hash-map pass over the input entities:
//!
//! 1. For every entity compute its canonical `GeomKey` (orientation-
//!    independent for Line, epsilon-quantised for all coordinates).
//! 2. Group `Handle`s by key in insertion order.
//! 3. The first `Handle` in each bucket is kept; the rest become
//!    duplicates to remove.

use acadrust::{EntityType, Handle};

use crate::modules::{IconKind, ModuleEvent, ToolDef};

pub const ICON: IconKind =
    IconKind::Svg(include_bytes!("../../../assets/icons/overkill.svg"));

pub fn tool() -> ToolDef {
    ToolDef {
        id: "OVERKILL",
        label: "Overkill",
        icon: ICON,
        event: ModuleEvent::Command("OVERKILL".to_string()),
    }
}

/// Canonical geometric fingerprint used to detect duplicates.
///
/// All coordinate and scalar fields are pre-quantised to integer keys
/// (see `quantise`), so the enum implements `Eq + Hash` via derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeomKey {
    Line { a: QPoint, b: QPoint },
    Circle { center: QPoint, radius: QScalar },
    Arc {
        center: QPoint,
        radius: QScalar,
        start_angle: QScalar,
        end_angle: QScalar,
    },
    Point(QPoint),
}

/// Quantised 3D point — three `i64` components keyed by tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QPoint(pub i64, pub i64, pub i64);

/// Quantised scalar (radius / angle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QScalar(pub i64);

/// Scale factor for `1e-6` tolerance.  Keys at `1.0 + 5e-7` and
/// `1.0 - 5e-7` may fall into different buckets — a conservative
/// false-negative (miss a duplicate) rather than a false-positive
/// (delete distinct geometry).
const QUANT_FACTOR: f64 = 1e6;

fn quantise(value: f64) -> i64 {
    (value * QUANT_FACTOR).round() as i64
}

fn qpoint(x: f64, y: f64, z: f64) -> QPoint {
    QPoint(quantise(x), quantise(y), quantise(z))
}

/// Order-independent Line key: the two endpoints are sorted lexicographically
/// so `Line(A→B)` and `Line(B→A)` produce the same key.
fn line_key(a: QPoint, b: QPoint) -> GeomKey {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    GeomKey::Line { a: lo, b: hi }
}

/// Compute the canonical key for a supported `EntityType`, or `None` for
/// unsupported entities (they are skipped by the duplicate detector and
/// therefore never flagged for removal).
pub fn geom_key(e: &EntityType) -> Option<GeomKey> {
    match e {
        EntityType::Line(l) => {
            let a = qpoint(l.start.x, l.start.y, l.start.z);
            let b = qpoint(l.end.x, l.end.y, l.end.z);
            Some(line_key(a, b))
        }
        EntityType::Circle(c) => {
            let center = qpoint(c.center.x, c.center.y, c.center.z);
            Some(GeomKey::Circle {
                center,
                radius: QScalar(quantise(c.radius)),
            })
        }
        EntityType::Arc(a) => {
            let center = qpoint(a.center.x, a.center.y, a.center.z);
            Some(GeomKey::Arc {
                center,
                radius: QScalar(quantise(a.radius)),
                start_angle: QScalar(quantise(a.start_angle)),
                end_angle: QScalar(quantise(a.end_angle)),
            })
        }
        EntityType::Point(p) => Some(GeomKey::Point(qpoint(
            p.location.x,
            p.location.y,
            p.location.z,
        ))),
        _ => None,
    }
}

/// Given a list of `(Handle, EntityType)` pairs, return the subset of
/// Handles that are duplicates of an earlier entry (by `geom_key`).
/// The first occurrence of each key is kept; later occurrences are
/// returned in encounter order.
///
/// Entities with `geom_key() == None` are left alone (never duplicated).
pub fn find_duplicates(entries: &[(Handle, EntityType)]) -> Vec<Handle> {
    use std::collections::HashSet;

    let mut seen: HashSet<GeomKey> = HashSet::with_capacity(entries.len());
    let mut dupes: Vec<Handle> = Vec::new();
    for (handle, entity) in entries {
        let Some(key) = geom_key(entity) else { continue };
        if !seen.insert(key) {
            dupes.push(*handle);
        }
    }
    dupes
}

#[cfg(test)]
mod tests {
    use super::*;
    use acadrust::entities::{Arc, Circle, EntityCommon, Line, Point};
    use acadrust::types::Vector3;
    use acadrust::Handle;

    fn h(value: u64) -> Handle {
        Handle::new(value)
    }

    fn line(a: (f64, f64, f64), b: (f64, f64, f64)) -> EntityType {
        let mut l = Line::default();
        l.common = EntityCommon::default();
        l.start = Vector3::new(a.0, a.1, a.2);
        l.end = Vector3::new(b.0, b.1, b.2);
        EntityType::Line(l)
    }

    fn circle(c: (f64, f64, f64), r: f64) -> EntityType {
        let mut ci = Circle::default();
        ci.common = EntityCommon::default();
        ci.center = Vector3::new(c.0, c.1, c.2);
        ci.radius = r;
        EntityType::Circle(ci)
    }

    fn arc(c: (f64, f64, f64), r: f64, s: f64, e: f64) -> EntityType {
        let mut a = Arc::default();
        a.common = EntityCommon::default();
        a.center = Vector3::new(c.0, c.1, c.2);
        a.radius = r;
        a.start_angle = s;
        a.end_angle = e;
        EntityType::Arc(a)
    }

    fn point(p: (f64, f64, f64)) -> EntityType {
        let mut pt = Point::default();
        pt.common = EntityCommon::default();
        pt.location = Vector3::new(p.0, p.1, p.2);
        EntityType::Point(pt)
    }

    #[test]
    fn identical_lines_are_duplicates() {
        let entries = vec![
            (h(1), line((0.0, 0.0, 0.0), (10.0, 0.0, 0.0))),
            (h(2), line((0.0, 0.0, 0.0), (10.0, 0.0, 0.0))),
        ];
        let dupes = find_duplicates(&entries);
        assert_eq!(dupes, vec![h(2)], "second identical line is the dup");
    }

    #[test]
    fn reversed_line_endpoints_count_as_same() {
        let entries = vec![
            (h(1), line((0.0, 0.0, 0.0), (10.0, 0.0, 0.0))),
            (h(2), line((10.0, 0.0, 0.0), (0.0, 0.0, 0.0))),
        ];
        let dupes = find_duplicates(&entries);
        assert_eq!(dupes, vec![h(2)], "A→B and B→A must collapse");
    }

    #[test]
    fn concentric_circles_same_radius_are_duplicates() {
        let entries = vec![
            (h(1), circle((0.0, 0.0, 0.0), 5.0)),
            (h(2), circle((0.0, 0.0, 0.0), 5.0)),
        ];
        assert_eq!(find_duplicates(&entries), vec![h(2)]);
    }

    #[test]
    fn different_radius_circles_are_kept() {
        let entries = vec![
            (h(1), circle((0.0, 0.0, 0.0), 5.0)),
            (h(2), circle((0.0, 0.0, 0.0), 6.0)),
        ];
        assert!(find_duplicates(&entries).is_empty());
    }

    #[test]
    fn arcs_differing_in_angle_are_kept() {
        let entries = vec![
            (h(1), arc((0.0, 0.0, 0.0), 5.0, 0.0, 180.0)),
            (h(2), arc((0.0, 0.0, 0.0), 5.0, 45.0, 180.0)),
        ];
        assert!(find_duplicates(&entries).is_empty());
    }

    #[test]
    fn identical_arcs_are_duplicates() {
        let entries = vec![
            (h(1), arc((0.0, 0.0, 0.0), 5.0, 0.0, 180.0)),
            (h(2), arc((0.0, 0.0, 0.0), 5.0, 0.0, 180.0)),
        ];
        assert_eq!(find_duplicates(&entries), vec![h(2)]);
    }

    #[test]
    fn line_and_circle_with_same_center_do_not_collide() {
        // A Circle and a Line whose endpoints happen to align on the same
        // coordinates must not collapse — GeomKey discriminates by variant.
        let entries = vec![
            (h(1), line((0.0, 0.0, 0.0), (5.0, 0.0, 0.0))),
            (h(2), circle((0.0, 0.0, 0.0), 5.0)),
        ];
        assert!(find_duplicates(&entries).is_empty());
    }

    #[test]
    fn identical_points_are_duplicates() {
        let entries = vec![
            (h(1), point((1.0, 2.0, 3.0))),
            (h(2), point((1.0, 2.0, 3.0))),
            (h(3), point((1.0, 2.0, 3.0001))),
        ];
        let dupes = find_duplicates(&entries);
        assert_eq!(dupes, vec![h(2)], "only exact duplicates (within tolerance) collapse");
    }

    #[test]
    fn tolerance_folds_sub_epsilon_diffs() {
        // Differences below 1e-6 fold into the same quantised bucket.
        let entries = vec![
            (h(1), line((0.0, 0.0, 0.0), (10.0, 0.0, 0.0))),
            (h(2), line((0.0, 0.0, 0.0), (10.0 + 1e-9, 0.0, 0.0))),
        ];
        assert_eq!(find_duplicates(&entries), vec![h(2)]);
    }

    #[test]
    fn empty_input_returns_empty_vec() {
        assert!(find_duplicates(&[]).is_empty());
    }

    #[test]
    fn keeps_first_occurrence_of_each_key() {
        // Three identical lines — handles 2 and 3 are duplicates in order.
        let entries = vec![
            (h(1), line((0.0, 0.0, 0.0), (1.0, 0.0, 0.0))),
            (h(2), line((0.0, 0.0, 0.0), (1.0, 0.0, 0.0))),
            (h(3), line((0.0, 0.0, 0.0), (1.0, 0.0, 0.0))),
        ];
        let dupes = find_duplicates(&entries);
        assert_eq!(dupes, vec![h(2), h(3)]);
    }
}
