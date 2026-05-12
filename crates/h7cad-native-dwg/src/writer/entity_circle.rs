//! AC1015 CIRCLE entity body writer (F5.M5.E1).
//!
//! Inverse of [`crate::read_circle_geometry`]. Mirrors the on-disk
//! layout documented in `entity_circle.rs`:
//!
//! ```text
//!   3BD  center
//!   BD   radius
//!   BT   thickness
//!   BE   extrusion (normal)
//! ```

use crate::bit_writer::BitWriter;
use crate::entity_circle::CircleGeometry;
use crate::DwgWriteError;

/// Write the AC1015 CIRCLE-specific payload for `geom` into `writer`.
pub fn write_circle_geometry(
    geom: CircleGeometry,
    writer: &mut BitWriter,
) -> Result<(), DwgWriteError> {
    writer.write_3bit_double(geom.center)?;
    writer.write_bit_double(geom.radius)?;
    writer.write_bit_thickness_r2000_plus(geom.thickness)?;
    writer.write_bit_extrusion_r2000_plus(geom.extrusion)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_circle_geometry, BitReader, BitWriter};

    fn round_trip(geom: CircleGeometry) -> CircleGeometry {
        let mut writer = BitWriter::new();
        write_circle_geometry(geom, &mut writer).expect("writes");
        let bytes = writer.into_bytes();
        let mut reader = BitReader::new(&bytes);
        read_circle_geometry(&mut reader).expect("reader recovers")
    }

    #[test]
    fn round_trips_default_circle() {
        let geom = CircleGeometry {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn round_trips_nontrivial_center_and_thickness() {
        let geom = CircleGeometry {
            center: [1.0, 2.0, 3.0],
            radius: 4.5,
            thickness: 1.25,
            extrusion: [1.0, 1.0, 1.0],
        };
        assert_eq!(round_trip(geom), geom);
    }

    #[test]
    fn uses_compact_default_encoding() {
        let geom = CircleGeometry {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
            thickness: 0.0,
            extrusion: [0.0, 0.0, 1.0],
        };
        let mut writer = BitWriter::new();
        write_circle_geometry(geom, &mut writer).expect("writes");
        // 3BD center (3 * 2 bits) + BD radius=1.0 (2 bits)
        // + BT zero (1 bit) + BE default normal (1 bit).
        assert_eq!(writer.position_in_bits(), 10);
        assert_eq!(round_trip(geom), geom);
    }
}
