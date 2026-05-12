//! AC1015 HATCH entity body writer (F5.M5.E20).
//!
//! Inverse of [`crate::read_hatch_geometry`]. HATCH is the most
//! complex entity in the M5 cohort because it nests three levels of
//! variable-length data (`boundary_paths` → `edges` → per-edge
//! variants) and carries an optional pattern definition block.
//!
//! ```text
//!   BD   elevation               (reader-discarded → writer: 0.0)
//!   3BD  extrusion
//!   T    pattern_name
//!   B    solid_fill
//!   B    is_associative          (reader-discarded → writer: 0)
//!   BL   num_boundary_paths
//!   for each path:
//!     BL   flags
//!     if (flags & 2) == 2 (polyline path):
//!       B    has_bulge
//!       B    closed
//!       BL   num_vertices
//!       (2RD vertex + BD bulge?) × num_vertices
//!     else:
//!       BL   num_edges
//!       per edge:
//!         u8 type
//!         match type {
//!           1 => 2RD start + 2RD end                       // Line
//!           2 => 2RD center + BD radius + BD start + BD end + B is_ccw  // CircularArc
//!           3 => 2RD center + 2RD major + BD minor_ratio + BD start + BD end + B is_ccw  // EllipticArc
//!           // type 4 (Spline) is NOT emitted; model has no Spline variant
//!         }
//!     BL   boundary_handle_count   // writer always writes 0
//!   BS   style                    (reader-discarded → writer: 0)
//!   BS   pattern_type             (reader-discarded → writer: 0)
//!   if !solid_fill:                                         // E20b: pattern block
//!     BD   pattern_angle          (reader-discarded → writer: 0.0)
//!     BD   pattern_scale          (reader-discarded → writer: 1.0)
//!     B    is_double              (reader-discarded → writer: 0)
//!     BS   num_pattern_lines      (writer always 0)
//!   BL   num_seeds                 (writer always 0)
//! ```
//!
//! Known limitations (model-side gaps the writer cannot fill):
//!
//! - `boundary_handle_count` is always written as 0; HATCH boundary
//!   associativity with source entities is not preserved.
//! - `pattern` definition is reduced to its minimal shape (angle=0,
//!   scale=1, num_lines=0) because `EntityData::Hatch` does not
//!   carry pattern-line metadata. AutoCAD will render
//!   non-solid HATCH with a blank pattern.
//! - `num_seeds` is always 0; flood-fill seed points are not modelled.
//! - HATCH spline edges (wire type 4) are not emitted because
//!   `HatchEdge` has no `Spline` variant.
//! - `elevation` is always 0 on the wire; the reader discards it.
//!
//! The common-entity-header `owner_handle` / `lineweight` limitations
//! shared with M5.E1…E12 / E21 also apply here until M5.C-full ships.

use crate::bit_writer::BitWriter;
use crate::entity_hatch::HatchGeometry;
use crate::DwgWriteError;
use h7cad_native_model::{HatchBoundaryPath, HatchEdge};

const HATCH_PATH_POLYLINE_FLAG: i32 = 2;
const EDGE_TYPE_LINE: u8 = 1;
const EDGE_TYPE_CIRCULAR_ARC: u8 = 2;
const EDGE_TYPE_ELLIPTIC_ARC: u8 = 3;

/// Write the AC1015 HATCH-specific payload for `geom`.
///
/// The handle stream is unused — `boundary_handle_count` is always
/// emitted as `0` so no `_handle_writer` writes are required. The
/// parameter is kept in the signature for symmetry with other
/// dual-stream entity writers (MTEXT, INSERT, ATTRIB).
pub fn write_hatch_geometry(
    geom: &HatchGeometry,
    main_writer: &mut BitWriter,
    _handle_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    main_writer.write_bit_double(0.0)?;
    main_writer.write_3bit_double(geom.extrusion)?;
    main_writer.write_text_ascii(&geom.pattern_name)?;
    main_writer.write_bit(if geom.solid_fill { 1 } else { 0 })?;
    main_writer.write_bit(0)?;

    let num_paths = i32::try_from(geom.boundary_paths.len()).map_err(|_| {
        DwgWriteError::InvalidValue(format!(
            "HATCH boundary_paths count {} exceeds i32::MAX",
            geom.boundary_paths.len()
        ))
    })?;
    main_writer.write_bit_long(num_paths)?;
    for path in &geom.boundary_paths {
        write_boundary_path(path, main_writer)?;
    }

    main_writer.write_bit_short(0)?;
    main_writer.write_bit_short(0)?;

    if !geom.solid_fill {
        main_writer.write_bit_double(0.0)?;
        main_writer.write_bit_double(1.0)?;
        main_writer.write_bit(0)?;
        main_writer.write_bit_short(0)?;
    }

    main_writer.write_bit_long(0)?;
    Ok(())
}

