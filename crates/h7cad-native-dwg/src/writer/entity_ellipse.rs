//! AC1015 ELLIPSE entity body writer (F5.M5.E9).
//!
//! Inverse of [`crate::read_ellipse_geometry`]:
//!
//! ```text
//!   3BD  center
//!   3BD  major_axis_endpoint
//!   3BD  extrusion
//!   BD   ratio (minor/major)
//!   BD   start_param
//!   BD   end_param
//! ```
//!
//! Note: unlike CIRCLE/ARC the extrusion is encoded as a plain
//! `3BD` triple — the R2000+ short-circuit `BE` format is **not**
//! used for ELLIPSE.

use crate::bit_writer::BitWriter;
use crate::entity_ellipse::EllipseGeometry;
use crate::DwgWriteError;

/// Write the AC1015 ELLIPSE-specific payload for `geom` into `writer`.
pub fn write_ellipse_geometry(
    geom: EllipseGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    writer.write_3bit_double(geom.center)?;
    writer.write_3bit_double(geom.major_axis)?;
    writer.write_3bit_double(geom.extrusion)?;
    writer.write_bit_double(geom.ratio)?;
    writer.write_bit_double(geom.start_param)?;
    writer.write_bit_double(geom.end_param)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_ellipse_geometry, BitReader, BitWriter};

    fn round_trip(geom: EllipseGeometry) -> EllipseGeometry {
        let mut writer = BitWriter::new();
        write_ellipse_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_ellipse_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn ellipse_geometry_round_trips_canonical_full_ellipse() {
        let geom = EllipseGeometry {
            center: [0.0, 0.0, 0.0],
            major_axis: [1.0, 0.0, 0.0],
            extrusion: [0.0, 0.0, 1.0],
            ratio: 0.5,
            start_param: 0.0,
            end_param: std::f64::consts::TAU,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn ellipse_geometry_round_trips_offset_arc() {
        let geom = EllipseGeometry {
            center: [3.5, -2.25, 7.0],
            major_axis: [2.0, 1.0, 0.5],
            extrusion: [0.0, 0.0, 1.0],
            ratio: 0.25,
            start_param: 0.25,
            end_param: 2.75,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn ellipse_geometry_round_trips_default_axis_unit_ratio() {
        // Stress the compact prefix encoding: center / major_axis are
        // canonical zeros / unit vector, extrusion is canonical OCS Z,
        // ratio = 1.0, start = 0.0, end = 1.0 — all hit the BD/3BD
        // compact paths. This exercises the same byte pattern the
        // reader's `ellipse_shortest_encoding` test asserts on the
        // read side.
        let geom = EllipseGeometry {
            center: [0.0, 0.0, 0.0],
            major_axis: [1.0, 0.0, 0.0],
            extrusion: [0.0, 0.0, 1.0],
            ratio: 1.0,
            start_param: 0.0,
            end_param: 1.0,
        };
        assert_eq!(round_trip(geom), geom);
    }
}
