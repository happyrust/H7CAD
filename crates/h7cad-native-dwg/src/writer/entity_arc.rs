//! AC1015 ARC entity body writer (F5.M5.E2).
//!
//! Inverse of [`crate::read_arc_geometry`]. ARC is CIRCLE plus two
//! trailing BitDouble angles:
//!
//! ```text
//!   3BD  center
//!   BD   radius
//!   BT   thickness
//!   BE   extrusion (normal)
//!   BD   start_angle
//!   BD   end_angle
//! ```

use crate::bit_writer::BitWriter;
use crate::entity_arc::ArcGeometry;
use crate::DwgWriteError;

/// Write the AC1015 ARC-specific payload for `geom` into `writer`.
pub fn write_arc_geometry(geom: ArcGeometry, writer: &mut BitWriter) -> Result<(), DwgWriteError> {
    writer.write_3bit_double(geom.center)?;
    writer.write_bit_double(geom.radius)?;
    writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    writer.write_bit_double(geom.start_angle)?;
    writer.write_bit_double(geom.end_angle)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_arc_geometry, BitReader, BitWriter};

    fn round_trip(geom: ArcGeometry) -> ArcGeometry {
        let mut writer = BitWriter::new();
        write_arc_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_arc_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn round_trips_default_arc() {
        let geom = ArcGeometry {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
            start_angle: 0.0,
            end_angle: 0.0,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn round_trips_nontrivial_sweep() {
        let geom = ArcGeometry {
            center: [1.0, 2.0, 3.0],
            radius: 4.5,
            thickness: 1.25,
            extrusion: [1.0, 1.0, 1.0],
            start_angle: 0.25,
            end_angle: 2.75,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn uses_compact_default_encoding() {
        let geom = ArcGeometry {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
            start_angle: 0.0,
            end_angle: 0.0,
        };
        let mut writer = BitWriter::new();
        write_arc_geometry(geom, &mut writer).expect("writes");
        // CIRCLE compact payload (10 bits) + two zero BD angles (2 bits each).
        assert_eq!(writer.position_in_bits(), 14);
        assert_eq!(round_trip(geom), geom);
    }
}