fn write_boundary_path(
    path: &HatchBoundaryPath,
    main_writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    main_writer.write_bit_long(path.flags)?;
    let is_polyline = (path.flags & HATCH_PATH_POLYLINE_FLAG) != 0;
    if is_polyline {
        let (closed, vertices) = path
            .edges
            .iter()
            .find_map(|edge| match edge {
                HatchEdge::Polyline { closed, vertices } => Some((*closed, vertices)),
                _ => None,
            })
            .ok_or_else(|| {
                DwgWriteError::InvalidValue(
                    "HATCH polyline-flag boundary path must contain a HatchEdge::Polyline"
                        .to_string(),
                )
            })?;
        let has_bulge = vertices.iter().any(|v| v[2] != 0.0);
        main_writer.write_bit(if has_bulge { 1 } else { 0 })?;
        main_writer.write_bit(if closed { 1 } else { 0 })?;
        let num_vertices = i32::try_from(vertices.len()).map_err(|_| {
            DwgWriteError::InvalidValue(format!(
                "HATCH polyline vertex count {} exceeds i32::MAX",
                vertices.len()
            ))
        })?;
        main_writer.write_bit_long(num_vertices)?;
        for v in vertices {
            main_writer.write_raw_f64_le(v[0])?;
            main_writer.write_raw_f64_le(v[1])?;
            if has_bulge {
                main_writer.write_bit_double(v[2])?;
            }
        }
    } else {
        let num_edges = i32::try_from(path.edges.len()).map_err(|_| {
            DwgWriteError::InvalidValue(format!(
                "HATCH boundary edge count {} exceeds i32::MAX",
                path.edges.len()
            ))
        })?;
        main_writer.write_bit_long(num_edges)?;
        for edge in &path.edges {
            write_hatch_edge(edge, main_writer)?;
        }
    }
    main_writer.write_bit_long(0)?;
    Ok(())
}

