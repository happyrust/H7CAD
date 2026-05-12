//! AC1015 POINT entity body writer (F5.M5.E3).
//!
//! Inverse of [`crate::read_point_geometry`]. The native model stores
//! the point location on `EntityData::Point`; thickness and extrusion
//! remain on the common [`h7cad_native_model::Entity`] fields, and
//! `x_axis_angle` is currently emitted by callers as a geometry field.
//!
//! ```text
//!   3BD  location
//!   BT   thickness
//!   BE   extrusion (normal)
//!   BD   x_axis_angle
//! ```

use crate::bit_writer::BitWriter;
use crate::entity_point::PointGeometry;
use crate::DwgWriteError;

/// Write the AC1015 POINT-specific payload for `geom` into `writer`.
pub fn write_point_geometry(
    geom: PointGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    writer.write_3bit_double(geom.position)?;
    writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    writer.write_bit_double(geom.x_axis_angle)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_point_geometry, BitReader, BitWriter};

    fn round_trip(geom: PointGeometry) -> PointGeometry {
        let mut writer = BitWriter::new();
        write_point_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_point_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn round_trips_default_point() {
        let geom = PointGeometry {
            position: [0.0, 0.0, 0.0],
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
            x_axis_angle: 0.0,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn round_trips_nontrivial_position_and_angle() {
        let geom = PointGeometry {
            position: [1.0, 2.0, 3.0],
            thickness: 1.25,
            extrusion: [1.0, 1.0, 1.0],
            x_axis_angle: 0.5,
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn uses_compact_default_encoding() {
        let geom = PointGeometry {
            position: [0.0, 0.0, 0.0],
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
            x_axis_angle: 0.0,
        };
        let mut writer = BitWriter::new();
        write_point_geometry(geom, &mut writer).expect("writes");
        // 3BD position (3 * 2 bits) + BT zero (1 bit)
        // + BE default normal (1 bit) + BD angle zero (2 bits).
        assert_eq!(writer.position_in_bits(), 10);
        assert_eq!(round_trip(geom), geom);
    }
}