fn write_hatch_edge(edge: &HatchEdge, main_writer: &mut BitWriter) -> Result<(), DwgWriteError> {
    match edge {
        HatchEdge::Line { start, end } => {
            main_writer.write_raw_u8(EDGE_TYPE_LINE)?;
            main_writer.write_raw_f64_le(start[0])?;
            main_writer.write_raw_f64_le(start[1])?;
            main_writer.write_raw_f64_le(end[0])?;
            main_writer.write_raw_f64_le(end[1])?;
        }
        HatchEdge::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            is_ccw,
        } => {
            main_writer.write_raw_u8(EDGE_TYPE_CIRCULAR_ARC)?;
            main_writer.write_raw_f64_le(center[0])?;
            main_writer.write_raw_f64_le(center[1])?;
            main_writer.write_bit_double(*radius)?;
            main_writer.write_bit_double(*start_angle)?;
            main_writer.write_bit_double(*end_angle)?;
            main_writer.write_bit(if *is_ccw { 1 } else { 0 })?;
        }
        HatchEdge::EllipticArc {
            center,
            major_endpoint,
            minor_ratio,
            start_angle,
            end_angle,
            is_ccw,
        } => {
            main_writer.write_raw_u8(EDGE_TYPE_ELLIPTIC_ARC)?;
            main_writer.write_raw_f64_le(center[0])?;
            main_writer.write_raw_f64_le(center[1])?;
            main_writer.write_raw_f64_le(major_endpoint[0])?;
            main_writer.write_raw_f64_le(major_endpoint[1])?;
            main_writer.write_bit_double(*minor_ratio)?;
            main_writer.write_bit_double(*start_angle)?;
            main_writer.write_bit_double(*end_angle)?;
            main_writer.write_bit(if *is_ccw { 1 } else { 0 })?;
        }
        HatchEdge::Polyline { .. } => {
            return Err(DwgWriteError::InvalidValue(
                "HatchEdge::Polyline must appear inside a polyline-flag boundary path, \
                 not as a typed edge under a non-polyline path"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_hatch_geometry, BitReader, BitWriter};
    use h7cad_native_model::Handle;

    fn round_trip(geom: HatchGeometry, object_handle: Handle) -> HatchGeometry {
        let mut main = BitWriter::new();
        let mut handle = BitWriter::new();
        write_hatch_geometry(&geom, &mut main, &mut handle).expect("writes");
        let main_bytes = main.into_bytes();
        let handle_bytes = handle.into_bytes();
        let mut main_reader = BitReader::new(&main_bytes);
        let mut handle_reader = BitReader::new(&handle_bytes);
        read_hatch_geometry(&mut main_reader, &mut handle_reader, object_handle)
            .expect("reader recovers")
    }

    #[test]
    fn hatch_round_trips_empty_solid() {
        let geom = HatchGeometry {
            pattern_name: "SOLID".to_string(),
            solid_fill: true,
            boundary_paths: Vec::new(),
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);
    }

    #[test]
    fn hatch_round_trips_single_line_boundary() {
        let geom = HatchGeometry {
            pattern_name: "SOLID".to_string(),
            solid_fill: true,
            boundary_paths: vec![HatchBoundaryPath {
                flags: 0,
                edges: vec![
                    HatchEdge::Line {
                        start: [0.0, 0.0],
                        end: [1.0, 0.0],
                    },
                    HatchEdge::Line {
                        start: [1.0, 0.0],
                        end: [1.0, 1.0],
                    },
                ],
            }],
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);
    }

    #[test]
    fn hatch_round_trips_circular_arc_boundary() {
        let geom = HatchGeometry {
            pattern_name: "SOLID".to_string(),
            solid_fill: true,
            boundary_paths: vec![HatchBoundaryPath {
                flags: 0,
                edges: vec![HatchEdge::CircularArc {
                    center: [5.0, 5.0],
                    radius: 3.0,
                    start_angle: 0.0,
                    end_angle: std::f64::consts::PI,
                    is_ccw: true,
                }],
            }],
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);

        // Toggle is_ccw to verify the bit round-trips both ways.
        let mut cw = geom.clone();
        if let HatchEdge::CircularArc { is_ccw, .. } = &mut cw.boundary_paths[0].edges[0] {
            *is_ccw = false;
        }
        assert_eq!(round_trip(cw.clone(), Handle::new(0x10)), cw);
    }

    #[test]
    fn hatch_round_trips_elliptic_arc_boundary() {
        let geom = HatchGeometry {
            pattern_name: "SOLID".to_string(),
            solid_fill: true,
            boundary_paths: vec![HatchBoundaryPath {
                flags: 0,
                edges: vec![HatchEdge::EllipticArc {
                    center: [10.0, 20.0],
                    major_endpoint: [3.0, 0.0],
                    minor_ratio: 0.5,
                    start_angle: 0.25,
                    end_angle: 2.75,
                    is_ccw: true,
                }],
            }],
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);
    }

    #[test]
    fn hatch_round_trips_polyline_path_with_bulge() {
        let geom = HatchGeometry {
            pattern_name: "SOLID".to_string(),
            solid_fill: true,
            boundary_paths: vec![HatchBoundaryPath {
                flags: HATCH_PATH_POLYLINE_FLAG,
                edges: vec![HatchEdge::Polyline {
                    closed: true,
                    vertices: vec![
                        [0.0, 0.0, 0.0],
                        [1.0, 0.0, 0.5],
                        [1.0, 1.0, 0.0],
                        [0.0, 1.0, 0.0],
                    ],
                }],
            }],
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);
    }

    #[test]
    fn hatch_round_trips_pattern_block_not_solid() {
        // !solid_fill triggers the minimal pattern block.
        let geom = HatchGeometry {
            pattern_name: "ANSI31".to_string(),
            solid_fill: false,
            boundary_paths: vec![HatchBoundaryPath {
                flags: 0,
                edges: vec![HatchEdge::Line {
                    start: [0.0, 0.0],
                    end: [1.0, 0.0],
                }],
            }],
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom.clone(), Handle::new(0x10)), geom);
    }

    #[test]
    fn hatch_rejects_polyline_flag_without_polyline_edge() {
        let geom = HatchGeometry {
            pattern_name: "SOLID".to_string(),
            solid_fill: true,
            boundary_paths: vec![HatchBoundaryPath {
                flags: HATCH_PATH_POLYLINE_FLAG,
                edges: vec![HatchEdge::Line {
                    start: [0.0, 0.0],
                    end: [1.0, 0.0],
                }],
            }],
            extrusion: [0.0, 0.0, 1.0],
        };
        let mut main = BitWriter::new();
        let mut handle = BitWriter::new();
        let err = write_hatch_geometry(&geom, &mut main, &mut handle)
            .expect_err("polyline-flag boundary without HatchEdge::Polyline must be rejected");
        match err {
            DwgWriteError::InvalidValue(msg) => assert!(
                msg.contains("polyline") && msg.contains("HatchEdge::Polyline"),
                "error must name both the flag and the required variant; got `{msg}`"
            ),
            other => panic!("expected InvalidValue, got {other:?}"),
        }
    }
}
